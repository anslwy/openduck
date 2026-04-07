use serde::Deserialize;
use tracing::info;

#[derive(Clone, Deserialize)]
struct AudioPayload {
    data: Vec<f32>,
}

#[tauri::command]
fn ping() {
    info!("Backend: ping command received");
}

#[tauri::command]
fn receive_audio_chunk(payload: AudioPayload) {
    info!(
        "Command receive_audio_chunk called: {} samples",
        payload.data.len()
    );
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("Starting OpenDuck application");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![receive_audio_chunk, ping])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
