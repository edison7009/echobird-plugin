use std::fs;
fn echobird_dir() -> std::path::PathBuf {
    dirs::home_dir().unwrap_or_default().join(".echobird")
}
fn ensure_config_dir() {
    let _ = fs::create_dir_all(echobird_dir().join("config"));
}
const ENCRYPTED_PREFIX: &str = "enc:v1:";

/// Check if a key is already encrypted
fn is_encrypted(key: &str) -> bool {
    key.starts_with(ENCRYPTED_PREFIX)
}

/// Get stable machine fingerprint (does NOT change on app upgrade/reinstall)
/// Windows: HKLM\SOFTWARE\Microsoft\Cryptography\MachineGuid
/// macOS: IOPlatformUUID via ioreg
/// Linux: /etc/machine-id
fn get_machine_fingerprint() -> String {
    static FINGERPRINT: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
    machine_fingerprint_with(
        &FINGERPRINT,
        probe_machine_fingerprint,
        fallback_machine_fingerprint,
    )
}

fn machine_fingerprint_with(
    cache: &std::sync::Mutex<Option<String>>,
    probe: impl FnOnce() -> Option<String>,
    fallback: impl FnOnce() -> String,
) -> String {
    let mut cached = cache
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(fingerprint) = cached.as_ref() {
        return fingerprint.clone();
    }
    if let Some(fingerprint) = probe() {
        if !fingerprint.is_empty() {
            *cached = Some(fingerprint.clone());
        }
        return fingerprint;
    }
    drop(cached);
    fallback()
}

fn probe_machine_fingerprint() -> Option<String> {
    #[cfg(windows)]
    {
        if let Ok(output) = crate::utils::process::command("reg")
            .args([
                "query",
                r"HKLM\SOFTWARE\Microsoft\Cryptography",
                "/v",
                "MachineGuid",
            ])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            // Output format: "    MachineGuid    REG_SZ    <guid>"
            if let Some(line) = text.lines().find(|l| l.contains("MachineGuid")) {
                if let Some(guid) = line.split_whitespace().last() {
                    return Some(guid.to_string());
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = crate::utils::process::command("ioreg")
            .args(["-rd1", "-c", "IOPlatformExpertDevice"])
            .output()
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                if line.contains("IOPlatformUUID") {
                    if let Some(uuid) = line.split('"').nth(3) {
                        return Some(uuid.to_string());
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(id) = std::fs::read_to_string("/etc/machine-id") {
            let trimmed = id.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }

    None
}

fn fallback_machine_fingerprint() -> String {
    // Fallback: hostname via env var
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "echobird-fallback".to_string())
}

/// Get or create the persistent encryption key, derived with machine fingerprint.
/// Actual AES key = HMAC-SHA256(random_file_key, machine_fingerprint)
/// This ensures: same machine + same file = works; different machine = self-destruct.
fn get_encryption_key() -> Result<[u8; 32], String> {
    let key_path = echobird_dir().join("config").join(".encryption_key");

    let file_key = if key_path.exists() {
        let data =
            fs::read(&key_path).map_err(|e| format!("Failed to read encryption key: {}", e))?;
        if data.len() == 32 {
            data
        } else {
            log::warn!("[ModelManager] Invalid encryption key file, regenerating");
            generate_key_file(&key_path)?
        }
    } else {
        generate_key_file(&key_path)?
    };

    // Derive actual key: HMAC-SHA256(file_key, machine_fingerprint + username)
    let fingerprint = get_machine_fingerprint();
    let username = std::env::var("USERNAME")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "default".to_string());
    derive_encryption_key(&file_key, &fingerprint, &username)
}

fn derive_encryption_key(
    file_key: &[u8],
    fingerprint: &str,
    username: &str,
) -> Result<[u8; 32], String> {
    use hmac::Mac;
    type HmacSha256 = hmac::Hmac<sha2::Sha256>;

    let identity = format!("{}|{}", fingerprint, username);

    let mut mac = <HmacSha256 as Mac>::new_from_slice(file_key)
        .map_err(|e| format!("HMAC init error: {}", e))?;
    hmac::Mac::update(&mut mac, identity.as_bytes());
    let result = mac.finalize().into_bytes();

    let mut derived_key = [0u8; 32];
    derived_key.copy_from_slice(&result);
    Ok(derived_key)
}

fn generate_key_file(key_path: &std::path::Path) -> Result<Vec<u8>, String> {
    use rand::RngCore;
    let mut key = vec![0u8; 32];
    rand::thread_rng().fill_bytes(&mut key);

    ensure_config_dir();
    fs::write(key_path, &key).map_err(|e| format!("Failed to write encryption key: {}", e))?;
    log::info!("[ModelManager] Generated new encryption key");
    Ok(key)
}

/// Encrypt API key using AES-256-GCM.
/// Stores "enc:v1:<hex(nonce + ciphertext)>" in models.json.
fn encrypt_api_key(plain_key: &str) -> String {
    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit};
    use rand::RngCore;

    if plain_key.is_empty() || is_encrypted(plain_key) || plain_key == "local" {
        return plain_key.to_string();
    }

    let key_bytes = match get_encryption_key() {
        Ok(k) => k,
        Err(e) => {
            log::error!("[ModelManager] Encryption key error: {}", e);
            return plain_key.to_string();
        }
    };

    let cipher = Aes256Gcm::new((&key_bytes).into());

    // Generate random 12-byte nonce
    let mut nonce_bytes = [0u8; 12];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = aes_gcm::Nonce::from_slice(&nonce_bytes);

    match cipher.encrypt(nonce, plain_key.as_bytes()) {
        Ok(ciphertext) => {
            // Concatenate nonce + ciphertext, encode as hex
            let mut combined = Vec::with_capacity(12 + ciphertext.len());
            combined.extend_from_slice(&nonce_bytes);
            combined.extend_from_slice(&ciphertext);
            let hex = combined
                .iter()
                .map(|b| format!("{:02x}", b))
                .collect::<String>();
            log::info!("[ModelManager] API key encrypted successfully");
            format!("{}{}", ENCRYPTED_PREFIX, hex)
        }
        Err(e) => {
            log::error!("[ModelManager] AES encryption failed: {}", e);
            plain_key.to_string()
        }
    }
}

/// Decrypt API key using AES-256-GCM.
fn decrypt_api_key(stored_key: &str) -> String {
    use aes_gcm::{aead::Aead, Aes256Gcm, KeyInit};

    if stored_key.is_empty() || !is_encrypted(stored_key) {
        return stored_key.to_string();
    }

    let hex_data = &stored_key[ENCRYPTED_PREFIX.len()..];

    // Decode hex to bytes
    let combined: Vec<u8> = match (0..hex_data.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_data[i..i + 2], 16))
        .collect::<Result<Vec<u8>, _>>()
    {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("[ModelManager] Hex decode failed: {}", e);
            return String::new();
        }
    };

    if combined.len() < 13 {
        log::error!("[ModelManager] Encrypted data too short");
        return String::new();
    }

    let key_bytes = match get_encryption_key() {
        Ok(k) => k,
        Err(e) => {
            log::error!("[ModelManager] Encryption key error: {}", e);
            return String::new();
        }
    };

    let cipher = Aes256Gcm::new((&key_bytes).into());
    let nonce = aes_gcm::Nonce::from_slice(&combined[..12]);
    let ciphertext = &combined[12..];

    match cipher.decrypt(nonce, ciphertext) {
        Ok(plaintext) => String::from_utf8(plaintext).unwrap_or_else(|_| {
            log::error!("[ModelManager] Decrypted key is not valid UTF-8");
            String::new()
        }),
        Err(e) => {
            log::error!("[ModelManager] AES decryption failed: {}", e);
            String::new()
        }
    }
}

/// Get usable plaintext API key (auto-decrypts if needed)
pub fn decrypt_key_for_use(api_key: &str) -> String {
    decrypt_api_key(api_key)
}

/// Encrypt a key/password for storage (public, reused by SSH)
pub fn encrypt_key_for_storage(plain_key: &str) -> String {
    encrypt_api_key(plain_key)
}
