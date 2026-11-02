//! Isolated browser windows and attempt-scoped desktop login credentials.
use std::sync::{Arc, Mutex};
use tauri::{webview::NewWindowFeatures, Manager, WebviewUrl, WebviewWindowBuilder};
mod desktop;
use desktop::DesktopLogin;

pub(crate) const LOGIN_TIMEOUT_SECONDS: i64 = 10 * 60;
const BLANK: &str = "about:blank";

#[derive(Clone)]
pub(crate) struct Login {
    pub id: String,
    pub expires: i64,
    cancelled: Arc<Mutex<bool>>,
    desktop: Option<DesktopLogin>,
    hidden: bool,
}

impl Login {
    pub fn start(
        app: &tauri::AppHandle,
        title: &'static str,
        url: &str,
        domains: &'static [&'static str],
    ) -> Result<Self, String> {
        Self::start_with(
            app,
            title,
            url.parse().map_err(|_| "accountError.format")?,
            domains,
            None,
            false,
            LOGIN_TIMEOUT_SECONDS,
        )
    }

    pub fn start_desktop(
        app: &tauri::AppHandle,
        title: &'static str,
        scheme: &'static str,
        domains: &'static [&'static str],
    ) -> Result<Self, String> {
        let desktop = DesktopLogin::new(scheme);
        Self::start_with(
            app,
            title,
            desktop.url(),
            domains,
            Some(desktop),
            false,
            LOGIN_TIMEOUT_SECONDS,
        )
    }

    /// An invisible, isolated browser for requests that must come from the
    /// provider's own page. It opens blank so cookies can be set first.
    pub fn start_hidden(
        app: &tauri::AppHandle,
        title: &'static str,
        domains: &'static [&'static str],
        timeout_seconds: i64,
    ) -> Result<Self, String> {
        Self::start_with(
            app,
            title,
            BLANK.parse().map_err(|_| "accountError.format")?,
            domains,
            None,
            true,
            timeout_seconds,
        )
    }

    fn start_with(
        app: &tauri::AppHandle,
        title: &'static str,
        url: url::Url,
        domains: &'static [&'static str],
        desktop: Option<DesktopLogin>,
        hidden: bool,
        timeout_seconds: i64,
    ) -> Result<Self, String> {
        let login = Self {
            id: uuid::Uuid::new_v4().to_string(),
            expires: chrono::Utc::now().timestamp() + timeout_seconds,
            cancelled: Default::default(),
            desktop,
            hidden,
        };
        login.open(app, title, url, domains, None)?;
        let timeout = login.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(timeout_seconds as u64)).await;
            timeout.cancel(&app);
        });
        Ok(login)
    }

    /// The attempt's first window. It stays on the provider's site while
    /// sign-in popups come and go.
    pub fn window(&self, app: &tauri::AppHandle) -> Option<tauri::WebviewWindow> {
        app.get_webview_window(&self.prefix())
    }

    fn prefix(&self) -> String {
        format!("account-login-{}", self.id)
    }

    fn open(
        &self,
        app: &tauri::AppHandle,
        title: &'static str,
        url: url::Url,
        domains: &'static [&'static str],
        features: Option<NewWindowFeatures>,
    ) -> Result<tauri::WebviewWindow, String> {
        self.check()?;
        if !permitted(&url, domains) {
            return Err("accountError.authResponse".into());
        }
        let root = features.is_none();
        let label = if root {
            self.prefix()
        } else {
            format!("{}-{}", self.prefix(), uuid::Uuid::new_v4())
        };
        let popup_login = self.clone();
        let popup_app = app.clone();
        let navigation_login = self.clone();
        // A popup must start blank: the browser attaches it to its opener and
        // loads the requested page itself. Loading the page here would detach
        // it, so sign-in results could not reach the opener.
        let initial = if root {
            url
        } else {
            BLANK.parse().map_err(|_| "accountError.format")?
        };
        let mut builder = WebviewWindowBuilder::new(app, label, WebviewUrl::External(initial))
            .title(title)
            .inner_size(950.0, 730.0)
            .incognito(true)
            .on_navigation(move |url| navigation_login.navigate(url, domains))
            .on_new_window(move |url, features| {
                if !popup_login.navigate(&url, domains) {
                    return tauri::webview::NewWindowResponse::Deny;
                }
                match popup_login.open(&popup_app, title, url, domains, Some(features)) {
                    Ok(window) => tauri::webview::NewWindowResponse::Create { window },
                    Err(_) => tauri::webview::NewWindowResponse::Deny,
                }
            });
        // WebView2's InPrivate controllers share the Default profile within an
        // environment. A different environment per attempt prevents account reuse.
        #[cfg(not(target_os = "macos"))]
        {
            builder = builder.data_directory(self.data_directory());
        }
        if let Some(features) = features {
            builder = builder.window_features(features);
        }
        if root && self.hidden {
            builder = builder.visible(false).focused(false).skip_taskbar(true);
        }
        let window = builder.build().map_err(|_| "accountError.browser")?;
        if root {
            let login = self.clone();
            let app = app.clone();
            window.on_window_event(move |event| {
                if matches!(
                    event,
                    tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
                ) {
                    login.cancel(&app);
                }
            });
        }
        if self.check().is_err() {
            let _ = window.close();
            return Err("accountError.cancelled".into());
        }
        Ok(window)
    }

    pub fn check(&self) -> Result<(), String> {
        check(
            *self.cancelled.lock().map_err(|_| "accountError.busy")?,
            self.expires,
            chrono::Utc::now().timestamp(),
        )
    }

    fn navigate(&self, url: &url::Url, domains: &[&str]) -> bool {
        if self.check().is_err() {
            return false;
        }
        if let Some(desktop) = &self.desktop {
            desktop.capture(url);
        }
        permitted(url, domains)
    }

    pub fn desktop_token(&self) -> Result<Option<String>, String> {
        self.check()?;
        self.desktop.as_ref().ok_or("accountError.format")?.token()
    }

    // Call only from async commands: WebView2 cookie reads deadlock on the UI thread.
    pub fn cookies(
        &self,
        app: &tauri::AppHandle,
    ) -> Result<Vec<tauri::webview::Cookie<'static>>, String> {
        self.check()?;
        let mut cookies = Vec::new();
        for (label, window) in app.webview_windows() {
            if label == self.prefix() || label.starts_with(&format!("{}-", self.prefix())) {
                cookies.extend(window.cookies().map_err(|_| "accountError.read")?);
            }
        }
        self.check()?;
        Ok(cookies)
    }

    pub fn complete<T>(
        &self,
        app: &tauri::AppHandle,
        save: impl FnOnce() -> Result<T, String>,
    ) -> Result<T, String> {
        let result = self.save(save)?;
        self.close(app);
        Ok(result)
    }

    fn save<T>(&self, save: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        let result = {
            let mut cancelled = self.cancelled.lock().map_err(|_| "accountError.busy")?;
            check(*cancelled, self.expires, chrono::Utc::now().timestamp())?;
            let result = save()?;
            *cancelled = true;
            result
        };
        Ok(result)
    }

    pub fn cancel(&self, app: &tauri::AppHandle) {
        let changed = if let Ok(mut cancelled) = self.cancelled.lock() {
            let changed = !*cancelled;
            *cancelled = true;
            changed
        } else {
            false
        };
        if changed {
            self.close(app);
        }
    }

    fn close(&self, app: &tauri::AppHandle) {
        for (label, window) in app.webview_windows() {
            if label == self.prefix() || label.starts_with(&format!("{}-", self.prefix())) {
                let _ = window.close();
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let directory = self.data_directory();
            tauri::async_runtime::spawn(async move {
                // The browser process can briefly retain profile handles after Close.
                for _ in 0..5 {
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    if !directory.exists() {
                        break;
                    }
                    let Ok(root) = std::env::temp_dir().canonicalize() else {
                        break;
                    };
                    let Ok(resolved) = directory.canonicalize() else {
                        break;
                    };
                    if resolved.parent() != Some(root.as_path())
                        || resolved.file_name() != directory.file_name()
                    {
                        break;
                    }
                    if std::fs::remove_dir_all(&resolved).is_ok() {
                        break;
                    }
                }
            });
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn data_directory(&self) -> std::path::PathBuf {
        std::env::temp_dir().join(self.prefix())
    }
}

fn check(cancelled: bool, expires: i64, now: i64) -> Result<(), String> {
    if cancelled {
        return Err("accountError.cancelled".into());
    }
    if now >= expires {
        return Err("accountError.expired".into());
    }
    Ok(())
}

pub(crate) fn seal(value: &str) -> Result<String, String> {
    let sealed = super::model_manager::encrypt_key_for_storage(
        &serde_json::to_string(value).map_err(|_| "accountError.format")?,
    );
    if !sealed.starts_with("enc:v1:") {
        return Err("accountError.keychain".into());
    }
    Ok(sealed)
}

pub(crate) fn unseal(value: &str) -> Result<String, String> {
    if !value.starts_with("enc:v1:") {
        return Err("accountError.format".into());
    }
    serde_json::from_str(&super::model_manager::decrypt_key_for_use(value))
        .map_err(|_| "accountError.keychain".into())
}

/// Blank pages have no content or credentials; popups start there before the
/// browser loads the page that was requested.
fn permitted(url: &url::Url, domains: &[&str]) -> bool {
    url.as_str() == BLANK || allowed(url, domains)
}

fn allowed(url: &url::Url, domains: &[&str]) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port_or_known_default() == Some(443)
        && url.host_str().is_some_and(|host| {
            domains
                .iter()
                .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn login_navigation_accepts_only_the_actual_https_provider_hosts() {
        for (url, valid) in [
            ("https://claude.ai/login", true),
            ("https://accounts.google.com/", true),
            ("https://claude.ai.evil.test/", false),
            ("https://evilclaude.ai/", false),
            ("http://claude.ai/", false),
            ("https://claude.ai:444/", false),
            ("https://user@claude.ai/", false),
            ("echobird://apply", false),
        ] {
            assert_eq!(
                allowed(&url.parse().unwrap(), &["claude.ai", "google.com"]),
                valid
            );
        }
    }

    #[test]
    fn only_the_exact_blank_page_is_added_for_popups() {
        let domains = ["claude.ai"];
        let blank: url::Url = BLANK.parse().unwrap();
        assert!(!allowed(&blank, &domains));
        assert!(permitted(&blank, &domains));
        for url in [
            "about:srcdoc",
            "about:blank#x",
            "about:blank?x",
            "data:text/html,x",
            "javascript:alert(1)",
            "https://evil.test/",
        ] {
            assert!(!permitted(&url.parse().unwrap(), &domains), "{url}");
        }
        assert!(permitted(
            &"https://claude.ai/login".parse().unwrap(),
            &domains
        ));
    }

    #[test]
    fn cancellation_and_deadline_reject_late_results_without_affecting_another_attempt() {
        assert!(check(false, 61, 60).is_ok());
        assert_eq!(check(false, 61, 61).unwrap_err(), "accountError.expired");
        assert_eq!(check(true, 61, 60).unwrap_err(), "accountError.cancelled");
        let first = Login {
            id: "first".into(),
            expires: i64::MAX,
            cancelled: Default::default(),
            desktop: None,
            hidden: false,
        };
        let second = Login {
            id: "second".into(),
            expires: i64::MAX,
            cancelled: Default::default(),
            desktop: None,
            hidden: false,
        };
        *first.cancelled.lock().unwrap() = true;
        assert!(first.clone().check().is_err());
        assert!(second.check().is_ok());
        assert_ne!(first.prefix(), second.prefix());
        let mut writes = 0;
        assert!(first
            .save(|| {
                writes += 1;
                Ok(())
            })
            .is_err());
        second
            .save(|| {
                writes += 1;
                Ok(())
            })
            .unwrap();
        assert!(second
            .save(|| {
                writes += 1;
                Ok(())
            })
            .is_err());
        assert_eq!(writes, 1);
    }

    #[test]
    fn desktop_callback_stays_inside_the_attempt_and_cannot_complete_after_cancel_or_expiry() {
        for (scheme, route) in [
            ("manus", "manus://login"),
            ("cue", "cue://oauth/desktop-callback"),
        ] {
            let desktop = DesktopLogin::new(scheme);
            let nonce = desktop
                .url()
                .query_pairs()
                .find(|(key, _)| key == "nonce")
                .unwrap()
                .1
                .into_owned();
            let mut callback = url::Url::parse(route).unwrap();
            callback
                .query_pairs_mut()
                .append_pair("nonce", &nonce)
                .append_pair("token", "fixture-session");
            let mut login = Login {
                id: uuid::Uuid::new_v4().to_string(),
                expires: chrono::Utc::now().timestamp() + LOGIN_TIMEOUT_SECONDS,
                cancelled: Default::default(),
                desktop: Some(desktop),
                hidden: false,
            };
            assert!(!login.navigate(&callback, &["manus.im"]));
            assert_eq!(
                login.desktop_token().unwrap().as_deref(),
                Some("fixture-session")
            );
            *login.cancelled.lock().unwrap() = true;
            assert!(!login.navigate(&callback, &["manus.im"]));
            assert_eq!(login.desktop_token().unwrap_err(), "accountError.cancelled");
            *login.cancelled.lock().unwrap() = false;
            login.expires = chrono::Utc::now().timestamp() - 1;
            assert!(!login.navigate(&callback, &["manus.im"]));
            assert_eq!(login.desktop_token().unwrap_err(), "accountError.expired");
        }
        let now = chrono::Utc::now().timestamp();
        assert!(check(false, now + LOGIN_TIMEOUT_SECONDS, now + 61).is_ok());
        assert_eq!(
            check(false, now + LOGIN_TIMEOUT_SECONDS, now + 600).unwrap_err(),
            "accountError.expired"
        );
    }
}
