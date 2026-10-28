mod operations;
#[allow(dead_code)]
mod services;
mod voice_stream;
#[allow(dead_code)]
mod models {
    pub mod model;
}
mod utils {
    pub mod process;
    #[cfg(not(test))]
    pub mod platform {
        pub fn echobird_dir() -> std::path::PathBuf {
            dirs::home_dir().unwrap_or_default().join(".echobird")
        }
    }
}

use axum::{
    extract::{DefaultBodyLimit, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::post,
    Json, Router,
};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::io::{BufRead, Read, Write};

#[derive(Clone)]
struct Host {
    app: tauri::AppHandle,
    provider: String,
    secret: String,
    client: reqwest::Client,
}

async fn operation(
    State(host): State<Host>,
    headers: HeaderMap,
    Json(request): Json<Value>,
) -> Response {
    if headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        != Some(host.secret.as_str())
    {
        return services::account_proxy::ProxyError::new(
            StatusCode::UNAUTHORIZED,
            "Extension authentication failed",
        )
        .into_response();
    }
    let mut response = match operations::dispatch(&host, &request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    if response
        .extensions()
        .get::<services::account_proxy::ProxyFailureMetadata>()
        .is_some_and(|metadata| metadata.model_scope)
    {
        response.headers_mut().insert(
            "x-echobird-model-scope",
            axum::http::HeaderValue::from_static("1"),
        );
    }
    if response
        .headers()
        .get("content-type")
        .is_some_and(|value| value == "text/event-stream")
    {
        let (parts, body) = response.into_parts();
        let events = body.into_data_stream().map(|chunk| {
            Ok::<_, std::convert::Infallible>(match chunk {
                Ok(bytes) => bytes,
                Err(error) => stream_failure(&error),
            })
        });
        response = Response::from_parts(parts, axum::body::Body::from_stream(events));
    }
    response
}

fn stream_failure(error: &(dyn std::error::Error + 'static)) -> bytes::Bytes {
    let mut source = Some(error);
    while let Some(error) = source {
        if let Some(error) = error.downcast_ref::<services::account_proxy::ProxyError>() {
            let value = json!({"error":{"message":error.message,"code":error.error_code,"type":error.error_type,"status":error.status.as_u16(),"scope":if error.model_scope{"model"}else{"account"},"retry_after":error.retry_after.as_ref().and_then(|value|value.to_str().ok())}});
            return bytes::Bytes::from(format!("data: {value}\n\n"));
        }
        source = error
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
            .map(|inner| inner as &(dyn std::error::Error + 'static))
            .or_else(|| error.source());
    }
    bytes::Bytes::from_static(b"data: {\"error\":{\"code\":\"server_error\",\"status\":502,\"message\":\"Account extension stream failed\"}}\n\n")
}

fn main() {
    let mut input = std::io::BufReader::new(std::io::stdin());
    let mut line = String::new();
    if input
        .by_ref()
        .take(1024 * 1024 + 1)
        .read_line(&mut line)
        .is_err()
        || line.len() > 1024 * 1024
    {
        return;
    }
    let Ok(config) = serde_json::from_str::<Value>(&line) else {
        return;
    };
    let Some(provider) = config["provider"]
        .as_str()
        .filter(|value| {
            [
                "claudecode",
                "claudedesktop",
                "doubaowork",
                "dola",
                "workbuddy",
                "workbuddyai",
                "trae",
                "traecn",
            ]
            .contains(value)
        })
        .map(str::to_owned)
    else {
        return;
    };
    let Some(secret) = config["secret"]
        .as_str()
        .filter(|value| value.len() >= 32)
        .map(str::to_owned)
    else {
        return;
    };
    if services::tool_manager::initialize(config["tools"].clone()).is_err() {
        return;
    }
    let app = tauri::Builder::default()
        .setup(move |app| {
            let host = Host {
                app: app.handle().clone(),
                provider,
                secret,
                client: reqwest::Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .build()?,
            };
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                // The parent owns stdin. EOF ends the worker and its login windows.
                let mut line = String::new();
                while input.read_line(&mut line).is_ok_and(|size| size > 0) {
                    line.clear();
                }
                handle.exit(0);
            });
            tauri::async_runtime::spawn(async move {
                let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
                    Ok(listener) => listener,
                    Err(_) => {
                        host.app.exit(1);
                        return;
                    }
                };
                let address = listener.local_addr().unwrap();
                println!("{}", json!({"protocol":1,"port":address.port()}));
                let _ = std::io::stdout().flush();
                let router = Router::new()
                    .route("/operation", post(operation))
                    .route(
                        "/voice/{account}",
                        axum::routing::get(voice_stream::upgrade),
                    )
                    .layer(DefaultBodyLimit::max(48 * 1024 * 1024))
                    .with_state(host);
                let _ = axum::serve(listener, router).await;
            });
            Ok(())
        })
        .build(tauri::generate_context!());
    if let Ok(app) = app {
        app.run(|_, event| {
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                api.prevent_exit();
            }
        });
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    #[test]
    fn typed_stream_failures_preserve_standard_status_scope_and_retry_metadata() {
        let mut error =
            services::account_proxy::ProxyError::new(StatusCode::TOO_MANY_REQUESTS, "Model limit");
        error.error_code = Some("rate_limit_exceeded".into());
        error.model_scope = true;
        error.retry_after = Some(axum::http::HeaderValue::from_static("172800"));
        let wrapped = std::io::Error::other(error);
        let bytes = stream_failure(&wrapped);
        let text = std::str::from_utf8(&bytes).unwrap();
        let value: Value =
            serde_json::from_str(text.trim().strip_prefix("data: ").unwrap()).unwrap();
        assert_eq!(value["error"]["status"], 429);
        assert_eq!(value["error"]["scope"], "model");
        assert_eq!(value["error"]["retry_after"], "172800");
    }
    #[test]
    fn unknown_stream_failures_do_not_expose_private_diagnostics() {
        let bytes = stream_failure(&std::io::Error::other("private fixture token"));
        let text = std::str::from_utf8(&bytes).unwrap();
        assert!(text.contains("server_error"));
        assert!(!text.contains("private fixture"));
    }
}
