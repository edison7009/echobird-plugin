//! Claude's web API answers only requests from a claude.ai page; direct
//! client requests receive a bot challenge. Requests therefore run inside the
//! isolated login browser, which sends its own session cookies.
use super::*;
use std::time::Instant;
use tauri::webview::Cookie as WebCookie;

const HOST: &str = "claude.ai";
/// Hosts the background browser may load: Claude, and the challenge frame
/// Claude's edge may show before serving its pages.
pub(super) const BACKGROUND_DOMAINS: &[&str] = &["claude.ai", "challenges.cloudflare.com"];
/// A small first-party document. Opening it as a page lets the browser
/// complete any edge challenge before requests are made from it.
const BACKGROUND_PAGE: &str = "https://claude.ai/api/bootstrap";
const EVAL_TIMEOUT: Duration = Duration::from_secs(5);
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(15);
const RETRY_DELAY: Duration = Duration::from_millis(500);
/// Slower retries while the edge challenges the page, so it is not flooded.
const CHALLENGE_DELAY: Duration = Duration::from_secs(2);
const READ_DELAY: Duration = Duration::from_millis(200);
const CHALLENGED: &str = "accountError.quota|HTTP 403";

/// Result of one page request. `Waiting` means the page cannot answer yet
/// (not on Claude, navigating, or challenged) and holds the error to report
/// if the caller stops waiting.
#[derive(Debug, PartialEq)]
pub(super) enum Fetch {
    Ready(Value),
    Waiting(&'static str),
}

#[derive(Deserialize)]
struct Reply {
    status: i64,
    #[serde(default, rename = "type")]
    content_type: String,
    #[serde(default)]
    body: String,
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).expect("strings serialize")
}

fn start_script(key: &str, endpoint: &str, org: &str, timeout: Duration) -> String {
    // The result is stored only while EchoBird still waits for it; an
    // abandoned request is aborted and leaves nothing on the page.
    format!(
        r#"(() => {{
  const key = {key};
  if (location.protocol !== "https:" || location.hostname !== {host}) return "offsite";
  window[key] = null;
  const store = (value) => {{
    if (window[key] === null) window[key] = value;
  }};
  fetch({url}, {{
    credentials: "include",
    redirect: "manual",
    cache: "no-store",
    signal: AbortSignal.timeout({timeout}),
    headers: {{ accept: "application/json", "x-organization-uuid": {org} }},
  }})
    .then(async (response) => {{
      const body = await response.text();
      store({{
        status: response.status,
        type: response.headers.get("content-type") || "",
        body,
      }});
    }})
    .catch(() => store({{ status: -1, type: "", body: "" }}));
  return "started";
}})()"#,
        key = quoted(key),
        host = quoted(HOST),
        url = quoted(&format!("/api/{endpoint}")),
        org = quoted(org),
        timeout = timeout.as_millis(),
    )
}

fn discard_script(key: &str) -> String {
    format!("delete window[{}];", quoted(key))
}

fn read_script(key: &str) -> String {
    format!(
        r#"(() => {{
  const key = {key};
  if (!(key in window)) return "gone";
  const value = window[key];
  if (value === null) return "pending";
  delete window[key];
  return JSON.stringify(value);
}})()"#,
        key = quoted(key)
    )
}

/// Runs a script and returns the string it produced; every platform hands
/// the result back JSON-encoded. `None` means the page gave no answer: some
/// platforms drop results of scripts run before a page finishes loading, and
/// a page that is navigating may not answer in time.
async fn eval(window: &tauri::WebviewWindow, script: String, limit: Duration) -> Option<String> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let sender = std::sync::Mutex::new(Some(sender));
    window
        .eval_with_callback(script, move |raw| {
            if let Some(sender) = sender.lock().ok().and_then(|mut sender| sender.take()) {
                let _ = sender.send(raw);
            }
        })
        .ok()?;
    let raw = tokio::time::timeout(limit.min(EVAL_TIMEOUT), receiver)
        .await
        .ok()?
        .ok()?;
    serde_json::from_str::<String>(&raw).ok()
}

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

/// Maps a page response onto the errors direct requests used to report.
fn interpret(reply: Reply) -> Result<Fetch, String> {
    let json = reply.content_type.contains("json");
    match reply.status {
        -1 => Ok(Fetch::Waiting("accountError.network")),
        401 => Err("accountError.loginRequired".into()),
        403 if !json => Ok(Fetch::Waiting(CHALLENGED)),
        200..=299 => serde_json::from_str(&reply.body)
            .map(Fetch::Ready)
            .map_err(|_| "accountError.authResponse".into()),
        0 => Err("accountError.authResponse".into()),
        status => Err(format!("accountError.quota|HTTP {status}")),
    }
}

/// One request, ending by `deadline`: start it, then read until it
/// finishes, the page goes away or the time is up.
async fn attempt(
    window: &tauri::WebviewWindow,
    endpoint: &str,
    org: &str,
    deadline: Instant,
) -> Result<Fetch, String> {
    let deadline = deadline.min(Instant::now() + ATTEMPT_TIMEOUT);
    let waiting = Ok(Fetch::Waiting("accountError.network"));
    let key = format!("__echobird_{}", uuid::Uuid::new_v4().simple());
    let start = start_script(&key, endpoint, org, remaining(deadline));
    if eval(window, start, remaining(deadline)).await.as_deref() != Some("started") {
        return waiting;
    }
    while Instant::now() < deadline {
        tokio::time::sleep(READ_DELAY.min(remaining(deadline))).await;
        match eval(window, read_script(&key), remaining(deadline))
            .await
            .as_deref()
        {
            // No answer yet: the page may be busy; keep reading.
            None | Some("pending") => continue,
            // The page navigated and dropped the request.
            Some("gone") => return waiting,
            Some(raw) => {
                let reply = serde_json::from_str(raw).map_err(|_| "accountError.authResponse")?;
                return interpret(reply);
            }
        }
    }
    let _ = window.eval(discard_script(&key));
    waiting
}

/// Requests `/api/{endpoint}` from the window's Claude page, retrying while
/// the page cannot answer until `deadline`. `check` reports a closed or
/// expired browser attempt, which ends the wait with its own error.
pub(super) async fn get(
    window: &tauri::WebviewWindow,
    endpoint: &str,
    org: &str,
    deadline: Instant,
    check: impl Fn() -> Result<(), String>,
) -> Result<Fetch, String> {
    let mut waiting = "accountError.network";
    while Instant::now() < deadline {
        check()?;
        match attempt(window, endpoint, org, deadline).await? {
            Fetch::Ready(value) => return Ok(Fetch::Ready(value)),
            Fetch::Waiting(reason) => waiting = reason,
        }
        let delay = if waiting == CHALLENGED {
            CHALLENGE_DELAY
        } else {
            RETRY_DELAY
        };
        tokio::time::sleep(delay.min(remaining(deadline))).await;
    }
    check()?;
    Ok(Fetch::Waiting(waiting))
}

pub(super) fn profile_endpoint(org: &str) -> String {
    format!("bootstrap/{org}/app_start?statsig_hashing_algorithm=djb2&growthbook_format=sdk&include_system_prompts=false")
}

pub(super) fn usage_endpoint(org: &str) -> String {
    format!("organizations/{org}/usage")
}

/// Gives a background browser the saved session, then opens a Claude page.
/// Cookie writes report no platform failure, so the stored session is read
/// back before use.
pub(super) fn sign_in(
    window: &tauri::WebviewWindow,
    session: &str,
    org: &str,
) -> Result<(), String> {
    for cookie in [
        WebCookie::build(("sessionKey", session.to_owned()))
            .domain(".claude.ai")
            .path("/")
            .secure(true)
            .http_only(true)
            .same_site(tauri::webview::cookie::SameSite::Lax)
            .build(),
        WebCookie::build(("lastActiveOrg", org.to_owned()))
            .domain(".claude.ai")
            .path("/")
            .secure(true)
            .same_site(tauri::webview::cookie::SameSite::Lax)
            .build(),
    ] {
        window
            .set_cookie(cookie)
            .map_err(|_| "accountError.browser")?;
    }
    let stored = window
        .cookies_for_url(
            "https://claude.ai/"
                .parse()
                .map_err(|_| "accountError.format")?,
        )
        .map_err(|_| "accountError.browser")?;
    for (name, value) in [("sessionKey", session), ("lastActiveOrg", org)] {
        if !stored
            .iter()
            .any(|cookie| cookie.name() == name && cookie.value() == value)
        {
            return Err("accountError.browser".into());
        }
    }
    window
        .navigate(BACKGROUND_PAGE.parse().map_err(|_| "accountError.format")?)
        .map_err(|_| "accountError.browser".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(status: i64, content_type: &str, body: &str) -> Reply {
        Reply {
            status,
            content_type: content_type.into(),
            body: body.into(),
        }
    }

    #[test]
    fn page_responses_keep_the_direct_request_error_mapping() {
        assert_eq!(
            interpret(reply(200, "application/json", r#"{"account":1}"#)).unwrap(),
            Fetch::Ready(json!({"account":1}))
        );
        assert_eq!(
            interpret(reply(401, "application/json", "{}")).unwrap_err(),
            "accountError.loginRequired"
        );
        assert_eq!(
            interpret(reply(
                403,
                "text/html; charset=UTF-8",
                "<title>Just a moment...</title>"
            ))
            .unwrap(),
            Fetch::Waiting(CHALLENGED)
        );
        assert_eq!(
            interpret(reply(403, "application/json", "{}")).unwrap_err(),
            "accountError.quota|HTTP 403"
        );
        assert_eq!(
            interpret(reply(500, "text/html", "")).unwrap_err(),
            "accountError.quota|HTTP 500"
        );
        assert_eq!(
            interpret(reply(-1, "", "")).unwrap(),
            Fetch::Waiting("accountError.network")
        );
        assert_eq!(
            interpret(reply(0, "", "")).unwrap_err(),
            "accountError.authResponse"
        );
        assert_eq!(
            interpret(reply(200, "application/json", "not json")).unwrap_err(),
            "accountError.authResponse"
        );
    }

    #[test]
    fn page_scripts_quote_every_value_and_stay_on_claude() {
        let org = "12345678-1234-1234-1234-123456789abc";
        let script = start_script(
            "__echobird_k",
            &usage_endpoint(org),
            org,
            Duration::from_millis(1500),
        );
        assert!(script.contains("AbortSignal.timeout(1500)"));
        assert!(script.contains("if (window[key] === null) window[key] = value;"));
        assert!(script.contains(r#"location.hostname !== "claude.ai""#));
        assert!(script.contains(&format!(r#""/api/organizations/{org}/usage""#)));
        assert!(script.contains(r#"credentials: "include""#));
        let hostile = start_script("k", "x\"); alert(1); (\"", org, ATTEMPT_TIMEOUT);
        assert!(hostile.contains(r#""/api/x\"); alert(1); (\"""#));
        assert!(read_script("__echobird_k").contains(r#"const key = "__echobird_k";"#));
        assert_eq!(
            discard_script("__echobird_k"),
            r#"delete window["__echobird_k"];"#
        );
    }
}
