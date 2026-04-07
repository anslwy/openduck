use serde::Deserialize;
use std::sync::Mutex;
use tauri::State;
use tracing::info;

#[derive(Clone, Deserialize)]
struct AudioPayload {
    data: Vec<f32>,
}

struct AppState {
    audio_buffer: Mutex<Vec<f32>>,
    silent_chunks_count: Mutex<usize>,
    speaking_chunks_count: Mutex<usize>,
    is_speaking: Mutex<bool>,
}

const SILENCE_THRESHOLD: f32 = 0.02; // Increased threshold for RMS
const SILENCE_DURATION_CHUNKS: usize = 150; // Longer silence to ensure end of thought
const MIN_SPEAKING_CHUNKS: usize = 10; // Require ~30ms of sound to trigger
const SAMPLE_RATE: u32 = 44100;

#[tauri::command]
fn ping() {
    info!("Backend: ping command received");
}

#[tauri::command]
fn receive_audio_chunk(payload: AudioPayload, state: State<'_, AppState>) {
    let mut buffer = state.audio_buffer.lock().unwrap();
    let mut silent_count = state.silent_chunks_count.lock().unwrap();
    let mut speaking_count = state.speaking_chunks_count.lock().unwrap();
    let mut is_speaking = state.is_speaking.lock().unwrap();

    // Calculate RMS energy of the chunk
    let rms = (payload.data.iter().map(|&x| x * x).sum::<f32>() / payload.data.len() as f32).sqrt();

    if rms > SILENCE_THRESHOLD {
        *speaking_count += 1;
        *silent_count = 0;
    } else {
        *silent_count += 1;
        if *silent_count > 5 {
            // Reset speaking count if we hit a small gap of silence
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
            info!("Silence detected, saving audio file...");
            save_audio_to_file(&buffer);
            buffer.clear();
            *is_speaking = false;
            *silent_count = 0;
            *speaking_count = 0;
        }
    }
}

fn save_audio_to_file(samples: &[f32]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let filename = format!(
        "output_{}.wav",
        std::time::UNIX_EPOCH.elapsed().unwrap().as_secs()
    );
    let mut writer = hound::WavWriter::create(filename, spec).unwrap();
    for &sample in samples {
        let amplitude = (sample * i16::MAX as f32) as i16;
        writer.write_sample(amplitude).unwrap();
    }
    writer.finalize().unwrap();
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
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![receive_audio_chunk, ping])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
