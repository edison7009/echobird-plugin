use super::{
    claude_voice, doubao_voice, trae_global_voice, trae_voice, workbuddy_voice, ProxyError,
};
use crate::models::model::AccountProvider;
use axum::{body::Bytes, http::StatusCode};
use serde_json::Value;

pub(crate) fn media_timeout(size: usize) -> std::time::Duration {
    std::time::Duration::from_secs(60 + (size as u64 / 32000))
}
pub(crate) struct SpeechRequest<'a> {
    pub provider: AccountProvider,
    pub audio: &'a Bytes,
    pub filename: &'a str,
    pub mime: &'a str,
}
pub(crate) async fn transcribe_with<F, Fut>(
    client: &reqwest::Client,
    request: SpeechRequest<'_>,
    mut resolve: F,
) -> Result<String, ProxyError>
where
    F: FnMut(Option<Value>) -> Fut,
    Fut: std::future::Future<Output = Result<Value, String>>,
{
    match request.provider {
        AccountProvider::Trae => trae_global_voice::validate_audio(request.audio, request.mime)?,
        AccountProvider::Traecn => trae_voice::validate_audio(request.audio, request.mime)?,
        _ => {}
    }
    let mut credentials = resolve(None)
        .await
        .map_err(super::super::account_model_sources::credential_error)?;
    for attempt in 0..2 {
        let result = match request.provider {
            AccountProvider::Trae => {
                trae_global_voice::transcribe(
                    client,
                    &credentials,
                    request.audio,
                    request.filename,
                    request.mime,
                )
                .await
            }
            AccountProvider::Traecn => {
                trae_voice::transcribe(
                    client,
                    &credentials,
                    request.audio,
                    request.filename,
                    request.mime,
                )
                .await
            }
            AccountProvider::ClaudeCode => {
                claude_voice::transcribe(
                    client,
                    &credentials,
                    request.audio,
                    request.filename,
                    request.mime,
                )
                .await
            }
            AccountProvider::WorkBuddy | AccountProvider::WorkBuddyAi => {
                workbuddy_voice::transcribe_for_provider(
                    client,
                    request.provider,
                    &credentials,
                    request.audio,
                    request.filename,
                    request.mime,
                )
                .await
            }
            AccountProvider::DoubaoWork => {
                doubao_voice::transcribe(
                    client,
                    &credentials,
                    request.audio,
                    request.filename,
                    request.mime,
                )
                .await
            }
            _ => {
                return Err(ProxyError::new(
                    StatusCode::BAD_REQUEST,
                    "accountError.unavailable",
                ))
            }
        };

        match result {
            Err(error) if error.status == StatusCode::UNAUTHORIZED && attempt == 0 => {
                credentials = resolve(Some(credentials))
                    .await
                    .map_err(super::super::account_model_sources::credential_error)?;
            }
            result => return result,
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    #[tokio::test]
    async fn trae_compatibility_invalid_cn_audio_does_not_load_or_refresh_credentials() {
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = calls.clone();
        let audio = Bytes::from_static(b"not a wave file");
        let error = transcribe_with(
            &reqwest::Client::new(),
            SpeechRequest {
                provider: AccountProvider::Traecn,
                audio: &audio,
                filename: "speech.wav",
                mime: "audio/wav",
            },
            move |_| {
                observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                std::future::ready(Ok(
                    json!({"edition":"traecn","region":"CN","accessToken":"fixture"}),
                ))
            },
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}
