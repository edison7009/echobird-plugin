use super::*;
use serde_json::json;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::protocol::CloseFrame;

#[derive(Clone, Copy)]
enum Mode {
    Normal,
    Silence,
    Error,
    CloseEarly,
    CloseNormally,
    Hold,
    Reject,
}

#[derive(Default)]
struct Recording {
    headers: axum::http::HeaderMap,
    uri: String,
    audio: Vec<u8>,
    controls: Vec<Value>,
}

struct Server {
    url: String,
    recording: Arc<Mutex<Recording>>,
    closed: Arc<AtomicBool>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[allow(clippy::result_large_err)] // The handshake callback must return an HTTP error response.
async fn mock(mode: Mode) -> Server {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let recording = Arc::new(Mutex::new(Recording::default()));
    let observed = recording.clone();
    let closed = Arc::new(AtomicBool::new(false));
    let finished = closed.clone();
    let task = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let capture = observed.clone();
        let socket =
            tokio_tungstenite::accept_hdr_async(tcp, move |req: &Request, res: Response| {
                let mut record = capture.lock().unwrap();
                record.headers = req.headers().clone();
                record.uri = req.uri().to_string();
                if matches!(mode, Mode::Reject) {
                    let mut response = ErrorResponse::new(Some("private upstream error".into()));
                    *response.status_mut() = StatusCode::UNAUTHORIZED;
                    response
                        .headers_mut()
                        .insert("retry-after", "2".parse().unwrap());
                    return Err(response);
                }
                Ok(res)
            })
            .await;
        let Ok(mut socket) = socket else { return };
        let mut first = true;
        while let Some(Ok(message)) = socket.next().await {
            let mut replies = Vec::new();
            match message {
                Message::Binary(bytes) => {
                    observed.lock().unwrap().audio.extend_from_slice(&bytes);
                    match mode {
                        Mode::Normal if first => {
                            replies.push(json!({"type":"TranscriptInterim", "data":"未修订文本"}));
                        }
                        Mode::Normal | Mode::CloseNormally => {
                            replies.push(json!({"type":"TranscriptInterim", "data":"第二段。"}));
                        }
                        Mode::Error => {
                            replies.push(json!({"type":"TranscriptError", "description":"private transcript selected-token"}));
                        }
                        Mode::CloseEarly => {
                            socket
                                .close(Some(CloseFrame {
                                    code: CloseCode::Error,
                                    reason: "private early failure".into(),
                                }))
                                .await
                                .unwrap();
                            break;
                        }
                        _ => {}
                    }
                    first = false;
                }
                Message::Text(text) => {
                    let control: Value = serde_json::from_str(&text).unwrap();
                    observed.lock().unwrap().controls.push(control.clone());
                    if control["type"] == "CloseStream" {
                        match mode {
                            Mode::Normal => {
                                replies
                                    .push(json!({"type":"TranscriptText", "data":" Final tail. "}));
                                replies.push(json!({"type":"TranscriptEndpoint"}));
                            }
                            Mode::Silence => replies.push(json!({"type":"TranscriptEndpoint"})),
                            Mode::CloseNormally => {
                                socket.close(None).await.unwrap();
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
            for reply in replies {
                socket
                    .send(Message::Text(reply.to_string().into()))
                    .await
                    .unwrap();
            }
            if matches!(mode, Mode::Error) && !first {
                break;
            }
        }
        finished.store(true, Ordering::Release);
    });
    Server {
        url,
        recording,
        closed,
        task,
    }
}

fn wav(size: usize) -> Bytes {
    let mut data = vec![0; 44 + size];
    data[..4].copy_from_slice(b"RIFF");
    data[4..8].copy_from_slice(&(36 + size as u32).to_le_bytes());
    data[8..16].copy_from_slice(b"WAVEfmt ");
    data[16..20].copy_from_slice(&16u32.to_le_bytes());
    data[20..22].copy_from_slice(&1u16.to_le_bytes());
    data[22..24].copy_from_slice(&1u16.to_le_bytes());
    data[24..28].copy_from_slice(&16_000u32.to_le_bytes());
    data[28..32].copy_from_slice(&32_000u32.to_le_bytes());
    data[32..34].copy_from_slice(&2u16.to_le_bytes());
    data[34..36].copy_from_slice(&16u16.to_le_bytes());
    data[36..40].copy_from_slice(b"data");
    data[40..44].copy_from_slice(&(size as u32).to_le_bytes());
    for (index, value) in data[44..].iter_mut().enumerate() {
        *value = (index % 251) as u8;
    }
    data.into()
}

fn credentials(server: &Server) -> Value {
    json!({"accessToken":"selected-token", "baseUrl":"https://wrong.invalid", "_voice_test_endpoint":server.url})
}

async fn request(server: &Server) -> Result<String, ProxyError> {
    transcribe(
        &reqwest::Client::new(),
        &credentials(server),
        &wav(80_000),
        "recording.wav",
        "audio/wav",
    )
    .await
}

#[tokio::test]
async fn uses_native_oauth_contract_and_discards_revised_interims() {
    let server = mock(Mode::Normal).await;
    assert_eq!(request(&server).await.unwrap(), "Final tail.");
    let record = server.recording.lock().unwrap();
    assert_eq!(record.headers["authorization"], "Bearer selected-token");
    assert_eq!(record.headers["x-app"], "cli");
    assert_eq!(
        record.headers["anthropic-client-platform"],
        "claude_code_cli"
    );
    assert!(record.headers["user-agent"]
        .to_str()
        .unwrap()
        .starts_with("EchoBird/"));
    let url = url::Url::parse(&format!("http://fixture{}", record.uri)).unwrap();
    assert_eq!(url.path(), "/api/ws/speech_to_text/voice_stream");
    let query: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
    assert_eq!(query["encoding"], "linear16");
    assert_eq!(query["sample_rate"], "16000");
    assert_eq!(query["channels"], "1");
    assert_eq!(query["language"], "en");
    assert_eq!(query["endpointing_ms"], "300");
    assert_eq!(query["utterance_end_ms"], "1000");
    assert_eq!(query["use_conversation_engine"], "true");
    assert_eq!(record.audio, wav(80_000)[44..]);
    assert_eq!(
        record.controls,
        [json!({"type":"KeepAlive"}), json!({"type":"CloseStream"})]
    );
}

#[tokio::test]
async fn allows_empty_final_and_promotes_pending_text_only_after_normal_final_close() {
    let server = mock(Mode::Silence).await;
    assert_eq!(request(&server).await.unwrap(), "");
    let server = mock(Mode::Silence).await;
    assert_eq!(
        transcribe(
            &reqwest::Client::new(),
            &credentials(&server),
            &wav(2),
            "recording.wav",
            "audio/x-wav",
        )
        .await
        .unwrap(),
        ""
    );
    let server = mock(Mode::CloseNormally).await;
    assert_eq!(request(&server).await.unwrap(), "第二段。");
}

#[tokio::test]
async fn preserves_rejected_status_without_returning_upstream_body() {
    let server = mock(Mode::Reject).await;
    let error = request(&server).await.unwrap_err();
    assert_eq!(error.status, StatusCode::UNAUTHORIZED);
    assert_eq!(error.retry_after.unwrap(), "2");
    assert!(!error.message.contains("private"));
    assert!(!error.message.contains("selected-token"));
}

#[tokio::test]
async fn never_returns_partial_text_for_server_error_abnormal_close_or_final_timeout() {
    for (mode, status) in [
        (Mode::Error, StatusCode::BAD_GATEWAY),
        (Mode::CloseEarly, StatusCode::BAD_GATEWAY),
        (Mode::Hold, StatusCode::GATEWAY_TIMEOUT),
    ] {
        let server = mock(mode).await;
        let error = request(&server).await.unwrap_err();
        assert_eq!(error.status, status);
        assert!(!error.message.contains("private"));
        assert!(!error.message.contains("selected-token"));
    }
}

#[tokio::test]
async fn cancellation_drops_the_owned_socket() {
    let server = mock(Mode::Hold).await;
    let credentials = credentials(&server);
    let task = tokio::spawn(async move {
        transcribe(
            &reqwest::Client::new(),
            &credentials,
            &wav(320_000),
            "recording.wav",
            "audio/wav",
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while server.recording.lock().unwrap().audio.is_empty() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::timeout(Duration::from_secs(2), async {
        while !server.closed.load(Ordering::Acquire) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[test]
fn joins_finalized_segments_like_the_native_consumer() {
    let mut text = String::new();
    for segment in [" First segment. ", " ", " Final tail. "] {
        append_segment(&mut text, segment);
    }
    assert_eq!(text, "First segment. Final tail.");
}

#[test]
fn rejects_unsupported_or_malformed_wav_before_network_access() {
    assert!(pcm(b"RIFF").is_err());
    assert!(pcm(&wav(0)).is_err());
    let audio = wav(10);
    assert_eq!(pcm(&audio).unwrap(), &audio[44..]);
    for offset in [0, 4, 8, 12, 16, 20, 22, 24, 28, 32, 34, 36, 40] {
        let mut changed = audio.to_vec();
        changed[offset] ^= 1;
        assert_eq!(pcm(&changed).unwrap_err().status, StatusCode::BAD_REQUEST);
    }
    assert!(pcm(&wav(1)).is_err());
}

#[tokio::test]
async fn invalid_mime_and_empty_pcm_fail_before_connecting() {
    let credentials =
        json!({"accessToken":"selected-token", "_voice_test_endpoint":"ws://127.0.0.1:9"});
    for mime in ["audio/mpeg", "application/octet-stream", ""] {
        let error = transcribe(
            &reqwest::Client::new(),
            &credentials,
            &wav(2),
            "recording.wav",
            mime,
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, StatusCode::BAD_REQUEST);
    }
    let error = transcribe(
        &reqwest::Client::new(),
        &credentials,
        &wav(0),
        "recording.wav",
        "audio/wav",
    )
    .await
    .unwrap_err();
    assert_eq!(error.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn missing_or_invalid_tokens_fail_before_connecting() {
    for token in [Value::Null, json!(""), json!("private\nsecret")] {
        let error = transcribe(
            &reqwest::Client::new(),
            &json!({"accessToken":token}),
            &wav(2),
            "recording.wav",
            "audio/wav",
        )
        .await
        .unwrap_err();
        assert_eq!(error.status, StatusCode::UNAUTHORIZED);
        assert!(!error.message.contains("private"));
    }
}
