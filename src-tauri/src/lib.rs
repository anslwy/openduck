use serde::{Deserialize, Serialize};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{State, Manager};
use tracing::{info, error};
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::CommandEvent;

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
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

struct AppState {
    audio_buffer: Mutex<Vec<f32>>,
    silent_chunks_count: Mutex<usize>,
    speaking_chunks_count: Mutex<usize>,
    is_speaking: Mutex<bool>,
    server_process: Mutex<Option<tauri_plugin_shell::process::CommandChild>>,
    server_port: Mutex<Option<u16>>,
}

const SILENCE_THRESHOLD: f32 = 0.02; 
const SILENCE_DURATION_CHUNKS: usize = 150; 
const MIN_SPEAKING_CHUNKS: usize = 10; 
const SAMPLE_RATE: u32 = 44100;

#[tauri::command]
fn ping() {
    info!("Backend: ping command received");
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
async fn start_server(app_handle: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
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
            .args(&[
                "--server",
                "--model",
                "mlx-community/gemma-3n-E4B-it-4bit",
                "--port",
                &port_arg,
            ]);

        let (mut rx, child) = sidecar_command.spawn().map_err(|e| e.to_string())?;
        tauri::async_runtime::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(line) => {
                        info!("MLX Server stdout: {}", String::from_utf8_lossy(&line).trim());
                    }
                    CommandEvent::Stderr(line) => {
                        error!("MLX Server stderr: {}", String::from_utf8_lossy(&line).trim());
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
fn receive_audio_chunk(
    payload: AudioPayload,
    state: State<'_, AppState>,
    _app_handle: tauri::AppHandle,
) {
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
            let server_port = {
                let port_guard = state.server_port.lock().unwrap();
                *port_guard
            };
            if let Some(port) = server_port {
                process_audio_with_server(&buffer, port);
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

fn process_audio_with_server(samples: &[f32], server_port: u16) {
    let audio_path = create_temp_wav_path();
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = match hound::WavWriter::create(&audio_path, spec) {
        Ok(writer) => writer,
        Err(e) => {
            error!("Failed to create temp WAV file at {}: {}", audio_path.display(), e);
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

    tauri::async_runtime::spawn(async move {
        let client = reqwest::Client::new();
        info!("Sending audio to MLX Server (saved at: {})", audio_path.display());

        let request = ChatRequest {
            model: "mlx-community/gemma-3n-E4B-it-4bit".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: vec![
                    ChatContent::InputAudio {
                        input_audio: InputAudio {
                            data: audio_path.to_string_lossy().into_owned(),
                            format: "wav".to_string(),
                        }
                    },
                    ChatContent::Text { text: "Please respond to the audio above.".to_string() },
                ],
            }],
            max_tokens: 128,
        };

        let url = format!("{}/v1/chat/completions", server_base_url(server_port));
        match client.post(url)
            .json(&request)
            .send()
            .await {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Ok(chat_resp) = resp.json::<ChatResponse>().await {
                        if let Some(choice) = chat_resp.choices.get(0) {
                            info!("MLX Server Output: {}", choice.message.content);
                        }
                    }
                } else {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_else(|e| format!("Failed to read error body: {}", e));
                    error!("MLX Server returned error {}: {}", status, body);
                }
            }
            Err(e) => error!("Failed to call MLX Server: {}", e),
        }
    });
}

fn create_temp_wav_path() -> PathBuf {
    let timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    let mut path = std::env::current_dir().unwrap_or_default();
    
    // If we are in src-tauri, go up to the project root
    if path.ends_with("src-tauri") {
        path.pop();
    }
    
    path.push("target");
    path.push("audio_debug");
    
    // Ensure the directory exists
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

#[tauri::command]
async fn check_model_status() -> bool {
    let home = std::env::var("HOME").unwrap_or_default();
    let cache_path = std::path::Path::new(&home)
        .join(".cache/huggingface/hub/models--mlx-community--gemma-3n-E4B-it-4bit");
    cache_path.exists()
}

#[tauri::command]
async fn download_model(app_handle: tauri::AppHandle) -> Result<(), String> {
    let sidecar_command = app_handle
        .shell()
        .sidecar("mlx-handler")
        .map_err(|e| e.to_string())?
        .args(&[
            "--model",
            "mlx-community/gemma-3n-E4B-it-4bit",
            "--prompt",
            "test",
            "--max-tokens",
            "1",
        ]);

    info!("Starting model download...");
    let output = sidecar_command.output().await.map_err(|e| e.to_string())?;
    
    if output.status.success() {
        info!("Model downloaded successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Download failed: {}", stderr))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("Starting OpenDuck application");

    tauri::Builder::default()
        .manage(AppState {
            audio_buffer: Mutex::new(Vec::new()),
            silent_chunks_count: Mutex::new(0),
            speaking_chunks_count: Mutex::new(0),
            is_speaking: Mutex::new(false),
            server_process: Mutex::new(None),
            server_port: Mutex::new(None),
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            receive_audio_chunk, 
            ping,
            check_model_status,
            download_model,
            is_server_running,
            start_server,
            stop_server
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
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
