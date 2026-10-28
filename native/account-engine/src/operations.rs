use crate::{
    models::model::{AccountProvider, AccountSource},
    services::{
        self,
        account_proxy::{self, ProxyError},
        claude_code_accounts as code, claude_code_oauth as oauth,
        claude_desktop_accounts as desktop, doubao_accounts as doubao, trae_accounts as trae,
        workbuddy_accounts as work,
    },
    Host,
};
use axum::{http::StatusCode, response::Response};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use serde_json::{json, Value};

fn failure(error: String) -> ProxyError {
    ProxyError::new(StatusCode::BAD_REQUEST, error)
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, ProxyError> {
    value[key]
        .as_str()
        .ok_or_else(|| failure("accountError.format".into()))
}
fn response(value: impl Serialize) -> Result<Response, ProxyError> {
    Ok(account_proxy::json_response(
        &serde_json::to_value(value).map_err(|_| failure("accountError.format".into()))?,
    ))
}
fn provider(id: &str) -> Result<AccountProvider, ProxyError> {
    Ok(match id {
        "claudecode" => AccountProvider::ClaudeCode,
        "workbuddy" => AccountProvider::WorkBuddy,
        "workbuddyai" => AccountProvider::WorkBuddyAi,
        "doubaowork" => AccountProvider::DoubaoWork,
        "dola" => AccountProvider::Dola,
        "trae" => AccountProvider::Trae,
        "traecn" => AccountProvider::Traecn,
        _ => return Err(failure("accountError.unavailable".into())),
    })
}

pub(crate) async fn dispatch(host: &Host, request: &Value) -> Result<Response, ProxyError> {
    let method = text(request, "operation")?;
    let input = &request["params"];
    let id = input["accountId"].as_str().unwrap_or_default();
    let login = input["loginId"].as_str().unwrap_or_default();
    let trae_edition = if host.provider == "traecn" {
        trae::Edition::Traecn
    } else {
        trae::Edition::Trae
    };
    let work_edition = if host.provider == "workbuddyai" {
        work::Edition::Ai
    } else {
        work::Edition::Cn
    };
    let doubao_edition = if host.provider == "dola" {
        doubao::Edition::Dola
    } else {
        doubao::Edition::DoubaoWork
    };
    match method {
        "accounts.options" => {
            let account_provider = provider(&host.provider)?;
            let saved = match host.provider.as_str() {
                "trae" | "traecn" => serde_json::to_value(
                    trae::list_saved_accounts(trae_edition)
                        .await
                        .map_err(failure)?,
                ),
                "claudecode" => {
                    serde_json::to_value(code::list_saved_accounts().await.map_err(failure)?)
                }
                "workbuddy" | "workbuddyai" => serde_json::to_value(
                    work::list_saved_accounts(work_edition)
                        .await
                        .map_err(failure)?,
                ),
                _ => serde_json::to_value(doubao::list(doubao_edition).await.map_err(failure)?),
            }
            .map_err(|_| failure("accountError.format".into()))?;
            let accounts:Vec<Value>=saved.as_array().into_iter().flatten().map(|account|{
                let id=account["id"].as_str().unwrap_or_default();
                let label=account[if host.provider=="claudecode"{"email"}else{"name"}].as_str().unwrap_or(id);
                json!({"source":{"provider":account_provider,"accountId":id},"label":label,"plan":account["plan"],"modelLabels":account_proxy::cached_model_labels(account_provider,id),"freeModelIds":account_proxy::cached_free_model_ids(account_provider,id)})
            }).collect();
            response(accounts)
        }
        "accounts" | "accounts.saved" => match host.provider.as_str() {
            "trae" | "traecn" => response(
                if method == "accounts" {
                    trae::list_accounts(trae_edition).await
                } else {
                    trae::list_saved_accounts(trae_edition).await
                }
                .map_err(failure)?,
            ),
            "claudecode" => response(
                if method == "accounts" {
                    code::list().await
                } else {
                    code::list_saved_accounts().await
                }
                .map_err(failure)?,
            ),
            "claudedesktop" => response(desktop::list().await.map_err(failure)?),
            "workbuddy" | "workbuddyai" => response(
                if method == "accounts" {
                    work::list(work_edition).await
                } else {
                    work::list_saved_accounts(work_edition).await
                }
                .map_err(failure)?,
            ),
            _ => response(doubao::list(doubao_edition).await.map_err(failure)?),
        },
        "login.start" => match host.provider.as_str() {
            "trae" | "traecn" => response(trae::start_login(trae_edition).await.map_err(failure)?),
            "claudecode" => response(oauth::start().map_err(failure)?),
            "claudedesktop" => response(
                desktop::start_login(host.app.clone())
                    .await
                    .map_err(failure)?,
            ),
            "workbuddy" | "workbuddyai" => {
                response(work::start_login(work_edition).await.map_err(failure)?)
            }
            _ => response(
                doubao::start_login(host.app.clone(), doubao_edition)
                    .await
                    .map_err(failure)?,
            ),
        },
        "login.complete" => response(
            oauth::complete(login, text(input, "code")?)
                .await
                .map_err(failure)?,
        ),
        "login.poll" => match host.provider.as_str() {
            "trae" | "traecn" => response(trae::poll_login(login).await.map_err(failure)?),
            "claudedesktop" => response(
                desktop::poll_login(&host.app, login)
                    .await
                    .map_err(failure)?,
            ),
            "workbuddy" | "workbuddyai" => {
                response(work::poll_login(login).await.map_err(failure)?)
            }
            "doubaowork" | "dola" => response(
                doubao::poll_login(&host.app, login)
                    .await
                    .map_err(failure)?,
            ),
            _ => Err(failure("accountError.unavailable".into())),
        },
        "login.cancel" => {
            match host.provider.as_str() {
                "trae" | "traecn" => trae::cancel_login(login),
                "claudecode" => oauth::cancel(login),
                "claudedesktop" => desktop::cancel_login(&host.app, login).await,
                "workbuddy" | "workbuddyai" => work::cancel_login(login),
                _ => doubao::cancel_login(&host.app, login),
            }
            .map_err(failure)?;
            response(Value::Null)
        }
        "account.apply" => match host.provider.as_str() {
            "trae" | "traecn" => response(
                trae::activate_account(trae_edition, id)
                    .await
                    .map_err(failure)?,
            ),
            "claudecode" => response(code::switch(id).await.map_err(failure)?),
            "claudedesktop" => response(desktop::switch(id).await.map_err(failure)?),
            "workbuddy" | "workbuddyai" => {
                response(work::switch(work_edition, id).await.map_err(failure)?)
            }
            _ => response(doubao::apply(doubao_edition, id).await.map_err(failure)?),
        },
        "account.refresh" => match host.provider.as_str() {
            "trae" | "traecn" => response(
                trae::refresh_quota(trae_edition, id)
                    .await
                    .map_err(failure)?,
            ),
            "claudecode" => response(code::refresh(id).await.map_err(failure)?),
            "claudedesktop" => response(desktop::refresh(id).await.map_err(failure)?),
            "workbuddy" | "workbuddyai" => {
                response(work::refresh(work_edition, id).await.map_err(failure)?)
            }
            _ => response(doubao::refresh(doubao_edition, id).await.map_err(failure)?),
        },
        "account.claim" => match host.provider.as_str() {
            "trae" | "traecn" => {
                response(trae::claim_daily(trae_edition, id).await.map_err(failure)?)
            }
            "workbuddy" | "workbuddyai" => {
                response(work::claim_daily(work_edition, id).await.map_err(failure)?)
            }
            "doubaowork" | "dola" => response(
                doubao::claim_daily(doubao_edition, id)
                    .await
                    .map_err(failure)?,
            ),
            _ => Err(failure("accountError.unavailable".into())),
        },
        "account.remove" => {
            match host.provider.as_str() {
                "trae" | "traecn" => trae::delete_account(trae_edition, id).await,
                "claudecode" => code::delete(id).await,
                "claudedesktop" => desktop::delete(id).await,
                "workbuddy" | "workbuddyai" => work::delete(work_edition, id).await,
                _ => doubao::delete(doubao_edition, id).await,
            }
            .map_err(failure)?;
            response(Value::Null)
        }
        "history.merge" => {
            if host.provider == "claudedesktop" {
                if input["onlyIfStopped"] == true {
                    services::claude_desktop_history::prepare_if_stopped().await
                } else {
                    services::claude_desktop_history::prepare(None).await
                }
                .map_err(failure)?;
            }
            response(Value::Null)
        }
        "connection" => {
            if matches!(host.provider.as_str(), "trae" | "traecn") {
                let metadata = trae::connection_metadata(trae_edition, id).map_err(failure)?;
                return response(account_proxy::trae_connection_base(
                    provider(&host.provider)?,
                    &metadata,
                )?);
            }
            let base = match host.provider.as_str() {
                "claudecode" => "https://api.anthropic.com",
                "workbuddy" => "https://copilot.tencent.com",
                "workbuddyai" => "https://www.workbuddy.ai",
                "doubaowork" => "https://www.doubao.com",
                "dola" => "https://www.dola.com",
                _ => return Err(failure("accountError.unavailable".into())),
            };
            response(base)
        }
        "models" | "proxy" | "voice" => {
            let source = AccountSource {
                provider: provider(&host.provider)?,
                account_id: id.into(),
            };
            if method == "voice" {
                let audio = bytes::Bytes::from(
                    STANDARD
                        .decode(text(input, "audio")?)
                        .map_err(|_| failure("Invalid audio".into()))?,
                );
                return response(account_proxy::voice::transcribe_with(
                    &host.client,
                    account_proxy::voice::SpeechRequest { provider:source.provider, audio:&audio, filename:input["filename"].as_str().unwrap_or("speech.wav"), mime:input["mime"].as_str().unwrap_or("audio/wav") },
                    |rejected| {
                        let source = &source;
                        async move {
                            if let Some(rejected) = rejected {
                                if source.provider == AccountProvider::DoubaoWork {
                                    doubao::speech::speech_credentials(id, Some(&rejected)).await
                                } else {
                                    services::account_model_sources::credentials_after_rejection(source,&rejected).await
                                }
                            } else if source.provider == AccountProvider::DoubaoWork {
                                doubao::speech::speech_credentials(id, input.get("rejected")).await
                            } else {
                                services::account_model_sources::credentials(source, false).await
                            }
                        }
                    }
                ).await?);
            }
            let mut credentials = services::account_model_sources::credentials(&source, false)
                .await
                .map_err(failure)?;
            credentials["_echobird_saved_account_id"] = json!(id);
            if method == "models" {
                return response(
                    account_proxy::list_models(source.provider, credentials, &host.client).await?,
                );
            }
            let model = text(input, "model")?;
            for attempt in 0..2 {
                let result = account_proxy::route(
                    source.provider,
                    credentials.clone(),
                    model,
                    input["body"].clone(),
                    &host.client,
                )
                .await;
                match result {
                    Err(error) if error.status == StatusCode::UNAUTHORIZED && attempt == 0 => {
                        credentials = services::account_model_sources::credentials_after_rejection(
                            &source,
                            &credentials,
                        )
                        .await
                        .map_err(failure)?
                    }
                    result => return result,
                }
            }
            unreachable!()
        }
        "metadata" => response(
            json!({"labels":account_proxy::cached_model_labels(provider(&host.provider)?,id),"freeModelIds":account_proxy::cached_free_model_ids(provider(&host.provider)?,id)}),
        ),
        _ => Err(failure("Unknown account extension operation".into())),
    }
}
