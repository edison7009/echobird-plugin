use std::path::{Path, PathBuf};

include!("owned_process.rs");

#[derive(serde::Deserialize)]
struct Process {
    pid: u32,
    path: PathBuf,
}

fn processes() -> Result<Vec<Process>, String> {
    #[cfg(windows)]
    {
        let output=crate::utils::process::command("powershell.exe")
            .args(["-NoProfile","-NonInteractive","-Command","$ErrorActionPreference='Stop'; [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); $rows=@(Get-CimInstance Win32_Process -ErrorAction Stop | Where-Object { $_.ExecutablePath } | ForEach-Object { @{pid=$_.ProcessId;path=$_.ExecutablePath} }); ConvertTo-Json -InputObject $rows -Compress"])
            .output().map_err(|_|"accountError.closeClient")?;
        if !output.status.success() {
            return Err("accountError.closeClient".into());
        }
        serde_json::from_slice(&output.stdout).map_err(|_| "accountError.closeClient".into())
    }
    #[cfg(unix)]
    {
        let output = crate::utils::process::command("/bin/ps")
            .args(["-A", "-o", "pid="])
            .output()
            .map_err(|_| "accountError.closeClient")?;
        if !output.status.success() {
            return Err("accountError.closeClient".into());
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| {
                let pid = line.trim().parse().ok()?;
                Some(Process {
                    pid,
                    path: native_process_path(pid)?,
                })
            })
            .collect())
    }
}

fn owns(tool: &str, path: &Path) -> bool {
    #[cfg(windows)]
    if tool == "claudedesktop" {
        return claude_desktop_windows_process(
            path,
            super::tool_manager::get_tool_exe_path(tool).as_deref(),
            super::tool_manager::get_tool_launch_uri(tool).as_deref(),
        );
    }
    let Some(exe) = super::tool_manager::get_tool_exe_path(tool) else {
        #[cfg(windows)]
        if let Some(uri) = super::tool_manager::get_tool_launch_uri(tool) {
            let family = uri
                .strip_prefix("shell:AppsFolder\\")
                .and_then(|value| value.split('!').next())
                .unwrap_or_default();
            if let Some((name, publisher)) = family.rsplit_once('_') {
                let value = path
                    .to_string_lossy()
                    .replace('/', "\\")
                    .to_ascii_lowercase();
                let Some((package, relative)) = value
                    .split("\\windowsapps\\")
                    .nth(1)
                    .and_then(|value| value.split_once('\\'))
                else {
                    return false;
                };
                return package.starts_with(&format!("{}_", name.to_ascii_lowercase()))
                    && package.ends_with(&format!("_{}", publisher.to_ascii_lowercase()))
                    && matches!(relative, "claude.exe" | "app\\claude.exe");
            }
        }
        return false;
    };
    #[cfg(windows)]
    {
        let normalize = |value: &str| value.replace('/', "\\").to_ascii_lowercase();
        let exe = normalize(&exe);
        let path = normalize(&path.to_string_lossy());
        if path == exe {
            return true;
        }
        if let Some((root, name)) = exe.rsplit_once('\\') {
            return path
                .strip_prefix(&format!("{root}\\"))
                .and_then(|relative| relative.split_once('\\'))
                .is_some_and(|(directory, file)| {
                    directory
                        .strip_prefix("app-")
                        .is_some_and(|version| version.starts_with(|c: char| c.is_ascii_digit()))
                        && file == name
                });
        }
        false
    }
    #[cfg(unix)]
    {
        std::fs::canonicalize(exe).is_ok_and(|exe| path == exe)
    }
}

pub async fn desktop_tool_is_running(tool: &str) -> Result<bool, String> {
    let tool = tool.to_owned();
    tokio::task::spawn_blocking(move || {
        processes().map(|items| items.iter().any(|process| owns(&tool, &process.path)))
    })
    .await
    .map_err(|_| "accountError.closeClient")?
}

pub async fn stop_desktop_for_account(tool: &str) -> Result<(), String> {
    let tool = tool.to_owned();
    let check = tool.clone();
    tokio::task::spawn_blocking(move || {
        for process in processes()?
            .into_iter()
            .filter(|process| owns(&tool, &process.path))
        {
            #[cfg(windows)]
            {
                crate::utils::process::command("powershell.exe")
                    .args(["-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; $entry=Get-CimInstance Win32_Process -Filter ('ProcessId = '+$env:ECHOBIRD_ACCOUNT_PID) -ErrorAction Stop; if($entry -and $entry.ExecutablePath -eq $env:ECHOBIRD_ACCOUNT_EXE){ & taskkill.exe /PID $entry.ProcessId /T /F >$null 2>&1; exit $LASTEXITCODE }; exit 1"])
                    .env("ECHOBIRD_ACCOUNT_PID",process.pid.to_string()).env("ECHOBIRD_ACCOUNT_EXE",&process.path)
                    .output()
                    .map_err(|_| "accountError.stopClient")?;
            }
            #[cfg(unix)]
            {
                if native_process_path(process.pid).as_ref()==Some(&process.path) && unsafe { libc::kill(process.pid as i32, libc::SIGTERM) } != 0 && std::io::Error::last_os_error().raw_os_error()!=Some(libc::ESRCH) {
                    return Err("accountError.stopClient".into());
                }
            }
        }
        Ok::<_, String>(())
    })
    .await
    .map_err(|_| "accountError.stopClient")??;
    for _ in 0..30 {
        if !desktop_tool_is_running(&check).await? {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    Err("accountError.stopClient".into())
}

#[cfg(unix)]
fn native_process_path(pid: u32) -> Option<PathBuf> {
    #[cfg(target_os = "linux")]
    {
        std::fs::read_link(format!("/proc/{pid}/exe")).ok()
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::unix::ffi::OsStringExt;
        let mut buffer = [0u8; 4096];
        if unsafe { libc::proc_pidpath(pid as i32, buffer.as_mut_ptr().cast(), 4096) } <= 0 {
            return None;
        }
        let path = std::ffi::CStr::from_bytes_until_nul(&buffer).ok()?;
        Some(std::ffi::OsString::from_vec(path.to_bytes().to_vec()).into())
    }
}

#[cfg(any(windows, test))]
fn claude_desktop_windows_process(
    path: &std::path::Path,
    executable: Option<&str>,
    launch_uri: Option<&str>,
) -> bool {
    let normalized = |value: &str| value.replace('/', "\\").to_ascii_lowercase();
    let path = normalized(&path.to_string_lossy());
    if let Some(executable) = executable.filter(|value| !value.is_empty()).map(normalized) {
        if path == executable {
            return true;
        }
        // Squirrel's launcher lives beside app-<version>/Claude.exe.
        if let Some((root, "claude.exe")) = executable.rsplit_once('\\') {
            if path
                .strip_prefix(&format!("{root}\\"))
                .and_then(|relative| relative.split_once('\\'))
                .is_some_and(|(directory, file)| {
                    directory
                        .strip_prefix("app-")
                        .is_some_and(|version| version.starts_with(|c: char| c.is_ascii_digit()))
                        && file == "claude.exe"
                })
            {
                return true;
            }
        }
    }
    let Some(family) = launch_uri
        .and_then(|uri| uri.strip_prefix("shell:AppsFolder\\"))
        .and_then(|uri| uri.split('!').next())
    else {
        return false;
    };
    let Some((name, publisher)) = family.rsplit_once('_') else {
        return false;
    };
    let Some((_, package_path)) = path.split_once("\\windowsapps\\") else {
        return false;
    };
    let Some((package, relative)) = package_path.split_once('\\') else {
        return false;
    };
    package.starts_with(&format!("{}_", name.to_ascii_lowercase()))
        && package.ends_with(&format!("_{}", publisher.to_ascii_lowercase()))
        && matches!(relative, "claude.exe" | "app\\claude.exe")
}

#[cfg(test)]
mod tests {
    #[test]
    fn claude_desktop_shutdown_excludes_standalone_cli_and_supports_store_and_squirrel() {
        use super::claude_desktop_windows_process;
        let executable = Some(r"C:\Users\fixture\AppData\Local\AnthropicClaude\Claude.exe");
        let uri = Some(r"shell:AppsFolder\Claude_pzs8sxrjxfjjc!Claude");
        for (path, expected) in [
            (
                r"c:/Users/fixture/AppData/Local/AnthropicClaude/claude.exe",
                true,
            ),
            (
                r"C:\Users\fixture\AppData\Local\AnthropicClaude\app-1.2.3\Claude.exe",
                true,
            ),
            (r"C:\Users\fixture\.local\bin\claude.exe", false),
            (
                r"C:\Users\fixture\AppData\Local\AnthropicClaude\resources\claude.exe",
                false,
            ),
            (
                r"C:\Users\fixture\AppData\Local\AnthropicClaude\app-1.2.3\resources\claude.exe",
                false,
            ),
            (r"C:\Other\Claude.exe", false),
            (
                r"C:\Program Files\WindowsApps\Claude_1.2.3.0_x64__pzs8sxrjxfjjc\app\Claude.exe",
                true,
            ),
            (
                r"D:\WindowsApps\Claude_1.2.3.0_x64__pzs8sxrjxfjjc\Claude.exe",
                true,
            ),
            (
                r"C:\Program Files\WindowsApps\Claude_1.2.3.0_x64__other\app\Claude.exe",
                false,
            ),
            (
                r"C:\Program Files\WindowsApps\Claude_1.2.3.0_x64__pzs8sxrjxfjjc\resources\claude.exe",
                false,
            ),
        ] {
            assert_eq!(
                claude_desktop_windows_process(std::path::Path::new(path), executable, uri),
                expected,
                "{path}"
            );
        }
        assert!(!claude_desktop_windows_process(
            std::path::Path::new(r"C:\Other\Claude.exe"),
            None,
            None
        ));
        assert!(claude_desktop_windows_process(
            std::path::Path::new(
                r"C:\Program Files\WindowsApps\Claude_1.2.3.0_x64__pzs8sxrjxfjjc\app\Claude.exe"
            ),
            None,
            uri
        ));
        assert!(!claude_desktop_windows_process(
            std::path::Path::new(
                r"C:\Program Files\WindowsApps\Claude_1.2.3.0_x64__pzs8sxrjxfjjc\resources\claude.exe"
            ),
            None,
            uri
        ));
    }
}
