use crate::{
    models::model::{AccountProvider, AccountSource},
    services::{
        account_model_sources,
        account_proxy::{workbuddy_voice::stream, ProxyError},
    },
    Host,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::{HeaderMap, StatusCode},
    response::Response,
};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;

pub(crate) async fn upgrade(
    State(host): State<Host>,
    Path(account): Path<String>,
    headers: HeaderMap,
    upgrade: WebSocketUpgrade,
) -> Response {
    if host.provider != "workbuddy"
        || headers
            .get("authorization")
            .and_then(|value| value.to_str().ok())
            != Some(host.secret.as_str())
    {
        return ProxyError::new(StatusCode::UNAUTHORIZED, "Extension authentication failed")
            .into_response();
    }
    let source = AccountSource {
        provider: AccountProvider::WorkBuddy,
        account_id: account,
    };
    let connection = stream::connect_with(|rejected| {
        let source = &source;
        async move {
            match rejected {
                Some(rejected) => {
                    account_model_sources::credentials_after_rejection(source, &rejected).await
                }
                None => account_model_sources::credentials(source, false).await,
            }
        }
    })
    .await;
    match connection {
        Ok((socket, ready)) => upgrade
            .max_message_size(1024 * 1024)
            .on_upgrade(move |client| relay(client, socket, ready)),
        Err(error) => error.into_response(),
    }
}

async fn relay(client: WebSocket, upstream: stream::Socket, ready: stream::Event) {
    let (mut client_sink, mut client_events) = client.split();
    let (mut upstream_sink, mut upstream_events) = upstream.split();
    if client_sink
        .send(Message::Text(
            json!({"text":ready.text,"finalResult":ready.final_result})
                .to_string()
                .into(),
        ))
        .await
        .is_err()
    {
        return;
    }
    let send = async {
        while let Some(Ok(message)) = client_events.next().await {
            match message {
                Message::Binary(bytes) => {
                    upstream_sink
                        .send(tokio_tungstenite::tungstenite::Message::Binary(bytes))
                        .await
                        .ok()?;
                }
                Message::Close(_) => {
                    let _ = upstream_sink.close().await;
                    return Some(());
                }
                _ => {}
            }
        }
        Some(())
    };
    let receive = async {
        while let Some(Ok(message)) = upstream_events.next().await {
            match stream::event(message) {
                Ok(Some(event)) => {
                    client_sink
                        .send(Message::Text(
                            json!({"text":event.text,"finalResult":event.final_result})
                                .to_string()
                                .into(),
                        ))
                        .await
                        .ok()?;
                }
                Err(_) => {
                    let _ = client_sink
                        .send(Message::Text(
                            json!({"error":"transcribe"}).to_string().into(),
                        ))
                        .await;
                    return None;
                }
                _ => {}
            }
        }
        Some(())
    };
    tokio::select! {_=send=>{},_=receive=>{}}
}
