use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager, State};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::oneshot;
use tokio::sync::Mutex as AsyncMutex;
use tracing::{error, info, warn};

const GEMMA_MODEL: &str = "mlx-community/gemma-3n-E4B-it-4bit";
const GEMMA_CACHE_DIR: &str = "models--mlx-community--gemma-3n-E4B-it-4bit";
const CSM_MODEL_REPO: &str = "senstella/csm-expressiva-1b";
const CSM_CACHE_DIR: &str = "models--senstella--csm-expressiva-1b";
const CSM_MODEL_FILE: &str = "mlx-ckpt.safetensors";
const CSM_SPEAKER_ID: u32 = 4;
const SILENCE_THRESHOLD: f32 = 0.02;
const SILENCE_DURATION_CHUNKS: usize = 150;
const MIN_SPEAKING_CHUNKS: usize = 10;
const SAMPLE_RATE: u32 = 44_100;
const CSM_MAX_AUDIO_LENGTH_MS: u32 = 10_000;
const CSM_TEMPERATURE: f32 = 0.3;
const CSM_TOP_K: u32 = 20;
const CSM_AUDIO_START_EVENT: &str = "csm-audio-start";
const CSM_AUDIO_CHUNK_EVENT: &str = "csm-audio-chunk";
const CSM_AUDIO_DONE_EVENT: &str = "csm-audio-done";
const CSM_AUDIO_STOP_EVENT: &str = "csm-audio-stop";
const CSM_ERROR_EVENT: &str = "csm-error";
const CSM_STATUS_EVENT: &str = "csm-status";
const CALL_STAGE_EVENT: &str = "call-stage";
const MODEL_DOWNLOAD_EVENT: &str = "model-download-progress";
const CSM_STARTUP_TIMEOUT_SECS: u64 = 180;
const CSM_STDERR_TAIL_LIMIT: usize = 8;
const CSM_MALE_REFERENCE_AUDIO_FILE: &str = "sample-male.mp3";
const CSM_FEMALE_REFERENCE_AUDIO_FILE: &str = "sample-female.mp3";
const MAX_CONVERSATION_TURNS: usize = 12;
const MAX_SPOKEN_SENTENCES: usize = 2;
const VOICE_SYSTEM_PROMPT: &str = "You are in a live voice call. Reply like a natural spoken conversation. Use plain sentences only. Never use markdown, bullets, headings, numbered lists, code fences, tables, emojis, or stage directions. Keep responses concise, direct, and easy to speak aloud. Respond with no more than 2 short sentences.";
const TRANSCRIPTION_PROMPT: &str =
    "Transcribe exactly what the user said in the audio. Return only the transcript as plain text. No markdown, no quotes, no commentary.";

#[derive(Clone, Copy)]
enum CsmVoice {
    Male,
    Female,
}

impl CsmVoice {
    fn from_key(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "male" => Ok(Self::Male),
            "female" => Ok(Self::Female),
            other => Err(format!("Unsupported CSM voice: {other}")),
        }
    }

    fn file_name(self) -> &'static str {
        match self {
            Self::Male => CSM_MALE_REFERENCE_AUDIO_FILE,
            Self::Female => CSM_FEMALE_REFERENCE_AUDIO_FILE,
        }
    }
}

#[derive(Clone, Deserialize)]
struct AudioPayload {
    data: Vec<f32>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: Vec<ChatContent>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ChatContent {
    Text { text: String },
    InputAudio { input_audio: InputAudio },
}

#[derive(Serialize)]
struct InputAudio {
    data: String,
    format: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    max_tokens: u32,
    stream: bool,
}

#[derive(Clone)]
struct CsmProcess {
    child: Arc<AsyncMutex<Child>>,
    stdin: Arc<AsyncMutex<ChildStdin>>,
}

#[derive(Clone)]
struct ConversationTurn {
    user_text: String,
    assistant_text: String,
}

struct ActiveGeneration {
    id: u64,
    handle: tauri::async_runtime::JoinHandle<()>,
}

#[derive(Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
}

#[derive(Deserialize)]
struct ChatCompletionChoice {
    message: ChatCompletionMessage,
}

#[derive(Deserialize)]
struct ChatCompletionMessage {
    content: Option<serde_json::Value>,
}

struct AppState {
    audio_buffer: Mutex<Vec<f32>>,
    silent_chunks_count: Mutex<usize>,
    speaking_chunks_count: Mutex<usize>,
    is_speaking: Mutex<bool>,
    server_process: Mutex<Option<tauri_plugin_shell::process::CommandChild>>,
    server_port: Mutex<Option<u16>>,
    csm_process: Mutex<Option<CsmProcess>>,
    csm_ready: Mutex<bool>,
    csm_startup_message: Mutex<Option<String>>,
    csm_stderr_tail: Mutex<VecDeque<String>>,
    selected_csm_voice: Mutex<CsmVoice>,
    selected_csm_quantized: Mutex<bool>,
    next_csm_request_id: AtomicU64,
    next_generation_id: AtomicU64,
    active_generation: Mutex<Option<ActiveGeneration>>,
    conversation_turns: Mutex<VecDeque<ConversationTurn>>,
    conversation_session_id: AtomicU64,
}

#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CsmWorkerEvent {
    Status {
        message: String,
    },
    Ready {
        sample_rate: Option<u32>,
    },
    Chunk {
        request_id: u64,
        audio_wav_base64: String,
    },
    Done {
        request_id: u64,
    },
    Error {
        request_id: Option<u64>,
        message: String,
    },
}

#[derive(Clone, Serialize)]
struct CsmAudioStartEvent {
    request_id: u64,
    text: String,
    total_segments: usize,
}

#[derive(Clone, Serialize)]
struct CsmAudioChunkEvent {
    request_id: u64,
    audio_wav_base64: String,
}

#[derive(Clone, Serialize)]
struct CsmAudioDoneEvent {
    request_id: u64,
}

#[derive(Clone, Serialize)]
struct CsmAudioStopEvent {}

#[derive(Clone, Serialize)]
struct CsmErrorEvent {
    request_id: Option<u64>,
    message: String,
}

#[derive(Clone, Serialize)]
struct CsmStatusEvent {
    message: String,
}

#[derive(Clone, Serialize)]
struct CallStageEvent {
    phase: String,
    message: String,
}

#[derive(Clone, Serialize)]
struct ModelDownloadEvent {
    model: String,
    phase: String,
    message: String,
    progress: Option<f32>,
    indeterminate: bool,
}

#[derive(Clone, Deserialize)]
struct DownloadWorkerEvent {
    #[serde(rename = "type")]
    event_type: String,
    model: String,
    message: String,
    progress: Option<f32>,
    #[serde(default)]
    indeterminate: bool,
}

#[tauri::command]
fn ping() {
    info!("Backend: ping command received");
}

#[tauri::command]
async fn reset_call_session(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    cancel_active_generation(&app_handle, false).await;
    reset_call_session_state(state.inner());
    reset_csm_reference_context(&app_handle).await?;
    Ok(())
}

#[tauri::command]
async fn is_server_running(state: State<'_, AppState>) -> Result<bool, String> {
    let port = {
        let port_guard = state.server_port.lock().unwrap();
        *port_guard
    };
    let Some(port) = port else {
        return Ok(false);
    };

    let client = reqwest::Client::new();
    let url = format!("{}/v1/models", server_base_url(port));
    match client.get(url).send().await {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(_) => Ok(false),
    }
}

#[tauri::command]
async fn is_csm_running(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(csm_process_is_ready(state.inner()).await)
}

#[tauri::command]
async fn start_server(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let port;
    {
        let mut process_guard = state.server_process.lock().unwrap();
        let mut port_guard = state.server_port.lock().unwrap();
        if process_guard.is_some() {
            return Ok(());
        }

        port = reserve_free_port()?;
        info!("Starting MLX Server on port {}...", port);
        let port_arg = port.to_string();
        let sidecar_command = app_handle
            .shell()
            .sidecar("mlx-handler")
            .map_err(|e| e.to_string())?
            .args(&["--server", "--model", GEMMA_MODEL, "--port", &port_arg]);

        let (mut rx, child) = sidecar_command.spawn().map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(line) => {
                        info!(
                            "MLX Server stdout: {}",
                            String::from_utf8_lossy(&line).trim()
                        );
                    }
                    CommandEvent::Stderr(line) => {
                        error!(
                            "MLX Server stderr: {}",
                            String::from_utf8_lossy(&line).trim()
                        );
                    }
                    CommandEvent::Error(err) => {
                        error!("MLX Server process error: {}", err);
                    }
                    CommandEvent::Terminated(payload) => {
                        info!(
                            "MLX Server terminated with code {:?}, signal {:?}",
                            payload.code, payload.signal
                        );
                    }
                    _ => {}
                }
            }
        });

        *process_guard = Some(child);
        *port_guard = Some(port);
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    Ok(())
}

#[tauri::command]
async fn start_csm_server(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    start_csm_server_inner(&app_handle, state.inner()).await
}

#[tauri::command]
fn get_csm_quantize(state: State<'_, AppState>) -> bool {
    *state.selected_csm_quantized.lock().unwrap()
}

#[tauri::command]
fn set_csm_quantize(state: State<'_, AppState>, enabled: bool) {
    let mut quantized_guard = state.selected_csm_quantized.lock().unwrap();
    *quantized_guard = enabled;
}

#[tauri::command]
async fn set_csm_voice(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    voice: String,
) -> Result<(), String> {
    let selected_voice = CsmVoice::from_key(&voice)?;
    let context_audio = resolve_csm_context_audio_file(&app_handle, selected_voice)?;

    {
        let mut selected_voice_guard = state.selected_csm_voice.lock().unwrap();
        *selected_voice_guard = selected_voice;
    }

    apply_csm_voice_context(state.inner(), &context_audio).await
}

async fn start_csm_server_inner(
    app_handle: &tauri::AppHandle,
    state: &AppState,
) -> Result<(), String> {
    if csm_process_is_ready(state).await {
        return Ok(());
    }

    stop_csm_server_inner(state).await?;
    reset_csm_startup_state(state);
    update_csm_startup_message(app_handle, Some("Starting CSM worker...".to_string()), true);

    let python_executable = resolve_gemma_python_executable(app_handle)?;
    let python_home = python_executable
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| "Failed to resolve Gemma Python home".to_string())?;
    let csm_site_packages = resolve_csm_site_packages(app_handle)?;
    let csm_script = resolve_resource_file(app_handle, "csm_stream.py")?;

    info!("Starting CSM worker with {}", python_executable.display());

    let mut command = Command::new(&python_executable);
    command
        .arg(&csm_script)
        .arg("--server")
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONHOME", &python_home)
        .env("PYTHONPATH", &csm_site_packages)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let should_quantize_csm = *state.selected_csm_quantized.lock().unwrap();
    if should_quantize_csm {
        info!("Starting CSM worker with quantization enabled");
        command.arg("--quantize");
    } else {
        info!("Starting CSM worker without quantization");
    }

    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start CSM worker: {e}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to open stdin for CSM worker".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to open stdout for CSM worker".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to open stderr for CSM worker".to_string())?;

    let child = Arc::new(AsyncMutex::new(child));
    let stdin = Arc::new(AsyncMutex::new(stdin));

    {
        let mut csm_process_guard = state.csm_process.lock().unwrap();
        *csm_process_guard = Some(CsmProcess {
            child: child.clone(),
            stdin: stdin.clone(),
        });
    }
    {
        let mut csm_ready_guard = state.csm_ready.lock().unwrap();
        *csm_ready_guard = false;
    }

    let (ready_tx, ready_rx) = oneshot::channel();
    let ready_tx = Arc::new(Mutex::new(Some(ready_tx)));

    tauri::async_runtime::spawn(csm_stdout_task(
        app_handle.clone(),
        stdout,
        ready_tx.clone(),
    ));
    tauri::async_runtime::spawn(csm_stderr_task(app_handle.clone(), stderr));
    tauri::async_runtime::spawn(csm_exit_monitor(
        app_handle.clone(),
        child.clone(),
        ready_tx.clone(),
    ));

    match tokio::time::timeout(
        std::time::Duration::from_secs(CSM_STARTUP_TIMEOUT_SECS),
        ready_rx,
    )
    .await
    {
        Ok(Ok(Ok(()))) => {
            update_csm_startup_message(app_handle, None, false);
            Ok(())
        }
        Ok(Ok(Err(message))) => {
            let _ = stop_csm_server_inner(state).await;
            Err(message)
        }
        Ok(Err(_)) => {
            let message =
                csm_startup_failure_message(state, "CSM worker closed before it became ready");
            let _ = stop_csm_server_inner(state).await;
            Err(message)
        }
        Err(_) => {
            let message =
                csm_startup_failure_message(state, "Timed out while loading the CSM worker");
            let _ = stop_csm_server_inner(state).await;
            Err(message)
        }
    }
}

#[tauri::command]
async fn stop_server(state: State<'_, AppState>) -> Result<(), String> {
    let mut process_guard = state.server_process.lock().unwrap();
    let mut port_guard = state.server_port.lock().unwrap();
    if let Some(child) = process_guard.take() {
        info!("Stopping MLX Server...");
        child.kill().map_err(|e| e.to_string())?;
    }
    *port_guard = None;
    Ok(())
}

#[tauri::command]
async fn stop_csm_server(state: State<'_, AppState>) -> Result<(), String> {
    stop_csm_server_inner(state.inner()).await
}

#[tauri::command]
fn receive_audio_chunk(
    payload: AudioPayload,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) {
    if payload.data.is_empty() {
        return;
    }

    let mut buffer = state.audio_buffer.lock().unwrap();
    let mut silent_count = state.silent_chunks_count.lock().unwrap();
    let mut speaking_count = state.speaking_chunks_count.lock().unwrap();
    let mut is_speaking = state.is_speaking.lock().unwrap();

    let rms = (payload.data.iter().map(|&x| x * x).sum::<f32>() / payload.data.len() as f32).sqrt();

    if rms > SILENCE_THRESHOLD {
        *speaking_count += 1;
        *silent_count = 0;
    } else {
        *silent_count += 1;
        if *silent_count > 5 {
            *speaking_count = 0;
        }
    }

    if !*is_speaking && *speaking_count >= MIN_SPEAKING_CHUNKS {
        *is_speaking = true;
        info!("Speech detected");
    }

    if *is_speaking {
        buffer.extend_from_slice(&payload.data);

        if *silent_count >= SILENCE_DURATION_CHUNKS {
            info!("Silence detected, sending to MLX Server...");
            emit_call_stage(&app_handle, "processing_audio", "Processing Audio");
            let server_port = {
                let port_guard = state.server_port.lock().unwrap();
                *port_guard
            };
            if let Some(port) = server_port {
                process_audio_with_server(&buffer, port, app_handle);
            } else {
                error!("MLX Server is not running, skipping audio request");
            }
            buffer.clear();
            *is_speaking = false;
            *silent_count = 0;
            *speaking_count = 0;
        }
    }
}

fn process_audio_with_server(samples: &[f32], server_port: u16, app_handle: tauri::AppHandle) {
    let audio_path = create_temp_wav_path();
    let generation_id;
    let conversation_session_id;
    {
        let state = app_handle.state::<AppState>();
        generation_id = state.next_generation_id.fetch_add(1, Ordering::Relaxed);
        conversation_session_id = current_conversation_session_id(state.inner());
    }
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = match hound::WavWriter::create(&audio_path, spec) {
        Ok(writer) => writer,
        Err(e) => {
            error!(
                "Failed to create temp WAV file at {}: {}",
                audio_path.display(),
                e
            );
            return;
        }
    };

    for &sample in samples {
        let amplitude = (sample * i16::MAX as f32) as i16;
        if let Err(e) = writer.write_sample(amplitude) {
            error!("Failed to write temp WAV sample: {}", e);
            let _ = std::fs::remove_file(&audio_path);
            return;
        }
    }

    if let Err(e) = writer.finalize() {
        error!("Failed to finalize temp WAV file: {}", e);
        let _ = std::fs::remove_file(&audio_path);
        return;
    }

    let app_handle_for_task = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        info!(
            "Sending audio to MLX Server (saved at: {})",
            audio_path.display()
        );

        match transcribe_audio_with_gemma(server_port, &audio_path).await {
            Ok(user_text) => {
                let _ = std::fs::remove_file(&audio_path);

                if user_text.is_empty() {
                    warn!("Gemma transcription was empty, skipping response generation");
                    emit_call_stage(&app_handle_for_task, "listening", "Listening");
                    return;
                }

                if !is_meaningful_transcript(&user_text) {
                    info!(
                        "Ignoring non-meaningful transcript for interruption: {:?}",
                        user_text
                    );
                    emit_call_stage(&app_handle_for_task, "listening", "Listening");
                    return;
                }

                if current_conversation_session_id(app_handle_for_task.state::<AppState>().inner())
                    != conversation_session_id
                {
                    return;
                }

                if active_generation_is_newer(
                    app_handle_for_task.state::<AppState>().inner(),
                    generation_id,
                ) {
                    info!(
                        "Skipping stale transcript for generation {} because a newer reply is active",
                        generation_id
                    );
                    emit_call_stage(&app_handle_for_task, "listening", "Listening");
                    return;
                }

                info!("Gemma transcription: {}", user_text);
                interrupt_active_generation(&app_handle_for_task).await;

                if current_conversation_session_id(app_handle_for_task.state::<AppState>().inner())
                    != conversation_session_id
                {
                    return;
                }

                emit_call_stage(&app_handle_for_task, "thinking", "Thinking");
                start_response_generation(
                    &app_handle_for_task,
                    server_port,
                    generation_id,
                    conversation_session_id,
                    user_text,
                );
            }
            Err(err) => {
                emit_csm_error(
                    &app_handle_for_task,
                    CsmErrorEvent {
                        request_id: None,
                        message: err.clone(),
                    },
                );
                error!("Failed to transcribe audio with Gemma: {}", err);
                let _ = std::fs::remove_file(&audio_path);
                emit_call_stage(&app_handle_for_task, "listening", "Listening");
            }
        }
    });
}

fn start_response_generation(
    app_handle: &tauri::AppHandle,
    server_port: u16,
    generation_id: u64,
    conversation_session_id: u64,
    user_text: String,
) {
    let app_handle_for_task = app_handle.clone();
    let handle = tauri::async_runtime::spawn(async move {
        match stream_gemma_response_to_csm(&app_handle_for_task, server_port, &user_text).await {
            Ok(response_text) => {
                if response_text.is_empty() {
                    emit_call_stage(&app_handle_for_task, "listening", "Listening");
                    return;
                }

                if current_conversation_session_id(app_handle_for_task.state::<AppState>().inner())
                    != conversation_session_id
                {
                    return;
                }

                append_conversation_turn(
                    app_handle_for_task.state::<AppState>().inner(),
                    user_text,
                    response_text,
                );
            }
            Err(err) => {
                emit_csm_error(
                    &app_handle_for_task,
                    CsmErrorEvent {
                        request_id: None,
                        message: err.clone(),
                    },
                );
                error!("Failed to stream Gemma response via CSM: {}", err);
                emit_call_stage(&app_handle_for_task, "listening", "Listening");
            }
        }

        clear_active_generation_if_matches(&app_handle_for_task, generation_id);
    });

    if !register_active_generation_if_newer(
        app_handle.state::<AppState>().inner(),
        generation_id,
        handle,
    ) {
        warn!(
            "Skipping response generation {} because a newer generation is already active",
            generation_id
        );
    }
}

async fn transcribe_audio_with_gemma(
    server_port: u16,
    audio_path: &Path,
) -> Result<String, String> {
    let client = reqwest::Client::new();
    let request = ChatRequest {
        model: GEMMA_MODEL.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: vec![
                ChatContent::InputAudio {
                    input_audio: InputAudio {
                        data: audio_path.to_string_lossy().into_owned(),
                        format: "wav".to_string(),
                    },
                },
                ChatContent::Text {
                    text: TRANSCRIPTION_PROMPT.to_string(),
                },
            ],
        }],
        max_tokens: 192,
        stream: false,
    };

    let response = client
        .post(format!(
            "{}/v1/chat/completions",
            server_base_url(server_port)
        ))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to call MLX Server for transcription: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("Failed to read error body: {e}"));
        return Err(format!(
            "MLX Server returned transcription error {status}: {body}"
        ));
    }

    let payload = response
        .json::<ChatCompletionResponse>()
        .await
        .map_err(|e| format!("Failed to parse Gemma transcription response: {e}"))?;

    let transcript_value = payload
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.content)
        .ok_or_else(|| "Gemma transcription response did not include content".to_string())?;

    let transcript = extract_chat_content_text(transcript_value);
    let sanitized_transcript = sanitize_for_voice_output(&transcript);

    if sanitized_transcript.is_empty() {
        warn!(
            "Gemma returned an empty transcription after sanitization. Raw transcription: {:?}",
            transcript
        );
    }

    Ok(sanitized_transcript)
}

async fn stream_gemma_response_to_csm(
    app_handle: &tauri::AppHandle,
    server_port: u16,
    user_text: &str,
) -> Result<String, String> {
    start_csm_server_inner(app_handle, app_handle.state::<AppState>().inner()).await?;

    let client = reqwest::Client::new();
    let conversation_turns = {
        let state = app_handle.state::<AppState>();
        let turns = state
            .conversation_turns
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        turns
    };
    let mut messages = vec![ChatMessage {
        role: "system".to_string(),
        content: vec![ChatContent::Text {
            text: VOICE_SYSTEM_PROMPT.to_string(),
        }],
    }];

    for turn in conversation_turns {
        messages.push(ChatMessage {
            role: "user".to_string(),
            content: vec![ChatContent::Text {
                text: turn.user_text,
            }],
        });
        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: vec![ChatContent::Text {
                text: turn.assistant_text,
            }],
        });
    }

    messages.push(ChatMessage {
        role: "user".to_string(),
        content: vec![ChatContent::Text {
            text: user_text.to_string(),
        }],
    });

    let request = ChatRequest {
        model: GEMMA_MODEL.to_string(),
        messages,
        max_tokens: 64,
        stream: false,
    };

    let response = client
        .post(format!(
            "{}/v1/chat/completions",
            server_base_url(server_port)
        ))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Failed to call MLX Server: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("Failed to read error body: {e}"));
        return Err(format!("MLX Server returned error {status}: {body}"));
    }

    let response_id = allocate_csm_response_id(app_handle);
    let payload = response
        .json::<ChatCompletionResponse>()
        .await
        .map_err(|e| format!("Failed to parse Gemma response: {e}"))?;

    let response_text = payload
        .choices
        .into_iter()
        .next()
        .and_then(|choice| choice.message.content)
        .map(extract_chat_content_text)
        .map(|text| sanitize_for_voice_output(&text))
        .unwrap_or_default();

    if response_text.is_empty() {
        warn!("Gemma returned an empty response, skipping CSM synthesis");
    } else {
        let spoken_response = prepare_spoken_response_for_csm(&response_text);
        if spoken_response.is_empty() {
            warn!("Gemma response became empty after spoken-response trimming, skipping CSM synthesis");
            return Ok(response_text);
        }

        emit_call_stage(app_handle, "generating_audio", "Generating Audio");
        app_handle
            .emit(
                CSM_AUDIO_START_EVENT,
                CsmAudioStartEvent {
                    request_id: response_id,
                    text: response_text.clone(),
                    total_segments: 1,
                },
            )
            .map_err(|e| e.to_string())?;
        info!(
            "Queueing CSM response as a single synthesis request: {}",
            spoken_response
        );
        send_csm_synthesis_request(app_handle, response_id, &spoken_response).await?;
        info!("MLX Server Output: {}", response_text);
    }

    if let Err(err) = finalize_csm_response(app_handle, response_id).await {
        warn!(
            "Failed to finalize CSM response context for {}: {}",
            response_id, err
        );
    }

    Ok(response_text)
}

fn allocate_csm_response_id(app_handle: &tauri::AppHandle) -> u64 {
    app_handle
        .state::<AppState>()
        .next_csm_request_id
        .fetch_add(1, Ordering::Relaxed)
}

async fn send_csm_synthesis_request(
    app_handle: &tauri::AppHandle,
    request_id: u64,
    text: &str,
) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    if !csm_process_is_ready(state.inner()).await {
        return Err("CSM is not loaded. Load the CSM model before starting a call.".to_string());
    }

    let process = {
        let csm_process_guard = state.csm_process.lock().unwrap();
        csm_process_guard
            .clone()
            .ok_or_else(|| "CSM worker is unavailable".to_string())?
    };

    let request = serde_json::json!({
        "type": "synthesize",
        "request_id": request_id,
        "text": text,
        "speaker": CSM_SPEAKER_ID,
        "max_audio_length_ms": CSM_MAX_AUDIO_LENGTH_MS,
        "temperature": CSM_TEMPERATURE,
        "top_k": CSM_TOP_K,
    });

    let mut stdin = process.stdin.lock().await;
    stdin
        .write_all(request.to_string().as_bytes())
        .await
        .map_err(|e| format!("Failed to send text to CSM worker: {e}"))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|e| format!("Failed to terminate CSM request: {e}"))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("Failed to flush CSM request: {e}"))?;

    Ok(())
}

async fn finalize_csm_response(
    app_handle: &tauri::AppHandle,
    request_id: u64,
) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let process = {
        let csm_process_guard = state.csm_process.lock().unwrap();
        csm_process_guard.clone()
    };

    let Some(process) = process else {
        return Ok(());
    };

    let request = serde_json::json!({
        "type": "finalize_response",
        "request_id": request_id,
    });

    let mut stdin = process.stdin.lock().await;
    stdin
        .write_all(request.to_string().as_bytes())
        .await
        .map_err(|e| format!("Failed to finalize CSM response context: {e}"))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|e| format!("Failed to terminate CSM finalize request: {e}"))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("Failed to flush CSM finalize request: {e}"))?;

    Ok(())
}

async fn reset_csm_reference_context(app_handle: &tauri::AppHandle) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    let process = {
        let csm_process_guard = state.csm_process.lock().unwrap();
        csm_process_guard.clone()
    };

    let Some(process) = process else {
        return Ok(());
    };

    let request = serde_json::json!({
        "type": "reset_context",
    });

    let mut stdin = process.stdin.lock().await;
    stdin
        .write_all(request.to_string().as_bytes())
        .await
        .map_err(|e| format!("Failed to reset CSM reference context: {e}"))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|e| format!("Failed to terminate CSM reset request: {e}"))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("Failed to flush CSM reset request: {e}"))?;

    Ok(())
}

async fn apply_csm_voice_context(state: &AppState, context_audio: &Path) -> Result<(), String> {
    let process = {
        let csm_process_guard = state.csm_process.lock().unwrap();
        csm_process_guard.clone()
    };

    let Some(process) = process else {
        return Ok(());
    };

    let request = serde_json::json!({
        "type": "set_context",
        "context_audio_path": context_audio.to_string_lossy(),
    });

    let mut stdin = process.stdin.lock().await;
    stdin
        .write_all(request.to_string().as_bytes())
        .await
        .map_err(|e| format!("Failed to update CSM voice context: {e}"))?;
    stdin
        .write_all(b"\n")
        .await
        .map_err(|e| format!("Failed to terminate CSM voice update: {e}"))?;
    stdin
        .flush()
        .await
        .map_err(|e| format!("Failed to flush CSM voice update: {e}"))?;

    Ok(())
}

fn extract_chat_content_text(content: serde_json::Value) -> String {
    match content {
        serde_json::Value::String(text) => text,
        serde_json::Value::Array(parts) => parts
            .into_iter()
            .filter_map(|part| {
                part.get("text")
                    .and_then(|text| text.as_str())
                    .map(|text| text.to_string())
            })
            .collect::<Vec<_>>()
            .join(" "),
        other => other.to_string(),
    }
}

fn sanitize_for_voice_output(text: &str) -> String {
    let mut cleaned_lines = Vec::new();

    for line in text.lines() {
        let mut cleaned = line.trim();
        if cleaned.is_empty() {
            continue;
        }

        cleaned = cleaned
            .trim_start_matches(|ch: char| matches!(ch, '*' | '-' | '#' | '>' | '`'))
            .trim_start();
        cleaned = trim_leading_list_marker(cleaned);

        let cleaned = cleaned
            .replace("**", "")
            .replace("__", "")
            .replace('`', "")
            .replace('*', "")
            .replace('#', "");
        let cleaned = cleaned.trim();

        if !cleaned.is_empty() {
            cleaned_lines.push(cleaned.to_string());
        }
    }

    collapse_whitespace(&cleaned_lines.join(" "))
}

fn is_meaningful_transcript(text: &str) -> bool {
    let normalized = collapse_whitespace(text);
    if normalized.is_empty() {
        return false;
    }

    let words = normalized
        .split(|ch: char| !ch.is_alphanumeric() && ch != '\'')
        .filter_map(|word| {
            let trimmed = word.trim_matches('\'');
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_ascii_lowercase())
            }
        })
        .collect::<Vec<_>>();

    if words.is_empty() {
        return false;
    }

    !words.iter().all(|word| {
        matches!(
            word.as_str(),
            "uh" | "um" | "umm" | "hmm" | "hm" | "mmm" | "mm" | "erm" | "ah" | "eh" | "huh" | "mhm"
        )
    })
}

fn trim_leading_list_marker(text: &str) -> &str {
    let bytes = text.as_bytes();
    let mut idx = 0;

    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }

    if idx > 0 && idx < bytes.len() && matches!(bytes[idx], b'.' | b')' | b':') {
        return text[idx + 1..].trim_start();
    }

    text
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn prepare_spoken_response_for_csm(text: &str) -> String {
    truncate_to_spoken_sentence_limit(text, MAX_SPOKEN_SENTENCES).replace(",", "")
}

fn truncate_to_spoken_sentence_limit(text: &str, max_sentences: usize) -> String {
    let normalized = collapse_whitespace(text);
    if normalized.is_empty() || max_sentences == 0 {
        return String::new();
    }

    let mut sentence_count = 0;
    for (idx, ch) in normalized.char_indices() {
        if matches!(ch, '.' | '!' | '?' | '\n') {
            sentence_count += 1;
            if sentence_count >= max_sentences {
                let end = expand_speech_boundary(&normalized, idx + ch.len_utf8());
                return normalized[..end].trim().to_string();
            }
        }
    }

    normalized
}

fn expand_speech_boundary(text: &str, mut end: usize) -> usize {
    while end < text.len() {
        let Some(ch) = text[end..].chars().next() else {
            break;
        };

        if ch.is_whitespace() || matches!(ch, '"' | '\'' | ')' | ']' | '}') {
            end += ch.len_utf8();
            continue;
        }

        break;
    }

    end
}

async fn csm_process_is_ready(state: &AppState) -> bool {
    let process = {
        let csm_process_guard = state.csm_process.lock().unwrap();
        csm_process_guard.clone()
    };

    let Some(process) = process else {
        let mut csm_ready_guard = state.csm_ready.lock().unwrap();
        *csm_ready_guard = false;
        return false;
    };

    let mut child = process.child.lock().await;
    match child.try_wait() {
        Ok(None) => {
            let csm_ready_guard = state.csm_ready.lock().unwrap();
            *csm_ready_guard
        }
        Ok(Some(status)) => {
            warn!("CSM worker exited with status {}", status);
            drop(child);
            let mut csm_process_guard = state.csm_process.lock().unwrap();
            *csm_process_guard = None;
            let mut csm_ready_guard = state.csm_ready.lock().unwrap();
            *csm_ready_guard = false;
            reset_csm_startup_state(state);
            false
        }
        Err(err) => {
            error!("Failed to check CSM worker status: {}", err);
            false
        }
    }
}

async fn stop_csm_server_inner(state: &AppState) -> Result<(), String> {
    let process = {
        let mut csm_process_guard = state.csm_process.lock().unwrap();
        csm_process_guard.take()
    };
    {
        let mut csm_ready_guard = state.csm_ready.lock().unwrap();
        *csm_ready_guard = false;
    }
    reset_csm_startup_state(state);

    let Some(process) = process else {
        return Ok(());
    };

    {
        let mut stdin = process.stdin.lock().await;
        let _ = stdin.write_all(br#"{"type":"shutdown"}"#).await;
        let _ = stdin.write_all(b"\n").await;
        let _ = stdin.flush().await;
    }

    let mut child = process.child.lock().await;
    if let Err(err) = child.kill().await {
        warn!("Failed to kill CSM worker cleanly: {}", err);
    }

    Ok(())
}

async fn csm_stdout_task(
    app_handle: tauri::AppHandle,
    stdout: ChildStdout,
    ready_tx: Arc<Mutex<Option<oneshot::Sender<Result<(), String>>>>>,
) {
    let mut lines = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<CsmWorkerEvent>(&line) {
            Ok(CsmWorkerEvent::Status { message }) => {
                info!("CSM worker status: {}", message);
                update_csm_startup_message(&app_handle, Some(message), true);
            }
            Ok(CsmWorkerEvent::Ready { sample_rate }) => {
                if let Some(sample_rate) = sample_rate {
                    info!("CSM worker ready at {} Hz", sample_rate);
                } else {
                    info!("CSM worker ready");
                }
                let state = app_handle.state::<AppState>();
                let mut csm_ready_guard = state.csm_ready.lock().unwrap();
                *csm_ready_guard = true;
                update_csm_startup_message(&app_handle, None, false);
                send_ready_signal(&ready_tx, Ok(()));
            }
            Ok(CsmWorkerEvent::Chunk {
                request_id,
                audio_wav_base64,
            }) => {
                if let Err(err) = app_handle.emit(
                    CSM_AUDIO_CHUNK_EVENT,
                    CsmAudioChunkEvent {
                        request_id,
                        audio_wav_base64,
                    },
                ) {
                    error!("Failed to emit CSM audio chunk: {}", err);
                }
            }
            Ok(CsmWorkerEvent::Done { request_id }) => {
                if let Err(err) =
                    app_handle.emit(CSM_AUDIO_DONE_EVENT, CsmAudioDoneEvent { request_id })
                {
                    error!("Failed to emit CSM completion: {}", err);
                }
            }
            Ok(CsmWorkerEvent::Error {
                request_id,
                message,
            }) => {
                error!("CSM worker error: {}", message);
                update_csm_startup_message(&app_handle, Some(message.clone()), true);
                emit_csm_error(
                    &app_handle,
                    CsmErrorEvent {
                        request_id,
                        message: message.clone(),
                    },
                );
                send_ready_signal(&ready_tx, Err(message));
            }
            Err(err) => {
                error!("Failed to parse CSM worker message: {} ({})", err, line);
            }
        }
    }

    let state = app_handle.state::<AppState>();
    {
        let mut csm_ready_guard = state.csm_ready.lock().unwrap();
        *csm_ready_guard = false;
    }
    {
        let mut csm_process_guard = state.csm_process.lock().unwrap();
        *csm_process_guard = None;
    }

    send_ready_signal(
        &ready_tx,
        Err(csm_startup_failure_message(
            state.inner(),
            "CSM worker stopped before completing startup",
        )),
    );
}

async fn csm_stderr_task(app_handle: tauri::AppHandle, stderr: ChildStderr) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        error!("CSM worker stderr: {}", line);
        push_csm_stderr_line(app_handle.state::<AppState>().inner(), line);
    }
}

async fn csm_exit_monitor(
    app_handle: tauri::AppHandle,
    child: Arc<AsyncMutex<Child>>,
    ready_tx: Arc<Mutex<Option<oneshot::Sender<Result<(), String>>>>>,
) {
    loop {
        {
            let state = app_handle.state::<AppState>();
            let csm_ready_guard = state.csm_ready.lock().unwrap();
            if *csm_ready_guard {
                return;
            }
        }

        let exit_status = {
            let mut child_guard = child.lock().await;
            match child_guard.try_wait() {
                Ok(status) => status,
                Err(err) => {
                    error!("Failed while waiting for CSM worker startup: {}", err);
                    send_ready_signal(
                        &ready_tx,
                        Err(csm_startup_failure_message(
                            app_handle.state::<AppState>().inner(),
                            &format!("Failed to inspect the CSM worker process: {err}"),
                        )),
                    );
                    return;
                }
            }
        };

        if let Some(status) = exit_status {
            send_ready_signal(
                &ready_tx,
                Err(csm_startup_failure_message(
                    app_handle.state::<AppState>().inner(),
                    &format!("CSM worker exited with status {status}"),
                )),
            );
            return;
        }

        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

fn send_ready_signal(
    ready_tx: &Arc<Mutex<Option<oneshot::Sender<Result<(), String>>>>>,
    result: Result<(), String>,
) {
    if let Some(tx) = ready_tx.lock().unwrap().take() {
        let _ = tx.send(result);
    }
}

fn emit_csm_error(app_handle: &tauri::AppHandle, payload: CsmErrorEvent) {
    if let Err(err) = app_handle.emit(CSM_ERROR_EVENT, payload) {
        error!("Failed to emit CSM error event: {}", err);
    }
}

fn emit_csm_audio_stop(app_handle: &tauri::AppHandle) {
    if let Err(err) = app_handle.emit(CSM_AUDIO_STOP_EVENT, CsmAudioStopEvent {}) {
        error!("Failed to emit CSM audio stop event: {}", err);
    }
}

fn emit_csm_status(app_handle: &tauri::AppHandle, payload: CsmStatusEvent) {
    if let Err(err) = app_handle.emit(CSM_STATUS_EVENT, payload) {
        error!("Failed to emit CSM status event: {}", err);
    }
}

fn emit_call_stage(app_handle: &tauri::AppHandle, phase: &str, message: &str) {
    if let Err(err) = app_handle.emit(
        CALL_STAGE_EVENT,
        CallStageEvent {
            phase: phase.to_string(),
            message: message.to_string(),
        },
    ) {
        error!("Failed to emit call stage event: {}", err);
    }
}

fn emit_model_download_event(app_handle: &tauri::AppHandle, payload: ModelDownloadEvent) {
    if let Err(err) = app_handle.emit(MODEL_DOWNLOAD_EVENT, payload) {
        error!("Failed to emit model download event: {}", err);
    }
}

async fn interrupt_active_generation(app_handle: &tauri::AppHandle) {
    cancel_active_generation(app_handle, true).await;
}

async fn cancel_active_generation(app_handle: &tauri::AppHandle, stop_csm_worker: bool) {
    let state = app_handle.state::<AppState>();
    let active_generation = {
        let mut active_generation_guard = state.active_generation.lock().unwrap();
        active_generation_guard.take()
    };

    if let Some(active_generation) = active_generation {
        info!("Interrupting active generation {}", active_generation.id);
        active_generation.handle.abort();
    }

    emit_csm_audio_stop(app_handle);

    if stop_csm_worker {
        if let Err(err) = stop_csm_server_inner(state.inner()).await {
            warn!("Failed to stop CSM worker during interruption: {}", err);
        }
    } else if let Err(err) = reset_csm_reference_context(app_handle).await {
        warn!(
            "Failed to reset CSM reference context during interruption: {}",
            err
        );
    }
}

fn clear_active_generation_if_matches(app_handle: &tauri::AppHandle, generation_id: u64) {
    let state = app_handle.state::<AppState>();
    let mut active_generation_guard = state.active_generation.lock().unwrap();
    let should_clear = active_generation_guard
        .as_ref()
        .map(|active_generation| active_generation.id == generation_id)
        .unwrap_or(false);

    if should_clear {
        active_generation_guard.take();
    }
}

fn active_generation_is_newer(state: &AppState, generation_id: u64) -> bool {
    state
        .active_generation
        .lock()
        .unwrap()
        .as_ref()
        .map(|active_generation| active_generation.id > generation_id)
        .unwrap_or(false)
}

fn register_active_generation_if_newer(
    state: &AppState,
    generation_id: u64,
    handle: tauri::async_runtime::JoinHandle<()>,
) -> bool {
    let mut active_generation_guard = state.active_generation.lock().unwrap();

    if active_generation_guard
        .as_ref()
        .map(|active_generation| active_generation.id > generation_id)
        .unwrap_or(false)
    {
        handle.abort();
        return false;
    }

    if let Some(previous_generation) = active_generation_guard.take() {
        previous_generation.handle.abort();
    }

    *active_generation_guard = Some(ActiveGeneration {
        id: generation_id,
        handle,
    });
    true
}

fn append_conversation_turn(state: &AppState, user_text: String, assistant_text: String) {
    let mut turns = state.conversation_turns.lock().unwrap();
    turns.push_back(ConversationTurn {
        user_text,
        assistant_text,
    });

    while turns.len() > MAX_CONVERSATION_TURNS {
        turns.pop_front();
    }
}

fn current_conversation_session_id(state: &AppState) -> u64 {
    state.conversation_session_id.load(Ordering::Relaxed)
}

fn reset_call_session_state(state: &AppState) {
    {
        let mut audio_buffer = state.audio_buffer.lock().unwrap();
        audio_buffer.clear();
    }
    {
        let mut silent_chunks_count = state.silent_chunks_count.lock().unwrap();
        *silent_chunks_count = 0;
    }
    {
        let mut speaking_chunks_count = state.speaking_chunks_count.lock().unwrap();
        *speaking_chunks_count = 0;
    }
    {
        let mut is_speaking = state.is_speaking.lock().unwrap();
        *is_speaking = false;
    }
    {
        let mut turns = state.conversation_turns.lock().unwrap();
        turns.clear();
    }

    state
        .conversation_session_id
        .fetch_add(1, Ordering::Relaxed);
}

fn reset_csm_startup_state(state: &AppState) {
    {
        let mut startup_message_guard = state.csm_startup_message.lock().unwrap();
        *startup_message_guard = None;
    }
    let mut stderr_tail_guard = state.csm_stderr_tail.lock().unwrap();
    stderr_tail_guard.clear();
}

fn update_csm_startup_message(
    app_handle: &tauri::AppHandle,
    message: Option<String>,
    emit_event: bool,
) {
    let state = app_handle.state::<AppState>();
    {
        let mut startup_message_guard = state.csm_startup_message.lock().unwrap();
        *startup_message_guard = message.clone();
    }

    if emit_event {
        if let Some(message) = message {
            emit_csm_status(app_handle, CsmStatusEvent { message });
        }
    }
}

fn push_csm_stderr_line(state: &AppState, line: String) {
    let mut stderr_tail_guard = state.csm_stderr_tail.lock().unwrap();
    stderr_tail_guard.push_back(line);
    while stderr_tail_guard.len() > CSM_STDERR_TAIL_LIMIT {
        stderr_tail_guard.pop_front();
    }
}

fn csm_startup_failure_message(state: &AppState, base: &str) -> String {
    let mut message = base.to_string();

    if let Some(stage) = state.csm_startup_message.lock().unwrap().clone() {
        if !stage.trim().is_empty() {
            message.push_str(&format!(". Last stage: {}", stage.trim()));
        }
    }

    let stderr_tail_guard = state.csm_stderr_tail.lock().unwrap();
    if let Some(last_stderr) = stderr_tail_guard
        .iter()
        .rev()
        .find(|line| !line.trim().is_empty())
    {
        message.push_str(&format!(". Last stderr: {}", last_stderr.trim()));
    }

    message
}

fn create_temp_wav_path() -> PathBuf {
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    let mut path = std::env::current_dir().unwrap_or_default();
    if path.ends_with("src-tauri") {
        path.pop();
    }

    path.push("target");
    path.push("audio_debug");

    let _ = std::fs::create_dir_all(&path);

    path.push(format!("openduck-audio-{}.wav", timestamp_ms));
    path
}

fn reserve_free_port() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    listener
        .local_addr()
        .map(|addr| addr.port())
        .map_err(|e| e.to_string())
}

fn server_base_url(port: u16) -> String {
    format!("http://127.0.0.1:{}", port)
}

fn huggingface_model_cache_exists(model_dir_name: &str) -> bool {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::Path::new(&home)
        .join(".cache/huggingface/hub")
        .join(model_dir_name)
        .exists()
}

fn huggingface_cached_file_exists(model_dir_name: &str, file_name: &str) -> bool {
    let home = std::env::var("HOME").unwrap_or_default();
    let model_dir = std::path::Path::new(&home)
        .join(".cache/huggingface/hub")
        .join(model_dir_name);

    if !model_dir.exists() {
        return false;
    }

    let refs_main = model_dir.join("refs").join("main");
    if let Ok(revision) = std::fs::read_to_string(&refs_main) {
        let snapshot_file = model_dir
            .join("snapshots")
            .join(revision.trim())
            .join(file_name);
        if snapshot_file.exists() {
            return true;
        }
    }

    let snapshots_dir = model_dir.join("snapshots");
    let Ok(entries) = std::fs::read_dir(snapshots_dir) else {
        return false;
    };

    for entry in entries.flatten() {
        if entry.path().join(file_name).exists() {
            return true;
        }
    }

    false
}

fn resolve_gemma_python_executable(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    resolve_existing_path_dev_first(
        app_handle,
        Path::new("resources/python_env/bin/python3"),
        "bundled Gemma Python interpreter",
    )
}

fn resolve_csm_site_packages(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    resolve_existing_path_dev_first(
        app_handle,
        Path::new("resources/csm_env/venv/lib/python3.11/site-packages"),
        "bundled CSM site-packages",
    )
}

fn resolve_resource_file(
    app_handle: &tauri::AppHandle,
    file_name: &str,
) -> Result<PathBuf, String> {
    resolve_existing_path_dev_first(
        app_handle,
        &PathBuf::from("resources").join(file_name),
        file_name,
    )
}

fn resolve_existing_path_dev_first(
    app_handle: &tauri::AppHandle,
    relative_path: &Path,
    label: &str,
) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("src-tauri").join(relative_path));
        candidates.push(current_dir.join(relative_path));
        if current_dir.ends_with("src-tauri") {
            candidates.push(current_dir.join(relative_path));
        }
    }

    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        candidates.push(resource_dir.join(relative_path));
        if let Ok(stripped) = relative_path.strip_prefix("resources") {
            candidates.push(resource_dir.join(stripped));
        }
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| format!("Unable to locate {} at {}", label, relative_path.display()))
}

fn resolve_csm_context_audio_file(
    app_handle: &tauri::AppHandle,
    voice: CsmVoice,
) -> Result<PathBuf, String> {
    let reference_audio_file = voice.file_name();
    let mut candidates = Vec::new();

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join(reference_audio_file));
        candidates.push(
            current_dir
                .join("src-tauri")
                .join("..")
                .join(reference_audio_file),
        );

        if current_dir.ends_with("src-tauri") {
            candidates.push(current_dir.join("..").join(reference_audio_file));
        }
    }

    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        candidates.push(resource_dir.join(reference_audio_file));
        candidates.push(resource_dir.join("resources").join(reference_audio_file));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| format!("Unable to locate {}", reference_audio_file))
}

fn reap_stale_model_processes(app_handle: &tauri::AppHandle) {
    for resource_name in ["patch_mlx_vlm.py", "csm_stream.py"] {
        let Ok(resource_path) = resolve_resource_file(app_handle, resource_name) else {
            warn!(
                "Skipping stale worker cleanup because {} could not be located",
                resource_name
            );
            continue;
        };

        match std::process::Command::new("pkill")
            .args(["-f", &resource_path.to_string_lossy()])
            .status()
        {
            Ok(status) if status.success() => {
                info!(
                    "Reaped stale model workers matching {}",
                    resource_path.display()
                );
            }
            Ok(status) if status.code() == Some(1) => {}
            Ok(status) => {
                warn!(
                    "pkill returned {} while cleaning up stale workers for {}",
                    status,
                    resource_path.display()
                );
            }
            Err(err) => {
                warn!(
                    "Failed to run pkill while cleaning stale workers for {}: {}",
                    resource_path.display(),
                    err
                );
            }
        }
    }
}

#[tauri::command]
async fn check_model_status() -> bool {
    huggingface_model_cache_exists(GEMMA_CACHE_DIR)
}

#[tauri::command]
async fn check_csm_status() -> bool {
    huggingface_cached_file_exists(CSM_CACHE_DIR, CSM_MODEL_FILE)
}

#[tauri::command]
async fn download_model(app_handle: tauri::AppHandle) -> Result<(), String> {
    let python_executable = resolve_gemma_python_executable(&app_handle)?;
    run_hf_download(&app_handle, python_executable, "gemma", GEMMA_MODEL, &[]).await
}

#[tauri::command]
async fn download_csm_model(app_handle: tauri::AppHandle) -> Result<(), String> {
    let python_executable = resolve_gemma_python_executable(&app_handle)?;
    run_hf_download(
        &app_handle,
        python_executable,
        "csm",
        CSM_MODEL_REPO,
        &[CSM_MODEL_FILE],
    )
    .await
}

async fn run_hf_download(
    app_handle: &tauri::AppHandle,
    python_executable: PathBuf,
    model_key: &str,
    repo_id: &str,
    allow_patterns: &[&str],
) -> Result<(), String> {
    let script = resolve_resource_file(app_handle, "hf_download.py")?;
    let python_home = python_executable
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| format!("Failed to resolve Python home for {model_key}"))?;

    info!("Starting {} download for repo {}", model_key, repo_id);

    let mut command = Command::new(&python_executable);
    command
        .arg(&script)
        .arg("--repo-id")
        .arg(repo_id)
        .arg("--model")
        .arg(model_key)
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONHOME", python_home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if model_key == "csm" {
        let csm_site_packages = resolve_csm_site_packages(app_handle)?;
        command.env("PYTHONPATH", csm_site_packages);
    }

    for pattern in allow_patterns {
        command.arg("--allow-pattern").arg(pattern);
    }

    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start {model_key} downloader: {e}"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("Failed to capture stdout for {model_key} download"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("Failed to capture stderr for {model_key} download"))?;

    let stderr_handle = tauri::async_runtime::spawn(async move {
        let mut stderr_lines = BufReader::new(stderr).lines();
        let mut collected = Vec::new();
        while let Ok(Some(line)) = stderr_lines.next_line().await {
            if !line.trim().is_empty() {
                error!("Downloader stderr: {}", line);
                collected.push(line);
            }
        }
        collected.join("\n")
    });

    let mut stdout_lines = BufReader::new(stdout).lines();
    let mut last_error_message: Option<String> = None;

    while let Ok(Some(line)) = stdout_lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<DownloadWorkerEvent>(trimmed) {
            Ok(event) => {
                let phase = match event.event_type.as_str() {
                    "completed" => "completed",
                    "error" => "error",
                    _ => "progress",
                };

                emit_model_download_event(
                    app_handle,
                    ModelDownloadEvent {
                        model: event.model.clone(),
                        phase: phase.to_string(),
                        message: event.message.clone(),
                        progress: event.progress,
                        indeterminate: event.indeterminate,
                    },
                );

                if event.event_type == "error" {
                    last_error_message = Some(event.message);
                }
            }
            Err(err) => {
                warn!(
                    "Failed to parse downloader output for {}: {} ({})",
                    model_key, err, trimmed
                );
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for {model_key} downloader: {e}"))?;
    let stderr_output = stderr_handle.await.unwrap_or_default();

    if status.success() {
        info!("{} download completed successfully", model_key);
        Ok(())
    } else {
        Err(last_error_message
            .or_else(|| {
                let trimmed = stderr_output.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .unwrap_or_else(|| format!("{model_key} download failed with status {status}")))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("Starting OpenDuck application");
    let default_csm_quantized =
        std::env::var("OPEN_DUCK_CSM_QUANTIZE").ok().as_deref() == Some("1");

    tauri::Builder::default()
        .setup(|app| {
            reap_stale_model_processes(app.handle());
            Ok(())
        })
        .manage(AppState {
            audio_buffer: Mutex::new(Vec::new()),
            silent_chunks_count: Mutex::new(0),
            speaking_chunks_count: Mutex::new(0),
            is_speaking: Mutex::new(false),
            server_process: Mutex::new(None),
            server_port: Mutex::new(None),
            csm_process: Mutex::new(None),
            csm_ready: Mutex::new(false),
            csm_startup_message: Mutex::new(None),
            csm_stderr_tail: Mutex::new(VecDeque::new()),
            selected_csm_voice: Mutex::new(CsmVoice::Male),
            selected_csm_quantized: Mutex::new(default_csm_quantized),
            next_csm_request_id: AtomicU64::new(1),
            next_generation_id: AtomicU64::new(1),
            active_generation: Mutex::new(None),
            conversation_turns: Mutex::new(VecDeque::new()),
            conversation_session_id: AtomicU64::new(1),
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            receive_audio_chunk,
            ping,
            reset_call_session,
            check_model_status,
            check_csm_status,
            download_model,
            download_csm_model,
            is_server_running,
            is_csm_running,
            start_server,
            start_csm_server,
            get_csm_quantize,
            set_csm_quantize,
            set_csm_voice,
            stop_server,
            stop_csm_server
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                let state = window.state::<AppState>();
                let mut process_guard = state.server_process.lock().unwrap();
                let mut port_guard = state.server_port.lock().unwrap();
                if let Some(child) = process_guard.take() {
                    info!("Killing MLX Server on app exit...");
                    let _: Result<(), _> = child.kill();
                }
                *port_guard = None;

                let app_handle = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    let _ = stop_csm_server_inner(state.inner()).await;
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
