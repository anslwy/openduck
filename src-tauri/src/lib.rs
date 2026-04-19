//! Main Tauri application entry point and runtime coordinator for the OpenDuck desktop app.
mod constants;
mod frontend_events;
mod model_variants;
mod vad;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use constants::*;
use frontend_events::*;
use model_variants::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::Menu;
#[cfg(target_os = "macos")]
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    tray::TrayIconBuilder,
};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};
use tokio::sync::oneshot;
use tokio::sync::Mutex as AsyncMutex;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Clone, Deserialize)]
struct AudioPayload {
    data: Vec<f32>,
    #[serde(default)]
    sample_rate: Option<u32>,
    #[serde(default)]
    playback_reference: Option<Vec<f32>>,
    #[serde(default)]
    playback_active: bool,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: Vec<ChatContent>,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ChatContent {
    Text {
        text: String,
    },
    #[serde(rename = "image_url")]
    InputImage {
        image_url: ImageUrlContent,
    },
    InputAudio {
        input_audio: InputAudio,
    },
}

#[derive(Serialize)]
struct ImageUrlContent {
    url: String,
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
    stream: bool,
}

#[derive(Clone)]
struct CsmProcess {
    child: Arc<AsyncMutex<Child>>,
    stdin: Arc<AsyncMutex<ChildStdin>>,
}

#[derive(Clone)]
struct SttProcess {
    child: Arc<AsyncMutex<Child>>,
    stdin: Arc<AsyncMutex<ChildStdin>>,
    pending_requests: Arc<AsyncMutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
}

#[derive(Clone)]
struct DownloadProcess {
    child: Arc<AsyncMutex<Child>>,
}

#[derive(Clone, Deserialize)]
struct DownloadManifestFile {
    file_size: u64,
    local_path: PathBuf,
    blob_path: Option<PathBuf>,
    incomplete_path: Option<PathBuf>,
}

#[derive(Clone, Deserialize)]
struct DownloadManifest {
    total_bytes: Option<u64>,
    files: Vec<DownloadManifestFile>,
}

#[derive(Clone)]
struct TrackedDownloadState {
    latest_event: ModelDownloadEvent,
    manifest: Option<DownloadManifest>,
}

#[derive(Clone, Copy)]
struct ProcessMemorySnapshot {
    ppid: u32,
    rss_kb: u64,
}

#[derive(Clone, Serialize)]
struct ModelMemoryUsageEntry {
    key: String,
    label: String,
    detail: Option<String>,
    bytes: u64,
    root_pid: u32,
    process_count: usize,
}

#[derive(Clone, Default, Serialize)]
struct ModelMemoryUsageSnapshot {
    total_bytes: u64,
    models: Vec<ModelMemoryUsageEntry>,
}

#[derive(Clone, Serialize, Deserialize)]
struct ConversationTurn {
    user_entry_id: u64,
    assistant_entry_id: u64,
    user_text: String,
    assistant_text: String,
    #[serde(default)]
    image_paths: Vec<PathBuf>,
    #[serde(default)]
    user_image_data_urls: Vec<String>,

    #[serde(skip_serializing, default)]
    image_path: Option<PathBuf>,
    #[serde(skip_serializing, default)]
    user_image_data_url: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct SessionMetadata {
    id: String,
    title: String,
    created_at: u64,
    updated_at: u64,
}

#[derive(Serialize, Clone)]
struct SearchResult {
    session_id: String,
    session_title: String,
    matched_text: String,
    updated_at: u64,
}

#[derive(Serialize, Deserialize)]
struct SessionData {
    metadata: SessionMetadata,
    turns: Vec<ConversationTurn>,
}

struct TempImageFile {
    path: Option<PathBuf>,
}

impl TempImageFile {
    fn new(path: PathBuf) -> Self {
        Self { path: Some(path) }
    }

    fn path(&self) -> &Path {
        self.path
            .as_deref()
            .expect("temp image file path should always be present while borrowed")
    }

    fn release(mut self) -> PathBuf {
        self.path
            .take()
            .expect("temp image file path should exist when released")
    }
}

impl Drop for TempImageFile {
    fn drop(&mut self) {
        if let Some(path) = self.path.take() {
            remove_temp_image_file(&path);
        }
    }
}

struct ActiveGeneration {
    id: u64,
    handle: tauri::async_runtime::JoinHandle<()>,
    cancellation_token: Arc<AtomicBool>,
}

#[derive(Clone, Copy)]
struct AutoContinueTracker {
    assistant_entry_id: u64,
    continuation_count: u32,
    blocked: bool,
}

enum ResponseGenerationMode {
    LatestUserTurn {
        user_text: String,
        latest_audio_wav_base64: Option<String>,
        latest_image_paths: Vec<PathBuf>,
    },
    AssistantAutoContinue {
        assistant_entry_id: u64,
        assistant_text_prefix: String,
    },
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
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Deserialize)]
struct ChatCompletionStreamChunk {
    choices: Vec<ChatCompletionStreamChoice>,
}

#[derive(Deserialize)]
struct ChatCompletionStreamChoice {
    #[serde(default)]
    delta: Option<ChatCompletionStreamDelta>,
    #[serde(default)]
    message: Option<ChatCompletionMessage>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChatCompletionStreamDelta {
    content: Option<serde_json::Value>,
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct PersistedExternalLlmProviderConfig {
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    api_key: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PersistedExternalLlmProvidersConfig {
    #[serde(default = "default_ollama_provider_config")]
    ollama: PersistedExternalLlmProviderConfig,
    #[serde(default = "default_lmstudio_provider_config")]
    lmstudio: PersistedExternalLlmProviderConfig,
    #[serde(default = "default_openai_compatible_provider_config")]
    openai_compatible: PersistedExternalLlmProviderConfig,
}

#[derive(Clone, Serialize, Deserialize)]
struct PersistedAppConfig {
    #[serde(default = "default_persisted_app_config_version")]
    version: u32,
    #[serde(default)]
    external_llm_providers: PersistedExternalLlmProvidersConfig,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TrayIconVariant {
    Default,
    Muted,
    Listening,
    Processing,
    Thinking,
    ImagePasted,
}

#[derive(Default)]
struct LiveTranscriptionState {
    utterance_id: Option<u64>,
    last_emitted_text: String,
    cached_text: Option<String>,
    cached_voiced_samples: usize,
    next_attempt_id: u64,
    in_flight_attempt_id: Option<u64>,
    in_flight_voiced_samples: usize,
    in_flight_started_in_silence: bool,
    in_flight_handle: Option<tauri::async_runtime::JoinHandle<Result<String, String>>>,
}

struct AppState {
    audio_buffer: Mutex<Vec<f32>>,
    pre_audio_buffer: Mutex<VecDeque<f32>>,
    silent_chunks_count: Mutex<usize>,
    speaking_chunks_count: Mutex<usize>,
    current_utterance_voiced_samples: Mutex<usize>,
    is_speaking: Mutex<bool>,
    next_utterance_id: AtomicU64,
    live_transcription: Mutex<LiveTranscriptionState>,
    runtime_setup_lock: AsyncMutex<()>,
    selected_gemma_variant: Mutex<GemmaVariant>,
    loaded_gemma_variant: Mutex<Option<GemmaVariant>>,
    gemma_download_process: Mutex<Option<DownloadProcess>>,
    csm_download_process: Mutex<Option<DownloadProcess>>,
    stt_download_process: Mutex<Option<DownloadProcess>>,
    gemma_download_state: Mutex<Option<TrackedDownloadState>>,
    csm_download_state: Mutex<Option<TrackedDownloadState>>,
    stt_download_state: Mutex<Option<TrackedDownloadState>>,
    gemma_download_cancel_requested: Mutex<bool>,
    csm_download_cancel_requested: Mutex<bool>,
    stt_download_cancel_requested: Mutex<bool>,
    server_process: Mutex<Option<tauri_plugin_shell::process::CommandChild>>,
    server_port: Mutex<Option<u16>>,
    csm_process: Mutex<Option<CsmProcess>>,
    csm_ready: Mutex<bool>,
    csm_startup_message: Mutex<Option<String>>,
    csm_stderr_tail: Mutex<VecDeque<String>>,
    selected_csm_model: Mutex<CsmModelVariant>,
    loaded_csm_model: Mutex<Option<CsmModelVariant>>,
    stt_process: Mutex<Option<SttProcess>>,
    stt_ready: Mutex<bool>,
    stt_startup_message: Mutex<Option<String>>,
    stt_stderr_tail: Mutex<VecDeque<String>>,
    selected_stt_model: Mutex<SttModelVariant>,
    loaded_stt_model: Mutex<Option<SttModelVariant>>,
    selected_csm_voice: Mutex<CsmVoice>,
    selected_csm_quantized: Mutex<bool>,
    csm_reference_audio_path: Mutex<Option<PathBuf>>,
    csm_reference_text: Mutex<Option<String>>,
    next_csm_request_id: AtomicU64,
    next_stt_request_id: AtomicU64,
    next_generation_id: AtomicU64,
    active_generation: Mutex<Option<ActiveGeneration>>,
    conversation_turns: Mutex<VecDeque<ConversationTurn>>,
    conversation_image_paths: Mutex<Vec<PathBuf>>,
    pending_screen_captures: Mutex<Vec<PathBuf>>,
    screen_capture_in_progress: Mutex<bool>,
    transient_tray_title: Mutex<Option<String>>,
    transient_tray_icon: Mutex<Option<TrayIconVariant>>,
    call_stage_phase: Mutex<String>,
    voice_system_prompt: Mutex<String>,
    conversation_session_id: AtomicU64,
    current_session_id: Mutex<Option<String>>,
    current_session_title: Mutex<Option<String>>,
    call_started_at: Mutex<Option<Instant>>,
    processing_audio_started_at: Mutex<Option<Instant>>,
    processing_audio_latency_request_id: Mutex<Option<u64>>,
    tray_timer_generation: AtomicU64,
    tray_title_override_generation: AtomicU64,
    call_in_progress: Mutex<bool>,
    call_muted: Mutex<bool>,
    tts_playback_active: Mutex<bool>,
    tray_pong_playback_enabled: Mutex<bool>,
    tray_pong_playback_hydrated: Mutex<bool>,
    tray_pong_playback_modified_before_hydration: Mutex<bool>,
    end_of_utterance_silence_ms: Mutex<u32>,
    auto_continue_silence_ms: Mutex<Option<u32>>,
    auto_continue_max_count: Mutex<Option<u32>>,
    auto_continue_timer_generation: AtomicU64,
    auto_continue_tracker: Mutex<Option<AutoContinueTracker>>,
    llm_context_turn_limit: Mutex<Option<usize>>,
    llm_image_history_limit: Mutex<Option<usize>>,
    conversation_log_has_visible_images: Mutex<bool>,
    selected_ollama_model: Mutex<String>,
    ollama_base_url: Mutex<String>,
    ollama_api_key: Mutex<Option<String>>,
    selected_lmstudio_model: Mutex<String>,
    lmstudio_base_url: Mutex<String>,
    lmstudio_api_key: Mutex<Option<String>>,
    selected_openai_compatible_model: Mutex<String>,
    openai_compatible_base_url: Mutex<String>,
    openai_compatible_api_key: Mutex<Option<String>>,
    global_shortcut_look_at_screen_region: Mutex<String>,
    global_shortcut_look_at_screen_region_hydrated: Mutex<bool>,
    global_shortcut_look_at_screen_region_modified_before_hydration: Mutex<bool>,
    global_shortcut_look_at_entire_screen: Mutex<String>,
    global_shortcut_look_at_entire_screen_hydrated: Mutex<bool>,
    global_shortcut_look_at_entire_screen_modified_before_hydration: Mutex<bool>,
    global_shortcut_toggle_mute: Mutex<String>,
    global_shortcut_toggle_mute_hydrated: Mutex<bool>,
    global_shortcut_toggle_mute_modified_before_hydration: Mutex<bool>,
    global_shortcut_interrupt: Mutex<String>,
    global_shortcut_interrupt_hydrated: Mutex<bool>,
    global_shortcut_interrupt_modified_before_hydration: Mutex<bool>,
    next_conversation_entry_id: AtomicU64,
    last_tray_icon_variant: Mutex<Option<TrayIconVariant>>,
    last_tray_menu_state: Mutex<Option<TrayMenuState>>,
    last_tray_title: Mutex<Option<String>>,
    is_quitting: Mutex<bool>,
    vad: Mutex<Option<vad::Silero>>,
    screen_capture_child: Mutex<Option<std::process::Child>>,
}

#[derive(Debug, Clone, PartialEq)]
struct TrayMenuState {
    call_in_progress: bool,
    call_muted: bool,
    tray_pong_playback_enabled: bool,
    screen_capture_in_progress: bool,
    has_pending_screen_capture: bool,
    has_conversation_image_history: bool,
    gemma_loaded: bool,
    stt_loaded: bool,
    csm_loaded: bool,
    memory_snapshot_summary: String,
    region_shortcut: String,
    entire_screen_shortcut: String,
    toggle_mute_shortcut: String,
    interrupt_shortcut: String,
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
    Timing {
        request_id: u64,
        text: String,
        elapsed_ms: f64,
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

#[derive(Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum SttWorkerEvent {
    Status {
        message: String,
    },
    Ready {},
    Transcription {
        request_id: u64,
        text: String,
    },
    Error {
        request_id: Option<u64>,
        message: String,
    },
}

#[derive(Clone, Copy)]
enum DownloadModel {
    Gemma,
    Csm,
    Stt,
}

impl DownloadModel {
    fn from_key(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gemma" => Ok(Self::Gemma),
            "csm" => Ok(Self::Csm),
            "stt" => Ok(Self::Stt),
            other => Err(format!("Unsupported download model: {other}")),
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Gemma => "gemma",
            Self::Csm => "csm",
            Self::Stt => "stt",
        }
    }
}

#[derive(Clone, Deserialize)]
struct DownloadManifestWorkerEvent {
    total_bytes: Option<u64>,
    files: Vec<DownloadManifestFile>,
}

#[derive(Clone, Deserialize)]
struct DownloadProgressWorkerEvent {
    #[serde(rename = "type")]
    event_type: String,
    model: String,
    message: String,
    progress: Option<f32>,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
    #[serde(default)]
    indeterminate: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContactExportPayload {
    name: String,
    prompt: String,
    icon_data_url: Option<String>,
    ref_audio: Option<String>,
    ref_text: Option<String>,
    output_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContactExportResult {
    saved_path: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct BuildInfo {
    app_name: String,
    version: String,
    version_label: Option<String>,
    build_channel: Option<String>,
    build_number: Option<String>,
    git_sha: Option<String>,
    git_short_sha: Option<String>,
    build_id: Option<String>,
    is_dirty: bool,
    dirty_files: Option<Vec<String>>,
    copy_text: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppUpdateInfo {
    version: String,
    current_version: String,
    notes: Option<String>,
    published_at: Option<String>,
    target: String,
}

impl Default for PersistedExternalLlmProviderConfig {
    fn default() -> Self {
        default_openai_compatible_provider_config()
    }
}

impl Default for PersistedExternalLlmProvidersConfig {
    fn default() -> Self {
        Self {
            ollama: default_ollama_provider_config(),
            lmstudio: default_lmstudio_provider_config(),
            openai_compatible: default_openai_compatible_provider_config(),
        }
    }
}

impl Default for PersistedAppConfig {
    fn default() -> Self {
        Self {
            version: default_persisted_app_config_version(),
            external_llm_providers: PersistedExternalLlmProvidersConfig::default(),
        }
    }
}

struct PendingAppUpdate(Mutex<Option<Update>>);

const BUILD_VERSION: &str = env!("OPEN_DUCK_BUILD_VERSION");
const BUILD_LABEL: &str = env!("OPEN_DUCK_BUILD_LABEL");
const BUILD_CHANNEL: &str = env!("OPEN_DUCK_BUILD_CHANNEL");
const BUILD_NUMBER: &str = env!("OPEN_DUCK_BUILD_NUMBER");
const BUILD_ID: &str = env!("OPEN_DUCK_BUILD_ID");
const BUILD_GIT_SHA: &str = env!("OPEN_DUCK_GIT_SHA");
const BUILD_GIT_DIRTY: &str = env!("OPEN_DUCK_GIT_DIRTY");
const BUILD_GIT_DIRTY_FILES: Option<&str> = option_env!("OPEN_DUCK_GIT_DIRTY_FILES");
const BUILD_UPDATER_PUBLIC_KEY: &str = env!("OPEN_DUCK_UPDATER_PUBLIC_KEY");
const BUILD_UPDATER_ENDPOINT: &str = env!("OPEN_DUCK_UPDATER_ENDPOINT");
const OPENDUCK_CONFIG_FILE_NAME: &str = "config.json";

fn default_persisted_app_config_version() -> u32 {
    1
}

fn default_ollama_base_url() -> String {
    "http://127.0.0.1:11434".to_string()
}

fn default_lmstudio_base_url() -> String {
    "http://127.0.0.1:1234".to_string()
}

fn default_openai_compatible_base_url() -> String {
    String::new()
}

fn default_ollama_provider_config() -> PersistedExternalLlmProviderConfig {
    PersistedExternalLlmProviderConfig {
        base_url: default_ollama_base_url(),
        api_key: None,
    }
}

fn default_lmstudio_provider_config() -> PersistedExternalLlmProviderConfig {
    PersistedExternalLlmProviderConfig {
        base_url: default_lmstudio_base_url(),
        api_key: None,
    }
}

fn default_openai_compatible_provider_config() -> PersistedExternalLlmProviderConfig {
    PersistedExternalLlmProviderConfig {
        base_url: default_openai_compatible_base_url(),
        api_key: None,
    }
}

fn compiled_build_value(raw_value: &'static str) -> Option<String> {
    let trimmed = raw_value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn compiled_build_dirty() -> bool {
    matches!(
        BUILD_GIT_DIRTY.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn compiled_updater_public_key() -> Option<String> {
    compiled_build_value(BUILD_UPDATER_PUBLIC_KEY)
}

fn compiled_updater_endpoint() -> Option<String> {
    compiled_build_value(BUILD_UPDATER_ENDPOINT)
}

fn build_info_copy_text(build_info: &BuildInfo) -> String {
    let mut lines = vec![build_info.app_name.clone()];

    if let Some(label) = &build_info.version_label {
        lines.push(format!("Version: {} ({label})", build_info.version));
    } else {
        lines.push(format!("Version: {}", build_info.version));
    }

    if let Some(channel) = &build_info.build_channel {
        lines.push(format!("Channel: {channel}"));
    }

    if let Some(build_number) = &build_info.build_number {
        lines.push(format!("Build Number: {build_number}"));
    }

    if let Some(git_sha) = &build_info.git_sha {
        lines.push(format!("Commit: {git_sha}"));
    }

    if let Some(build_id) = &build_info.build_id {
        lines.push(format!("Build ID: {build_id}"));
    }

    if build_info.is_dirty {
        lines.push("Working Tree: Local Changes".to_string());
        if let Some(files) = &build_info.dirty_files {
            for file in files {
                lines.push(format!("  - {file}"));
            }
        }
    }

    lines.join("\n")
}

fn current_build_info() -> BuildInfo {
    let app_name = "OpenDuck".to_string();
    let version = compiled_build_value(BUILD_VERSION).unwrap_or_else(|| "0.0.0".to_string());
    let version_label = compiled_build_value(BUILD_LABEL);
    let build_channel = compiled_build_value(BUILD_CHANNEL);
    let build_number = compiled_build_value(BUILD_NUMBER);
    let git_sha = compiled_build_value(BUILD_GIT_SHA);
    let build_id = compiled_build_value(BUILD_ID);
    let is_dirty = compiled_build_dirty();
    let git_short_sha = git_sha
        .as_ref()
        .map(|sha| sha.chars().take(12).collect::<String>())
        .filter(|sha| !sha.is_empty());

    let mut dirty_files = None;
    if is_dirty {
        if let Some(files_raw) = BUILD_GIT_DIRTY_FILES {
            let parsed: Vec<String> = files_raw
                .split('\n')
                .map(|line| line.trim())
                .filter(|line| line.len() > 3)
                .map(|line| line[3..].trim().to_string())
                .collect();
            if !parsed.is_empty() {
                dirty_files = Some(parsed);
            }
        }
    }

    let mut build_info = BuildInfo {
        app_name,
        version,
        version_label,
        build_channel,
        build_number,
        git_sha,
        git_short_sha,
        build_id,
        is_dirty,
        dirty_files,
        copy_text: String::new(),
    };
    build_info.copy_text = build_info_copy_text(&build_info);
    build_info
}

fn build_app_updater(app_handle: &AppHandle) -> Result<tauri_plugin_updater::Updater, String> {
    let public_key = compiled_updater_public_key().ok_or_else(|| {
        "Updater is not configured for this build. Add src-tauri/updater-public-key.pem or set OPEN_DUCK_UPDATER_PUBLIC_KEY before building the app.".to_string()
    })?;
    let endpoint = compiled_updater_endpoint()
        .ok_or_else(|| "Updater endpoint is not configured for this build.".to_string())?;
    let endpoint = reqwest::Url::parse(&endpoint)
        .map_err(|err| format!("Updater endpoint is not a valid URL: {err}"))?;

    app_handle
        .updater_builder()
        .pubkey(public_key)
        .endpoints(vec![endpoint])
        .map_err(|err| err.to_string())?
        .build()
        .map_err(|err| err.to_string())
}

fn app_update_info(update: &Update) -> AppUpdateInfo {
    AppUpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
        published_at: update
            .raw_json
            .get("pub_date")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        target: update.target.clone(),
    }
}

#[tauri::command]
fn ping() {
    info!("Backend: ping command received");
}

#[tauri::command]
fn get_build_info() -> BuildInfo {
    current_build_info()
}

#[tauri::command]
fn get_global_shortcut_look_at_entire_screen(state: State<'_, AppState>) -> String {
    state
        .global_shortcut_look_at_entire_screen
        .lock()
        .unwrap()
        .clone()
}

#[tauri::command]
fn initialize_global_shortcut_look_at_entire_screen(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    shortcut_str: String,
) -> String {
    let mut hydrated = state
        .global_shortcut_look_at_entire_screen_hydrated
        .lock()
        .unwrap();
    let mut modified_before_hydration = state
        .global_shortcut_look_at_entire_screen_modified_before_hydration
        .lock()
        .unwrap();
    let mut current_shortcut_str = state.global_shortcut_look_at_entire_screen.lock().unwrap();

    if !*hydrated {
        if !*modified_before_hydration && *current_shortcut_str != shortcut_str {
            // Unregister old
            if let Ok(old_shortcut) = current_shortcut_str.parse::<Shortcut>() {
                let _ = app_handle.global_shortcut().unregister(old_shortcut);
            }

            // Register new
            if let Ok(new_shortcut) = shortcut_str.parse::<Shortcut>() {
                if let Err(err) = app_handle.global_shortcut().register(new_shortcut) {
                    error!("Failed to register new global shortcut: {}", err);
                } else {
                    *current_shortcut_str = shortcut_str;
                }
            }
        }

        *hydrated = true;
        *modified_before_hydration = false;
    }

    let effective_shortcut = current_shortcut_str.clone();
    drop(current_shortcut_str);
    drop(modified_before_hydration);
    drop(hydrated);

    effective_shortcut
}

#[tauri::command]
fn set_global_shortcut_look_at_entire_screen(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    shortcut_str: String,
) -> Result<String, String> {
    {
        let mut current_shortcut_str = state.global_shortcut_look_at_entire_screen.lock().unwrap();

        if *current_shortcut_str == shortcut_str {
            return Ok(shortcut_str);
        }

        // Try to parse the new shortcut first to validate it
        let new_shortcut = shortcut_str
            .parse::<Shortcut>()
            .map_err(|err| format!("Invalid shortcut format: {}", err))?;

        // Unregister old
        if let Ok(old_shortcut) = current_shortcut_str.parse::<Shortcut>() {
            let _ = app_handle.global_shortcut().unregister(old_shortcut);
        }

        // Register new
        app_handle
            .global_shortcut()
            .register(new_shortcut)
            .map_err(|err| format!("Failed to register global shortcut: {}", err))?;

        *current_shortcut_str = shortcut_str.clone();
    }

    let hydrated = state
        .global_shortcut_look_at_entire_screen_hydrated
        .lock()
        .unwrap();
    if !*hydrated {
        let mut modified_before_hydration = state
            .global_shortcut_look_at_entire_screen_modified_before_hydration
            .lock()
            .unwrap();
        *modified_before_hydration = true;
    }
    drop(hydrated);

    refresh_tray_menu(&app_handle);

    Ok(shortcut_str)
}

#[tauri::command]
fn get_global_shortcut_look_at_screen_region(state: State<'_, AppState>) -> String {
    state
        .global_shortcut_look_at_screen_region
        .lock()
        .unwrap()
        .clone()
}

#[tauri::command]
fn initialize_global_shortcut_look_at_screen_region(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    shortcut_str: String,
) -> String {
    let mut hydrated = state
        .global_shortcut_look_at_screen_region_hydrated
        .lock()
        .unwrap();
    let mut modified_before_hydration = state
        .global_shortcut_look_at_screen_region_modified_before_hydration
        .lock()
        .unwrap();
    let mut current_shortcut_str = state.global_shortcut_look_at_screen_region.lock().unwrap();

    if !*hydrated {
        if !*modified_before_hydration && *current_shortcut_str != shortcut_str {
            // Unregister old
            if let Ok(old_shortcut) = current_shortcut_str.parse::<Shortcut>() {
                let _ = app_handle.global_shortcut().unregister(old_shortcut);
            }

            // Register new
            if let Ok(new_shortcut) = shortcut_str.parse::<Shortcut>() {
                if let Err(err) = app_handle.global_shortcut().register(new_shortcut) {
                    error!("Failed to register new global shortcut: {}", err);
                } else {
                    *current_shortcut_str = shortcut_str;
                }
            }
        }

        *hydrated = true;
        *modified_before_hydration = false;
    }

    let effective_shortcut = current_shortcut_str.clone();
    drop(current_shortcut_str);
    drop(modified_before_hydration);
    drop(hydrated);

    effective_shortcut
}

#[tauri::command]
fn set_global_shortcut_look_at_screen_region(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    shortcut_str: String,
) -> Result<String, String> {
    {
        let mut current_shortcut_str = state.global_shortcut_look_at_screen_region.lock().unwrap();

        if *current_shortcut_str == shortcut_str {
            return Ok(shortcut_str);
        }

        // Try to parse the new shortcut first to validate it
        let new_shortcut = shortcut_str
            .parse::<Shortcut>()
            .map_err(|err| format!("Invalid shortcut format: {}", err))?;

        // Unregister old
        if let Ok(old_shortcut) = current_shortcut_str.parse::<Shortcut>() {
            let _ = app_handle.global_shortcut().unregister(old_shortcut);
        }

        // Register new
        app_handle
            .global_shortcut()
            .register(new_shortcut)
            .map_err(|err| format!("Failed to register global shortcut: {}", err))?;

        *current_shortcut_str = shortcut_str.clone();
    }

    let hydrated = state
        .global_shortcut_look_at_screen_region_hydrated
        .lock()
        .unwrap();
    if !*hydrated {
        let mut modified_before_hydration = state
            .global_shortcut_look_at_screen_region_modified_before_hydration
            .lock()
            .unwrap();
        *modified_before_hydration = true;
    }
    drop(hydrated);

    refresh_tray_menu(&app_handle);

    Ok(shortcut_str)
}

#[tauri::command]
fn get_global_shortcut_toggle_mute(state: State<'_, AppState>) -> String {
    state.global_shortcut_toggle_mute.lock().unwrap().clone()
}

#[tauri::command]
fn initialize_global_shortcut_toggle_mute(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    shortcut_str: String,
) -> String {
    let mut hydrated = state.global_shortcut_toggle_mute_hydrated.lock().unwrap();
    let mut modified_before_hydration = state
        .global_shortcut_toggle_mute_modified_before_hydration
        .lock()
        .unwrap();
    let mut current_shortcut_str = state.global_shortcut_toggle_mute.lock().unwrap();

    if !*hydrated {
        if !*modified_before_hydration && *current_shortcut_str != shortcut_str {
            if let Ok(old_shortcut) = current_shortcut_str.parse::<Shortcut>() {
                let _ = app_handle.global_shortcut().unregister(old_shortcut);
            }

            if let Ok(new_shortcut) = shortcut_str.parse::<Shortcut>() {
                if let Err(err) = app_handle.global_shortcut().register(new_shortcut) {
                    error!("Failed to register new global shortcut: {}", err);
                } else {
                    *current_shortcut_str = shortcut_str;
                }
            }
        }

        *hydrated = true;
        *modified_before_hydration = false;
    }

    let effective_shortcut = current_shortcut_str.clone();
    drop(current_shortcut_str);
    drop(modified_before_hydration);
    drop(hydrated);

    effective_shortcut
}

#[tauri::command]
fn get_global_shortcut_interrupt(state: State<'_, AppState>) -> String {
    state.global_shortcut_interrupt.lock().unwrap().clone()
}

#[tauri::command]
fn initialize_global_shortcut_interrupt(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    shortcut_str: String,
) -> String {
    let mut hydrated = state.global_shortcut_interrupt_hydrated.lock().unwrap();
    let mut modified_before_hydration = state
        .global_shortcut_interrupt_modified_before_hydration
        .lock()
        .unwrap();
    let mut current_shortcut_str = state.global_shortcut_interrupt.lock().unwrap();

    if !*hydrated {
        if !*modified_before_hydration && *current_shortcut_str != shortcut_str {
            if let Ok(old_shortcut) = current_shortcut_str.parse::<Shortcut>() {
                let _ = app_handle.global_shortcut().unregister(old_shortcut);
            }

            if let Ok(new_shortcut) = shortcut_str.parse::<Shortcut>() {
                if let Err(err) = app_handle.global_shortcut().register(new_shortcut) {
                    error!("Failed to register new global shortcut: {}", err);
                } else {
                    *current_shortcut_str = shortcut_str;
                }
            }
        }

        *hydrated = true;
        *modified_before_hydration = false;
    }

    let effective_shortcut = current_shortcut_str.clone();
    drop(current_shortcut_str);
    drop(modified_before_hydration);
    drop(hydrated);

    effective_shortcut
}

#[tauri::command]
fn set_global_shortcut_interrupt(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    shortcut_str: String,
) -> Result<String, String> {
    {
        let mut current_shortcut_str = state.global_shortcut_interrupt.lock().unwrap();

        if *current_shortcut_str == shortcut_str {
            return Ok(shortcut_str);
        }

        let new_shortcut = shortcut_str
            .parse::<Shortcut>()
            .map_err(|err| format!("Invalid shortcut format: {}", err))?;

        if let Ok(old_shortcut) = current_shortcut_str.parse::<Shortcut>() {
            let _ = app_handle.global_shortcut().unregister(old_shortcut);
        }

        app_handle
            .global_shortcut()
            .register(new_shortcut)
            .map_err(|err| format!("Failed to register global shortcut: {}", err))?;

        *current_shortcut_str = shortcut_str.clone();
    }

    let hydrated = state.global_shortcut_interrupt_hydrated.lock().unwrap();
    if !*hydrated {
        let mut modified_before_hydration = state
            .global_shortcut_interrupt_modified_before_hydration
            .lock()
            .unwrap();
        *modified_before_hydration = true;
    }
    drop(hydrated);

    refresh_tray_menu(&app_handle);

    Ok(shortcut_str)
}

#[tauri::command]
fn set_global_shortcut_toggle_mute(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    shortcut_str: String,
) -> Result<String, String> {
    {
        let mut current_shortcut_str = state.global_shortcut_toggle_mute.lock().unwrap();

        if *current_shortcut_str == shortcut_str {
            return Ok(shortcut_str);
        }

        let new_shortcut = shortcut_str
            .parse::<Shortcut>()
            .map_err(|err| format!("Invalid shortcut format: {}", err))?;

        if let Ok(old_shortcut) = current_shortcut_str.parse::<Shortcut>() {
            let _ = app_handle.global_shortcut().unregister(old_shortcut);
        }

        app_handle
            .global_shortcut()
            .register(new_shortcut)
            .map_err(|err| format!("Failed to register global shortcut: {}", err))?;

        *current_shortcut_str = shortcut_str.clone();
    }

    let hydrated = state.global_shortcut_toggle_mute_hydrated.lock().unwrap();
    if !*hydrated {
        let mut modified_before_hydration = state
            .global_shortcut_toggle_mute_modified_before_hydration
            .lock()
            .unwrap();
        *modified_before_hydration = true;
    }
    drop(hydrated);

    refresh_tray_menu(&app_handle);

    Ok(shortcut_str)
}

#[tauri::command]
async fn check_for_app_update(
    app_handle: AppHandle,
    pending_update: State<'_, PendingAppUpdate>,
) -> Result<Option<AppUpdateInfo>, String> {
    let update = build_app_updater(&app_handle)?
        .check()
        .await
        .map_err(|err| err.to_string())?;
    let metadata = update.as_ref().map(app_update_info);
    *pending_update.0.lock().unwrap() = update;
    Ok(metadata)
}

#[tauri::command]
async fn install_app_update(pending_update: State<'_, PendingAppUpdate>) -> Result<(), String> {
    let Some(update) = pending_update.0.lock().unwrap().take() else {
        return Err("There is no pending app update to install.".to_string());
    };

    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
fn restart_app(app_handle: AppHandle) {
    app_handle.restart();
}

#[tauri::command]
async fn refresh_runtime_caches(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Refreshing runtime caches...");

    // Stop all background processes that might be using the runtime
    let _ = stop_server_inner(state.inner());
    let _ = stop_csm_server_inner(state.inner()).await;
    let _ = stop_stt_server_inner(state.inner()).await;

    let runtime_root = resolve_runtime_root(&app_handle)?;
    let temp_dir = app_handle
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("openduck"))
        .join("runtime-bootstrap");

    if runtime_root.exists() {
        info!("Removing runtime directory: {}", runtime_root.display());
        std::fs::remove_dir_all(&runtime_root)
            .map_err(|err| format!("Failed to remove runtime directory: {err}"))?;
    }

    if temp_dir.exists() {
        info!(
            "Removing runtime-bootstrap directory: {}",
            temp_dir.display()
        );
        std::fs::remove_dir_all(&temp_dir)
            .map_err(|err| format!("Failed to remove runtime-bootstrap directory: {err}"))?;
    }

    info!("Restarting application...");
    app_handle.restart();

    #[allow(unreachable_code)]
    Ok(())
}

#[tauri::command]
async fn capture_screen_selection(app_handle: AppHandle) -> Result<(), String> {
    capture_screen_selection_inner(&app_handle).await
}

#[tauri::command]
fn clear_pending_screen_capture(app_handle: AppHandle) {
    clear_pending_screen_capture_inner(&app_handle, true);
}

#[tauri::command]
fn attach_pasted_screen_capture(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    data_url: String,
) -> Result<(), String> {
    if !*state.call_in_progress.lock().unwrap() {
        return Err("Pasted screenshots are only available during an active call.".to_string());
    }

    let Some(path) = write_data_url_to_temp_file(&data_url) else {
        return Err("Failed to decode pasted screenshot.".to_string());
    };

    resize_image_file_for_context(&path);
    attach_pending_screen_capture(
        &app_handle,
        path,
        "Pasted screenshot attached to your next turn.",
    );
    Ok(())
}

#[tauri::command]
fn set_voice_system_prompt(state: State<'_, AppState>, prompt: String) {
    let trimmed_prompt = prompt.trim();
    let next_prompt = if trimmed_prompt.is_empty() {
        DEFAULT_VOICE_SYSTEM_PROMPT.to_string()
    } else {
        trimmed_prompt.to_string()
    };

    *state.voice_system_prompt.lock().unwrap() = next_prompt;
}

#[tauri::command]
fn export_contact_profile(payload: ContactExportPayload) -> Result<ContactExportResult, String> {
    let export_path = normalize_contact_export_path(&payload.output_path)?;
    let export_json = serde_json::json!({
        "version": 1,
        "name": payload.name.trim(),
        "prompt": payload.prompt,
        "iconDataUrl": payload.icon_data_url,
        "refAudio": payload.ref_audio,
        "refText": payload.ref_text,
    });
    let encoded = serde_json::to_vec_pretty(&export_json).map_err(|err| err.to_string())?;

    if let Some(parent) = export_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create export directory at {}: {err}",
                parent.display()
            )
        })?;
    }

    std::fs::write(&export_path, encoded).map_err(|err| {
        format!(
            "Failed to export contact to {}: {err}",
            export_path.display()
        )
    })?;

    Ok(ContactExportResult {
        saved_path: export_path.display().to_string(),
    })
}

#[tauri::command]
async fn reset_call_session(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    cancel_active_generation(&app_handle, false).await;
    set_tts_playback_active_state(&app_handle, false);
    reset_call_session_state(state.inner());
    clear_pending_screen_capture_inner(&app_handle, true);
    emit_call_stage(&app_handle, "idle", "");
    reset_csm_reference_context(&app_handle).await?;
    Ok(())
}

#[tauri::command]
async fn interrupt_tts(app_handle: tauri::AppHandle) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    mark_latest_assistant_turn_auto_continue_consumed(state.inner());
    set_tts_playback_active_state(&app_handle, false);
    interrupt_active_generation(&app_handle).await;
    Ok(())
}

#[tauri::command]
fn set_tts_playback_active(app_handle: tauri::AppHandle, active: bool) {
    set_tts_playback_active_state(&app_handle, active);
}

#[tauri::command]
fn set_pong_playback_enabled(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) {
    {
        let mut tray_pong_playback_enabled = state.tray_pong_playback_enabled.lock().unwrap();
        *tray_pong_playback_enabled = enabled;
    }

    if !*state.tray_pong_playback_hydrated.lock().unwrap() {
        *state
            .tray_pong_playback_modified_before_hydration
            .lock()
            .unwrap() = true;
    }

    refresh_tray_menu(&app_handle);
}

#[tauri::command]
fn initialize_pong_playback_preference(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> bool {
    let mut tray_pong_playback_hydrated = state.tray_pong_playback_hydrated.lock().unwrap();
    let mut tray_pong_playback_modified_before_hydration = state
        .tray_pong_playback_modified_before_hydration
        .lock()
        .unwrap();
    let mut tray_pong_playback_enabled = state.tray_pong_playback_enabled.lock().unwrap();
    let mut changed = false;

    if !*tray_pong_playback_hydrated {
        if !*tray_pong_playback_modified_before_hydration && *tray_pong_playback_enabled != enabled
        {
            *tray_pong_playback_enabled = enabled;
            changed = true;
        }

        *tray_pong_playback_hydrated = true;
        *tray_pong_playback_modified_before_hydration = false;
    }

    let effective_enabled = *tray_pong_playback_enabled;
    drop(tray_pong_playback_enabled);
    drop(tray_pong_playback_modified_before_hydration);
    drop(tray_pong_playback_hydrated);

    if changed {
        refresh_tray_menu(&app_handle);
    }

    effective_enabled
}

fn clamp_end_of_utterance_silence_ms(milliseconds: u32) -> u32 {
    milliseconds.clamp(
        MIN_END_OF_UTTERANCE_SILENCE_MS,
        MAX_END_OF_UTTERANCE_SILENCE_MS,
    )
}

fn clamp_auto_continue_silence_ms(milliseconds: u32) -> u32 {
    milliseconds.clamp(MIN_AUTO_CONTINUE_SILENCE_MS, MAX_AUTO_CONTINUE_SILENCE_MS)
}

fn clamp_auto_continue_max_count(count: u32) -> u32 {
    count.clamp(MIN_AUTO_CONTINUE_MAX_COUNT, MAX_AUTO_CONTINUE_MAX_COUNT)
}

fn clamp_llm_context_turn_limit(limit: u32) -> usize {
    limit.clamp(
        MIN_LLM_CONTEXT_TURN_LIMIT as u32,
        MAX_LLM_CONTEXT_TURN_LIMIT as u32,
    ) as usize
}

fn clamp_llm_image_history_limit(limit: u32) -> usize {
    limit.clamp(
        MIN_LLM_IMAGE_HISTORY_LIMIT as u32,
        MAX_LLM_IMAGE_HISTORY_LIMIT as u32,
    ) as usize
}

#[tauri::command]
fn set_end_of_utterance_silence_ms(state: State<'_, AppState>, milliseconds: u32) -> u32 {
    let effective_milliseconds = clamp_end_of_utterance_silence_ms(milliseconds);
    *state.end_of_utterance_silence_ms.lock().unwrap() = effective_milliseconds;
    effective_milliseconds
}

#[tauri::command]
fn set_auto_continue_silence_ms(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    milliseconds: Option<u32>,
) -> Option<u32> {
    let effective_milliseconds = milliseconds.map(clamp_auto_continue_silence_ms);
    *state.auto_continue_silence_ms.lock().unwrap() = effective_milliseconds;
    cancel_auto_continue_timer(state.inner());
    if effective_milliseconds.is_some() {
        maybe_schedule_auto_continue_after_tts_idle(&app_handle);
    }
    effective_milliseconds
}

#[tauri::command]
fn set_auto_continue_max_count(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    count: Option<u32>,
) -> Option<u32> {
    let effective_count = count.map(clamp_auto_continue_max_count);
    *state.auto_continue_max_count.lock().unwrap() = effective_count;
    cancel_auto_continue_timer(state.inner());
    if *state.auto_continue_silence_ms.lock().unwrap() != DEFAULT_AUTO_CONTINUE_SILENCE_MS {
        maybe_schedule_auto_continue_after_tts_idle(&app_handle);
    }
    effective_count
}

#[tauri::command]
fn set_llm_context_turn_limit(state: State<'_, AppState>, limit: Option<u32>) -> Option<u32> {
    let effective_limit = limit.map(clamp_llm_context_turn_limit);
    *state.llm_context_turn_limit.lock().unwrap() = effective_limit;
    effective_limit.map(|value| value as u32)
}

#[tauri::command]
fn set_llm_image_history_limit(state: State<'_, AppState>, limit: Option<u32>) -> Option<u32> {
    let effective_limit = limit.map(clamp_llm_image_history_limit);
    *state.llm_image_history_limit.lock().unwrap() = effective_limit;
    effective_limit.map(|value| value as u32)
}

#[tauri::command]
fn delete_conversation_context_entry(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    entry_id: u64,
) -> Result<Option<u64>, String> {
    let mut removed_image_paths = Vec::new();
    let mut other_entry_id = None;
    let mut updated = false;

    {
        let mut conversation_turns = state.conversation_turns.lock().unwrap();
        let mut index_to_remove = None;

        for (i, turn) in conversation_turns.iter().enumerate() {
            if turn.user_entry_id == entry_id {
                index_to_remove = Some(i);
                other_entry_id = Some(turn.assistant_entry_id);
                break;
            }
            if turn.assistant_entry_id == entry_id {
                index_to_remove = Some(i);
                other_entry_id = Some(turn.user_entry_id);
                break;
            }
        }

        if let Some(i) = index_to_remove {
            let removed_turn = conversation_turns.remove(i).unwrap();
            removed_image_paths = removed_turn.image_paths;
            updated = true;
            info!(
                "Deleted conversation turn containing entry {}. Other entry in turn was {}.",
                entry_id,
                other_entry_id.unwrap_or(0)
            );
        }
    }

    if !updated {
        warn!(
            "Attempted to delete entry {}, but it was not found in the active context.",
            entry_id
        );
        return Err("Conversation entry is no longer part of the active context.".to_string());
    }

    for image_path in removed_image_paths {
        unregister_conversation_image_path(state.inner(), &image_path);
        remove_temp_image_file(&image_path);
    }

    if let Err(err) = save_current_session(&app_handle, state.inner()) {
        error!("Failed to save session after entry deletion: {}", err);
    }

    Ok(other_entry_id)
}

#[tauri::command]
fn update_conversation_context_entry(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    entry_id: u64,
    text: String,
    clear_images: bool,
) -> Result<(), String> {
    let normalized_text = text.trim().to_string();
    let mut removed_image_paths = Vec::new();
    let mut updated = false;

    {
        let mut conversation_turns = state.conversation_turns.lock().unwrap();

        for turn in conversation_turns.iter_mut() {
            if turn.user_entry_id == entry_id {
                let keep_existing_images = !turn.image_paths.is_empty() && !clear_images;
                if normalized_text.is_empty() && !keep_existing_images {
                    return Err(
                        "Conversation user entries need text or an attached image.".to_string()
                    );
                }

                turn.user_text = normalized_text.clone();
                if clear_images {
                    removed_image_paths = std::mem::take(&mut turn.image_paths);
                    turn.user_image_data_urls.clear();
                }
                updated = true;
                info!(
                    "Updated user entry {} with text: {:?}",
                    entry_id, turn.user_text
                );
                break;
            }

            if turn.assistant_entry_id == entry_id {
                if clear_images {
                    return Err("Assistant entries do not contain removable images.".to_string());
                }
                if normalized_text.is_empty() {
                    return Err("Conversation assistant entries cannot be empty.".to_string());
                }

                turn.assistant_text = normalized_text.clone();
                updated = true;
                info!(
                    "Updated assistant entry {} with text: {:?}",
                    entry_id, turn.assistant_text
                );
                break;
            }
        }
    }

    if !updated {
        return Err(format!(
            "Entry ID {} is no longer part of the active context.",
            entry_id
        ));
    }

    for path in removed_image_paths {
        unregister_conversation_image_path(state.inner(), &path);
        remove_temp_image_file(&path);
    }

    if let Err(err) = save_current_session(&app_handle, state.inner()) {
        error!("Failed to save session: {}", err);
    }

    Ok(())
}

#[tauri::command]
fn clear_conversation_context_images(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> usize {
    let removed_count = clear_conversation_context_images_inner(state.inner());
    if removed_count > 0 {
        emit_conversation_image_history_cleared(&app_handle);
        emit_overlay_notification(
            &app_handle,
            OverlayNotificationEvent {
                message: "OpenDuck: Cleared Image History".to_string(),
            },
        );
    }
    removed_count
}

#[tauri::command]
fn sync_conversation_log_has_visible_images(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    visible: bool,
) {
    set_conversation_log_has_visible_images_state(&app_handle, state.inner(), visible);
}

fn clear_conversation_context_images_inner(state: &AppState) -> usize {
    let removed_image_paths = {
        let mut conversation_turns = state.conversation_turns.lock().unwrap();
        let mut removed_paths = Vec::new();

        for turn in conversation_turns.iter_mut() {
            turn.user_image_data_urls.clear();
            removed_paths.extend(std::mem::take(&mut turn.image_paths));
        }

        removed_paths
    };

    if removed_image_paths.is_empty() {
        return 0;
    }

    let removed_image_path_set = removed_image_paths.iter().cloned().collect::<HashSet<_>>();
    state
        .conversation_image_paths
        .lock()
        .unwrap()
        .retain(|candidate| !removed_image_path_set.contains(candidate));

    for image_path in &removed_image_paths {
        remove_temp_image_file(image_path);
    }

    removed_image_path_set.len()
}

fn has_conversation_image_history(state: &AppState) -> bool {
    state
        .conversation_turns
        .lock()
        .unwrap()
        .iter()
        .any(|turn| !turn.image_paths.is_empty() || !turn.user_image_data_urls.is_empty())
}

fn set_conversation_log_has_visible_images_state(
    app_handle: &AppHandle,
    state: &AppState,
    visible: bool,
) {
    let changed = {
        let mut conversation_log_has_visible_images =
            state.conversation_log_has_visible_images.lock().unwrap();
        if *conversation_log_has_visible_images == visible {
            false
        } else {
            *conversation_log_has_visible_images = visible;
            true
        }
    };

    if changed {
        refresh_tray_menu(app_handle);
    }
}

#[tauri::command]
async fn is_server_running(state: State<'_, AppState>) -> Result<bool, String> {
    let selected_variant = selected_gemma_variant(state.inner());
    let loaded_variant = loaded_gemma_variant(state.inner());

    if selected_variant.is_external() {
        if loaded_variant != Some(selected_variant) {
            return Ok(false);
        }
        return Ok(external_llm_service_is_running(state.inner(), selected_variant).await);
    }

    let port = {
        let port_guard = state.server_port.lock().unwrap();
        *port_guard
    };
    let Some(port) = port else {
        return Ok(false);
    };

    Ok(gemma_server_is_running_on_port(port).await)
}

fn normalize_optional_api_key(key: Option<String>) -> Option<String> {
    key.and_then(|value| {
        if value.trim().is_empty() {
            None
        } else {
            Some(value.trim().to_string())
        }
    })
}

fn normalize_base_url(url: String) -> String {
    url.trim().to_string()
}

fn selected_external_llm_model(state: &AppState, variant: GemmaVariant) -> Option<String> {
    match variant {
        GemmaVariant::Ollama => Some(state.selected_ollama_model.lock().unwrap().clone()),
        GemmaVariant::LmStudio => Some(state.selected_lmstudio_model.lock().unwrap().clone()),
        GemmaVariant::OpenAiCompatible => Some(
            state
                .selected_openai_compatible_model
                .lock()
                .unwrap()
                .clone(),
        ),
        GemmaVariant::E4b | GemmaVariant::E2b => None,
    }
}

fn external_llm_base_url(state: &AppState, variant: GemmaVariant) -> Option<String> {
    match variant {
        GemmaVariant::Ollama => Some(state.ollama_base_url.lock().unwrap().clone()),
        GemmaVariant::LmStudio => Some(state.lmstudio_base_url.lock().unwrap().clone()),
        GemmaVariant::OpenAiCompatible => {
            Some(state.openai_compatible_base_url.lock().unwrap().clone())
        }
        GemmaVariant::E4b | GemmaVariant::E2b => None,
    }
}

fn external_llm_api_key(state: &AppState, variant: GemmaVariant) -> Option<String> {
    match variant {
        GemmaVariant::Ollama => state.ollama_api_key.lock().unwrap().clone(),
        GemmaVariant::LmStudio => state.lmstudio_api_key.lock().unwrap().clone(),
        GemmaVariant::OpenAiCompatible => state.openai_compatible_api_key.lock().unwrap().clone(),
        GemmaVariant::E4b | GemmaVariant::E2b => None,
    }
}

async fn ollama_service_is_running(state: &AppState) -> bool {
    let client = reqwest::Client::new();
    let base_url = state.ollama_base_url.lock().unwrap().clone();
    if base_url.trim().is_empty() {
        return false;
    }
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let mut request = client.get(url);

    if let Some(api_key) = state.ollama_api_key.lock().unwrap().as_ref() {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    match request.send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

async fn openai_compatible_service_is_running(base_url: String, api_key: Option<String>) -> bool {
    if base_url.trim().is_empty() {
        return false;
    }

    let client = reqwest::Client::new();
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let mut request = client.get(url);

    if let Some(api_key) = api_key.as_ref() {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    match request.send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

async fn lmstudio_service_is_running(state: &AppState) -> bool {
    let base_url = state.lmstudio_base_url.lock().unwrap().clone();
    let api_key = state.lmstudio_api_key.lock().unwrap().clone();
    openai_compatible_service_is_running(base_url, api_key).await
}

async fn openai_compatible_api_service_is_running(state: &AppState) -> bool {
    let base_url = state.openai_compatible_base_url.lock().unwrap().clone();
    let api_key = state.openai_compatible_api_key.lock().unwrap().clone();
    openai_compatible_service_is_running(base_url, api_key).await
}

async fn external_llm_service_is_running(state: &AppState, variant: GemmaVariant) -> bool {
    match variant {
        GemmaVariant::Ollama => ollama_service_is_running(state).await,
        GemmaVariant::LmStudio => lmstudio_service_is_running(state).await,
        GemmaVariant::OpenAiCompatible => openai_compatible_api_service_is_running(state).await,
        GemmaVariant::E4b | GemmaVariant::E2b => false,
    }
}

#[tauri::command]
async fn check_ollama_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(ollama_service_is_running(state.inner()).await)
}

#[tauri::command]
async fn check_lmstudio_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(lmstudio_service_is_running(state.inner()).await)
}

#[tauri::command]
async fn check_openai_compatible_status(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(openai_compatible_api_service_is_running(state.inner()).await)
}

async fn fetch_openai_compatible_models(
    base_url: String,
    api_key: Option<String>,
    provider_label: &str,
) -> Result<Vec<String>, String> {
    if base_url.trim().is_empty() {
        return Err(format!("{provider_label} base URL is not configured."));
    }

    let client = reqwest::Client::new();
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let mut request = client.get(url);

    if let Some(api_key) = api_key.as_ref() {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    let resp = request
        .send()
        .await
        .map_err(|e| format!("Failed to connect to {provider_label}: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "{provider_label} returned error: {}",
            resp.status()
        ));
    }

    let payload = resp
        .json::<OpenAiModelsResponse>()
        .await
        .map_err(|e| format!("Failed to parse {provider_label} models: {e}"))?;

    Ok(payload.data.into_iter().map(|m| m.id).collect())
}

#[tauri::command]
async fn get_ollama_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let client = reqwest::Client::new();
    let base_url = state.ollama_base_url.lock().unwrap().clone();
    let url = format!("{}/api/tags", base_url.trim_end_matches('/'));
    let mut request = client.get(url);

    if let Some(api_key) = state.ollama_api_key.lock().unwrap().as_ref() {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }

    let resp = request
        .send()
        .await
        .map_err(|e| format!("Failed to connect to Ollama: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Ollama returned error: {}", resp.status()));
    }

    let payload = resp
        .json::<OllamaTagsResponse>()
        .await
        .map_err(|e| format!("Failed to parse Ollama models: {e}"))?;

    Ok(payload.models.into_iter().map(|m| m.name).collect())
}

#[tauri::command]
async fn get_lmstudio_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let base_url = state.lmstudio_base_url.lock().unwrap().clone();
    let api_key = state.lmstudio_api_key.lock().unwrap().clone();
    fetch_openai_compatible_models(base_url, api_key, "LM Studio").await
}

#[tauri::command]
async fn get_openai_compatible_models(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let base_url = state.openai_compatible_base_url.lock().unwrap().clone();
    let api_key = state.openai_compatible_api_key.lock().unwrap().clone();
    fetch_openai_compatible_models(base_url, api_key, "OpenAI-compatible API").await
}

#[tauri::command]
fn get_ollama_model(state: State<'_, AppState>) -> String {
    state.selected_ollama_model.lock().unwrap().clone()
}

#[tauri::command]
fn get_lmstudio_model(state: State<'_, AppState>) -> String {
    state.selected_lmstudio_model.lock().unwrap().clone()
}

#[tauri::command]
fn get_openai_compatible_model(state: State<'_, AppState>) -> String {
    state
        .selected_openai_compatible_model
        .lock()
        .unwrap()
        .clone()
}

#[tauri::command]
fn set_ollama_model(state: State<'_, AppState>, model: String) {
    let mut model_guard = state.selected_ollama_model.lock().unwrap();
    *model_guard = model;
}

#[tauri::command]
fn set_lmstudio_model(state: State<'_, AppState>, model: String) {
    let mut model_guard = state.selected_lmstudio_model.lock().unwrap();
    *model_guard = model;
}

#[tauri::command]
fn set_openai_compatible_model(state: State<'_, AppState>, model: String) {
    let mut model_guard = state.selected_openai_compatible_model.lock().unwrap();
    *model_guard = model;
}

#[tauri::command]
fn get_ollama_config(state: State<'_, AppState>) -> (String, Option<String>) {
    let url = state.ollama_base_url.lock().unwrap().clone();
    let key = state.ollama_api_key.lock().unwrap().clone();
    (url, key)
}

#[tauri::command]
fn get_lmstudio_config(state: State<'_, AppState>) -> (String, Option<String>) {
    let url = state.lmstudio_base_url.lock().unwrap().clone();
    let key = state.lmstudio_api_key.lock().unwrap().clone();
    (url, key)
}

#[tauri::command]
fn get_openai_compatible_config(state: State<'_, AppState>) -> (String, Option<String>) {
    let url = state.openai_compatible_base_url.lock().unwrap().clone();
    let key = state.openai_compatible_api_key.lock().unwrap().clone();
    (url, key)
}

#[tauri::command]
fn set_ollama_config(
    state: State<'_, AppState>,
    url: String,
    key: Option<String>,
) -> Result<(), String> {
    {
        let mut url_guard = state.ollama_base_url.lock().unwrap();
        *url_guard = normalize_base_url(url);
    }
    {
        let mut key_guard = state.ollama_api_key.lock().unwrap();
        *key_guard = normalize_optional_api_key(key);
    }
    persist_external_llm_provider_configs(state.inner())
}

#[tauri::command]
fn set_lmstudio_config(
    state: State<'_, AppState>,
    url: String,
    key: Option<String>,
) -> Result<(), String> {
    {
        let mut url_guard = state.lmstudio_base_url.lock().unwrap();
        *url_guard = normalize_base_url(url);
    }
    {
        let mut key_guard = state.lmstudio_api_key.lock().unwrap();
        *key_guard = normalize_optional_api_key(key);
    }
    persist_external_llm_provider_configs(state.inner())
}

#[tauri::command]
fn set_openai_compatible_config(
    state: State<'_, AppState>,
    url: String,
    key: Option<String>,
) -> Result<(), String> {
    {
        let mut url_guard = state.openai_compatible_base_url.lock().unwrap();
        *url_guard = normalize_base_url(url);
    }
    {
        let mut key_guard = state.openai_compatible_api_key.lock().unwrap();
        *key_guard = normalize_optional_api_key(key);
    }
    persist_external_llm_provider_configs(state.inner())
}

#[tauri::command]
async fn is_csm_running(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(csm_process_is_ready(state.inner()).await)
}

#[tauri::command]
async fn is_stt_running(state: State<'_, AppState>) -> Result<bool, String> {
    if !selected_stt_model(state.inner()).uses_worker() {
        return Ok(true);
    }

    Ok(stt_process_is_ready(state.inner()).await)
}

#[tauri::command]
fn get_model_memory_usage(state: State<'_, AppState>) -> Result<ModelMemoryUsageSnapshot, String> {
    loaded_model_memory_snapshot(state.inner())
}

fn selected_gemma_variant(state: &AppState) -> GemmaVariant {
    *state.selected_gemma_variant.lock().unwrap()
}

fn loaded_gemma_variant(state: &AppState) -> Option<GemmaVariant> {
    *state.loaded_gemma_variant.lock().unwrap()
}

fn selected_csm_model(state: &AppState) -> CsmModelVariant {
    *state.selected_csm_model.lock().unwrap()
}

fn loaded_csm_model(state: &AppState) -> Option<CsmModelVariant> {
    *state.loaded_csm_model.lock().unwrap()
}

fn selected_stt_model(state: &AppState) -> SttModelVariant {
    *state.selected_stt_model.lock().unwrap()
}

fn loaded_stt_model(state: &AppState) -> Option<SttModelVariant> {
    *state.loaded_stt_model.lock().unwrap()
}

fn format_memory_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.2} GB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.0} MB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.0} KB", bytes / KIB)
    } else {
        format!("{} B", bytes as u64)
    }
}

fn snapshot_process_memory() -> Result<HashMap<u32, ProcessMemorySnapshot>, String> {
    let output = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,rss="])
        .output()
        .map_err(|err| format!("Failed to inspect process memory: {err}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "Failed to inspect process memory with ps: {}",
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut snapshots = HashMap::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut parts = trimmed.split_whitespace();
        let Some(pid) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Some(ppid) = parts.next().and_then(|value| value.parse::<u32>().ok()) else {
            continue;
        };
        let Some(rss_kb) = parts.next().and_then(|value| value.parse::<u64>().ok()) else {
            continue;
        };

        snapshots.insert(pid, ProcessMemorySnapshot { ppid, rss_kb });
    }

    Ok(snapshots)
}

fn aggregate_process_tree_memory(
    process_snapshots: &HashMap<u32, ProcessMemorySnapshot>,
    root_pid: u32,
) -> Option<(u64, usize)> {
    let mut children_by_parent: HashMap<u32, Vec<u32>> = HashMap::new();
    for (&pid, snapshot) in process_snapshots {
        children_by_parent
            .entry(snapshot.ppid)
            .or_default()
            .push(pid);
    }

    let mut visited = HashSet::new();
    let mut stack = vec![root_pid];
    let mut total_rss_kb = 0_u64;
    let mut process_count = 0_usize;

    while let Some(pid) = stack.pop() {
        if !visited.insert(pid) {
            continue;
        }

        let Some(snapshot) = process_snapshots.get(&pid) else {
            continue;
        };

        total_rss_kb = total_rss_kb.saturating_add(snapshot.rss_kb);
        process_count += 1;

        if let Some(children) = children_by_parent.get(&pid) {
            stack.extend(children.iter().copied());
        }
    }

    if process_count == 0 {
        None
    } else {
        Some((total_rss_kb.saturating_mul(1024), process_count))
    }
}

fn loaded_gemma_root_pid(state: &AppState) -> Option<u32> {
    state
        .server_process
        .lock()
        .unwrap()
        .as_ref()
        .map(|child| child.pid())
}

fn loaded_csm_root_pid(state: &AppState) -> Option<u32> {
    let process = {
        let process_guard = state.csm_process.lock().unwrap();
        process_guard.clone()
    }?;

    let child = process.child.try_lock().ok()?;
    child.id()
}

fn loaded_stt_root_pid(state: &AppState) -> Option<u32> {
    let process = {
        let process_guard = state.stt_process.lock().unwrap();
        process_guard.clone()
    }?;

    let child = process.child.try_lock().ok()?;
    child.id()
}

fn loaded_model_memory_snapshot(state: &AppState) -> Result<ModelMemoryUsageSnapshot, String> {
    let mut targets: Vec<(String, String, Option<String>, u32)> = Vec::new();
    let mut models = Vec::new();

    if let Some(loaded_variant) = loaded_gemma_variant(state) {
        let (label, detail) = if loaded_variant.is_external() {
            (
                loaded_variant.label().to_string(),
                selected_external_llm_model(state, loaded_variant),
            )
        } else {
            ("LLM".to_string(), Some(loaded_variant.label().to_string()))
        };

        if let Some(root_pid) = loaded_gemma_root_pid(state) {
            targets.push(("gemma".to_string(), label, detail, root_pid));
        } else if loaded_variant.is_external() {
            // External providers do not have a local PID we track.
            models.push(ModelMemoryUsageEntry {
                key: "gemma".to_string(),
                label,
                detail,
                bytes: 0,
                root_pid: 0,
                process_count: 0,
            });
        }
    }

    if let Some(loaded_variant) = loaded_stt_model(state) {
        if let Some(root_pid) = loaded_stt_root_pid(state) {
            targets.push((
                "stt".to_string(),
                "STT".to_string(),
                Some(loaded_variant.label().to_string()),
                root_pid,
            ));
        }
    }

    if let Some(loaded_variant) = loaded_csm_model(state) {
        if let Some(root_pid) = loaded_csm_root_pid(state) {
            targets.push((
                "csm".to_string(),
                "TTS".to_string(),
                Some(loaded_variant.label().to_string()),
                root_pid,
            ));
        }
    }

    if targets.is_empty() && models.is_empty() {
        return Ok(ModelMemoryUsageSnapshot::default());
    }

    let process_snapshots = if !targets.is_empty() {
        snapshot_process_memory()?
    } else {
        std::collections::HashMap::new()
    };
    let mut total_bytes = 0_u64;

    for (key, label, detail, root_pid) in targets {
        let Some((bytes, process_count)) =
            aggregate_process_tree_memory(&process_snapshots, root_pid)
        else {
            continue;
        };

        total_bytes = total_bytes.saturating_add(bytes);
        models.push(ModelMemoryUsageEntry {
            key,
            label,
            detail,
            bytes,
            root_pid,
            process_count,
        });
    }

    Ok(ModelMemoryUsageSnapshot {
        total_bytes,
        models,
    })
}

#[tauri::command]
fn get_gemma_variant(state: State<'_, AppState>) -> String {
    selected_gemma_variant(state.inner()).key().to_string()
}

#[tauri::command]
fn set_gemma_variant(state: State<'_, AppState>, variant: String) -> Result<(), String> {
    let selected_variant = GemmaVariant::from_key(&variant)?;

    if let Some(loaded_variant) = loaded_gemma_variant(state.inner()) {
        if loaded_variant != selected_variant {
            return Err(format!(
                "LLM {} is already loaded. Unload it before switching to {}.",
                loaded_variant.label(),
                selected_variant.label()
            ));
        }
    }

    let mut variant_guard = state.selected_gemma_variant.lock().unwrap();
    *variant_guard = selected_variant;
    Ok(())
}

#[tauri::command]
fn get_stt_model_variant(state: State<'_, AppState>) -> String {
    selected_stt_model(state.inner()).key().to_string()
}

#[tauri::command]
fn set_stt_model_variant(state: State<'_, AppState>, variant: String) -> Result<(), String> {
    let selected_variant = SttModelVariant::from_key(&variant)?;

    if active_download_process(state.inner(), DownloadModel::Stt).is_some() {
        return Err("An STT model download is already in progress.".to_string());
    }

    if let Some(loaded_variant) = loaded_stt_model(state.inner()) {
        if loaded_variant != selected_variant {
            return Err(format!(
                "{} is already loaded for STT. Unload it before switching to {}.",
                loaded_variant.label(),
                selected_variant.label()
            ));
        }
    }

    let mut variant_guard = state.selected_stt_model.lock().unwrap();
    *variant_guard = selected_variant;
    Ok(())
}

async fn ensure_runtime_dependencies_inner(
    app_handle: &tauri::AppHandle,
    state: &AppState,
) -> Result<(), String> {
    if runtime_dependencies_available(app_handle) {
        return Ok(());
    }

    let _guard = state.runtime_setup_lock.lock().await;
    if runtime_dependencies_available(app_handle) {
        return Ok(());
    }

    let runtime_root = resolve_runtime_root(app_handle)?;
    let setup_script = resolve_setup_script(app_handle)?;
    let patch_script = resolve_resource_file(app_handle, "patch_mlx_vlm.py")?;
    let resource_files_dir = patch_script
        .parent()
        .map(PathBuf::from)
        .ok_or_else(|| "Failed to resolve the bundled runtime resource directory".to_string())?;
    let temp_dir = app_handle
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("openduck"))
        .join("runtime-bootstrap");

    std::fs::create_dir_all(&runtime_root).map_err(|err| {
        format!(
            "Failed to create the OpenDuck runtime directory at {}: {err}",
            runtime_root.display()
        )
    })?;
    std::fs::create_dir_all(&temp_dir).map_err(|err| {
        format!(
            "Failed to create the OpenDuck cache directory at {}: {err}",
            temp_dir.display()
        )
    })?;

    emit_runtime_setup_status(
        app_handle,
        RuntimeSetupStatusEvent {
            phase: "starting".to_string(),
            message:
                "Preparing local Python runtime. This can take several minutes on first launch."
                    .to_string(),
        },
    );

    info!(
        "Bootstrapping local runtime with {} into {}",
        setup_script.display(),
        runtime_root.display()
    );

    let mut command = Command::new("/bin/bash");
    command
        .arg(&setup_script)
        .env("OPEN_DUCK_RUNTIME_ROOT", &runtime_root)
        .env("OPEN_DUCK_RESOURCE_FILES_DIR", &resource_files_dir)
        .env("OPEN_DUCK_PATCH_SERVER_SCRIPT", &patch_script)
        .env("OPEN_DUCK_TEMP_DIR", &temp_dir)
        .env("PYTHONUNBUFFERED", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|err| format!("Failed to start the OpenDuck runtime setup: {err}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture OpenDuck runtime setup stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to capture OpenDuck runtime setup stderr".to_string())?;

    let app_handle_for_stdout = app_handle.clone();
    let stdout_handle = tauri::async_runtime::spawn(async move {
        let mut stdout_lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = stdout_lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Some(message) = trimmed.strip_prefix("OPEN_DUCK_STATUS:") {
                let message = message.trim().to_string();
                info!("Runtime setup: {}", message);
                emit_runtime_setup_status(
                    &app_handle_for_stdout,
                    RuntimeSetupStatusEvent {
                        phase: "progress".to_string(),
                        message,
                    },
                );
            } else {
                info!("Runtime setup stdout: {}", trimmed);
            }
        }
    });

    let stderr_handle = tauri::async_runtime::spawn(async move {
        let mut stderr_lines = BufReader::new(stderr).lines();
        let mut collected = VecDeque::new();
        while let Ok(Some(line)) = stderr_lines.next_line().await {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            collected.push_back(trimmed.to_string());
            while collected.len() > 12 {
                collected.pop_front();
            }
            warn!("Runtime setup stderr: {}", trimmed);
        }

        collected
    });

    let status = child
        .wait()
        .await
        .map_err(|err| format!("Failed while waiting for the OpenDuck runtime setup: {err}"))?;

    if let Err(err) = stdout_handle.await {
        warn!("Failed to join the runtime setup stdout task: {}", err);
    }
    let stderr_tail = match stderr_handle.await {
        Ok(lines) => lines,
        Err(err) => {
            warn!("Failed to join the runtime setup stderr task: {}", err);
            VecDeque::new()
        }
    };

    if status.success() && runtime_dependencies_available(app_handle) {
        emit_runtime_setup_status(
            app_handle,
            RuntimeSetupStatusEvent {
                phase: "completed".to_string(),
                message: "Local Python runtime is ready.".to_string(),
            },
        );
        info!("Local runtime setup completed successfully");
        return Ok(());
    }

    let stderr_summary = stderr_tail
        .iter()
        .rev()
        .find(|line| !line.is_empty())
        .cloned();
    let error_message = if status.success() {
        "Runtime setup finished, but required Python dependencies are still missing.".to_string()
    } else if let Some(summary) = stderr_summary {
        format!("Runtime setup failed: {summary}")
    } else {
        format!("Runtime setup failed with status {status}")
    };

    emit_runtime_setup_status(
        app_handle,
        RuntimeSetupStatusEvent {
            phase: "error".to_string(),
            message: error_message.clone(),
        },
    );
    Err(error_message)
}

#[tauri::command]
async fn ensure_runtime_dependencies(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    ensure_runtime_dependencies_inner(&app_handle, state.inner()).await
}

#[tauri::command]
async fn start_server(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    ensure_runtime_dependencies_inner(&app_handle, state.inner()).await?;

    let selected_variant = selected_gemma_variant(state.inner());

    if selected_variant.is_external() {
        if !external_llm_service_is_running(state.inner(), selected_variant).await {
            return Err(format!(
                "{} service is not responding at the configured URL.",
                selected_variant.label()
            ));
        }

        let selected_model =
            selected_external_llm_model(state.inner(), selected_variant).unwrap_or_default();
        if selected_model.trim().is_empty() {
            return Err(format!(
                "Select a {} model before connecting.",
                selected_variant.label()
            ));
        }

        let mut port_guard = state.server_port.lock().unwrap();
        *port_guard = selected_variant.external_sentinel_port();
        let mut loaded_variant_guard = state.loaded_gemma_variant.lock().unwrap();
        *loaded_variant_guard = Some(selected_variant);
        return Ok(());
    }

    let port;
    {
        let mut process_guard = state.server_process.lock().unwrap();
        let mut port_guard = state.server_port.lock().unwrap();
        if process_guard.is_some() {
            if let Some(loaded_variant) = loaded_gemma_variant(state.inner()) {
                if loaded_variant != selected_variant {
                    return Err(format!(
                        "LLM {} is already loaded. Unload it before switching to {}.",
                        loaded_variant.label(),
                        selected_variant.label()
                    ));
                }
            }
            return Ok(());
        }

        port = reserve_free_port()?;
        info!("Starting MLX Server on port {}...", port);
        let port_arg = port.to_string();
        let sidecar_command = app_handle
            .shell()
            .sidecar("mlx-handler")
            .map_err(|e| e.to_string())?
            .env("OPEN_DUCK_RUNTIME_ROOT", resolve_runtime_root(&app_handle)?)
            .env(
                "OPEN_DUCK_PATCH_SERVER_SCRIPT",
                resolve_resource_file(&app_handle, "patch_mlx_vlm.py")?,
            )
            .args(&[
                "--server",
                "--model",
                selected_variant.repo_id().ok_or_else(|| {
                    format!(
                        "{} does not use a local MLX server.",
                        selected_variant.label()
                    )
                })?,
                "--port",
                &port_arg,
            ]);

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
        *state.loaded_gemma_variant.lock().unwrap() = Some(selected_variant);
    }

    if !wait_for_gemma_server_on_port(port, Duration::from_secs(GEMMA_STARTUP_TIMEOUT_SECS)).await {
        let _ = stop_server_inner(state.inner());
        return Err("Timed out while loading Gemma.".to_string());
    }

    refresh_tray_presentation(&app_handle);
    schedule_delayed_tray_refresh(app_handle.clone(), Duration::from_secs(10));

    Ok(())
}

#[tauri::command]
async fn start_csm_server(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    start_csm_server_inner(&app_handle, state.inner()).await?;
    refresh_tray_presentation(&app_handle);
    Ok(())
}

#[tauri::command]
async fn start_stt_server(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    start_stt_server_inner(&app_handle, state.inner()).await?;
    refresh_tray_presentation(&app_handle);
    Ok(())
}

#[tauri::command]
fn get_csm_quantize(state: State<'_, AppState>) -> bool {
    *state.selected_csm_quantized.lock().unwrap()
}

#[tauri::command]
fn get_csm_model_variant(state: State<'_, AppState>) -> String {
    selected_csm_model(state.inner()).key().to_string()
}

#[tauri::command]
fn set_csm_model_variant(state: State<'_, AppState>, variant: String) -> Result<(), String> {
    let selected_variant = CsmModelVariant::from_key(&variant)?;

    if active_download_process(state.inner(), DownloadModel::Csm).is_some() {
        return Err("A speech model download is already in progress.".to_string());
    }

    if let Some(loaded_variant) = loaded_csm_model(state.inner()) {
        if loaded_variant != selected_variant {
            return Err(format!(
                "{} is already loaded. Unload it before switching to {}.",
                loaded_variant.label(),
                selected_variant.label()
            ));
        }
    }

    let mut variant_guard = state.selected_csm_model.lock().unwrap();
    *variant_guard = selected_variant;
    Ok(())
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
    let context_audio = resolve_csm_context_audio_file(&app_handle, state.inner(), selected_voice)?;

    let context_text = if selected_voice == CsmVoice::Custom {
        state.csm_reference_text.lock().unwrap().clone()
    } else {
        None
    };

    {
        let mut selected_voice_guard = state.selected_csm_voice.lock().unwrap();
        *selected_voice_guard = selected_voice;
    }

    apply_csm_voice_context(state.inner(), &context_audio, context_text.as_deref()).await
}

#[tauri::command]
async fn set_csm_reference_voice(
    _app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    ref_audio_data_url: Option<String>,
    ref_text: Option<String>,
) -> Result<(), String> {
    let audio_path = if let Some(data_url) = ref_audio_data_url {
        write_data_url_to_temp_file(&data_url)
            .ok_or_else(|| "Failed to save custom reference audio".to_string())?
    } else {
        return Err("Reference audio is required for custom voice".to_string());
    };

    {
        let mut audio_path_guard = state.csm_reference_audio_path.lock().unwrap();
        *audio_path_guard = Some(audio_path.clone());
    }

    {
        let mut text_guard = state.csm_reference_text.lock().unwrap();
        *text_guard = ref_text.clone();
    }

    {
        let mut selected_voice_guard = state.selected_csm_voice.lock().unwrap();
        *selected_voice_guard = CsmVoice::Custom;
    }

    apply_csm_voice_context(state.inner(), &audio_path, ref_text.as_deref()).await
}

async fn start_csm_server_inner(
    app_handle: &tauri::AppHandle,
    state: &AppState,
) -> Result<(), String> {
    ensure_runtime_dependencies_inner(app_handle, state).await?;

    let selected_variant = selected_csm_model(state);
    if csm_process_is_ready(state).await {
        if loaded_csm_model(state) == Some(selected_variant) {
            return Ok(());
        }

        if let Some(loaded_variant) = loaded_csm_model(state) {
            return Err(format!(
                "{} is already loaded. Unload it before switching to {}.",
                loaded_variant.label(),
                selected_variant.label()
            ));
        }
    }

    stop_csm_server_inner(state).await?;
    reset_csm_startup_state(state);
    update_csm_startup_message(
        app_handle,
        Some(format!("Starting {} worker...", selected_variant.label())),
        true,
    );

    let python_executable = resolve_gemma_python_executable(app_handle)?;
    let python_home = python_executable
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| "Failed to resolve Gemma Python home".to_string())?;
    let speech_site_packages = resolve_speech_site_packages(app_handle, selected_variant)?;
    let csm_script = resolve_resource_file(app_handle, "csm_stream.py")?;
    let selected_voice = *state.selected_csm_voice.lock().unwrap();

    let (startup_context_audio, startup_context_text) = if selected_variant.uses_reference_audio() {
        let audio = resolve_csm_context_audio_file(app_handle, state, selected_voice).ok();
        let text = if selected_voice == CsmVoice::Custom {
            state.csm_reference_text.lock().unwrap().clone()
        } else {
            None
        };
        (audio, text)
    } else {
        (None, None)
    };

    info!("Starting CSM worker with {}", python_executable.display());

    let mut command = Command::new(&python_executable);
    command
        .arg(&csm_script)
        .arg("--server")
        .arg("--model")
        .arg(selected_variant.worker_key())
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONPYCACHEPREFIX", "/tmp/openduck-pycache")
        .env("NUMBA_CACHE_DIR", "/tmp/openduck-numba-cache")
        .env("PYTHONHOME", &python_home)
        .env("PYTHONPATH", &speech_site_packages)
        .env("HF_HUB_DISABLE_XET", "1")
        .env("PYTORCH_ENABLE_MPS_FALLBACK", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if let Some(context_audio) = &startup_context_audio {
        command.arg("--context-audio").arg(context_audio);
    }

    if let Some(context_text) = &startup_context_text {
        command.arg("--context-text").arg(context_text);
    }

    let should_quantize_csm = *state.selected_csm_quantized.lock().unwrap();
    if should_quantize_csm && selected_variant.supports_quantization() {
        info!("Starting CSM worker with quantization enabled");
        command.arg("--quantize");
    } else if should_quantize_csm {
        info!(
            "Skipping quantization because {} does not support it",
            selected_variant.label()
        );
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
    {
        let mut loaded_variant_guard = state.loaded_csm_model.lock().unwrap();
        *loaded_variant_guard = Some(selected_variant);
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

async fn start_stt_server_inner(
    app_handle: &tauri::AppHandle,
    state: &AppState,
) -> Result<(), String> {
    ensure_runtime_dependencies_inner(app_handle, state).await?;

    let selected_variant = selected_stt_model(state);
    if !selected_variant.uses_worker() {
        return Ok(());
    }

    if stt_process_is_ready(state).await {
        if loaded_stt_model(state) == Some(selected_variant) {
            return Ok(());
        }

        if let Some(loaded_variant) = loaded_stt_model(state) {
            return Err(format!(
                "{} is already loaded for STT. Unload it before switching to {}.",
                loaded_variant.label(),
                selected_variant.label()
            ));
        }
    }

    stop_stt_server_inner(state).await?;
    reset_stt_startup_state(state);
    update_stt_startup_message(
        app_handle,
        Some(format!("Starting {} worker...", selected_variant.label())),
        true,
    );

    let python_executable = resolve_gemma_python_executable(app_handle)?;
    let python_home = python_executable
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .ok_or_else(|| "Failed to resolve STT Python home".to_string())?;
    let stt_site_packages = resolve_stt_site_packages(app_handle)?;
    let stt_script = resolve_resource_file(app_handle, "stt_stream.py")?;
    let model_repo_id = selected_variant.repo_id().ok_or_else(|| {
        format!(
            "{} does not use a dedicated STT worker.",
            selected_variant.label()
        )
    })?;

    info!(
        "Starting STT worker for {} with {}",
        model_repo_id,
        python_executable.display()
    );

    let mut command = Command::new(&python_executable);
    command
        .arg(&stt_script)
        .arg("--server")
        .arg("--model")
        .arg(model_repo_id)
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONPYCACHEPREFIX", "/tmp/openduck-pycache")
        .env("NUMBA_CACHE_DIR", "/tmp/openduck-numba-cache")
        .env("PYTHONHOME", &python_home)
        .env("PYTHONPATH", &stt_site_packages)
        .env("HF_HUB_DISABLE_XET", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start STT worker: {e}"))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Failed to open stdin for the STT worker".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to open stdout for the STT worker".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Failed to open stderr for the STT worker".to_string())?;

    let child = Arc::new(AsyncMutex::new(child));
    let stdin = Arc::new(AsyncMutex::new(stdin));
    let pending_requests = Arc::new(AsyncMutex::new(HashMap::new()));

    {
        let mut stt_process_guard = state.stt_process.lock().unwrap();
        *stt_process_guard = Some(SttProcess {
            child: child.clone(),
            stdin: stdin.clone(),
            pending_requests: pending_requests.clone(),
        });
    }
    {
        let mut stt_ready_guard = state.stt_ready.lock().unwrap();
        *stt_ready_guard = false;
    }
    {
        let mut loaded_variant_guard = state.loaded_stt_model.lock().unwrap();
        *loaded_variant_guard = Some(selected_variant);
    }

    let (ready_tx, ready_rx) = oneshot::channel();
    let ready_tx = Arc::new(Mutex::new(Some(ready_tx)));

    tauri::async_runtime::spawn(stt_stdout_task(
        app_handle.clone(),
        stdout,
        pending_requests.clone(),
        ready_tx.clone(),
    ));
    tauri::async_runtime::spawn(stt_stderr_task(app_handle.clone(), stderr));
    tauri::async_runtime::spawn(stt_exit_monitor(
        app_handle.clone(),
        child.clone(),
        ready_tx.clone(),
    ));

    match tokio::time::timeout(
        std::time::Duration::from_secs(STT_STARTUP_TIMEOUT_SECS),
        ready_rx,
    )
    .await
    {
        Ok(Ok(Ok(()))) => {
            update_stt_startup_message(app_handle, None, false);
            Ok(())
        }
        Ok(Ok(Err(message))) => {
            let _ = stop_stt_server_inner(state).await;
            Err(message)
        }
        Ok(Err(_)) => {
            let message =
                stt_startup_failure_message(state, "STT worker closed before it became ready");
            let _ = stop_stt_server_inner(state).await;
            Err(message)
        }
        Err(_) => {
            let message =
                stt_startup_failure_message(state, "Timed out while loading the STT worker");
            let _ = stop_stt_server_inner(state).await;
            Err(message)
        }
    }
}

#[tauri::command]
async fn stop_server(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    stop_server_inner(state.inner())?;
    refresh_tray_presentation(&app_handle);
    Ok(())
}

fn stop_server_inner(state: &AppState) -> Result<(), String> {
    let mut process_guard = state.server_process.lock().unwrap();
    let mut port_guard = state.server_port.lock().unwrap();
    if let Some(child) = process_guard.take() {
        info!("Stopping MLX Server...");
        child.kill().map_err(|e| e.to_string())?;
    }
    *port_guard = None;
    *state.loaded_gemma_variant.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
async fn stop_csm_server(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    stop_csm_server_inner(state.inner()).await?;
    refresh_tray_presentation(&app_handle);
    Ok(())
}

#[tauri::command]
async fn stop_stt_server(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    stop_stt_server_inner(state.inner()).await?;
    refresh_tray_presentation(&app_handle);
    Ok(())
}

#[tauri::command]
fn start_call_timer(app_handle: AppHandle, state: State<'_, AppState>, muted: bool) {
    {
        let mut call_in_progress_guard = state.call_in_progress.lock().unwrap();
        *call_in_progress_guard = true;
    }
    {
        let mut call_muted_guard = state.call_muted.lock().unwrap();
        *call_muted_guard = muted;
    }
    refresh_tray_presentation(&app_handle);
    emit_call_stage(&app_handle, "listening", "Listening");
    start_call_timer_inner(&app_handle, state.inner());
}

#[tauri::command]
fn stop_call_timer(app_handle: AppHandle, state: State<'_, AppState>) {
    {
        let mut call_in_progress_guard = state.call_in_progress.lock().unwrap();
        *call_in_progress_guard = false;
    }
    {
        let mut call_muted_guard = state.call_muted.lock().unwrap();
        *call_muted_guard = false;
    }
    cancel_auto_continue_timer(state.inner());
    refresh_tray_presentation(&app_handle);
    stop_call_timer_inner(&app_handle, state.inner());
    emit_call_stage(&app_handle, "idle", "");
}

#[tauri::command]
fn set_call_muted(app_handle: AppHandle, state: State<'_, AppState>, muted: bool) {
    let call_in_progress = *state.call_in_progress.lock().unwrap();
    {
        let mut call_muted_guard = state.call_muted.lock().unwrap();
        *call_muted_guard = muted;
    }
    refresh_tray_presentation(&app_handle);
    if call_in_progress {
        emit_overlay_notification(
            &app_handle,
            OverlayNotificationEvent {
                message: format!(
                    "OpenDuck: {}",
                    if muted { "Muted" } else { "Unmuted" }
                ),
            },
        );
    }
}

struct PreparedAudioChunk {
    samples: Vec<f32>,
    rms: f32,
}

fn calculate_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
}

fn suppress_playback_echo(mut payload: AudioPayload) -> PreparedAudioChunk {
    let mic_rms = calculate_rms(&payload.data);
    if !payload.playback_active {
        return PreparedAudioChunk {
            samples: payload.data,
            rms: mic_rms,
        };
    }

    let Some(playback_reference) = payload.playback_reference.as_deref() else {
        return PreparedAudioChunk {
            samples: payload.data,
            rms: mic_rms,
        };
    };

    if playback_reference.len() != payload.data.len() {
        return PreparedAudioChunk {
            samples: payload.data,
            rms: mic_rms,
        };
    }

    let reference_rms = calculate_rms(playback_reference);
    if reference_rms < PLAYBACK_REFERENCE_MIN_RMS {
        return PreparedAudioChunk {
            samples: payload.data,
            rms: mic_rms,
        };
    }

    let reference_energy = playback_reference
        .iter()
        .map(|sample| sample * sample)
        .sum::<f32>();
    if reference_energy <= f32::EPSILON {
        return PreparedAudioChunk {
            samples: payload.data,
            rms: mic_rms,
        };
    }

    let echo_gain = (payload
        .data
        .iter()
        .zip(playback_reference.iter())
        .map(|(sample, reference)| sample * reference)
        .sum::<f32>()
        / reference_energy)
        .clamp(0.0, PLAYBACK_ECHO_MAX_GAIN);

    if echo_gain <= 0.0 {
        return PreparedAudioChunk {
            samples: payload.data,
            rms: mic_rms,
        };
    }

    let mut residual_energy = 0.0;
    for (sample, reference) in payload.data.iter_mut().zip(playback_reference.iter()) {
        *sample = (*sample - echo_gain * reference).clamp(-1.0, 1.0);
        residual_energy += *sample * *sample;
    }

    PreparedAudioChunk {
        rms: (residual_energy / payload.data.len() as f32).sqrt(),
        samples: payload.data,
    }
}

const LIVE_STT_TICK_MS: u64 = 200;
const LIVE_STT_SPEECH_INTERVAL_MS: u64 = 1500;
const LIVE_STT_SILENCE_INTERVAL_MS: u64 = 450;
const LIVE_STT_MIN_BUFFER_MS: u32 = 450;
const LIVE_STT_MIN_VOICED_MS: u32 = 250;
const LIVE_STT_MIN_VOICED_MS_IN_SILENCE: u32 = 80;
const LIVE_STT_ENDPOINT_VOICED_GRACE_MS: u32 = 120;

fn noop_waker() -> std::task::Waker {
    unsafe fn clone(_: *const ()) -> std::task::RawWaker {
        std::task::RawWaker::new(std::ptr::null(), &VTABLE)
    }
    unsafe fn wake(_: *const ()) {}
    unsafe fn wake_by_ref(_: *const ()) {}
    unsafe fn drop(_: *const ()) {}

    static VTABLE: std::task::RawWakerVTable =
        std::task::RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    unsafe { std::task::Waker::from_raw(std::task::RawWaker::new(std::ptr::null(), &VTABLE)) }
}

fn samples_duration_ms(sample_count: usize, sample_rate: u32) -> u32 {
    if sample_count == 0 || sample_rate == 0 {
        return 0;
    }

    ((sample_count as u64) * 1000 / u64::from(sample_rate)) as u32
}

fn begin_live_transcription_utterance(state: &AppState) -> u64 {
    let utterance_id = state.next_utterance_id.fetch_add(1, Ordering::Relaxed);
    let mut live_guard = state.live_transcription.lock().unwrap();
    *live_guard = LiveTranscriptionState {
        utterance_id: Some(utterance_id),
        last_emitted_text: String::new(),
        cached_text: None,
        cached_voiced_samples: 0,
        next_attempt_id: 1,
        in_flight_attempt_id: None,
        in_flight_voiced_samples: 0,
        in_flight_started_in_silence: false,
        in_flight_handle: None,
    };
    utterance_id
}

fn take_live_transcription_handoff_for_endpoint(
    state: &AppState,
    utterance_voiced_samples: usize,
    capture_sample_rate: u32,
) -> (
    Option<String>,
    Option<tauri::async_runtime::JoinHandle<Result<String, String>>>,
) {
    let mut live_guard = state.live_transcription.lock().unwrap();

    let transcript = live_guard
        .cached_text
        .clone()
        .filter(|_| live_guard.cached_voiced_samples >= utterance_voiced_samples);

    let mut handle = None;
    if let Some(in_flight) = live_guard.in_flight_handle.take() {
        let coverage_delta_samples =
            utterance_voiced_samples.saturating_sub(live_guard.in_flight_voiced_samples);
        let coverage_delta_ms = samples_duration_ms(coverage_delta_samples, capture_sample_rate);
        let should_wait = transcript.is_none()
            && (live_guard.in_flight_started_in_silence
                || live_guard.in_flight_voiced_samples >= utterance_voiced_samples
                || coverage_delta_ms <= LIVE_STT_ENDPOINT_VOICED_GRACE_MS);

        if should_wait {
            handle = Some(in_flight);
        } else {
            in_flight.abort();
        }
    }

    *live_guard = LiveTranscriptionState::default();
    (transcript, handle)
}

fn start_live_transcription_loop(
    app_handle: tauri::AppHandle,
    utterance_id: u64,
    capture_sample_rate: u32,
) {
    tauri::async_runtime::spawn(async move {
        let mut last_attempted_at =
            Instant::now() - Duration::from_millis(LIVE_STT_SPEECH_INTERVAL_MS);

        loop {
            tokio::time::sleep(Duration::from_millis(LIVE_STT_TICK_MS)).await;

            // Step 1: If the utterance ended, stop looping.
            {
                let state = app_handle.state::<AppState>();
                let live_guard = state.live_transcription.lock().unwrap();
                if live_guard.utterance_id != Some(utterance_id) {
                    return;
                }
            }

            // Step 2: If we have an in-flight transcription that completed, commit it.
            let finished_attempt = {
                let state = app_handle.state::<AppState>();
                let mut live_guard = state.live_transcription.lock().unwrap();
                if live_guard.utterance_id != Some(utterance_id) {
                    return;
                }

                if live_guard.in_flight_handle.is_none() {
                    None
                } else {
                    let handle = live_guard
                        .in_flight_handle
                        .as_mut()
                        .expect("handle should exist when checked above");
                    let waker = noop_waker();
                    let mut cx = std::task::Context::from_waker(&waker);
                    match std::pin::Pin::new(handle).poll(&mut cx) {
                        std::task::Poll::Ready(result) => {
                            live_guard.in_flight_handle.take();
                            let attempt_id = live_guard.in_flight_attempt_id.take();
                            let voiced_samples = live_guard.in_flight_voiced_samples;
                            live_guard.in_flight_voiced_samples = 0;
                            live_guard.in_flight_started_in_silence = false;
                            Some((result, attempt_id, voiced_samples))
                        }
                        std::task::Poll::Pending => None,
                    }
                }
            };

            if let Some((result, attempt_id, attempt_voiced_samples)) = finished_attempt {
                match result {
                    Ok(Ok(transcript)) => {
                        if !transcript.is_empty() && is_meaningful_transcript(&transcript) {
                            let should_emit = {
                                let state = app_handle.state::<AppState>();
                                let mut live_guard = state.live_transcription.lock().unwrap();
                                if live_guard.utterance_id != Some(utterance_id) {
                                    return;
                                }

                                live_guard.cached_text = Some(transcript.clone());
                                live_guard.cached_voiced_samples = attempt_voiced_samples;
                                let changed = transcript != live_guard.last_emitted_text;
                                if changed {
                                    live_guard.last_emitted_text = transcript.clone();
                                }
                                changed
                            };

                            if should_emit {
                                emit_transcript_partial_event(
                                    &app_handle,
                                    TranscriptPartialEvent { text: transcript },
                                );
                            }
                        }
                    }
                    Ok(Err(err)) => {
                        debug!(
                            "Live STT attempt {:?} failed: {}",
                            attempt_id.unwrap_or_default(),
                            err
                        );
                    }
                    Err(err) => {
                        debug!(
                            "Live STT attempt {:?} join failed: {}",
                            attempt_id.unwrap_or_default(),
                            err
                        );
                    }
                }
            }

            // Step 3: Check whether we should start a new transcription attempt.
            let (buffer_len_samples, voiced_samples, silent_count) = {
                let state = app_handle.state::<AppState>();
                let voiced_samples = *state.current_utterance_voiced_samples.lock().unwrap();
                let silent_count = *state.silent_chunks_count.lock().unwrap();
                let buffer_len_samples = state.audio_buffer.lock().unwrap().len();
                (buffer_len_samples, voiced_samples, silent_count)
            };

            let buffer_len_ms = samples_duration_ms(buffer_len_samples, capture_sample_rate);
            if buffer_len_ms < LIVE_STT_MIN_BUFFER_MS {
                continue;
            }

            let voiced_ms = samples_duration_ms(voiced_samples, capture_sample_rate);
            let min_voiced_ms = if silent_count > 0 {
                LIVE_STT_MIN_VOICED_MS_IN_SILENCE
            } else {
                LIVE_STT_MIN_VOICED_MS
            };
            if voiced_ms < min_voiced_ms {
                continue;
            }

            let should_start_attempt = {
                let state = app_handle.state::<AppState>();
                let live_guard = state.live_transcription.lock().unwrap();
                if live_guard.utterance_id != Some(utterance_id) {
                    return;
                }

                live_guard.in_flight_handle.is_none()
                    && voiced_samples > live_guard.cached_voiced_samples
            };

            if !should_start_attempt {
                continue;
            }

            let min_interval_ms = if silent_count > 0 {
                LIVE_STT_SILENCE_INTERVAL_MS
            } else {
                LIVE_STT_SPEECH_INTERVAL_MS
            };
            let now = Instant::now();
            if now.duration_since(last_attempted_at) < Duration::from_millis(min_interval_ms) {
                continue;
            }
            last_attempted_at = now;

            let audio_samples = {
                let state = app_handle.state::<AppState>();
                let samples = state.audio_buffer.lock().unwrap().clone();
                samples
            };

            let audio_wav_base64 =
                match encode_audio_samples_as_wav_base64(&audio_samples, capture_sample_rate) {
                    Ok(audio_wav_base64) => audio_wav_base64,
                    Err(err) => {
                        warn!("Failed to encode live audio chunk for STT: {}", err);
                        continue;
                    }
                };

            let app_handle_for_task = app_handle.clone();
            let handle = tauri::async_runtime::spawn(async move {
                transcribe_audio_with_stt_worker(&app_handle_for_task, &audio_wav_base64).await
            });

            // Step 4: Store the in-flight attempt so endpointing can reuse it.
            let attempt_started_in_silence = silent_count > 0;
            {
                let state = app_handle.state::<AppState>();
                let mut live_guard = state.live_transcription.lock().unwrap();
                if live_guard.utterance_id != Some(utterance_id) {
                    handle.abort();
                    return;
                }

                if live_guard.in_flight_handle.is_some() {
                    handle.abort();
                    continue;
                }

                let attempt_id = live_guard.next_attempt_id;
                live_guard.next_attempt_id = attempt_id.saturating_add(1);
                live_guard.in_flight_attempt_id = Some(attempt_id);
                live_guard.in_flight_voiced_samples = voiced_samples;
                live_guard.in_flight_started_in_silence = attempt_started_in_silence;
                live_guard.in_flight_handle = Some(handle);
            }
        }
    });
}

#[tauri::command]
async fn receive_audio_chunk(
    payload: AudioPayload,
    state: State<'_, AppState>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    if payload.data.is_empty() {
        return Ok(());
    }

    let capture_sample_rate = resolve_capture_sample_rate(payload.sample_rate);
    let prepared_chunk = suppress_playback_echo(payload);
    if prepared_chunk.samples.is_empty() {
        return Ok(());
    }
    let configured_silence_ms = *state.end_of_utterance_silence_ms.lock().unwrap();
    let active_stt_model = selected_stt_model(state.inner());

    let mut is_really_speaking = false;
    if prepared_chunk.rms > SILENCE_THRESHOLD {
        is_really_speaking = true;

        if let Ok(_) = ensure_vad(&app_handle, state.inner()) {
            let mut vad_guard = state.vad.lock().unwrap();
            if let Some(vad) = vad_guard.as_mut() {
                let resampled = vad::resample_to_16k(&prepared_chunk.samples, capture_sample_rate);
                // Use VAD to filter out background noise
                if let Ok(prob) = vad.calc_level(&resampled) {
                    if prob < 0.001 {
                        is_really_speaking = false;
                    }
                }
            }
        }
    }

    let mut detected_speech = false;
    let mut live_transcription_utterance_id: Option<u64> = None;
    {
        let mut silent_count = state.silent_chunks_count.lock().unwrap();
        let mut speaking_count = state.speaking_chunks_count.lock().unwrap();
        let mut current_utterance_voiced_samples =
            state.current_utterance_voiced_samples.lock().unwrap();
        let mut is_speaking = state.is_speaking.lock().unwrap();

        if is_really_speaking {
            *speaking_count += 1;
            *silent_count = 0;
            *current_utterance_voiced_samples += prepared_chunk.samples.len();
        } else {
            *silent_count += 1;
            if *silent_count > 5 && !*is_speaking {
                *speaking_count = 0;
                *current_utterance_voiced_samples = 0;
            }
        }

        if !*is_speaking && *speaking_count >= MIN_SPEAKING_CHUNKS {
            *is_speaking = true;
            detected_speech = true;
            if active_stt_model.uses_worker() {
                live_transcription_utterance_id =
                    Some(begin_live_transcription_utterance(state.inner()));
            }
            info!("Speech detected, immediately interrupting active generation");
        }
    }

    if detected_speech {
        cancel_auto_continue_timer(state.inner());
        mark_latest_assistant_turn_auto_continue_consumed(state.inner());
        interrupt_active_generation(&app_handle).await;
        let mut buffer = state.audio_buffer.lock().unwrap();
        let mut pre_buffer = state.pre_audio_buffer.lock().unwrap();
        buffer.extend(pre_buffer.iter().copied());
        pre_buffer.clear();
    }

    if let Some(utterance_id) = live_transcription_utterance_id {
        start_live_transcription_loop(app_handle.clone(), utterance_id, capture_sample_rate);
    }

    let mut endpoint_audio: Option<Vec<f32>> = None;
    let mut endpoint_voiced_samples: usize = 0;
    {
        let mut buffer = state.audio_buffer.lock().unwrap();
        let mut silent_count = state.silent_chunks_count.lock().unwrap();
        let mut speaking_count = state.speaking_chunks_count.lock().unwrap();
        let mut current_utterance_voiced_samples =
            state.current_utterance_voiced_samples.lock().unwrap();
        let mut is_speaking = state.is_speaking.lock().unwrap();

        if *is_speaking {
            buffer.extend_from_slice(&prepared_chunk.samples);

            let silence_chunks_required = required_silence_chunks(
                capture_sample_rate,
                prepared_chunk.samples.len(),
                configured_silence_ms,
            );

            if *silent_count >= silence_chunks_required {
                endpoint_voiced_samples = *current_utterance_voiced_samples;
                endpoint_audio = Some(std::mem::take(&mut *buffer));
                {
                    let mut pre_buffer = state.pre_audio_buffer.lock().unwrap();
                    pre_buffer.clear();
                }
                *is_speaking = false;
                *silent_count = 0;
                *speaking_count = 0;
                *current_utterance_voiced_samples = 0;

                let mut vad_guard = state.vad.lock().unwrap();
                if let Some(vad) = vad_guard.as_mut() {
                    vad.reset();
                }
            }
        } else {
            let mut pre_buffer = state.pre_audio_buffer.lock().unwrap();
            pre_buffer.extend(prepared_chunk.samples.iter().copied());
            let max_pre_buffer_samples =
                (capture_sample_rate as u64 * PRE_SPEECH_BUFFER_MS as u64 / 1000) as usize;
            if pre_buffer.len() > max_pre_buffer_samples {
                let to_remove = pre_buffer.len() - max_pre_buffer_samples;
                pre_buffer.drain(..to_remove);
            }
        }
    }

    if let Some(endpoint_audio) = endpoint_audio {
        info!(
            "Silence detected; endpointing with configured {} ms silence before transcription.",
            configured_silence_ms
        );
        emit_call_stage(&app_handle, "processing_audio", "Processing Audio");
        if *state.tray_pong_playback_enabled.lock().unwrap() {
            emit_play_tray_pong(&app_handle);
        }

        let (cached_transcript, in_flight_transcript) = if active_stt_model.uses_worker() {
            take_live_transcription_handoff_for_endpoint(
                state.inner(),
                endpoint_voiced_samples,
                capture_sample_rate,
            )
        } else {
            (None, None)
        };

        process_audio_turn(
            endpoint_audio,
            capture_sample_rate,
            app_handle,
            cached_transcript,
            in_flight_transcript,
        );
    }
    Ok(())
}

fn required_silence_chunks(
    sample_rate: u32,
    chunk_sample_count: usize,
    silence_duration_ms: u32,
) -> usize {
    let samples_per_chunk = chunk_sample_count.max(1) as u64;
    let required_chunks = (u64::from(sample_rate) * u64::from(silence_duration_ms))
        .div_ceil(samples_per_chunk * 1000);

    required_chunks.max(1) as usize
}

fn process_audio_turn(
    samples: Vec<f32>,
    capture_sample_rate: u32,
    app_handle: tauri::AppHandle,
    cached_transcript: Option<String>,
    in_flight_transcript: Option<tauri::async_runtime::JoinHandle<Result<String, String>>>,
) {
    let generation_id;
    let conversation_session_id;
    let active_gemma_variant: GemmaVariant;
    let gemma_model;
    let active_stt_model: SttModelVariant;
    let server_port;
    {
        let state = app_handle.state::<AppState>();
        generation_id = state.next_generation_id.fetch_add(1, Ordering::Relaxed);
        conversation_session_id = current_conversation_session_id(state.inner());
        let variant = loaded_gemma_variant(state.inner())
            .unwrap_or_else(|| selected_gemma_variant(state.inner()));
        active_gemma_variant = variant;
        gemma_model = if variant.is_external() {
            selected_external_llm_model(state.inner(), variant).unwrap_or_default()
        } else {
            variant.repo_id().unwrap_or_default().to_string()
        };
        active_stt_model = selected_stt_model(state.inner());
        server_port = *state.server_port.lock().unwrap();
    }

    let Some(server_port) = server_port else {
        error!("Gemma server is not running, skipping audio request");
        emit_audio_turn_processing_error(
            &app_handle,
            "Gemma is not loaded. Load Gemma before starting a call.".to_string(),
        );
        return;
    };

    let cached_transcript = cached_transcript
        .map(|text| sanitize_for_voice_output(&text))
        .filter(|text| !text.is_empty());

    let app_handle_for_task = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        info!(
            "Dispatching audio turn for {} transcription",
            active_stt_model.label()
        );

        let mut audio_wav_base64: Option<String> = None;
        let transcription_result = if active_stt_model.uses_worker() {
            if let Some(transcript) = cached_transcript.clone() {
                Ok(transcript)
            } else if let Some(handle) = in_flight_transcript {
                match handle.await {
                    Ok(Ok(transcript)) => Ok(transcript),
                    Ok(Err(err)) => {
                        debug!("In-flight STT failed, falling back: {}", err);
                        let audio_wav_base64 = match audio_wav_base64.as_deref() {
                            Some(audio_wav_base64) => audio_wav_base64,
                            None => {
                                let encoded = match encode_audio_samples_as_wav_base64(
                                    &samples,
                                    capture_sample_rate,
                                ) {
                                    Ok(encoded) => encoded,
                                    Err(err) => {
                                        error!("{}", err);
                                        emit_audio_turn_processing_error(&app_handle_for_task, err);
                                        return;
                                    }
                                };
                                audio_wav_base64 = Some(encoded);
                                audio_wav_base64.as_deref().unwrap()
                            }
                        };

                        transcribe_audio_with_stt_worker(&app_handle_for_task, audio_wav_base64)
                            .await
                    }
                    Err(err) => {
                        debug!("In-flight STT join failed, falling back: {}", err);
                        let audio_wav_base64 = match audio_wav_base64.as_deref() {
                            Some(audio_wav_base64) => audio_wav_base64,
                            None => {
                                let encoded = match encode_audio_samples_as_wav_base64(
                                    &samples,
                                    capture_sample_rate,
                                ) {
                                    Ok(encoded) => encoded,
                                    Err(err) => {
                                        error!("{}", err);
                                        emit_audio_turn_processing_error(&app_handle_for_task, err);
                                        return;
                                    }
                                };
                                audio_wav_base64 = Some(encoded);
                                audio_wav_base64.as_deref().unwrap()
                            }
                        };

                        transcribe_audio_with_stt_worker(&app_handle_for_task, audio_wav_base64)
                            .await
                    }
                }
            } else {
                let audio_wav_base64 = match audio_wav_base64.as_deref() {
                    Some(audio_wav_base64) => audio_wav_base64,
                    None => {
                        let encoded =
                            match encode_audio_samples_as_wav_base64(&samples, capture_sample_rate)
                            {
                                Ok(encoded) => encoded,
                                Err(err) => {
                                    error!("{}", err);
                                    emit_audio_turn_processing_error(&app_handle_for_task, err);
                                    return;
                                }
                            };
                        audio_wav_base64 = Some(encoded);
                        audio_wav_base64.as_deref().unwrap()
                    }
                };

                transcribe_audio_with_stt_worker(&app_handle_for_task, audio_wav_base64).await
            }
        } else {
            let audio_wav_base64 = match audio_wav_base64.as_deref() {
                Some(audio_wav_base64) => audio_wav_base64,
                None => {
                    let encoded =
                        match encode_audio_samples_as_wav_base64(&samples, capture_sample_rate) {
                            Ok(encoded) => encoded,
                            Err(err) => {
                                error!("{}", err);
                                emit_audio_turn_processing_error(&app_handle_for_task, err);
                                return;
                            }
                        };
                    audio_wav_base64 = Some(encoded);
                    audio_wav_base64.as_deref().unwrap()
                }
            };

            transcribe_audio_with_gemma(
                active_gemma_variant,
                server_port,
                &gemma_model,
                audio_wav_base64,
            )
            .await
        };

        drop(samples);

        match transcription_result {
            Ok(user_text) => {
                if user_text.is_empty() {
                    warn!(
                        "{} transcription was empty, skipping response generation",
                        active_stt_model.label()
                    );
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

                info!("{} transcription: {}", active_stt_model.label(), user_text);
                interrupt_active_generation(&app_handle_for_task).await;

                if current_conversation_session_id(app_handle_for_task.state::<AppState>().inner())
                    != conversation_session_id
                {
                    return;
                }

                log_processing_audio_latency_for_audio(&app_handle_for_task);
                emit_call_stage(&app_handle_for_task, "thinking", "Thinking");
                let latest_screen_capture_paths =
                    take_pending_screen_captures(app_handle_for_task.state::<AppState>().inner());
                if !latest_screen_capture_paths.is_empty() {
                    set_conversation_log_has_visible_images_state(
                        &app_handle_for_task,
                        app_handle_for_task.state::<AppState>().inner(),
                        true,
                    );
                    emit_screen_capture_event(
                        &app_handle_for_task,
                        "consumed",
                        "Screens attached to this turn.",
                    );
                    refresh_tray_presentation(&app_handle_for_task);
                }
                emit_transcript_event(
                    &app_handle_for_task,
                    TranscriptEvent {
                        text: user_text.clone(),
                        image_paths: latest_screen_capture_paths
                            .iter()
                            .map(|path| path.to_string_lossy().into_owned())
                            .collect(),
                        image_data_urls: latest_screen_capture_paths
                            .iter()
                            .filter_map(|path| load_image_data_url(path))
                            .collect(),
                    },
                );
                let latest_audio_wav_base64 = if active_stt_model == SttModelVariant::Gemma {
                    audio_wav_base64
                } else {
                    None
                };
                start_response_generation(
                    &app_handle_for_task,
                    server_port,
                    generation_id,
                    conversation_session_id,
                    gemma_model,
                    user_text,
                    latest_audio_wav_base64,
                    latest_screen_capture_paths,
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
                error!(
                    "Failed to transcribe audio with {}: {}",
                    active_stt_model.label(),
                    err
                );
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
    gemma_model: String,
    user_text: String,
    latest_audio_wav_base64: Option<String>,
    latest_image_paths: Vec<PathBuf>,
) {
    cancel_auto_continue_timer(app_handle.state::<AppState>().inner());

    let app_handle_for_task = app_handle.clone();
    let cancellation_token = Arc::new(AtomicBool::new(false));
    let cancellation_token_for_task = cancellation_token.clone();
    let handle = tauri::async_runtime::spawn(async move {
        let latest_image_files: Vec<TempImageFile> = latest_image_paths
            .into_iter()
            .map(TempImageFile::new)
            .collect();
        match stream_gemma_response_to_csm(
            &app_handle_for_task,
            server_port,
            &gemma_model,
            ResponseGenerationMode::LatestUserTurn {
                user_text: user_text.clone(),
                latest_audio_wav_base64: latest_audio_wav_base64.clone(),
                latest_image_paths: latest_image_files
                    .iter()
                    .map(|file| file.path().to_path_buf())
                    .collect(),
            },
            cancellation_token_for_task,
        )
        .await
        {
            Ok((response_id, response_text)) => {
                if response_text.is_empty() {
                    emit_call_stage(&app_handle_for_task, "listening", "Listening");
                    return;
                }

                if current_conversation_session_id(app_handle_for_task.state::<AppState>().inner())
                    != conversation_session_id
                {
                    return;
                }

                let persisted_image_paths: Vec<PathBuf> = latest_image_files
                    .into_iter()
                    .map(TempImageFile::release)
                    .collect();
                let (user_entry_id, assistant_entry_id) = append_conversation_turn_with_save(
                    &app_handle_for_task,
                    app_handle_for_task.state::<AppState>().inner(),
                    user_text.clone(),
                    response_text.clone(),
                    persisted_image_paths,
                );

                let session_title = {
                    let state = app_handle_for_task.state::<AppState>();
                    let guard = state.current_session_title.lock().unwrap();
                    guard.clone()
                };

                emit_conversation_context_committed(
                    &app_handle_for_task,
                    ConversationContextCommittedEvent {
                        request_id: response_id,
                        user_entry_id,
                        assistant_entry_id,
                        user_text,
                        assistant_text: response_text,
                        session_title,
                    },
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
        cancellation_token,
    ) {
        warn!(
            "Skipping response generation {} because a newer generation is already active",
            generation_id
        );
    }
}

fn start_assistant_auto_continue_generation(
    app_handle: &tauri::AppHandle,
    assistant_entry_id: u64,
) {
    let generation_id;
    let conversation_session_id;
    let gemma_model;
    let server_port;
    let assistant_text_prefix;
    {
        let state = app_handle.state::<AppState>();
        let configured_max_count = *state.auto_continue_max_count.lock().unwrap();
        if !*state.call_in_progress.lock().unwrap() {
            return;
        }

        if *state.tts_playback_active.lock().unwrap() || *state.is_speaking.lock().unwrap() {
            return;
        }

        if state.active_generation.lock().unwrap().is_some() {
            return;
        }

        let Some(last_turn) = state.conversation_turns.lock().unwrap().back().cloned() else {
            return;
        };

        if last_turn.assistant_entry_id != assistant_entry_id
            || last_turn.assistant_text.trim().is_empty()
        {
            return;
        }

        if let Some(tracker) = *state.auto_continue_tracker.lock().unwrap() {
            if tracker.assistant_entry_id == assistant_entry_id {
                if tracker.blocked {
                    return;
                }

                if let Some(max_count) = configured_max_count {
                    if tracker.continuation_count >= max_count {
                        return;
                    }
                }
            }
        }

        generation_id = state.next_generation_id.fetch_add(1, Ordering::Relaxed);
        conversation_session_id = current_conversation_session_id(state.inner());
        let variant = loaded_gemma_variant(state.inner())
            .unwrap_or_else(|| selected_gemma_variant(state.inner()));
        gemma_model = if variant.is_external() {
            selected_external_llm_model(state.inner(), variant).unwrap_or_default()
        } else {
            variant.repo_id().unwrap_or_default().to_string()
        };
        server_port = *state.server_port.lock().unwrap();
        assistant_text_prefix = last_turn.assistant_text;
    }

    let Some(server_port) = server_port else {
        warn!("Gemma server is not running, skipping assistant auto-continue");
        return;
    };

    emit_call_stage(app_handle, "thinking", "Thinking");

    let app_handle_for_task = app_handle.clone();
    let cancellation_token = Arc::new(AtomicBool::new(false));
    let cancellation_token_for_task = cancellation_token.clone();
    let handle = tauri::async_runtime::spawn(async move {
        match stream_gemma_response_to_csm(
            &app_handle_for_task,
            server_port,
            &gemma_model,
            ResponseGenerationMode::AssistantAutoContinue {
                assistant_entry_id,
                assistant_text_prefix: assistant_text_prefix.clone(),
            },
            cancellation_token_for_task,
        )
        .await
        {
            Ok((_, response_text)) => {
                if response_text.is_empty() {
                    emit_call_stage(&app_handle_for_task, "listening", "Listening");
                    return;
                }

                if current_conversation_session_id(app_handle_for_task.state::<AppState>().inner())
                    != conversation_session_id
                {
                    return;
                }

                if let Err(err) = append_to_existing_assistant_turn_with_save(
                    &app_handle_for_task,
                    app_handle_for_task.state::<AppState>().inner(),
                    assistant_entry_id,
                    response_text,
                ) {
                    warn!("Failed to append assistant auto-continue text: {}", err);
                }
            }
            Err(err) => {
                emit_csm_error(
                    &app_handle_for_task,
                    CsmErrorEvent {
                        request_id: None,
                        message: err.clone(),
                    },
                );
                error!("Failed to auto-continue Gemma response via CSM: {}", err);
                emit_call_stage(&app_handle_for_task, "listening", "Listening");
            }
        }

        clear_active_generation_if_matches(&app_handle_for_task, generation_id);
    });

    if !register_active_generation_if_newer(
        app_handle.state::<AppState>().inner(),
        generation_id,
        handle,
        cancellation_token,
    ) {
        warn!(
            "Skipping assistant auto-continue {} because a newer generation is already active",
            generation_id
        );
    }
}

async fn transcribe_audio_with_gemma(
    variant: GemmaVariant,
    server_port: u16,
    gemma_model: &str,
    audio_wav_base64: &str,
) -> Result<String, String> {
    if variant.is_external() {
        return Err(format!(
            "{} does not support audio transcription. Please switch to Whisper Large V3 Turbo for STT.",
            variant.label()
        ));
    }
    let stt_started_at = Instant::now();
    let client = reqwest::Client::new();
    let request = ChatRequest {
        model: gemma_model.to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: vec![
                build_input_audio_content(audio_wav_base64),
                ChatContent::Text {
                    text: TRANSCRIPTION_PROMPT.to_string(),
                },
            ],
        }],
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

    info!(
        "STT response received in {:.1} ms ({} chars)",
        stt_started_at.elapsed().as_secs_f64() * 1000.0,
        sanitized_transcript.chars().count()
    );

    Ok(sanitized_transcript)
}

async fn transcribe_audio_with_stt_worker(
    app_handle: &tauri::AppHandle,
    audio_wav_base64: &str,
) -> Result<String, String> {
    let stt_started_at = Instant::now();
    let state = app_handle.state::<AppState>();
    let selected_variant = selected_stt_model(state.inner());

    if !selected_variant.uses_worker() {
        return Err(format!(
            "{} does not use a dedicated STT worker.",
            selected_variant.label()
        ));
    }

    if !stt_process_is_ready(state.inner()).await {
        info!(
            "STT worker was unavailable for {}. Attempting restart.",
            selected_variant.label()
        );
        start_stt_server_inner(app_handle, state.inner())
            .await
            .map_err(|err| {
                format!("The selected STT model stopped and could not be restarted: {err}")
            })?;
    }

    if !stt_process_is_ready(state.inner()).await {
        return Err("The selected STT model is not ready. Try loading it again.".to_string());
    }

    let process = {
        let stt_process_guard = state.stt_process.lock().unwrap();
        stt_process_guard
            .clone()
            .ok_or_else(|| "STT worker is unavailable".to_string())?
    };
    let request_id = state.next_stt_request_id.fetch_add(1, Ordering::Relaxed);
    let (response_tx, response_rx) = oneshot::channel();
    {
        let mut pending_requests = process.pending_requests.lock().await;
        pending_requests.insert(request_id, response_tx);
    }

    let request = serde_json::json!({
        "type": "transcribe",
        "request_id": request_id,
        "audio_wav_base64": audio_wav_base64,
    });

    let send_result = async {
        let mut stdin = process.stdin.lock().await;
        stdin
            .write_all(request.to_string().as_bytes())
            .await
            .map_err(|e| format!("Failed to send audio to the STT worker: {e}"))?;
        stdin
            .write_all(b"\n")
            .await
            .map_err(|e| format!("Failed to terminate the STT request: {e}"))?;
        stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush the STT request: {e}"))?;
        Ok::<(), String>(())
    }
    .await;

    if let Err(err) = send_result {
        let mut pending_requests = process.pending_requests.lock().await;
        pending_requests.remove(&request_id);
        return Err(err);
    }

    let transcript = response_rx
        .await
        .map_err(|_| "STT worker stopped before returning a transcription.".to_string())??;
    let sanitized_transcript = sanitize_for_voice_output(&transcript);

    if sanitized_transcript.is_empty() {
        warn!(
            "{} returned an empty transcription after sanitization. Raw transcription: {:?}",
            selected_variant.label(),
            transcript
        );
    }

    info!(
        "{} transcription received in {:.1} ms ({} chars)",
        selected_variant.label(),
        stt_started_at.elapsed().as_secs_f64() * 1000.0,
        sanitized_transcript.chars().count()
    );

    Ok(sanitized_transcript)
}

fn serialize_external_chat_request(request: &ChatRequest) -> serde_json::Value {
    let messages: Vec<serde_json::Value> = request
        .messages
        .iter()
        .map(|msg| {
            let content: Vec<serde_json::Value> = msg
                .content
                .iter()
                .filter_map(|c| match c {
                    ChatContent::Text { text } => Some(serde_json::json!({
                        "type": "text",
                        "text": text
                    })),
                    ChatContent::InputImage { image_url } => {
                        let data_url = if image_url.url.starts_with("data:") {
                            Some(image_url.url.clone())
                        } else {
                            load_image_data_url(Path::new(&image_url.url))
                        };

                        data_url.map(|url| {
                            serde_json::json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": url
                                }
                            })
                        })
                    }
                    ChatContent::InputAudio { .. } => None,
                })
                .collect();

            let content_value = if content
                .iter()
                .all(|c| c.get("type").and_then(|t| t.as_str()) == Some("text"))
            {
                let merged_text = content
                    .iter()
                    .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n");
                serde_json::Value::String(merged_text)
            } else {
                serde_json::Value::Array(content)
            };

            serde_json::json!({
                "role": msg.role,
                "content": content_value
            })
        })
        .collect();

    serde_json::json!({
        "model": request.model,
        "messages": messages,
        "stream": request.stream
    })
}

async fn stream_gemma_response_to_csm(
    app_handle: &tauri::AppHandle,
    server_port: u16,
    gemma_model: &str,
    mode: ResponseGenerationMode,
    cancellation_token: Arc<AtomicBool>,
) -> Result<(u64, String), String> {
    start_csm_server_inner(app_handle, app_handle.state::<AppState>().inner()).await?;

    let llm_started_at = Instant::now();
    let client = reqwest::Client::new();
    let (
        _conversation_session_id,
        conversation_turns,
        total_turn_count,
        llm_context_turn_limit,
        llm_context_text_chars,
        llm_context_trimmed_by_turn_limit,
        llm_image_history_limit,
    ) = {
        let state = app_handle.state::<AppState>();
        let session_id = current_conversation_session_id(state.inner());
        let turns = state
            .conversation_turns
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let llm_context_turn_limit = *state.llm_context_turn_limit.lock().unwrap();
        let selection = select_conversation_turns_for_llm_context(&turns, llm_context_turn_limit);
        let image_history_limit = *state.llm_image_history_limit.lock().unwrap();
        (
            session_id,
            selection.turns,
            turns.len(),
            llm_context_turn_limit,
            selection.total_text_chars,
            selection.trimmed_by_turn_limit,
            image_history_limit,
        )
    };
    let has_latest_audio = matches!(
        &mode,
        ResponseGenerationMode::LatestUserTurn {
            latest_audio_wav_base64: Some(_),
            ..
        }
    );
    let mut conversation_turn_image_urls = conversation_turns
        .iter()
        .map(resolve_user_turn_image_urls)
        .collect::<Vec<_>>();
    let mut latest_image_urls = match &mode {
        ResponseGenerationMode::LatestUserTurn {
            latest_image_paths, ..
        } => load_image_data_urls_from_paths(latest_image_paths.iter().map(PathBuf::as_path)),
        ResponseGenerationMode::AssistantAutoContinue { .. } => Vec::new(),
    };
    let total_image_count = conversation_turn_image_urls
        .iter()
        .map(Vec::len)
        .sum::<usize>()
        + latest_image_urls.len();
    apply_llm_image_history_limit(
        &mut conversation_turn_image_urls,
        &mut latest_image_urls,
        llm_image_history_limit,
    );
    let retained_image_count = conversation_turn_image_urls
        .iter()
        .map(Vec::len)
        .sum::<usize>()
        + latest_image_urls.len();
    if retained_image_count != total_image_count {
        info!(
            "Trimmed LLM image context from {} to {} images",
            total_image_count, retained_image_count
        );
    }
    let has_any_image_context = retained_image_count > 0;
    if conversation_turns.len() != total_turn_count {
        let effective_turn_limit = llm_context_turn_limit
            .map(|limit| limit.max(MIN_LLM_CONTEXT_TURN_LIMIT))
            .map(|limit| limit.to_string())
            .unwrap_or_else(|| "unlimited".to_string());
        let trim_reason = if llm_context_trimmed_by_turn_limit {
            format!("turn limit ({effective_turn_limit})")
        } else {
            "turn selection changed unexpectedly".to_string()
        };
        info!(
            "Trimmed LLM voice context from {} to {} turns ({} text chars kept, reason: {})",
            total_turn_count,
            conversation_turns.len(),
            llm_context_text_chars,
            trim_reason
        );
    }
    let mut messages = vec![ChatMessage {
        role: "system".to_string(),
        content: vec![ChatContent::Text {
            text: build_llm_system_prompt(
                &app_handle
                    .state::<AppState>()
                    .voice_system_prompt
                    .lock()
                    .unwrap()
                    .clone(),
                has_latest_audio,
                has_any_image_context,
            ),
        }],
    }];

    for (turn, image_urls) in conversation_turns
        .iter()
        .zip(conversation_turn_image_urls.iter())
    {
        messages.push(build_user_turn_message_with_image_urls(
            &turn.user_text,
            image_urls,
        ));
        messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: vec![ChatContent::Text {
                text: turn.assistant_text.clone(),
            }],
        });
    }

    let (assistant_text_prefix, append_to_assistant_entry_id) = match &mode {
        ResponseGenerationMode::LatestUserTurn {
            user_text,
            latest_audio_wav_base64,
            ..
        } => {
            messages.push(build_latest_user_turn_message_with_image_urls(
                user_text,
                latest_audio_wav_base64.as_deref(),
                &latest_image_urls,
            ));
            (None, None)
        }
        ResponseGenerationMode::AssistantAutoContinue {
            assistant_entry_id,
            assistant_text_prefix,
        } => {
            messages.push(ChatMessage {
                role: "user".to_string(),
                content: vec![ChatContent::Text {
                    text: AUTO_CONTINUE_PROMPT.to_string(),
                }],
            });
            (
                Some(assistant_text_prefix.as_str()),
                Some(*assistant_entry_id),
            )
        }
    };

    let mut request = ChatRequest {
        model: gemma_model.to_string(),
        messages,
        stream: true,
    };

    let state = app_handle.state::<AppState>();
    let loaded_variant = loaded_gemma_variant(state.inner())
        .unwrap_or_else(|| selected_gemma_variant(state.inner()));
    let is_external = loaded_variant.is_external();
    let provider_label = if is_external {
        loaded_variant.label()
    } else {
        "MLX"
    };
    let mut _temp_image_files = Vec::new();
    if !is_external {
        for msg in &mut request.messages {
            for content in &mut msg.content {
                if let ChatContent::InputImage { image_url } = content {
                    if image_url.url.starts_with("data:") {
                        if let Some(path) = write_data_url_to_temp_file(&image_url.url) {
                            image_url.url = path.to_string_lossy().into_owned();
                            _temp_image_files.push(TempImageFile::new(path));
                        }
                    }
                }
            }
        }
    }

    // Hide the logs for now
    // log_chat_request_debug(0, &request);

    let request_body = if is_external {
        serialize_external_chat_request(&request)
    } else {
        serde_json::to_value(&request).unwrap()
    };

    let base_url = if loaded_variant.is_external() {
        external_llm_base_url(state.inner(), loaded_variant)
            .ok_or_else(|| format!("Missing {} base URL.", loaded_variant.label()))?
    } else {
        server_base_url(server_port)
    };

    let mut request_builder = client
        .post(format!(
            "{}/v1/chat/completions",
            base_url.trim_end_matches('/')
        ))
        .json(&request_body);

    if let Some(api_key) = external_llm_api_key(state.inner(), loaded_variant).as_ref() {
        request_builder = request_builder.header("Authorization", format!("Bearer {}", api_key));
    }

    let mut response = request_builder
        .send()
        .await
        .map_err(|e| format!("Failed to call {provider_label} Server: {e}"))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("Failed to read error body: {e}"));
        return Err(format!(
            "{provider_label} Server returned error {status}: {body}"
        ));
    }

    let response_id = allocate_csm_response_id(app_handle);
    let mut raw_response_text = String::new();
    let mut raw_reasoning_text = String::new();
    let mut latest_response_text = String::new();
    let mut latest_reasoning_text = String::new();
    let mut queued_response_bytes = 0usize;
    let mut started_audio_response = false;
    let mut incomplete_stream_segment_started_at = None;
    let mut raw_body = Vec::new();
    let mut sse_buffer = Vec::new();
    let mut saw_stream_event = false;
    let mut saw_stream_done = false;

    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| format!("Failed while reading Gemma stream: {e}"))?
    {
        if cancellation_token.load(Ordering::Relaxed) {
            info!("Gemma stream cancelled by user request");
            if !raw_response_text.is_empty() {
                raw_response_text.push_str(" [Interrupted]");
            }
            saw_stream_done = true;
            break;
        }
        raw_body.extend_from_slice(&chunk);
        sse_buffer.extend(chunk.iter().copied().filter(|byte| *byte != b'\r'));

        while let Some(event_block) = drain_next_sse_event(&mut sse_buffer) {
            let event_text = String::from_utf8(event_block)
                .map_err(|err| format!("Failed to decode Gemma stream event: {err}"))?;
            match parse_gemma_stream_event(&event_text)? {
                ParsedGemmaStreamEvent::Ignore => {}
                ParsedGemmaStreamEvent::Done => {
                    saw_stream_event = true;
                    saw_stream_done = true;
                    break;
                }
                ParsedGemmaStreamEvent::Delta(text, reasoning) => {
                    saw_stream_event = true;
                    raw_response_text.push_str(&text);
                    raw_reasoning_text.push_str(&reasoning);
                    emit_streamed_response_update(
                        app_handle,
                        response_id,
                        &raw_response_text,
                        &raw_reasoning_text,
                        assistant_text_prefix,
                        append_to_assistant_entry_id,
                        &mut latest_response_text,
                        &mut latest_reasoning_text,
                        &mut queued_response_bytes,
                        &mut started_audio_response,
                        &mut incomplete_stream_segment_started_at,
                    )
                    .await?;
                }
            }
        }

        if saw_stream_done {
            break;
        }
    }

    if !saw_stream_done && !sse_buffer.is_empty() {
        let trailing_event = String::from_utf8(sse_buffer)
            .map_err(|err| format!("Failed to decode trailing Gemma stream event: {err}"))?;
        if !trailing_event.trim().is_empty() {
            match parse_gemma_stream_event(trailing_event.trim())? {
                ParsedGemmaStreamEvent::Ignore => {}
                ParsedGemmaStreamEvent::Done => {
                    saw_stream_event = true;
                }
                ParsedGemmaStreamEvent::Delta(text, reasoning) => {
                    saw_stream_event = true;
                    raw_response_text.push_str(&text);
                    raw_reasoning_text.push_str(&reasoning);
                    emit_streamed_response_update(
                        app_handle,
                        response_id,
                        &raw_response_text,
                        &raw_reasoning_text,
                        assistant_text_prefix,
                        append_to_assistant_entry_id,
                        &mut latest_response_text,
                        &mut latest_reasoning_text,
                        &mut queued_response_bytes,
                        &mut started_audio_response,
                        &mut incomplete_stream_segment_started_at,
                    )
                    .await?;
                }
            }
        }
    }

    if !saw_stream_event {
        let payload = serde_json::from_slice::<ChatCompletionResponse>(&raw_body)
            .map_err(|e| format!("Failed to parse Gemma response: {e}"))?;
        let choice = payload.choices.into_iter().next();
        raw_response_text = choice
            .as_ref()
            .and_then(|choice| choice.message.content.as_ref())
            .map(|v| extract_chat_content_text(v.clone()))
            .unwrap_or_default();
        raw_reasoning_text = choice
            .as_ref()
            .and_then(|choice| choice.message.reasoning_content.clone())
            .unwrap_or_default();
        latest_response_text = combine_assistant_message_text(
            assistant_text_prefix,
            &sanitize_for_voice_output(&raw_response_text),
        );
        latest_reasoning_text = raw_reasoning_text.clone();
    }

    let response_text = sanitize_for_voice_output(&raw_response_text);

    info!(
        "LLM response completed in {:.1} ms ({} chars)",
        llm_started_at.elapsed().as_secs_f64() * 1000.0,
        response_text.chars().count()
    );

    if response_text.is_empty() && raw_reasoning_text.is_empty() {
        warn!("Gemma returned an empty response, skipping CSM synthesis");
    } else {
        let display_response_text =
            combine_assistant_message_text(assistant_text_prefix, &response_text);
        if display_response_text != latest_response_text
            || raw_reasoning_text != latest_reasoning_text
        {
            emit_assistant_response(
                app_handle,
                AssistantResponseEvent {
                    request_id: response_id,
                    text: display_response_text.clone(),
                    reasoning_text: raw_reasoning_text.clone(),
                    is_final: false,
                    append_to_assistant_entry_id,
                },
            );
        }
        emit_assistant_response(
            app_handle,
            AssistantResponseEvent {
                request_id: response_id,
                text: display_response_text,
                reasoning_text: raw_reasoning_text.clone(),
                is_final: true,
                append_to_assistant_entry_id,
            },
        );
        let flushed_segments = queue_spoken_response_segments_for_csm(
            app_handle,
            response_id,
            &response_text,
            append_to_assistant_entry_id,
            &mut queued_response_bytes,
            &mut started_audio_response,
            true,
        )
        .await?;
        if flushed_segments == 0 && !started_audio_response {
            warn!(
                "Gemma response became empty after spoken-response preparation, skipping CSM synthesis"
            )
        }
        info!("MLX Server Output: {}", response_text);
    }

    if let Err(err) = finalize_csm_response(app_handle, response_id).await {
        warn!(
            "Failed to finalize CSM response context for {}: {}",
            response_id, err
        );
    }

    Ok((response_id, response_text))
}

async fn emit_streamed_response_update(
    app_handle: &tauri::AppHandle,
    response_id: u64,
    raw_response_text: &str,
    raw_reasoning_text: &str,
    assistant_text_prefix: Option<&str>,
    append_to_assistant_entry_id: Option<u64>,
    latest_response_text: &mut String,
    latest_reasoning_text: &mut String,
    queued_response_bytes: &mut usize,
    started_audio_response: &mut bool,
    incomplete_stream_segment_started_at: &mut Option<Instant>,
) -> Result<(), String> {
    let response_text = sanitize_for_voice_output(raw_response_text);
    let display_response_text =
        combine_assistant_message_text(assistant_text_prefix, &response_text);

    if display_response_text != *latest_response_text
        || raw_reasoning_text != *latest_reasoning_text
    {
        emit_assistant_response(
            app_handle,
            AssistantResponseEvent {
                request_id: response_id,
                text: display_response_text.clone(),
                reasoning_text: raw_reasoning_text.to_string(),
                is_final: false,
                append_to_assistant_entry_id,
            },
        );
        *latest_response_text = display_response_text;
        *latest_reasoning_text = raw_reasoning_text.to_string();
    }

    if response_text.is_empty() {
        *incomplete_stream_segment_started_at = None;
        return Ok(());
    }

    queue_spoken_response_segments_for_csm(
        app_handle,
        response_id,
        &response_text,
        append_to_assistant_entry_id,
        queued_response_bytes,
        started_audio_response,
        false,
    )
    .await?;

    let pending_response_text = &response_text[(*queued_response_bytes).min(response_text.len())..];
    if !contains_spoken_content(pending_response_text) {
        *incomplete_stream_segment_started_at = None;
        return Ok(());
    }

    if !pending_streamed_segment_is_in_first_sentence(&response_text, *queued_response_bytes) {
        *incomplete_stream_segment_started_at = None;
        return Ok(());
    }

    if incomplete_stream_segment_started_at.is_none() {
        *incomplete_stream_segment_started_at = Some(Instant::now());
    }

    if should_flush_incomplete_streamed_response_segment(
        pending_response_text,
        incomplete_stream_segment_started_at
            .as_ref()
            .map(Instant::elapsed),
    ) {
        info!(
            "Early-flushing incomplete streamed CSM segment after {} words",
            count_spoken_words(pending_response_text)
        );
        queue_spoken_response_segments_for_csm(
            app_handle,
            response_id,
            &response_text,
            append_to_assistant_entry_id,
            queued_response_bytes,
            started_audio_response,
            true,
        )
        .await?;
        *incomplete_stream_segment_started_at = if *queued_response_bytes < response_text.len() {
            Some(Instant::now())
        } else {
            None
        };
    }

    Ok(())
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
        info!(
            "Speech worker was unavailable for synthesis request {}. Attempting restart.",
            request_id
        );
        start_csm_server_inner(app_handle, state.inner())
            .await
            .map_err(|err| {
                format!("The speech worker stopped and could not be restarted: {err}")
            })?;
    }

    if !csm_process_is_ready(state.inner()).await {
        return Err("The selected speech model is not ready. Try loading it again.".to_string());
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
        "speaker": CSM_EXPRESSIVA_SPEAKER_ID,
        "voice": KOKORO_DEFAULT_VOICE,
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

fn log_processing_audio_latency_for_first_message_chunk(
    app_handle: &tauri::AppHandle,
    request_id: u64,
) {
    let Some(latency_ms) = current_processing_audio_latency_ms(app_handle) else {
        return;
    };

    info!(
        "Latency from processing_audio to first message chunk from LLM: {} ms",
        latency_ms
    );
    emit_processing_audio_latency(
        app_handle,
        ProcessingAudioLatencyEvent {
            kind: ProcessingAudioLatencyKind::FirstMessageChunk,
            request_id: Some(request_id),
            latency_ms,
        },
    );
}

fn current_processing_audio_latency_ms(app_handle: &tauri::AppHandle) -> Option<u64> {
    let state = app_handle.state::<AppState>();
    let started_at = state.processing_audio_started_at.lock().unwrap();
    started_at
        .as_ref()
        .map(|started_at| started_at.elapsed().as_millis() as u64)
}

fn log_processing_audio_latency_for_audio(app_handle: &tauri::AppHandle) {
    let Some(latency_ms) = current_processing_audio_latency_ms(app_handle) else {
        return;
    };

    info!("Latency from processing_audio to audio: {} ms", latency_ms);
    emit_processing_audio_latency(
        app_handle,
        ProcessingAudioLatencyEvent {
            kind: ProcessingAudioLatencyKind::Audio,
            request_id: None,
            latency_ms,
        },
    );
}

fn track_processing_audio_latency_request(app_handle: &tauri::AppHandle, request_id: u64) {
    let state = app_handle.state::<AppState>();
    if state.processing_audio_started_at.lock().unwrap().is_none() {
        return;
    }

    *state.processing_audio_latency_request_id.lock().unwrap() = Some(request_id);
}

fn log_processing_audio_latency_for_first_chunk(app_handle: &tauri::AppHandle, request_id: u64) {
    let state = app_handle.state::<AppState>();
    {
        let mut tracked_request_id = state.processing_audio_latency_request_id.lock().unwrap();
        if *tracked_request_id != Some(request_id) {
            return;
        }
        *tracked_request_id = None;
    }

    let Some(started_at) = state.processing_audio_started_at.lock().unwrap().take() else {
        return;
    };

    let latency_ms = started_at.elapsed().as_millis() as u64;
    info!(
        "Latency from processing_audio to first audio chunk: {} ms",
        latency_ms
    );
    emit_processing_audio_latency(
        app_handle,
        ProcessingAudioLatencyEvent {
            kind: ProcessingAudioLatencyKind::FirstAudioChunk,
            request_id: Some(request_id),
            latency_ms,
        },
    );
}

async fn queue_spoken_response_segments_for_csm(
    app_handle: &tauri::AppHandle,
    request_id: u64,
    response_text: &str,
    append_to_assistant_entry_id: Option<u64>,
    queued_response_bytes: &mut usize,
    started_audio_response: &mut bool,
    flush_incomplete_segment: bool,
) -> Result<usize, String> {
    let queued_start = (*queued_response_bytes).min(response_text.len());
    let pending_response_text = &response_text[queued_start..];
    let (spoken_segments, consumed_len) = if flush_incomplete_segment {
        (
            prepare_spoken_response_segments_for_csm(pending_response_text),
            pending_response_text.len(),
        )
    } else {
        prepare_completed_spoken_response_segments_for_csm(pending_response_text)
    };

    if spoken_segments.is_empty() {
        return Ok(0);
    }

    if !*started_audio_response {
        log_processing_audio_latency_for_first_message_chunk(app_handle, request_id);
        track_processing_audio_latency_request(app_handle, request_id);
        emit_call_stage(app_handle, "generating_audio", "Generating Audio");
        emit_csm_audio_start(
            app_handle,
            CsmAudioStartEvent {
                request_id,
                append_to_assistant_entry_id,
            },
        );
        *started_audio_response = true;
    }

    if spoken_segments.len() == 1 {
        info!(
            "Queueing CSM response as a single streamed synthesis request: {}",
            spoken_segments[0]
        );
    } else {
        info!(
            "Queueing CSM response across {} streamed synthesis segments",
            spoken_segments.len()
        );
    }

    for (index, spoken_segment) in spoken_segments.iter().enumerate() {
        info!(
            "Queueing streamed CSM response segment {}/{}: {}",
            index + 1,
            spoken_segments.len(),
            spoken_segment
        );
        emit_csm_audio_queued(
            app_handle,
            CsmAudioQueuedEvent {
                request_id,
                text: spoken_segment.clone(),
            },
        );
        send_csm_synthesis_request(app_handle, request_id, spoken_segment).await?;
    }

    *queued_response_bytes = queued_start + consumed_len;
    Ok(spoken_segments.len())
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

async fn apply_csm_voice_context(
    state: &AppState,
    context_audio: &Path,
    context_text: Option<&str>,
) -> Result<(), String> {
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
        "context_text": context_text.unwrap_or(""),
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

fn conversation_turn_text_char_count(turn: &ConversationTurn) -> usize {
    turn.user_text.chars().count() + turn.assistant_text.chars().count()
}

struct LlmContextSelection {
    turns: Vec<ConversationTurn>,
    total_text_chars: usize,
    trimmed_by_turn_limit: bool,
}

fn select_conversation_turns_for_llm_context(
    turns: &[ConversationTurn],
    turn_limit: Option<usize>,
) -> LlmContextSelection {
    if turns.is_empty() {
        return LlmContextSelection {
            turns: Vec::new(),
            total_text_chars: 0,
            trimmed_by_turn_limit: false,
        };
    }

    let effective_turn_limit = turn_limit.map(|limit| limit.max(MIN_LLM_CONTEXT_TURN_LIMIT));
    let mut selected_turns = Vec::new();
    let mut total_text_chars = 0usize;
    let mut trimmed_by_turn_limit = false;

    for turn in turns.iter().rev() {
        let keep_for_minimum = selected_turns.len() < MIN_LLM_CONTEXT_TURN_LIMIT;
        let turn_text_chars = conversation_turn_text_char_count(turn);
        let exceeds_turn_limit = effective_turn_limit
            .map(|limit| selected_turns.len() >= limit)
            .unwrap_or(false);

        if !keep_for_minimum && exceeds_turn_limit {
            trimmed_by_turn_limit |= exceeds_turn_limit;
            break;
        }

        total_text_chars += turn_text_chars;
        selected_turns.push(turn.clone());
    }

    selected_turns.reverse();
    LlmContextSelection {
        turns: selected_turns,
        total_text_chars,
        trimmed_by_turn_limit,
    }
}

fn load_image_data_urls_from_paths<'a>(
    image_paths: impl IntoIterator<Item = &'a Path>,
) -> Vec<String> {
    image_paths
        .into_iter()
        .filter_map(load_image_data_url)
        .collect()
}

fn resolve_user_turn_image_urls(turn: &ConversationTurn) -> Vec<String> {
    if !turn.user_image_data_urls.is_empty() {
        return turn.user_image_data_urls.clone();
    }

    load_image_data_urls_from_paths(turn.image_paths.iter().map(PathBuf::as_path))
}

fn apply_llm_image_history_limit(
    conversation_turn_image_urls: &mut [Vec<String>],
    latest_image_urls: &mut Vec<String>,
    image_limit: Option<usize>,
) {
    let Some(image_limit) = image_limit else {
        return;
    };

    let total_image_count = conversation_turn_image_urls
        .iter()
        .map(Vec::len)
        .sum::<usize>()
        + latest_image_urls.len();
    let mut images_to_drop = total_image_count.saturating_sub(image_limit);

    for image_urls in conversation_turn_image_urls
        .iter_mut()
        .chain(std::iter::once(latest_image_urls))
    {
        if images_to_drop == 0 {
            break;
        }

        let drop_count = images_to_drop.min(image_urls.len());
        image_urls.drain(..drop_count);
        images_to_drop -= drop_count;
    }
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

fn build_input_audio_content(audio_wav_base64: &str) -> ChatContent {
    ChatContent::InputAudio {
        input_audio: InputAudio {
            data: audio_wav_base64.to_string(),
            format: "wav".to_string(),
        },
    }
}

fn build_user_turn_message(
    user_text: &str,
    image_paths: &[PathBuf],
    user_image_data_urls: &[String],
) -> ChatMessage {
    let image_urls = if !user_image_data_urls.is_empty() {
        user_image_data_urls.to_vec()
    } else {
        load_image_data_urls_from_paths(image_paths.iter().map(PathBuf::as_path))
    };

    build_user_turn_message_with_image_urls(user_text, &image_urls)
}

fn build_user_turn_message_with_image_urls(user_text: &str, image_urls: &[String]) -> ChatMessage {
    let mut content = Vec::new();
    for image_data_url in image_urls {
        content.push(ChatContent::InputImage {
            image_url: ImageUrlContent {
                url: image_data_url.clone(),
            },
        });
    }
    content.push(ChatContent::Text {
        text: user_text.to_string(),
    });
    ChatMessage {
        role: "user".to_string(),
        content,
    }
}

fn append_assistant_message_text(existing_text: &str, continuation_text: &str) -> String {
    let trimmed_existing = existing_text.trim_end();
    let trimmed_continuation = continuation_text.trim_start();

    if trimmed_existing.is_empty() {
        return trimmed_continuation.to_string();
    }

    if trimmed_continuation.is_empty() {
        return trimmed_existing.to_string();
    }

    let starts_with_inline_punctuation = trimmed_continuation
        .chars()
        .next()
        .map(|ch| matches!(ch, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}'))
        .unwrap_or(false);

    if starts_with_inline_punctuation {
        format!("{trimmed_existing}{trimmed_continuation}")
    } else {
        format!("{trimmed_existing} {trimmed_continuation}")
    }
}

fn combine_assistant_message_text(
    assistant_text_prefix: Option<&str>,
    response_text: &str,
) -> String {
    assistant_text_prefix
        .map(|prefix| append_assistant_message_text(prefix, response_text))
        .unwrap_or_else(|| response_text.to_string())
}

fn build_llm_system_prompt(
    base_prompt: &str,
    include_audio_context: bool,
    include_image_context: bool,
) -> String {
    let mut sections = vec![base_prompt.to_string()];
    if include_audio_context {
        sections.push(AUDIO_CONTEXT_SYSTEM_PROMPT.to_string());
    }
    if include_image_context {
        sections.push(IMAGE_CONTEXT_SYSTEM_PROMPT.to_string());
    }

    sections.join("\n\n")
}

fn build_latest_user_turn_message(
    user_text: &str,
    latest_audio_wav_base64: Option<&str>,
    latest_image_paths: &[&Path],
) -> ChatMessage {
    let latest_image_urls = load_image_data_urls_from_paths(latest_image_paths.iter().copied());

    build_latest_user_turn_message_with_image_urls(
        user_text,
        latest_audio_wav_base64,
        &latest_image_urls,
    )
}

fn build_latest_user_turn_message_with_image_urls(
    user_text: &str,
    latest_audio_wav_base64: Option<&str>,
    latest_image_urls: &[String],
) -> ChatMessage {
    let mut content = Vec::new();

    for image_data_url in latest_image_urls {
        content.push(ChatContent::InputImage {
            image_url: ImageUrlContent {
                url: image_data_url.clone(),
            },
        });
    }

    if let Some(audio_wav_base64) = latest_audio_wav_base64 {
        content.push(ChatContent::InputAudio {
            input_audio: InputAudio {
                data: audio_wav_base64.to_string(),
                format: "wav".to_string(),
            },
        });
    }
    content.push(ChatContent::Text {
        text: user_text.to_string(),
    });
    ChatMessage {
        role: "user".to_string(),
        content,
    }
}

enum ParsedGemmaStreamEvent {
    Delta(String, String),
    Done,
    Ignore,
}

fn extract_stream_chunk_content(chunk: ChatCompletionStreamChunk) -> (String, String) {
    chunk
        .choices
        .into_iter()
        .find_map(|choice| {
            let content = choice
                .delta
                .as_ref()
                .and_then(|delta| delta.content.as_ref())
                .or_else(|| {
                    choice
                        .message
                        .as_ref()
                        .and_then(|message| message.content.as_ref())
                })
                .map(|v| extract_chat_content_text(v.clone()))
                .unwrap_or_default();

            let reasoning = choice
                .delta
                .as_ref()
                .and_then(|delta| delta.reasoning_content.as_ref())
                .or_else(|| {
                    choice
                        .message
                        .as_ref()
                        .and_then(|message| message.reasoning_content.as_ref())
                })
                .cloned()
                .unwrap_or_default();

            if content.is_empty() && reasoning.is_empty() {
                choice
                    .finish_reason
                    .as_ref()
                    .map(|_| (String::new(), String::new()))
            } else {
                Some((content, reasoning))
            }
        })
        .unwrap_or_default()
}

fn parse_gemma_stream_event(event_block: &str) -> Result<ParsedGemmaStreamEvent, String> {
    let mut data_lines = Vec::new();

    for line in event_block.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(':') {
            continue;
        }

        if let Some(data) = trimmed.strip_prefix("data:") {
            data_lines.push(data.trim_start());
        }
    }

    if data_lines.is_empty() {
        return Ok(ParsedGemmaStreamEvent::Ignore);
    }

    let data = data_lines.join("\n");
    if data == "[DONE]" {
        return Ok(ParsedGemmaStreamEvent::Done);
    }

    let chunk = serde_json::from_str::<ChatCompletionStreamChunk>(&data)
        .map_err(|err| format!("Failed to parse streamed Gemma chunk: {err}"))?;
    let (text, reasoning) = extract_stream_chunk_content(chunk);
    if text.is_empty() && reasoning.is_empty() {
        return Ok(ParsedGemmaStreamEvent::Ignore);
    }

    Ok(ParsedGemmaStreamEvent::Delta(text, reasoning))
}

fn drain_next_sse_event(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    let separator = buffer.windows(2).position(|window| window == b"\n\n")?;
    Some(buffer.drain(..separator + 2).take(separator).collect())
}

fn log_chat_request_debug(conversation_session_id: u64, request: &ChatRequest) {
    let mut messages_summary = Vec::new();
    for msg in &request.messages {
        let content_text = msg
            .content
            .iter()
            .find_map(|c| {
                if let ChatContent::Text { text } = c {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();
        messages_summary.push(serde_json::json!({
            "role": msg.role,
            "text": content_text.chars().take(50).collect::<String>(),
            "content_count": msg.content.len(),
        }));
    }

    match serde_json::to_string_pretty(&messages_summary) {
        Ok(messages_json) => debug!(
            "Sending chat request for conversation session {} with {} messages:\n{}",
            conversation_session_id,
            request.messages.len(),
            messages_json
        ),
        Err(err) => warn!(
            "Failed to serialize conversation log for session {}: {}",
            conversation_session_id, err
        ),
    }
}

fn encode_audio_samples_as_wav_base64(samples: &[f32], sample_rate: u32) -> Result<String, String> {
    let data_bytes_len = samples
        .len()
        .checked_mul(2)
        .ok_or_else(|| "Captured audio was too large to encode.".to_string())?;
    if data_bytes_len > u32::MAX as usize {
        return Err("Captured audio was too large to encode.".to_string());
    }

    let data_bytes_len = data_bytes_len as u32;
    let byte_rate = sample_rate
        .checked_mul(2)
        .ok_or_else(|| "Captured audio sample rate was invalid.".to_string())?;
    let riff_chunk_size = 36u32
        .checked_add(data_bytes_len)
        .ok_or_else(|| "Captured audio was too large to encode.".to_string())?;
    let mut wav_bytes = Vec::with_capacity(44 + data_bytes_len as usize);

    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&riff_chunk_size.to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVE");
    wav_bytes.extend_from_slice(b"fmt ");
    wav_bytes.extend_from_slice(&16u32.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&sample_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&byte_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&2u16.to_le_bytes());
    wav_bytes.extend_from_slice(&16u16.to_le_bytes());
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&data_bytes_len.to_le_bytes());

    for &sample in samples {
        let amplitude = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        wav_bytes.extend_from_slice(&amplitude.to_le_bytes());
    }

    Ok(BASE64_STANDARD.encode(wav_bytes))
}

#[cfg(test)]
fn input_audio_looks_inline(audio_data: &str) -> bool {
    audio_data.starts_with("data:") || audio_data.contains(";base64,") || {
        audio_data.len() >= 16
            && audio_data
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
    }
}

fn remove_temp_image_file(image_path: &Path) {
    match std::fs::remove_file(image_path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => warn!(
            "Failed to remove temp image file {}: {}",
            image_path.display(),
            err
        ),
    }
}

fn load_image_data_url(image_path: &Path) -> Option<String> {
    let path_str = image_path.to_string_lossy();
    if path_str.starts_with("data:") || path_str.contains(";base64,") || path_str.len() > 1024 {
        return None;
    }

    match std::fs::read(image_path) {
        Ok(bytes) => Some(format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(bytes)
        )),
        Err(err) => {
            warn!(
                "Failed to read temp image file {} for preview: {}",
                image_path.display(),
                err
            );
            None
        }
    }
}

fn write_data_url_to_temp_file(data_url: &str) -> Option<PathBuf> {
    let base64_data = if data_url.starts_with("data:image/png;base64,") {
        data_url.strip_prefix("data:image/png;base64,")?
    } else if data_url.starts_with("data:image/jpeg;base64,") {
        data_url.strip_prefix("data:image/jpeg;base64,")?
    } else if data_url.contains("base64,") {
        data_url.split("base64,").nth(1)?
    } else {
        return None;
    };

    let bytes = BASE64_STANDARD.decode(base64_data.trim()).ok()?;
    let path = create_temp_screen_capture_path();
    std::fs::write(&path, bytes).ok()?;
    Some(path)
}

#[cfg(target_os = "macos")]
fn resize_image_file_for_context(image_path: &Path) {
    let image_path_string = image_path.to_string_lossy().into_owned();
    let _ = std::process::Command::new("/usr/bin/sips")
        .args([
            "-Z",
            "1024",
            &image_path_string,
            "--out",
            &image_path_string,
        ])
        .output();
}

#[cfg(not(target_os = "macos"))]
fn resize_image_file_for_context(_image_path: &Path) {}

fn attach_pending_screen_capture(app_handle: &AppHandle, path: PathBuf, message: &str) {
    let state = app_handle.state::<AppState>();
    add_pending_screen_capture(state.inner(), path);
    if *state.tray_pong_playback_enabled.lock().unwrap() {
        emit_play_tray_pong(app_handle);
    }
    emit_screen_capture_event(app_handle, "ready", message);
    emit_overlay_notification(
        app_handle,
        OverlayNotificationEvent {
            message: "OpenDuck: Attached Screenshot".to_string(),
        },
    );
    show_temporary_tray_icon(app_handle, Duration::from_secs(3));
    refresh_tray_presentation(app_handle);
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
        let cleaned = strip_nonspoken_symbols(&cleaned);
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

fn strip_nonspoken_symbols(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());

    for ch in text.chars() {
        if is_emoji_joiner_or_modifier(ch) {
            continue;
        }

        if is_nonspoken_symbol(ch) {
            if !cleaned.ends_with(' ') {
                cleaned.push(' ');
            }
            continue;
        }

        cleaned.push(ch);
    }

    normalize_punctuation_spacing(&collapse_whitespace(&cleaned))
}

fn is_emoji_joiner_or_modifier(ch: char) -> bool {
    matches!(
        ch as u32,
        0x200D
            | 0x20E3
            | 0xFE0E
            | 0xFE0F
            | 0xE0020..=0xE007F
            | 0x1F3FB..=0x1F3FF
    )
}

fn is_nonspoken_symbol(ch: char) -> bool {
    matches!(
        ch as u32,
        0x00A9
            | 0x00AE
            | 0x203C
            | 0x2049
            | 0x2122
            | 0x2139
            | 0x2194..=0x2199
            | 0x21A9..=0x21AA
            | 0x231A..=0x231B
            | 0x2328
            | 0x23CF
            | 0x23E9..=0x23FA
            | 0x24C2
            | 0x25AA..=0x25AB
            | 0x25B6
            | 0x25C0
            | 0x25FB..=0x25FE
            | 0x2600..=0x27BF
            | 0x2934..=0x2935
            | 0x2B05..=0x2B07
            | 0x2B1B..=0x2B1C
            | 0x2B50
            | 0x2B55
            | 0x3030
            | 0x303D
            | 0x3297
            | 0x3299
            | 0x1F000..=0x1FAFF
    )
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_punctuation_spacing(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == ' '
            && chars
                .peek()
                .copied()
                .is_some_and(is_tight_trailing_punctuation)
        {
            continue;
        }

        normalized.push(ch);
    }

    normalized.trim().to_string()
}

fn is_tight_trailing_punctuation(ch: char) -> bool {
    matches!(ch, '.' | ',' | '!' | '?' | ':' | ';' | ')' | ']' | '}')
}

fn prepare_spoken_response_segments_for_csm(text: &str) -> Vec<String> {
    split_spoken_response_into_segments(text, MAX_SPOKEN_SENTENCES_PER_SEGMENT)
        .into_iter()
        .flat_map(|segment| split_long_spoken_segment_for_csm(&segment))
        .collect()
}

fn prepare_completed_spoken_response_segments_for_csm(text: &str) -> (Vec<String>, usize) {
    let (segments, consumed_len) =
        collect_spoken_response_segments(text, MAX_SPOKEN_SENTENCES_PER_SEGMENT, false);
    (
        segments
            .into_iter()
            .flat_map(|segment| split_long_spoken_segment_for_csm(&segment))
            .collect(),
        consumed_len,
    )
}

fn split_spoken_response_into_segments(text: &str, max_sentences: usize) -> Vec<String> {
    let normalized = collapse_whitespace(text);
    collect_spoken_response_segments(&normalized, max_sentences, true).0
}

fn collect_spoken_response_segments(
    normalized: &str,
    max_sentences: usize,
    include_trailing_incomplete: bool,
) -> (Vec<String>, usize) {
    if normalized.is_empty() || max_sentences == 0 {
        return (Vec::new(), 0);
    }

    let mut segments = Vec::new();
    let mut segment_start = 0;
    let mut sentence_count = 0;
    let mut consumed_len = 0;
    for (idx, ch) in normalized.char_indices() {
        if is_spoken_sentence_boundary(normalized, idx, ch) {
            sentence_count += 1;
            if sentence_count >= max_sentences {
                let end = expand_speech_boundary(normalized, idx + ch.len_utf8());
                let segment = normalized[segment_start..end].trim();
                if contains_spoken_content(segment) {
                    segments.push(segment.to_string());
                }
                segment_start = end;
                while segment_start < normalized.len() {
                    let Some(ch) = normalized[segment_start..].chars().next() else {
                        break;
                    };
                    if ch.is_whitespace() {
                        segment_start += ch.len_utf8();
                        continue;
                    }
                    break;
                }
                consumed_len = segment_start;
                sentence_count = 0;
            }
        }
    }

    if include_trailing_incomplete {
        let trailing_segment = normalized[segment_start..].trim();
        if contains_spoken_content(trailing_segment) {
            segments.push(trailing_segment.to_string());
            consumed_len = normalized.len();
        }
    }

    (segments, consumed_len)
}

fn is_spoken_sentence_boundary(text: &str, idx: usize, ch: char) -> bool {
    match ch {
        '.' => is_spoken_period_boundary(text, idx),
        '!' | '?' | '\n' => true,
        _ => false,
    }
}

fn is_spoken_period_boundary(text: &str, idx: usize) -> bool {
    let period_end = idx + '.'.len_utf8();
    let previous_char = text[..idx].chars().next_back();
    let next_char = text[period_end..].chars().next();

    if previous_char.is_some_and(|ch| ch.is_ascii_digit())
        && next_char.is_none_or(|ch| ch.is_ascii_digit())
    {
        return false;
    }

    true
}

fn split_long_spoken_segment_for_csm(segment: &str) -> Vec<String> {
    let normalized = collapse_whitespace(segment);
    let trimmed = normalized.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let words = trimmed.split_whitespace().collect::<Vec<_>>();
    if words.len() <= MAX_SPOKEN_WORDS_PER_SEGMENT {
        return normalize_spoken_chunks_for_csm(vec![(trimmed.to_string(), true)]);
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < words.len() {
        let target_end = (start + MAX_SPOKEN_WORDS_PER_SEGMENT).min(words.len());
        let hard_limit = (start + MAX_SPOKEN_WORDS_PER_SEGMENT_HARD_LIMIT).min(words.len());
        let end = choose_spoken_chunk_end(&words, start, target_end, hard_limit);

        let is_last = end == words.len();
        chunks.push((words[start..end].join(" "), is_last));
        start = end;
    }

    normalize_spoken_chunks_for_csm(chunks)
}

fn choose_spoken_chunk_end(
    words: &[&str],
    start: usize,
    target_end: usize,
    hard_limit: usize,
) -> usize {
    if hard_limit == words.len() {
        return words.len();
    }

    if let Some(preferred_end) = find_preferred_spoken_chunk_end(words, start, target_end) {
        let chunk_word_count = preferred_end.saturating_sub(start);
        if chunk_word_count >= 4 || preferred_end == words.len() {
            return preferred_end;
        }
    }

    find_extended_spoken_chunk_end(words, target_end, hard_limit).unwrap_or(target_end)
}

fn find_preferred_spoken_chunk_end(words: &[&str], start: usize, limit: usize) -> Option<usize> {
    for candidate in (start + 1..=limit).rev() {
        let Some(last_char) = trailing_spoken_word_punctuation(words[candidate - 1]) else {
            continue;
        };

        if matches!(last_char, ',' | ';' | ':' | '.' | '!' | '?') {
            return Some(candidate);
        }
    }

    None
}

fn find_extended_spoken_chunk_end(words: &[&str], start: usize, limit: usize) -> Option<usize> {
    find_spoken_chunk_end_with_punctuation(words, start, limit, true)
        .or_else(|| find_spoken_chunk_end_with_punctuation(words, start, limit, false))
}

fn find_spoken_chunk_end_with_punctuation(
    words: &[&str],
    start: usize,
    limit: usize,
    strong_only: bool,
) -> Option<usize> {
    for candidate in start + 1..=limit {
        let Some(last_char) = trailing_spoken_word_punctuation(words[candidate - 1]) else {
            continue;
        };

        if matches!(last_char, '.' | '!' | '?')
            || (!strong_only && matches!(last_char, ',' | ';' | ':'))
        {
            return Some(candidate);
        }
    }

    None
}

fn trailing_spoken_word_punctuation(word: &str) -> Option<char> {
    word.trim_end_matches(|ch: char| matches!(ch, '"' | '\'' | ')' | ']' | '}'))
        .chars()
        .last()
}

fn normalize_spoken_chunks_for_csm(chunks: Vec<(String, bool)>) -> Vec<String> {
    chunks
        .into_iter()
        .filter_map(|(chunk, is_last)| normalize_spoken_chunk_for_csm(&chunk, is_last))
        .collect()
}

fn normalize_spoken_chunk_for_csm(chunk: &str, is_last: bool) -> Option<String> {
    let mut cleaned = collapse_whitespace(chunk).replace(",", "");
    cleaned = cleaned.trim().to_string();
    if !contains_spoken_content(&cleaned) {
        return None;
    }

    while matches!(cleaned.chars().last(), Some(',' | ';' | ':' | '-' | '—')) {
        cleaned.pop();
        cleaned = cleaned.trim_end().to_string();
    }

    if !contains_spoken_content(&cleaned) {
        return None;
    }

    let has_terminal_punctuation = matches!(cleaned.chars().last(), Some('.' | '!' | '?'));
    if !has_terminal_punctuation && !is_last {
        cleaned.push('.');
    }

    Some(cleaned)
}

fn contains_spoken_content(text: &str) -> bool {
    text.chars().any(|ch| ch.is_alphanumeric())
}

fn count_spoken_words(text: &str) -> usize {
    text.split_whitespace()
        .filter(|word| word.chars().any(|ch| ch.is_alphanumeric()))
        .count()
}

fn pending_streamed_segment_is_in_first_sentence(
    response_text: &str,
    queued_response_bytes: usize,
) -> bool {
    let queued_start = queued_response_bytes.min(response_text.len());
    let completed_prefix = &response_text[..queued_start];

    !completed_prefix
        .char_indices()
        .any(|(idx, ch)| is_spoken_sentence_boundary(completed_prefix, idx, ch))
}

fn should_flush_incomplete_streamed_response_segment(
    text: &str,
    pending_elapsed: Option<Duration>,
) -> bool {
    if !contains_spoken_content(text) {
        return false;
    }

    count_spoken_words(text) >= STREAMING_INCOMPLETE_SEGMENT_FLUSH_WORDS
        || pending_elapsed
            .map(|elapsed| elapsed >= Duration::from_millis(STREAMING_INCOMPLETE_SEGMENT_FLUSH_MS))
            .unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::{
        apply_llm_image_history_limit, build_latest_user_turn_message, build_llm_system_prompt,
        build_user_turn_message, clamp_end_of_utterance_silence_ms, clamp_llm_context_turn_limit,
        clamp_llm_image_history_limit, parse_gemma_stream_event,
        pending_streamed_segment_is_in_first_sentence,
        prepare_completed_spoken_response_segments_for_csm,
        prepare_spoken_response_segments_for_csm, required_silence_chunks,
        resolve_capture_sample_rate, samples_duration_ms, sanitize_for_voice_output,
        select_conversation_turns_for_llm_context,
        should_flush_incomplete_streamed_response_segment, suppress_playback_echo,
        write_data_url_to_temp_file, AudioPayload, ConversationTurn, ParsedGemmaStreamEvent,
        AUDIO_CONTEXT_SYSTEM_PROMPT, DEFAULT_SAMPLE_RATE, IMAGE_CONTEXT_SYSTEM_PROMPT,
        MAX_SPOKEN_WORDS_PER_SEGMENT_HARD_LIMIT,
    };
    use crate::constants::{
        DEFAULT_LLM_CONTEXT_TURN_LIMIT, END_OF_UTTERANCE_SILENCE_MS,
        MAX_END_OF_UTTERANCE_SILENCE_MS, MAX_LLM_CONTEXT_TURN_LIMIT, MAX_LLM_IMAGE_HISTORY_LIMIT,
        MIN_END_OF_UTTERANCE_SILENCE_MS, MIN_LLM_CONTEXT_TURN_LIMIT, MIN_LLM_IMAGE_HISTORY_LIMIT,
        STREAMING_INCOMPLETE_SEGMENT_FLUSH_MS, STREAMING_INCOMPLETE_SEGMENT_FLUSH_WORDS,
    };
    use std::{fs, path::Path, time::Duration};

    const TEST_IMAGE_DATA_URL: &str =
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9sXv2t8AAAAASUVORK5CYII=";

    fn serialize_chat_messages_for_debug(
        messages: &[super::ChatMessage],
    ) -> Vec<serde_json::Value> {
        messages
            .iter()
            .map(|message| {
                let content = message
                    .content
                    .iter()
                    .map(|item| match item {
                        super::ChatContent::Text { text } => serde_json::json!({
                            "type": "text",
                            "text": text,
                        }),
                        super::ChatContent::InputImage { image_url } => {
                            let file_name = if image_url.url.starts_with("data:") {
                                None
                            } else {
                                Path::new(&image_url.url)
                                    .file_name()
                                    .and_then(|name| name.to_str())
                            };

                            match file_name {
                                Some(file_name) => serde_json::json!({
                                    "type": "image_url",
                                    "file_name": file_name,
                                }),
                                None => serde_json::json!({
                                    "type": "image_url",
                                    "image_url": image_url.url,
                                }),
                            }
                        }
                        super::ChatContent::InputAudio { input_audio } => {
                            let file_name = if super::input_audio_looks_inline(&input_audio.data) {
                                None
                            } else {
                                Path::new(&input_audio.data)
                                    .file_name()
                                    .and_then(|name| name.to_str())
                            };

                            match file_name {
                                Some(file_name) => serde_json::json!({
                                    "type": "input_audio",
                                    "file_name": file_name,
                                    "format": input_audio.format,
                                }),
                                None => serde_json::json!({
                                    "type": "input_audio",
                                    "format": input_audio.format,
                                }),
                            }
                        }
                    })
                    .collect::<Vec<_>>();

                serde_json::json!({
                    "role": message.role,
                    "content": content,
                })
            })
            .collect()
    }

    #[test]
    fn sanitize_for_voice_output_removes_plain_emoji() {
        assert_eq!(sanitize_for_voice_output("Hello 😊 there."), "Hello there.");
    }

    #[test]
    fn sanitize_for_voice_output_removes_joined_emoji_sequences() {
        assert_eq!(
            sanitize_for_voice_output("Family: 👨‍👩‍👧‍👦 Ready 👍🏽"),
            "Family: Ready"
        );
    }

    #[test]
    fn sanitize_for_voice_output_removes_flags_and_keycaps() {
        assert_eq!(sanitize_for_voice_output("Press 1️⃣ now 🇺🇸"), "Press 1 now");
    }

    #[test]
    fn sanitize_for_voice_output_preserves_spoken_unicode_text() {
        assert_eq!(
            sanitize_for_voice_output("Bonjour, ça va très bien."),
            "Bonjour, ça va très bien."
        );
    }

    #[test]
    fn spoken_segments_stay_clean_after_symbol_stripping() {
        assert_eq!(
            prepare_spoken_response_segments_for_csm(&sanitize_for_voice_output(
                "Sure 😊. I can help with that 👍."
            )),
            vec!["Sure.".to_string(), "I can help with that.".to_string()]
        );
    }

    #[test]
    fn completed_spoken_segments_wait_for_sentence_boundary() {
        let response_text = "Hello there. How are";
        let (segments, consumed_len) =
            prepare_completed_spoken_response_segments_for_csm(response_text);

        assert_eq!(segments, vec!["Hello there.".to_string()]);
        assert_eq!(&response_text[consumed_len..], "How are");
    }

    #[test]
    fn long_single_sentence_is_split_into_shorter_spoken_segments() {
        let response_text = "Alright, alright, settle down folks! You're looking at the one and only Monkey D. Luffy and I'm not your average primate because I'm a pirate with a dream bigger than the whole sea.";
        let segments = prepare_spoken_response_segments_for_csm(response_text);

        assert!(segments.len() > 1);
        assert!(segments.iter().all(|segment| {
            segment.split_whitespace().count() <= MAX_SPOKEN_WORDS_PER_SEGMENT_HARD_LIMIT
        }));
    }

    #[test]
    fn completed_spoken_segments_keep_short_finished_question_intact() {
        let response_text =
            "Is there a particular line or concept in this code you would like me to focus on? Next bit";
        let (segments, consumed_len) =
            prepare_completed_spoken_response_segments_for_csm(response_text);

        assert_eq!(
            segments,
            vec![
                "Is there a particular line or concept in this code you would like me to focus on?"
                    .to_string()
            ]
        );
        assert_eq!(&response_text[consumed_len..], "Next bit");
    }

    #[test]
    fn completed_spoken_segments_split_long_finished_sentence_without_losing_tail() {
        let response_text = "This is an extremely long finished sentence that should be broken into smaller chunks for speech synthesis because otherwise the speech worker can cut it off midway through playback. Next bit";
        let (segments, consumed_len) =
            prepare_completed_spoken_response_segments_for_csm(response_text);

        assert!(segments.len() > 1);
        assert_eq!(&response_text[consumed_len..], "Next bit");
    }

    #[test]
    fn completed_spoken_segments_drop_punctuation_only_ellipsis_tail() {
        let response_text = "Letting everyone peek under the hood, tinker, improve... Next bit";
        let (segments, consumed_len) =
            prepare_completed_spoken_response_segments_for_csm(response_text);

        assert_eq!(
            segments,
            vec!["Letting everyone peek under the hood tinker improve.".to_string()]
        );
        assert_eq!(&response_text[consumed_len..], "Next bit");
    }

    #[test]
    fn completed_spoken_segments_wait_for_decimal_suffix_before_flushing() {
        let response_text = "I am not familiar with Claude 3.";
        let (segments, consumed_len) =
            prepare_completed_spoken_response_segments_for_csm(response_text);

        assert!(segments.is_empty());
        assert_eq!(consumed_len, 0);
    }

    #[test]
    fn completed_spoken_segments_keep_decimal_model_names_intact() {
        let response_text = "I am not familiar with Claude 3.5. Are you asking about a specific AI";
        let (segments, consumed_len) =
            prepare_completed_spoken_response_segments_for_csm(response_text);

        assert_eq!(
            segments,
            vec!["I am not familiar with Claude 3.5.".to_string()]
        );
        assert_eq!(
            &response_text[consumed_len..],
            "Are you asking about a specific AI"
        );
    }

    #[test]
    fn spoken_segments_keep_decimal_model_names_intact() {
        let response_text =
            "I am not familiar with Claude 3.5. Are you asking about a specific AI model?";
        let segments = prepare_spoken_response_segments_for_csm(response_text);

        assert_eq!(
            segments,
            vec![
                "I am not familiar with Claude 3.5.".to_string(),
                "Are you asking about a specific AI model?".to_string()
            ]
        );
    }

    #[test]
    fn required_silence_chunks_scales_with_sample_rate() {
        assert_eq!(
            required_silence_chunks(44_100, 128, END_OF_UTTERANCE_SILENCE_MS),
            690
        );
        assert_eq!(
            required_silence_chunks(48_000, 128, END_OF_UTTERANCE_SILENCE_MS),
            750
        );
    }

    #[test]
    fn required_silence_chunks_respects_chunk_size() {
        assert_eq!(
            required_silence_chunks(48_000, 256, END_OF_UTTERANCE_SILENCE_MS),
            375
        );
        assert_eq!(
            required_silence_chunks(48_000, 1024, END_OF_UTTERANCE_SILENCE_MS),
            94
        );
    }

    #[test]
    fn sample_duration_tracks_sample_count() {
        assert_eq!(samples_duration_ms(48_000, 48_000), 1000);
        assert_eq!(samples_duration_ms(24_000, 48_000), 500);
    }

    #[test]
    fn llm_context_selection_prefers_recent_turns() {
        let turns = (0..10)
            .map(|index| ConversationTurn {
                user_entry_id: index * 2,
                assistant_entry_id: index * 2 + 1,
                user_text: format!("user turn {index}"),
                assistant_text: format!("assistant turn {index}"),
                image_paths: Vec::new(),
                user_image_data_urls: Vec::new(),
                image_path: None,
                user_image_data_url: None,
            })
            .collect::<Vec<_>>();

        let selected =
            select_conversation_turns_for_llm_context(&turns, DEFAULT_LLM_CONTEXT_TURN_LIMIT);

        assert_eq!(
            selected.turns.len(),
            DEFAULT_LLM_CONTEXT_TURN_LIMIT.unwrap()
        );
        assert!(selected.trimmed_by_turn_limit);
        assert_eq!(selected.turns.first().unwrap().user_text, "user turn 3");
        assert_eq!(
            selected.turns.last().unwrap().assistant_text,
            "assistant turn 9"
        );
    }

    #[test]
    fn llm_context_selection_can_be_unlimited() {
        let turns = (0..4)
            .map(|index| ConversationTurn {
                user_entry_id: index * 2,
                assistant_entry_id: index * 2 + 1,
                user_text: format!("u{index}"),
                assistant_text: format!("a{index}"),
                image_paths: Vec::new(),
                user_image_data_urls: Vec::new(),
                image_path: None,
                user_image_data_url: None,
            })
            .collect::<Vec<_>>();

        let selected = select_conversation_turns_for_llm_context(&turns, None);

        assert_eq!(selected.turns.len(), turns.len());
        assert!(!selected.trimmed_by_turn_limit);
        assert_eq!(selected.turns.first().unwrap().user_text, "u0");
        assert_eq!(selected.turns.last().unwrap().assistant_text, "a3");
    }

    #[test]
    fn incomplete_streamed_segments_flush_after_word_threshold() {
        let long_partial = vec!["word"; STREAMING_INCOMPLETE_SEGMENT_FLUSH_WORDS].join(" ");

        assert!(should_flush_incomplete_streamed_response_segment(
            &long_partial,
            None
        ));
    }

    #[test]
    fn incomplete_streamed_segments_flush_after_timeout() {
        assert!(should_flush_incomplete_streamed_response_segment(
            "Still thinking through this response",
            Some(Duration::from_millis(STREAMING_INCOMPLETE_SEGMENT_FLUSH_MS))
        ));
    }

    #[test]
    fn incomplete_streamed_segments_wait_when_short_and_fresh() {
        assert!(!should_flush_incomplete_streamed_response_segment(
            "Still thinking",
            Some(Duration::from_millis(150))
        ));
    }

    #[test]
    fn incomplete_streamed_segments_stay_eligible_within_first_sentence() {
        let response_text =
            "This first sentence has already been partially queued but it is still unfinished";
        let queued_response_bytes = "This first sentence has already".len();

        assert!(pending_streamed_segment_is_in_first_sentence(
            response_text,
            queued_response_bytes
        ));
    }

    #[test]
    fn incomplete_streamed_segments_do_not_early_flush_after_first_sentence() {
        let long_partial = vec!["word"; STREAMING_INCOMPLETE_SEGMENT_FLUSH_WORDS].join(" ");
        let response_text = format!("First sentence. {long_partial}");
        let queued_response_bytes = "First sentence. ".len();
        let pending_response_text = &response_text[queued_response_bytes..];

        assert!(!pending_streamed_segment_is_in_first_sentence(
            &response_text,
            queued_response_bytes
        ));
        assert!(should_flush_incomplete_streamed_response_segment(
            pending_response_text,
            None
        ));
    }

    #[test]
    fn clamp_end_of_utterance_silence_ms_stays_in_supported_range() {
        assert_eq!(
            clamp_end_of_utterance_silence_ms(MIN_END_OF_UTTERANCE_SILENCE_MS - 1),
            MIN_END_OF_UTTERANCE_SILENCE_MS
        );
        assert_eq!(
            clamp_end_of_utterance_silence_ms(MAX_END_OF_UTTERANCE_SILENCE_MS + 1),
            MAX_END_OF_UTTERANCE_SILENCE_MS
        );
    }

    #[test]
    fn clamp_auto_continue_silence_ms_stays_in_supported_range() {
        assert_eq!(
            clamp_auto_continue_silence_ms(MIN_AUTO_CONTINUE_SILENCE_MS - 1),
            MIN_AUTO_CONTINUE_SILENCE_MS
        );
        assert_eq!(
            clamp_auto_continue_silence_ms(MAX_AUTO_CONTINUE_SILENCE_MS + 1),
            MAX_AUTO_CONTINUE_SILENCE_MS
        );
    }

    #[test]
    fn assistant_message_continuations_append_cleanly() {
        assert_eq!(
            append_assistant_message_text("That covers the basics.", "Here is one more detail."),
            "That covers the basics. Here is one more detail."
        );
        assert_eq!(
            append_assistant_message_text("Wait", ", there is more."),
            "Wait, there is more."
        );
    }

    #[test]
    fn clamp_llm_context_turn_limit_stays_in_supported_range() {
        assert_eq!(
            clamp_llm_context_turn_limit((MIN_LLM_CONTEXT_TURN_LIMIT - 1) as u32),
            MIN_LLM_CONTEXT_TURN_LIMIT
        );
        assert_eq!(
            clamp_llm_context_turn_limit((MAX_LLM_CONTEXT_TURN_LIMIT + 1) as u32),
            MAX_LLM_CONTEXT_TURN_LIMIT
        );
    }

    #[test]
    fn clamp_llm_image_history_limit_stays_in_supported_range() {
        assert_eq!(
            clamp_llm_image_history_limit((MIN_LLM_IMAGE_HISTORY_LIMIT - 1) as u32),
            MIN_LLM_IMAGE_HISTORY_LIMIT
        );
        assert_eq!(
            clamp_llm_image_history_limit((MAX_LLM_IMAGE_HISTORY_LIMIT + 1) as u32),
            MAX_LLM_IMAGE_HISTORY_LIMIT
        );
    }

    #[test]
    fn gemma_stream_event_extracts_delta_text() {
        let event =
            parse_gemma_stream_event(r#"data: {"choices":[{"delta":{"content":"Hello there."}}]}"#)
                .unwrap();

        assert!(matches!(
            event,
            ParsedGemmaStreamEvent::Delta(text, reasoning)
                if text == "Hello there." && reasoning.is_empty()
        ));
    }

    #[test]
    fn gemma_stream_event_detects_done_signal() {
        let event = parse_gemma_stream_event("data: [DONE]").unwrap();

        assert!(matches!(event, ParsedGemmaStreamEvent::Done));
    }

    #[test]
    fn latest_user_turn_message_stays_text_only_without_audio() {
        let message = build_latest_user_turn_message("hello there", None, &[]);
        let serialized = serde_json::to_value(&message).unwrap();

        assert_eq!(serialized["role"], "user");
        assert_eq!(serialized["content"].as_array().unwrap().len(), 1);
        assert_eq!(serialized["content"][0]["type"], "text");
        assert_eq!(serialized["content"][0]["text"], "hello there");
    }

    #[test]
    fn latest_user_turn_message_includes_image_when_available() {
        let image_path =
            write_data_url_to_temp_file(TEST_IMAGE_DATA_URL).expect("test image should exist");
        let message = build_latest_user_turn_message("hello there", None, &[image_path.as_path()]);
        let serialized = serde_json::to_value(&message).unwrap();
        fs::remove_file(&image_path).ok();

        assert_eq!(serialized["role"], "user");
        assert_eq!(serialized["content"].as_array().unwrap().len(), 2);
        assert_eq!(serialized["content"][0]["type"], "image_url");
        assert!(serialized["content"][0]["image_url"]["url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert_eq!(serialized["content"][1]["type"], "text");
        assert_eq!(serialized["content"][1]["text"], "hello there");
    }

    #[test]
    fn latest_user_turn_message_includes_inline_audio_when_available() {
        let audio_wav_base64 = "UklGRiQAAABXQVZFZm10IBAAAAABAAEAIlYAAESsAAACABAAZGF0YQAAAAA=";
        let message = build_latest_user_turn_message("hello there", Some(audio_wav_base64), &[]);
        let serialized = serde_json::to_value(&message).unwrap();

        assert_eq!(serialized["role"], "user");
        assert_eq!(serialized["content"].as_array().unwrap().len(), 2);
        assert_eq!(serialized["content"][0]["type"], "input_audio");
        assert_eq!(
            serialized["content"][0]["input_audio"]["data"],
            audio_wav_base64
        );
        assert_eq!(serialized["content"][0]["input_audio"]["format"], "wav");
        assert_eq!(serialized["content"][1]["type"], "text");
        assert_eq!(serialized["content"][1]["text"], "hello there");
    }

    #[test]
    fn debug_chat_messages_show_image_file_name() {
        let image_path =
            write_data_url_to_temp_file(TEST_IMAGE_DATA_URL).expect("test image should exist");
        let messages = vec![build_latest_user_turn_message(
            "hello there",
            None,
            &[image_path.as_path()],
        )];
        let serialized = serialize_chat_messages_for_debug(&messages);
        fs::remove_file(&image_path).ok();

        assert_eq!(serialized[0]["role"], "user");
        assert_eq!(serialized[0]["content"].as_array().unwrap().len(), 2);
        assert_eq!(serialized[0]["content"][0]["type"], "image_url");
        assert!(serialized[0]["content"][0]["image_url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
        assert_eq!(serialized[0]["content"][1]["type"], "text");
        assert_eq!(serialized[0]["content"][1]["text"], "hello there");
    }

    #[test]
    fn debug_chat_messages_hide_inline_audio_payloads() {
        let messages = vec![build_latest_user_turn_message(
            "hello there",
            Some("UklGRiQAAABXQVZFZm10IBAAAAABAAEAIlYAAESsAAACABAAZGF0YQAAAAA="),
            &[],
        )];
        let serialized = serialize_chat_messages_for_debug(&messages);

        assert_eq!(serialized[0]["role"], "user");
        assert_eq!(serialized[0]["content"].as_array().unwrap().len(), 2);
        assert_eq!(serialized[0]["content"][0]["type"], "input_audio");
        assert_eq!(serialized[0]["content"][0]["format"], "wav");
        assert!(serialized[0]["content"][0].get("file_name").is_none());
        assert_eq!(serialized[0]["content"][1]["type"], "text");
        assert_eq!(serialized[0]["content"][1]["text"], "hello there");
    }

    #[test]
    fn user_turn_message_prefers_inline_images_without_duplication() {
        let image_path =
            write_data_url_to_temp_file(TEST_IMAGE_DATA_URL).expect("test image should exist");
        let image_paths = vec![image_path.clone()];
        let inline_images = vec![TEST_IMAGE_DATA_URL.to_string()];
        let message = build_user_turn_message("hello there", &image_paths, &inline_images);
        let serialized = serde_json::to_value(&message).unwrap();
        fs::remove_file(&image_path).ok();

        assert_eq!(serialized["role"], "user");
        assert_eq!(serialized["content"].as_array().unwrap().len(), 2);
        assert_eq!(serialized["content"][0]["type"], "image_url");
        assert_eq!(
            serialized["content"][0]["image_url"]["url"],
            TEST_IMAGE_DATA_URL
        );
        assert_eq!(serialized["content"][1]["type"], "text");
        assert_eq!(serialized["content"][1]["text"], "hello there");
    }

    #[test]
    fn llm_image_history_limit_keeps_only_latest_images() {
        let mut conversation_turn_image_urls = vec![
            vec!["turn-1-image-1".to_string(), "turn-1-image-2".to_string()],
            vec!["turn-2-image-1".to_string()],
            vec!["turn-3-image-1".to_string(), "turn-3-image-2".to_string()],
        ];
        let mut latest_image_urls = vec!["latest-image-1".to_string()];

        apply_llm_image_history_limit(
            &mut conversation_turn_image_urls,
            &mut latest_image_urls,
            Some(3),
        );

        assert!(conversation_turn_image_urls[0].is_empty());
        assert!(conversation_turn_image_urls[1].is_empty());
        assert_eq!(
            conversation_turn_image_urls[2],
            vec!["turn-3-image-1".to_string(), "turn-3-image-2".to_string()]
        );
        assert_eq!(latest_image_urls, vec!["latest-image-1".to_string()]);
    }

    #[test]
    fn llm_system_prompt_appends_context_only_when_needed() {
        let base_prompt = "Base prompt";

        assert_eq!(
            build_llm_system_prompt(base_prompt, false, false),
            "Base prompt"
        );
        assert_eq!(
            build_llm_system_prompt(base_prompt, true, false),
            format!("Base prompt\n\n{AUDIO_CONTEXT_SYSTEM_PROMPT}")
        );
        assert_eq!(
            build_llm_system_prompt(base_prompt, false, true),
            format!("Base prompt\n\n{IMAGE_CONTEXT_SYSTEM_PROMPT}")
        );
    }

    #[test]
    fn playback_echo_suppression_removes_reference_only_audio() {
        let playback_reference = vec![0.25, -0.2, 0.1, -0.05, 0.15, -0.1];
        let payload = AudioPayload {
            data: playback_reference
                .iter()
                .map(|sample| sample * 0.8)
                .collect(),
            sample_rate: Some(48_000),
            playback_reference: Some(playback_reference),
            playback_active: true,
        };

        let prepared_chunk = suppress_playback_echo(payload);

        assert!(prepared_chunk.rms < 0.001);
        assert!(prepared_chunk
            .samples
            .iter()
            .all(|sample| sample.abs() < 0.001));
    }

    #[test]
    fn playback_echo_suppression_keeps_near_end_speech_energy() {
        let playback_reference = vec![0.2, -0.18, 0.12, -0.08, 0.1, -0.06];
        let user_voice = [0.04, 0.03, -0.02, -0.05, 0.01, 0.02];
        let payload = AudioPayload {
            data: playback_reference
                .iter()
                .zip(user_voice.iter())
                .map(|(playback_sample, user_sample)| playback_sample * 0.75 + user_sample)
                .collect(),
            sample_rate: Some(48_000),
            playback_reference: Some(playback_reference),
            playback_active: true,
        };

        let prepared_chunk = suppress_playback_echo(payload);

        assert!(prepared_chunk.rms > 0.02);
        assert!(prepared_chunk
            .samples
            .iter()
            .zip(user_voice.iter())
            .all(|(sample, expected)| (sample - expected).abs() < 0.02));
    }

    #[test]
    fn capture_sample_rate_uses_valid_payload_rate() {
        assert_eq!(resolve_capture_sample_rate(Some(48_000)), 48_000);
    }

    #[test]
    fn capture_sample_rate_falls_back_when_missing_or_invalid() {
        assert_eq!(resolve_capture_sample_rate(None), DEFAULT_SAMPLE_RATE);
        assert_eq!(resolve_capture_sample_rate(Some(0)), DEFAULT_SAMPLE_RATE);
        assert_eq!(
            resolve_capture_sample_rate(Some(500_000)),
            DEFAULT_SAMPLE_RATE
        );
    }
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
            let mut loaded_csm_model_guard = state.loaded_csm_model.lock().unwrap();
            *loaded_csm_model_guard = None;
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
    let process = take_csm_process(state);
    reset_csm_runtime_state(state);

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

fn take_csm_process(state: &AppState) -> Option<CsmProcess> {
    let mut csm_process_guard = state.csm_process.lock().unwrap();
    csm_process_guard.take()
}

fn reset_csm_runtime_state(state: &AppState) {
    {
        let mut loaded_csm_model_guard = state.loaded_csm_model.lock().unwrap();
        *loaded_csm_model_guard = None;
    }
    {
        let mut csm_ready_guard = state.csm_ready.lock().unwrap();
        *csm_ready_guard = false;
    }
    reset_csm_startup_state(state);
}

fn stop_csm_server_for_exit(state: &AppState) {
    let process = take_csm_process(state);
    reset_csm_runtime_state(state);

    let Some(process) = process else {
        return;
    };

    let mut child = process.child.blocking_lock();
    if let Err(err) = child.start_kill() {
        warn!("Failed to stop CSM worker during app exit: {}", err);
    }
}

async fn stt_process_is_ready(state: &AppState) -> bool {
    let process = {
        let stt_process_guard = state.stt_process.lock().unwrap();
        stt_process_guard.clone()
    };

    let Some(process) = process else {
        let mut stt_ready_guard = state.stt_ready.lock().unwrap();
        *stt_ready_guard = false;
        return false;
    };

    let mut child = process.child.lock().await;
    match child.try_wait() {
        Ok(None) => {
            let stt_ready_guard = state.stt_ready.lock().unwrap();
            *stt_ready_guard
        }
        Ok(Some(status)) => {
            warn!("STT worker exited with status {}", status);
            drop(child);
            let mut stt_process_guard = state.stt_process.lock().unwrap();
            *stt_process_guard = None;
            let mut loaded_stt_model_guard = state.loaded_stt_model.lock().unwrap();
            *loaded_stt_model_guard = None;
            let mut stt_ready_guard = state.stt_ready.lock().unwrap();
            *stt_ready_guard = false;
            reset_stt_startup_state(state);
            false
        }
        Err(err) => {
            error!("Failed to check STT worker status: {}", err);
            false
        }
    }
}

async fn stop_stt_server_inner(state: &AppState) -> Result<(), String> {
    let process = take_stt_process(state);
    reset_stt_runtime_state(state);

    let Some(process) = process else {
        return Ok(());
    };

    {
        let mut stdin = process.stdin.lock().await;
        let _ = stdin.write_all(br#"{"type":"shutdown"}"#).await;
        let _ = stdin.write_all(b"\n").await;
        let _ = stdin.flush().await;
    }

    fail_pending_stt_requests(
        process.pending_requests.clone(),
        "The STT worker was stopped before returning a result.".to_string(),
    )
    .await;

    let mut child = process.child.lock().await;
    if let Err(err) = child.kill().await {
        warn!("Failed to kill the STT worker cleanly: {}", err);
    }

    Ok(())
}

fn take_stt_process(state: &AppState) -> Option<SttProcess> {
    let mut stt_process_guard = state.stt_process.lock().unwrap();
    stt_process_guard.take()
}

fn reset_stt_runtime_state(state: &AppState) {
    {
        let mut loaded_stt_model_guard = state.loaded_stt_model.lock().unwrap();
        *loaded_stt_model_guard = None;
    }
    {
        let mut stt_ready_guard = state.stt_ready.lock().unwrap();
        *stt_ready_guard = false;
    }
    reset_stt_startup_state(state);
}

fn stop_stt_server_for_exit(state: &AppState) {
    let process = take_stt_process(state);
    reset_stt_runtime_state(state);

    let Some(process) = process else {
        return;
    };

    {
        let mut pending_requests = process.pending_requests.blocking_lock();
        for (_, sender) in pending_requests.drain() {
            let _ = sender.send(Err(
                "The STT worker stopped while the app was exiting.".to_string()
            ));
        }
    }

    let mut child = process.child.blocking_lock();
    if let Err(err) = child.start_kill() {
        warn!("Failed to stop the STT worker during app exit: {}", err);
    }
}

fn cleanup_before_app_exit(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    *state.call_in_progress.lock().unwrap() = false;
    *state.call_muted.lock().unwrap() = false;
    cancel_auto_continue_timer(state.inner());
    clear_call_timer_state(state.inner());
    clear_pending_screen_capture_inner(app_handle, false);
    stop_server_inner(state.inner()).unwrap_or_else(|err| {
        warn!("Failed to stop MLX server during app exit: {}", err);
    });
    stop_csm_server_for_exit(state.inner());
    stop_stt_server_for_exit(state.inner());
}

fn request_app_quit(app_handle: &AppHandle) {
    {
        let state = app_handle.state::<AppState>();
        let mut quitting_guard = state.is_quitting.lock().unwrap();
        *quitting_guard = true;
    }

    cleanup_before_app_exit(app_handle);
    app_handle.exit(0);
}

#[cfg(target_os = "macos")]
fn refresh_tray_menu(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    let call_in_progress = *state.call_in_progress.lock().unwrap();
    let call_muted = *state.call_muted.lock().unwrap();
    let tray_pong_playback_enabled = *state.tray_pong_playback_enabled.lock().unwrap();
    let screen_capture_in_progress = *state.screen_capture_in_progress.lock().unwrap();
    let has_pending_screen_capture = has_pending_screen_capture(state.inner());
    let has_conversation_image_history = has_conversation_image_history(state.inner())
        || *state.conversation_log_has_visible_images.lock().unwrap();
    let gemma_loaded = loaded_gemma_variant(state.inner()).is_some();
    let stt_loaded = loaded_stt_model(state.inner()).is_some();
    let csm_loaded = loaded_csm_model(state.inner()).is_some();
    let any_models_loaded = gemma_loaded || stt_loaded || csm_loaded;
    let memory_snapshot = match loaded_model_memory_snapshot(state.inner()) {
        Ok(snapshot) => Some(snapshot),
        Err(err) => {
            error!("Failed to build tray memory snapshot: {}", err);
            None
        }
    };

    let summary_text = if let Some(snapshot) = memory_snapshot.as_ref() {
        if snapshot.total_bytes > 0 {
            format!("Memory Used: {}", format_memory_bytes(snapshot.total_bytes))
        } else if any_models_loaded {
            "Memory Used: unavailable".to_string()
        } else {
            "No models loaded".to_string()
        }
    } else if any_models_loaded {
        "Memory Used: unavailable".to_string()
    } else {
        "No models loaded".to_string()
    };
    let region_shortcut_str = state
        .global_shortcut_look_at_screen_region
        .lock()
        .unwrap()
        .clone();
    let entire_shortcut_str = state
        .global_shortcut_look_at_entire_screen
        .lock()
        .unwrap()
        .clone();
    let toggle_mute_shortcut_str = state.global_shortcut_toggle_mute.lock().unwrap().clone();
    let interrupt_shortcut_str = state.global_shortcut_interrupt.lock().unwrap().clone();

    let menu_state = TrayMenuState {
        call_in_progress,
        call_muted,
        tray_pong_playback_enabled,
        screen_capture_in_progress,
        has_pending_screen_capture,
        has_conversation_image_history,
        gemma_loaded,
        stt_loaded,
        csm_loaded,
        memory_snapshot_summary: summary_text.clone(),
        region_shortcut: region_shortcut_str.clone(),
        entire_screen_shortcut: entire_shortcut_str.clone(),
        toggle_mute_shortcut: toggle_mute_shortcut_str.clone(),
        interrupt_shortcut: interrupt_shortcut_str.clone(),
    };

    {
        let mut last_menu_state_guard = state.last_tray_menu_state.lock().unwrap();
        if *last_menu_state_guard == Some(menu_state.clone()) {
            return;
        }
        *last_menu_state_guard = Some(menu_state);
    }

    let mut builder = MenuBuilder::new(app_handle).text(TRAY_SHOW_MENU_ID, "Show OpenDuck");
    if call_in_progress {
        let look_at_screen_label = if screen_capture_in_progress {
            "Selecting Screen...".to_string()
        } else {
            format!("Look at Screen Region ({})", region_shortcut_str)
        };
        let look_at_screen_item =
            match MenuItemBuilder::with_id(TRAY_LOOK_AT_SCREEN_MENU_ID, look_at_screen_label)
                .enabled(!screen_capture_in_progress)
                .build(app_handle)
            {
                Ok(item) => item,
                Err(err) => {
                    error!("Failed to build tray look-at-screen item: {}", err);
                    return;
                }
            };
        builder = builder.item(&look_at_screen_item);

        let look_at_entire_screen_item = match MenuItemBuilder::with_id(
            TRAY_LOOK_AT_ENTIRE_SCREEN_MENU_ID,
            format!("Look at Entire Screen ({})", entire_shortcut_str),
        )
        .enabled(!screen_capture_in_progress)
        .build(app_handle)
        {
            Ok(item) => item,
            Err(err) => {
                error!("Failed to build tray look-at-entire-screen item: {}", err);
                return;
            }
        };
        builder = builder.item(&look_at_entire_screen_item);

        if has_pending_screen_capture {
            let attached_item = match MenuItemBuilder::with_id(
                TRAY_SCREEN_CAPTURE_STATUS_MENU_ID,
                "Screen Region Attached",
            )
            .enabled(false)
            .build(app_handle)
            {
                Ok(item) => item,
                Err(err) => {
                    error!(
                        "Failed to build tray screen attachment status item: {}",
                        err
                    );
                    return;
                }
            };
            builder = builder.item(&attached_item);

            if let Some(file_name) = pending_screen_capture_file_name(state.inner()) {
                let file_item = match MenuItemBuilder::with_id(
                    TRAY_SCREEN_CAPTURE_FILE_MENU_ID,
                    truncate_tray_label(&file_name, 38),
                )
                .enabled(false)
                .build(app_handle)
                {
                    Ok(item) => item,
                    Err(err) => {
                        error!("Failed to build tray screen attachment file item: {}", err);
                        return;
                    }
                };
                builder = builder.item(&file_item);
            }

            builder = builder.text(TRAY_CLEAR_SCREEN_CAPTURE_MENU_ID, "Clear Screen Region");
        }
    }
    builder = builder.separator();

    let summary_item = match MenuItemBuilder::with_id(TRAY_MEMORY_SUMMARY_MENU_ID, summary_text)
        .enabled(false)
        .build(app_handle)
    {
        Ok(item) => item,
        Err(err) => {
            error!("Failed to build tray memory summary item: {}", err);
            return;
        }
    };
    builder = builder.item(&summary_item);

    if let Some(snapshot) = memory_snapshot.as_ref() {
        for model in &snapshot.models {
            let item_id = match model.key.as_str() {
                "gemma" => TRAY_MEMORY_GEMMA_MENU_ID,
                "stt" => TRAY_MEMORY_STT_MENU_ID,
                "csm" => TRAY_MEMORY_CSM_MENU_ID,
                _ => continue,
            };

            let mut item_text = model.label.clone();
            if let Some(detail) = &model.detail {
                item_text.push_str(&format!(" ({detail})"));
            }

            if model.bytes > 0 {
                item_text.push_str(&format!(": {}", format_memory_bytes(model.bytes)));
            } else {
                item_text.push_str(": external");
            }

            let memory_item = match MenuItemBuilder::with_id(item_id, item_text)
                .enabled(false)
                .build(app_handle)
            {
                Ok(item) => item,
                Err(err) => {
                    error!("Failed to build tray memory item: {}", err);
                    return;
                }
            };
            builder = builder.item(&memory_item);
        }
    }

    if call_in_progress {
        let mute_label = if call_muted {
            format!("Unmute ({})", toggle_mute_shortcut_str)
        } else {
            format!("Mute ({})", toggle_mute_shortcut_str)
        };
        let clear_image_history_item =
            match MenuItemBuilder::with_id(TRAY_CLEAR_IMAGE_HISTORY_MENU_ID, "Clear Image History")
                .enabled(has_conversation_image_history)
                .build(app_handle)
            {
                Ok(item) => item,
                Err(err) => {
                    error!("Failed to build tray clear-image-history item: {}", err);
                    return;
                }
            };
        builder = builder.separator();
        builder = builder
            .text(TRAY_INTERRUPT_TTS_MENU_ID, format!("Interrupt ({})", interrupt_shortcut_str))
            .item(&clear_image_history_item)
            .text(TRAY_END_CALL_MENU_ID, "End Call")
            .text(TRAY_TOGGLE_MUTE_MENU_ID, mute_label);
    } else if any_models_loaded {
        builder = builder.separator();

        if gemma_loaded
            && !loaded_gemma_variant(state.inner())
                .map(|variant| variant.is_external())
                .unwrap_or(false)
        {
            builder = builder.text(TRAY_UNLOAD_GEMMA_MENU_ID, "Unload Gemma");
        }
        if stt_loaded {
            builder = builder.text(TRAY_UNLOAD_STT_MENU_ID, "Unload STT");
        }
        if csm_loaded {
            builder = builder.text(TRAY_UNLOAD_CSM_MENU_ID, "Unload TTS");
        }

        let unload_all_item =
            match MenuItemBuilder::with_id(TRAY_UNLOAD_ALL_MODELS_MENU_ID, "Unload All Models")
                .enabled(any_models_loaded)
                .build(app_handle)
            {
                Ok(item) => item,
                Err(err) => {
                    error!("Failed to build tray unload-all item: {}", err);
                    return;
                }
            };
        builder = builder.item(&unload_all_item);
    }

    let pong_playback_label = if tray_pong_playback_enabled {
        "Disable Pop Sound"
    } else {
        "Enable Pop Sound"
    };
    builder = builder
        .separator()
        .text(TRAY_TOGGLE_PONG_PLAYBACK_MENU_ID, pong_playback_label);

    let menu = match builder
        .separator()
        .text(TRAY_QUIT_MENU_ID, "Quit OpenDuck")
        .build()
    {
        Ok(menu) => menu,
        Err(err) => {
            error!("Failed to build tray menu: {}", err);
            return;
        }
    };

    let Some(tray) = app_handle.tray_by_id(TRAY_ICON_ID) else {
        return;
    };

    if let Err(err) = tray.set_menu(Some(menu)) {
        error!("Failed to update tray menu: {}", err);
    }
}

#[cfg(not(target_os = "macos"))]
fn refresh_tray_menu(_app_handle: &AppHandle) {}

#[cfg(target_os = "macos")]
fn create_tray(app_handle: &AppHandle) -> tauri::Result<()> {
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;

    let menu = MenuBuilder::new(app_handle)
        .text(TRAY_SHOW_MENU_ID, "Show OpenDuck")
        .separator()
        .text(TRAY_QUIT_MENU_ID, "Quit OpenDuck")
        .build()?;

    TrayIconBuilder::with_id(TRAY_ICON_ID)
        .menu(&menu)
        .icon(icon)
        .icon_as_template(true)
        .show_menu_on_left_click(true)
        .tooltip("OpenDuck")
        .on_menu_event(|app_handle, event| match event.id().as_ref() {
            TRAY_SHOW_MENU_ID => {
                if let Err(err) = show_main_window(app_handle) {
                    error!("Failed to show OpenDuck from tray: {}", err);
                }
            }
            TRAY_LOOK_AT_SCREEN_MENU_ID => {
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = capture_screen_selection_inner(&app_handle).await {
                        error!("Failed to capture screen from tray: {}", err);
                    }
                });
            }
            TRAY_LOOK_AT_ENTIRE_SCREEN_MENU_ID => {
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = capture_entire_screen_inner(&app_handle).await {
                        error!("Failed to capture entire screen from tray: {}", err);
                    }
                });
            }
            TRAY_CLEAR_SCREEN_CAPTURE_MENU_ID => {
                clear_pending_screen_capture_inner(app_handle, true);
            }
            TRAY_CLEAR_IMAGE_HISTORY_MENU_ID => {
                let state = app_handle.state::<AppState>();
                let removed_count = clear_conversation_context_images_inner(state.inner());
                set_conversation_log_has_visible_images_state(app_handle, state.inner(), false);
                emit_conversation_image_history_cleared(app_handle);
                if removed_count > 0 {
                    emit_overlay_notification(
                        app_handle,
                        OverlayNotificationEvent {
                            message: "OpenDuck: Cleared Image History".to_string(),
                        },
                    );
                }
                refresh_tray_presentation(app_handle);
            }
            TRAY_INTERRUPT_TTS_MENU_ID => {
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(err) = interrupt_tts(app_handle.clone()).await {
                        error!("Failed to interrupt speech from tray: {}", err);
                    }
                    emit_overlay_notification(
                        &app_handle,
                        OverlayNotificationEvent {
                            message: "OpenDuck: Interrupted".to_string(),
                        },
                    );
                });
            }
            TRAY_END_CALL_MENU_ID => {
                if let Err(err) = app_handle.emit(TRAY_END_CALL_EVENT, ()) {
                    error!("Failed to emit tray end call event: {}", err);
                }
            }
            TRAY_TOGGLE_MUTE_MENU_ID => {
                if let Err(err) = app_handle.emit(TRAY_TOGGLE_MUTE_EVENT, ()) {
                    error!("Failed to emit tray mute toggle event: {}", err);
                }
            }
            TRAY_TOGGLE_PONG_PLAYBACK_MENU_ID => {
                let state = app_handle.state::<AppState>();
                let enabled = {
                    let mut tray_pong_playback_enabled =
                        state.tray_pong_playback_enabled.lock().unwrap();
                    *tray_pong_playback_enabled = !*tray_pong_playback_enabled;
                    *tray_pong_playback_enabled
                };

                if !*state.tray_pong_playback_hydrated.lock().unwrap() {
                    *state
                        .tray_pong_playback_modified_before_hydration
                        .lock()
                        .unwrap() = true;
                }

                refresh_tray_menu(app_handle);
                emit_tray_pong_playback(app_handle, enabled);
            }
            TRAY_UNLOAD_GEMMA_MENU_ID => {
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    if *state.call_in_progress.lock().unwrap() {
                        refresh_tray_presentation(&app_handle);
                        return;
                    }

                    if let Err(err) = stop_server_inner(state.inner()) {
                        error!("Failed to unload Gemma from tray: {}", err);
                    }

                    refresh_tray_presentation(&app_handle);
                });
            }
            TRAY_UNLOAD_STT_MENU_ID => {
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    if *state.call_in_progress.lock().unwrap() {
                        refresh_tray_presentation(&app_handle);
                        return;
                    }

                    if let Err(err) = stop_stt_server_inner(state.inner()).await {
                        error!("Failed to unload STT from tray: {}", err);
                    }

                    refresh_tray_presentation(&app_handle);
                });
            }
            TRAY_UNLOAD_CSM_MENU_ID => {
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    if *state.call_in_progress.lock().unwrap() {
                        refresh_tray_presentation(&app_handle);
                        return;
                    }

                    if let Err(err) = stop_csm_server_inner(state.inner()).await {
                        error!("Failed to unload TTS from tray: {}", err);
                    }

                    refresh_tray_presentation(&app_handle);
                });
            }
            TRAY_UNLOAD_ALL_MODELS_MENU_ID => {
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<AppState>();
                    if *state.call_in_progress.lock().unwrap() {
                        refresh_tray_presentation(&app_handle);
                        return;
                    }

                    if let Err(err) = stop_stt_server_inner(state.inner()).await {
                        error!("Failed to unload STT from tray: {}", err);
                    }
                    if let Err(err) = stop_csm_server_inner(state.inner()).await {
                        error!("Failed to unload TTS from tray: {}", err);
                    }
                    if let Err(err) = stop_server_inner(state.inner()) {
                        error!("Failed to unload Gemma from tray: {}", err);
                    }

                    refresh_tray_presentation(&app_handle);
                });
            }
            TRAY_QUIT_MENU_ID => {
                request_app_quit(app_handle);
            }
            _ => {}
        })
        .build(app_handle)?;

    refresh_tray_presentation(app_handle);

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn create_tray(_app_handle: &AppHandle) -> tauri::Result<()> {
    Ok(())
}

#[cfg(target_os = "macos")]
fn build_app_menu(app_handle: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let app_menu = SubmenuBuilder::new(app_handle, "OpenDuck")
        .text(APP_MENU_ABOUT_MENU_ID, "About OpenDuck")
        .text(APP_MENU_CHECK_FOR_UPDATES_MENU_ID, "Check for Updates...")
        .separator()
        .item(&PredefinedMenuItem::services(app_handle, None)?)
        .separator()
        .item(&PredefinedMenuItem::hide(app_handle, None)?)
        .item(&PredefinedMenuItem::hide_others(app_handle, None)?)
        .separator()
        .item(&PredefinedMenuItem::quit(app_handle, None)?)
        .build()?;

    let file_menu = SubmenuBuilder::new(app_handle, "File")
        .item(&PredefinedMenuItem::close_window(app_handle, None)?)
        .build()?;

    let edit_menu = SubmenuBuilder::new(app_handle, "Edit")
        .item(&PredefinedMenuItem::undo(app_handle, None)?)
        .item(&PredefinedMenuItem::redo(app_handle, None)?)
        .separator()
        .item(&PredefinedMenuItem::cut(app_handle, None)?)
        .item(&PredefinedMenuItem::copy(app_handle, None)?)
        .item(&PredefinedMenuItem::paste(app_handle, None)?)
        .item(&PredefinedMenuItem::select_all(app_handle, None)?)
        .build()?;

    let window_menu = SubmenuBuilder::new(app_handle, "Window")
        .item(&PredefinedMenuItem::minimize(app_handle, None)?)
        .item(&PredefinedMenuItem::maximize(app_handle, None)?)
        .separator()
        .item(&PredefinedMenuItem::close_window(app_handle, None)?)
        .build()?;

    Menu::with_items(
        app_handle,
        &[&app_menu, &file_menu, &edit_menu, &window_menu],
    )
}

#[cfg(not(target_os = "macos"))]
fn build_app_menu(app_handle: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    Menu::default(app_handle)
}

fn handle_app_menu_event(app_handle: &AppHandle, event_id: &str) {
    if event_id != APP_MENU_ABOUT_MENU_ID && event_id != APP_MENU_CHECK_FOR_UPDATES_MENU_ID {
        return;
    }

    if let Err(err) = show_main_window(app_handle) {
        error!(
            "Failed to show OpenDuck before handling app menu action: {}",
            err
        );
        return;
    }

    if event_id == APP_MENU_ABOUT_MENU_ID {
        emit_show_about_modal(app_handle);
        return;
    }

    emit_trigger_app_update_check(app_handle);
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
            Ok(CsmWorkerEvent::Timing {
                request_id,
                text,
                elapsed_ms,
            }) => {
                info!(
                    "CSM synthesis completed in {:.1} ms for request {}: {}",
                    elapsed_ms, request_id, text
                );
            }
            Ok(CsmWorkerEvent::Chunk {
                request_id,
                audio_wav_base64,
            }) => {
                log_processing_audio_latency_for_first_chunk(&app_handle, request_id);
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
                warn!("Ignoring non-JSON speech worker stdout: {} ({})", err, line);
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
    {
        let mut loaded_csm_model_guard = state.loaded_csm_model.lock().unwrap();
        *loaded_csm_model_guard = None;
    }

    send_ready_signal(
        &ready_tx,
        Err(csm_startup_failure_message(
            state.inner(),
            "CSM worker stopped before completing startup",
        )),
    );
    refresh_tray_presentation(&app_handle);
}

async fn csm_stderr_task(app_handle: tauri::AppHandle, stderr: ChildStderr) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let preview = if line.chars().count() > 512 {
            let truncated = line.chars().take(512).collect::<String>();
            format!(
                "{truncated}...[truncated {} chars]",
                line.chars().count() - 512
            )
        } else {
            line.clone()
        };
        error!("CSM worker stderr: {}", preview);
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

async fn stt_stdout_task(
    app_handle: tauri::AppHandle,
    stdout: ChildStdout,
    pending_requests: Arc<AsyncMutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
    ready_tx: Arc<Mutex<Option<oneshot::Sender<Result<(), String>>>>>,
) {
    let mut lines = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<SttWorkerEvent>(&line) {
            Ok(SttWorkerEvent::Status { message }) => {
                info!("STT worker status: {}", message);
                update_stt_startup_message(&app_handle, Some(message), true);
            }
            Ok(SttWorkerEvent::Ready {}) => {
                info!("STT worker ready");
                let state = app_handle.state::<AppState>();
                let mut stt_ready_guard = state.stt_ready.lock().unwrap();
                *stt_ready_guard = true;
                update_stt_startup_message(&app_handle, None, false);
                send_ready_signal(&ready_tx, Ok(()));
            }
            Ok(SttWorkerEvent::Transcription { request_id, text }) => {
                if let Some(sender) = pending_requests.lock().await.remove(&request_id) {
                    let _ = sender.send(Ok(text));
                } else {
                    warn!(
                        "Ignoring STT transcription for unknown request {}",
                        request_id
                    );
                }
            }
            Ok(SttWorkerEvent::Error {
                request_id,
                message,
            }) => {
                error!("STT worker error: {}", message);
                update_stt_startup_message(&app_handle, Some(message.clone()), true);

                if let Some(request_id) = request_id {
                    if let Some(sender) = pending_requests.lock().await.remove(&request_id) {
                        let _ = sender.send(Err(message));
                    } else {
                        warn!("Ignoring STT error for unknown request {}", request_id);
                    }
                } else {
                    send_ready_signal(&ready_tx, Err(message));
                }
            }
            Err(err) => {
                warn!("Ignoring non-JSON STT worker stdout: {} ({})", err, line);
            }
        }
    }

    let failure_message = {
        let state = app_handle.state::<AppState>();
        {
            let mut stt_ready_guard = state.stt_ready.lock().unwrap();
            *stt_ready_guard = false;
        }
        {
            let mut stt_process_guard = state.stt_process.lock().unwrap();
            *stt_process_guard = None;
        }
        {
            let mut loaded_stt_model_guard = state.loaded_stt_model.lock().unwrap();
            *loaded_stt_model_guard = None;
        }

        stt_startup_failure_message(
            state.inner(),
            "STT worker stopped before completing startup",
        )
    };

    fail_pending_stt_requests(pending_requests, failure_message.clone()).await;
    send_ready_signal(&ready_tx, Err(failure_message));
    refresh_tray_presentation(&app_handle);
}

async fn stt_stderr_task(app_handle: tauri::AppHandle, stderr: ChildStderr) {
    let mut lines = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let preview = if line.chars().count() > 512 {
            let truncated = line.chars().take(512).collect::<String>();
            format!(
                "{truncated}...[truncated {} chars]",
                line.chars().count() - 512
            )
        } else {
            line.clone()
        };
        error!("STT worker stderr: {}", preview);
        push_stt_stderr_line(app_handle.state::<AppState>().inner(), line);
    }
}

async fn stt_exit_monitor(
    app_handle: tauri::AppHandle,
    child: Arc<AsyncMutex<Child>>,
    ready_tx: Arc<Mutex<Option<oneshot::Sender<Result<(), String>>>>>,
) {
    loop {
        {
            let state = app_handle.state::<AppState>();
            let stt_ready_guard = state.stt_ready.lock().unwrap();
            if *stt_ready_guard {
                return;
            }
        }

        let exit_status = {
            let mut child_guard = child.lock().await;
            match child_guard.try_wait() {
                Ok(status) => status,
                Err(err) => {
                    error!("Failed while waiting for STT worker startup: {}", err);
                    send_ready_signal(
                        &ready_tx,
                        Err(stt_startup_failure_message(
                            app_handle.state::<AppState>().inner(),
                            &format!("Failed to inspect the STT worker process: {err}"),
                        )),
                    );
                    return;
                }
            }
        };

        if let Some(status) = exit_status {
            send_ready_signal(
                &ready_tx,
                Err(stt_startup_failure_message(
                    app_handle.state::<AppState>().inner(),
                    &format!("STT worker exited with status {status}"),
                )),
            );
            return;
        }

        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
}

async fn fail_pending_stt_requests(
    pending_requests: Arc<AsyncMutex<HashMap<u64, oneshot::Sender<Result<String, String>>>>>,
    message: String,
) {
    let mut pending_requests_guard = pending_requests.lock().await;
    for (_, sender) in pending_requests_guard.drain() {
        let _ = sender.send(Err(message.clone()));
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

async fn interrupt_active_generation(app_handle: &tauri::AppHandle) {
    cancel_active_generation(app_handle, true).await;
}

async fn cancel_active_generation(app_handle: &tauri::AppHandle, stop_csm_worker: bool) {
    let state = app_handle.state::<AppState>();
    let active_generation = {
        let mut active_generation_guard = state.active_generation.lock().unwrap();
        active_generation_guard.take()
    };
    let had_active_generation = active_generation.is_some();

    if let Some(active_generation) = active_generation {
        info!("Interrupting active generation {}", active_generation.id);
        active_generation
            .cancellation_token
            .store(true, Ordering::Relaxed);
        // We give it a moment to handle cancellation gracefully (e.g. saving partial history)
        // but we still abort to ensure it doesn't hang forever if the loop is blocked.
        let handle = active_generation.handle;
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            handle.abort();
        });
    } else {
        // If no active generation, check if we should mark the last turn as interrupted
        // (this covers cases where LLM finished but TTS was still playing)
        let mut turns = state.conversation_turns.lock().unwrap();
        if let Some(last_turn) = turns.back_mut() {
            if !last_turn.assistant_text.contains("[Interrupted]") {
                info!("Marking last assistant turn as interrupted in history");
                drop(turns);
                let _ = save_current_session(app_handle, state.inner());
            }
        }
    }

    set_tts_playback_active_state(app_handle, false);
    emit_csm_audio_stop(app_handle);

    if stop_csm_worker && had_active_generation {
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

fn set_tts_playback_active_state(app_handle: &AppHandle, active: bool) {
    let state = app_handle.state::<AppState>();
    let changed = {
        let mut tts_playback_active_guard = state.tts_playback_active.lock().unwrap();
        if *tts_playback_active_guard == active {
            false
        } else {
            *tts_playback_active_guard = active;
            true
        }
    };

    if !changed {
        return;
    }

    cancel_auto_continue_timer(state.inner());
    if !active {
        maybe_schedule_auto_continue_after_tts_idle(app_handle);
    }
}

fn cancel_auto_continue_timer(state: &AppState) {
    state
        .auto_continue_timer_generation
        .fetch_add(1, Ordering::Relaxed);
}

fn reset_auto_continue_tracker(state: &AppState) {
    *state.auto_continue_tracker.lock().unwrap() = None;
}

fn mark_latest_assistant_turn_auto_continue_consumed(state: &AppState) {
    let latest_assistant_entry_id = state
        .conversation_turns
        .lock()
        .unwrap()
        .back()
        .map(|turn| turn.assistant_entry_id);
    let mut tracker = state.auto_continue_tracker.lock().unwrap();
    *tracker = latest_assistant_entry_id.map(|assistant_entry_id| match *tracker {
        Some(existing) if existing.assistant_entry_id == assistant_entry_id => {
            AutoContinueTracker {
                assistant_entry_id,
                continuation_count: existing.continuation_count,
                blocked: true,
            }
        }
        _ => AutoContinueTracker {
            assistant_entry_id,
            continuation_count: 0,
            blocked: true,
        },
    });
}

fn maybe_schedule_auto_continue_after_tts_idle(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    let Some(delay_ms) = *state.auto_continue_silence_ms.lock().unwrap() else {
        return;
    };

    if !*state.call_in_progress.lock().unwrap() {
        return;
    }

    if *state.tts_playback_active.lock().unwrap() || *state.is_speaking.lock().unwrap() {
        return;
    }

    if state.active_generation.lock().unwrap().is_some() {
        return;
    }

    let Some(last_turn) = state.conversation_turns.lock().unwrap().back().cloned() else {
        return;
    };

    if last_turn.assistant_text.trim().is_empty() {
        return;
    }

    let configured_max_count = *state.auto_continue_max_count.lock().unwrap();
    if let Some(tracker) = *state.auto_continue_tracker.lock().unwrap() {
        if tracker.assistant_entry_id == last_turn.assistant_entry_id {
            if tracker.blocked {
                return;
            }

            if let Some(max_count) = configured_max_count {
                if tracker.continuation_count >= max_count {
                    return;
                }
            }
        }
    }

    let generation = state
        .auto_continue_timer_generation
        .fetch_add(1, Ordering::Relaxed)
        + 1;
    let conversation_session_id = current_conversation_session_id(state.inner());
    let assistant_entry_id = last_turn.assistant_entry_id;
    let app_handle = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(u64::from(delay_ms))).await;

        let state = app_handle.state::<AppState>();
        if state.auto_continue_timer_generation.load(Ordering::Relaxed) != generation {
            return;
        }

        if !*state.call_in_progress.lock().unwrap() {
            return;
        }

        if *state.tts_playback_active.lock().unwrap() || *state.is_speaking.lock().unwrap() {
            return;
        }

        if state.active_generation.lock().unwrap().is_some() {
            return;
        }

        if current_conversation_session_id(state.inner()) != conversation_session_id {
            return;
        }

        let current_last_assistant_entry_id = state
            .conversation_turns
            .lock()
            .unwrap()
            .back()
            .map(|turn| turn.assistant_entry_id);
        if current_last_assistant_entry_id != Some(assistant_entry_id) {
            return;
        }

        start_assistant_auto_continue_generation(&app_handle, assistant_entry_id);
    });
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
    cancellation_token: Arc<AtomicBool>,
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
        previous_generation
            .cancellation_token
            .store(true, Ordering::Relaxed);
        previous_generation.handle.abort();
    }

    *active_generation_guard = Some(ActiveGeneration {
        id: generation_id,
        handle,
        cancellation_token,
    });
    true
}

fn unregister_conversation_image_path(state: &AppState, image_path: &Path) {
    let mut image_paths = state.conversation_image_paths.lock().unwrap();
    if let Some(index) = image_paths
        .iter()
        .position(|candidate| candidate == image_path)
    {
        image_paths.remove(index);
    }
}

fn append_conversation_turn(
    state: &AppState,
    user_text: String,
    assistant_text: String,
    image_paths: Vec<PathBuf>,
    user_image_data_urls: Vec<String>,
) -> (u64, u64) {
    reset_auto_continue_tracker(state);

    {
        let mut conversation_image_paths = state.conversation_image_paths.lock().unwrap();
        for image_path in &image_paths {
            conversation_image_paths.push(image_path.clone());
        }
    }

    let user_entry_id = state
        .next_conversation_entry_id
        .fetch_add(1, Ordering::Relaxed);
    let assistant_entry_id = state
        .next_conversation_entry_id
        .fetch_add(1, Ordering::Relaxed);
    let mut turns = state.conversation_turns.lock().unwrap();
    turns.push_back(ConversationTurn {
        user_entry_id,
        assistant_entry_id,
        user_text,
        assistant_text,
        image_paths,
        user_image_data_urls,
        image_path: None,
        user_image_data_url: None,
    });

    while turns.len() > MAX_CONVERSATION_TURNS {
        if let Some(removed_turn) = turns.pop_front() {
            for removed_image_path in removed_turn.image_paths {
                unregister_conversation_image_path(state, &removed_image_path);
                remove_temp_image_file(&removed_image_path);
            }
        }
    }

    (user_entry_id, assistant_entry_id)
}

fn append_conversation_turn_with_save(
    app_handle: &AppHandle,
    state: &AppState,
    user_text: String,
    assistant_text: String,
    image_paths: Vec<PathBuf>,
) -> (u64, u64) {
    let mut final_image_paths = Vec::new();
    let mut user_image_data_urls = Vec::new();

    for path in image_paths {
        let mut current_path = path;
        if let Some(data_url) = load_image_data_url(&current_path) {
            user_image_data_urls.push(data_url);
        }

        if let Ok(session_id) = state
            .current_session_id
            .lock()
            .unwrap()
            .as_ref()
            .ok_or("No session")
        {
            if let Ok(images_dir) = resolve_session_images_dir(app_handle, session_id) {
                if let Some(file_name) = current_path.file_name() {
                    let dest_path = images_dir.join(file_name);
                    if let Err(err) = std::fs::rename(&current_path, &dest_path) {
                        error!("Failed to move image to session directory: {}", err);
                    } else {
                        current_path = dest_path;
                    }
                }
            }
        }
        final_image_paths.push(current_path);
    }

    let res = append_conversation_turn(
        state,
        user_text,
        assistant_text,
        final_image_paths,
        user_image_data_urls,
    );

    // Auto-title if it's the first turn and no title is set
    {
        let mut title_guard = state.current_session_title.lock().unwrap();
        if title_guard.is_none() {
            let turns = state.conversation_turns.lock().unwrap();
            if turns.len() == 1 {
                let first_sentence = turns[0]
                    .user_text
                    .split(|c| c == '.' || c == '?' || c == '!')
                    .next()
                    .unwrap_or(&turns[0].user_text)
                    .trim();
                *title_guard = Some(first_sentence.chars().take(100).collect());
            }
        }
    }

    if let Err(err) = save_current_session(app_handle, state) {
        error!("Failed to save session: {}", err);
    }
    res
}

fn append_to_existing_assistant_turn_with_save(
    app_handle: &AppHandle,
    state: &AppState,
    assistant_entry_id: u64,
    continuation_text: String,
) -> Result<(u64, u64, String, String), String> {
    let trimmed_continuation = continuation_text.trim();
    if trimmed_continuation.is_empty() {
        return Err("Assistant continuation cannot be empty.".to_string());
    }

    let updated_turn = {
        let mut turns = state.conversation_turns.lock().unwrap();
        let Some(last_turn) = turns.back_mut() else {
            return Err("No conversation turn is available to continue.".to_string());
        };

        if last_turn.assistant_entry_id != assistant_entry_id {
            return Err(
                "The latest assistant turn changed before auto-continue completed.".to_string(),
            );
        }

        last_turn.assistant_text =
            append_assistant_message_text(&last_turn.assistant_text, trimmed_continuation);
        (
            last_turn.user_entry_id,
            last_turn.assistant_entry_id,
            last_turn.user_text.clone(),
            last_turn.assistant_text.clone(),
        )
    };

    let mut tracker = state.auto_continue_tracker.lock().unwrap();
    *tracker = Some(match *tracker {
        Some(existing) if existing.assistant_entry_id == assistant_entry_id => {
            AutoContinueTracker {
                assistant_entry_id,
                continuation_count: existing.continuation_count.saturating_add(1),
                blocked: false,
            }
        }
        _ => AutoContinueTracker {
            assistant_entry_id,
            continuation_count: 1,
            blocked: false,
        },
    });

    if let Err(err) = save_current_session(app_handle, state) {
        error!(
            "Failed to save session after assistant continuation: {}",
            err
        );
    }

    Ok(updated_turn)
}

fn has_pending_screen_capture(state: &AppState) -> bool {
    !state.pending_screen_captures.lock().unwrap().is_empty()
}

pub(crate) fn pending_screen_capture_file_name(state: &AppState) -> Option<String> {
    state
        .pending_screen_captures
        .lock()
        .unwrap()
        .last()
        .and_then(|path| path.file_name())
        .map(|name| name.to_string_lossy().into_owned())
}

pub(crate) fn pending_screen_capture_count(state: &AppState) -> usize {
    state.pending_screen_captures.lock().unwrap().len()
}

fn truncate_tray_label(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }

    if max_chars <= 3 {
        return "...".chars().take(max_chars).collect();
    }

    let prefix_chars = (max_chars - 3) / 2;
    let suffix_chars = max_chars - 3 - prefix_chars;
    let prefix: String = text.chars().take(prefix_chars).collect();
    let suffix: String = text
        .chars()
        .rev()
        .take(suffix_chars)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{prefix}...{suffix}")
}

fn add_pending_screen_capture(state: &AppState, path: PathBuf) {
    state.pending_screen_captures.lock().unwrap().push(path);
}

fn take_pending_screen_captures(state: &AppState) -> Vec<PathBuf> {
    std::mem::take(&mut *state.pending_screen_captures.lock().unwrap())
}

fn clear_pending_screen_capture_state(state: &AppState) -> bool {
    let previous = std::mem::take(&mut *state.pending_screen_captures.lock().unwrap());
    let had_pending = !previous.is_empty();
    for previous_path in previous {
        remove_temp_image_file(&previous_path);
    }
    had_pending
}

fn current_conversation_session_id(state: &AppState) -> u64 {
    state.conversation_session_id.load(Ordering::Relaxed)
}

fn reset_call_session_state(state: &AppState) {
    cancel_auto_continue_timer(state);

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
        let mut current_utterance_voiced_samples =
            state.current_utterance_voiced_samples.lock().unwrap();
        *current_utterance_voiced_samples = 0;
    }
    {
        let mut is_speaking = state.is_speaking.lock().unwrap();
        *is_speaking = false;
    }
    {
        let mut live_transcription = state.live_transcription.lock().unwrap();
        *live_transcription = LiveTranscriptionState::default();
    }
    {
        let mut turns = state.conversation_turns.lock().unwrap();
        turns.clear();
    }
    {
        let mut image_paths = state.conversation_image_paths.lock().unwrap();
        for image_path in image_paths.drain(..) {
            remove_temp_image_file(&image_path);
        }
    }
    state.next_conversation_entry_id.store(1, Ordering::Relaxed);
    reset_auto_continue_tracker(state);

    {
        let mut id_guard = state.current_session_id.lock().unwrap();
        *id_guard = None;
    }
    {
        let mut title_guard = state.current_session_title.lock().unwrap();
        *title_guard = None;
    }

    state
        .conversation_session_id
        .fetch_add(1, Ordering::Relaxed);
}

#[cfg(target_os = "macos")]
fn tray_icon_image(variant: TrayIconVariant) -> tauri::Result<tauri::image::Image<'static>> {
    let bytes = match variant {
        TrayIconVariant::Muted => include_bytes!("../icons/tray-template-muted.png").as_slice(),
        TrayIconVariant::Default => include_bytes!("../icons/tray-template.png").as_slice(),
        TrayIconVariant::Listening => {
            include_bytes!("../icons/tray-template-listening.png").as_slice()
        }
        TrayIconVariant::Processing => {
            include_bytes!("../icons/tray-template-processing.png").as_slice()
        }
        TrayIconVariant::Thinking => {
            include_bytes!("../icons/tray-template-thinking.png").as_slice()
        }
        TrayIconVariant::ImagePasted => {
            include_bytes!("../icons/tray-template-image-pasted.png").as_slice()
        }
    };

    tauri::image::Image::from_bytes(bytes)
}

fn clear_call_timer_state(state: &AppState) {
    state.tray_timer_generation.fetch_add(1, Ordering::Relaxed);
    state
        .tray_title_override_generation
        .fetch_add(1, Ordering::Relaxed);
    clear_transient_tray_title(state);
    clear_transient_tray_icon(state);
    let mut started_at_guard = state.call_started_at.lock().unwrap();
    *started_at_guard = None;
}

#[cfg(target_os = "macos")]
fn current_tray_title(app_handle: &AppHandle) -> Option<String> {
    let state = app_handle.state::<AppState>();

    if let Some(title) = state.transient_tray_title.lock().unwrap().clone() {
        return Some(title);
    }

    if state.call_started_at.lock().unwrap().is_some() {
        return None;
    }

    match loaded_model_memory_snapshot(state.inner()) {
        Ok(_) => None,
        Err(err) => {
            error!("Failed to build tray title memory summary: {}", err);
            None
        }
    }
}

#[cfg(target_os = "macos")]
fn update_tray_timer_title(app_handle: &AppHandle, title: Option<&str>) {
    let state = app_handle.state::<AppState>();
    {
        let mut last_title_guard = state.last_tray_title.lock().unwrap();
        if *last_title_guard == title.map(|s| s.to_string()) {
            return;
        }
        *last_title_guard = title.map(|s| s.to_string());
    }

    let Some(tray) = app_handle.tray_by_id(TRAY_ICON_ID) else {
        return;
    };

    if let Err(err) = tray.set_title(title) {
        error!("Failed to update tray title: {}", err);
    }
}

#[cfg(not(target_os = "macos"))]
fn update_tray_timer_title(_app_handle: &AppHandle, _title: Option<&str>) {}

#[cfg(target_os = "macos")]
fn refresh_tray_title(app_handle: &AppHandle) {
    let title = current_tray_title(app_handle);
    update_tray_timer_title(app_handle, title.as_deref());
}

#[cfg(not(target_os = "macos"))]
fn refresh_tray_title(_app_handle: &AppHandle) {}

#[cfg(target_os = "macos")]
fn refresh_tray_icon(app_handle: &AppHandle) {
    let state = app_handle.state::<AppState>();
    let Some(tray) = app_handle.tray_by_id(TRAY_ICON_ID) else {
        return;
    };

    let variant = {
        let transient_icon_guard = state.transient_tray_icon.lock().unwrap();
        if let Some(variant) = *transient_icon_guard {
            variant
        } else if *state.call_muted.lock().unwrap() {
            TrayIconVariant::Muted
        } else {
            let phase = state.call_stage_phase.lock().unwrap();
            match phase.as_str() {
                "listening" => TrayIconVariant::Listening,
                "processing_audio" => TrayIconVariant::Processing,
                "thinking" => TrayIconVariant::Thinking,
                _ => TrayIconVariant::Default,
            }
        }
    };

    {
        let mut last_variant_guard = state.last_tray_icon_variant.lock().unwrap();
        if *last_variant_guard == Some(variant) {
            return;
        }
        *last_variant_guard = Some(variant);
    }

    let icon = match tray_icon_image(variant) {
        Ok(icon) => icon,
        Err(err) => {
            error!("Failed to load tray icon: {}", err);
            return;
        }
    };

    if let Err(err) = tray.set_icon(Some(icon)) {
        error!("Failed to update tray icon: {}", err);
    }
    if let Err(err) = tray.set_icon_as_template(true) {
        error!("Failed to mark tray icon as template: {}", err);
    }
}

#[cfg(not(target_os = "macos"))]
fn refresh_tray_icon(_app_handle: &AppHandle) {}

pub(crate) fn refresh_tray_presentation(app_handle: &AppHandle) {
    refresh_tray_menu(app_handle);
    refresh_tray_icon(app_handle);
    refresh_tray_title(app_handle);
}

fn clear_transient_tray_title(state: &AppState) {
    let mut transient_title_guard = state.transient_tray_title.lock().unwrap();
    *transient_title_guard = None;
}

fn clear_transient_tray_icon(state: &AppState) {
    let mut transient_icon_guard = state.transient_tray_icon.lock().unwrap();
    *transient_icon_guard = None;
}

#[cfg(target_os = "macos")]
fn show_temporary_tray_icon(app_handle: &AppHandle, duration: Duration) {
    let state = app_handle.state::<AppState>();
    let generation = state
        .tray_title_override_generation
        .fetch_add(1, Ordering::Relaxed)
        + 1;

    {
        let mut transient_icon_guard = state.transient_tray_icon.lock().unwrap();
        *transient_icon_guard = Some(TrayIconVariant::ImagePasted);
    }
    refresh_tray_icon(app_handle);

    let app_handle_clone = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(duration).await;

        let state = app_handle_clone.state::<AppState>();
        if state.tray_title_override_generation.load(Ordering::Relaxed) != generation {
            return;
        }

        clear_transient_tray_icon(state.inner());
        refresh_tray_icon(&app_handle_clone);
    });
}

#[cfg(not(target_os = "macos"))]
fn show_temporary_tray_icon(_app_handle: &AppHandle, _duration: Duration) {}

#[allow(dead_code)]
#[cfg(target_os = "macos")]
fn show_temporary_tray_title(app_handle: &AppHandle, title: &str, duration: Duration) {
    let state = app_handle.state::<AppState>();
    let generation = state
        .tray_title_override_generation
        .fetch_add(1, Ordering::Relaxed)
        + 1;

    {
        let mut transient_title_guard = state.transient_tray_title.lock().unwrap();
        *transient_title_guard = Some(title.to_string());
    }

    refresh_tray_title(app_handle);

    let app_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(duration).await;

        let state = app_handle.state::<AppState>();
        if state.tray_title_override_generation.load(Ordering::Relaxed) != generation {
            return;
        }

        clear_transient_tray_title(state.inner());
        refresh_tray_title(&app_handle);
    });
}

#[cfg(not(target_os = "macos"))]
fn show_temporary_tray_title(_app_handle: &AppHandle, _title: &str, _duration: Duration) {}

fn start_call_timer_inner(app_handle: &AppHandle, state: &AppState) {
    let started_at = Instant::now();
    let generation = state.tray_timer_generation.fetch_add(1, Ordering::Relaxed) + 1;

    {
        let mut started_at_guard = state.call_started_at.lock().unwrap();
        *started_at_guard = Some(started_at);
    }

    refresh_tray_title(app_handle);

    let app_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;

            let state = app_handle.state::<AppState>();
            if state.tray_timer_generation.load(Ordering::Relaxed) != generation {
                break;
            }

            if state.call_started_at.lock().unwrap().is_none() {
                break;
            }

            refresh_tray_title(&app_handle);
        }
    });
}

fn stop_call_timer_inner(app_handle: &AppHandle, state: &AppState) {
    clear_call_timer_state(state);
    refresh_tray_title(app_handle);
}

fn show_main_window(app_handle: &AppHandle) -> Result<(), String> {
    let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Err("Failed to find the main OpenDuck window".to_string());
    };

    let _ = window.unminimize();
    window.show().map_err(|err| err.to_string())?;
    window.set_focus().map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn webview_window_is_visible_to_user(window: &tauri::WebviewWindow) -> Result<bool, String> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);

    window
        .with_webview(move |webview| unsafe {
            use objc2_app_kit::{NSWindow, NSWindowOcclusionState};

            let ns_window: &NSWindow = &*webview.ns_window().cast();
            let is_visible_to_user = ns_window.isVisible()
                && !ns_window.isMiniaturized()
                && ns_window
                    .occlusionState()
                    .contains(NSWindowOcclusionState::Visible);
            let _ = tx.send(is_visible_to_user);
        })
        .map_err(|err| err.to_string())?;

    rx.recv()
        .map_err(|err| format!("Failed to query main window visibility: {err}"))
}

#[cfg(not(target_os = "macos"))]
fn webview_window_is_visible_to_user(window: &tauri::WebviewWindow) -> Result<bool, String> {
    let is_visible = window.is_visible().map_err(|err| err.to_string())?;
    let is_minimized = window.is_minimized().map_err(|err| err.to_string())?;
    Ok(is_visible && !is_minimized)
}

#[tauri::command]
fn is_main_window_visible_to_user(app_handle: AppHandle) -> Result<bool, String> {
    let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) else {
        return Err("Failed to find the main OpenDuck window".to_string());
    };

    webview_window_is_visible_to_user(&window)
}

fn clear_pending_screen_capture_inner(app_handle: &AppHandle, emit_event: bool) {
    let state = app_handle.state::<AppState>();
    let had_pending = clear_pending_screen_capture_state(state.inner());
    if emit_event {
        let message = if had_pending {
            "Screen attachment cleared."
        } else {
            "No screen attachment to clear."
        };
        emit_screen_capture_event(app_handle, "cleared", message);
    }
    refresh_tray_presentation(app_handle);
}

async fn capture_screen_selection_inner(app_handle: &AppHandle) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    if !*state.call_in_progress.lock().unwrap() {
        return Err("Look at Screen Region is only available during an active call.".to_string());
    }

    {
        let mut in_progress_guard = state.screen_capture_in_progress.lock().unwrap();
        if *in_progress_guard {
            return Err("A screen capture is already in progress.".to_string());
        }
        *in_progress_guard = true;
    }

    refresh_tray_presentation(app_handle);
    let capture_prompt = if has_pending_screen_capture(state.inner()) {
        "Select a screen region to replace the current attachment."
    } else {
        "Select a screen region to attach to your next turn."
    };
    emit_screen_capture_event(app_handle, "capturing", capture_prompt);

    let capture_result = run_interactive_screen_capture(app_handle).await;

    {
        let mut in_progress_guard = state.screen_capture_in_progress.lock().unwrap();
        *in_progress_guard = false;
    }

    if !*state.call_in_progress.lock().unwrap() {
        if let Ok(Some(path)) = &capture_result {
            remove_temp_image_file(path);
        }
        refresh_tray_presentation(app_handle);
        return Ok(());
    }

    match capture_result {
        Ok(Some(path)) => {
            attach_pending_screen_capture(
                app_handle,
                path,
                "Screen region attached to your next turn.",
            );
            Ok(())
        }
        Ok(None) => {
            let message = if has_pending_screen_capture(state.inner()) {
                "Screen selection cancelled. The previous screen region is still attached."
            } else {
                "Screen selection cancelled."
            };
            emit_screen_capture_event(app_handle, "cancelled", message);
            refresh_tray_presentation(app_handle);
            Ok(())
        }
        Err(err) => {
            let message = if has_pending_screen_capture(state.inner()) {
                format!(
                    "Screen capture failed. The previous screen region is still attached. {err}"
                )
            } else {
                format!("Screen capture failed. {err}")
            };
            emit_screen_capture_event(app_handle, "error", &message);
            refresh_tray_presentation(app_handle);
            Err(err)
        }
    }
}

async fn capture_entire_screen_inner(app_handle: &AppHandle) -> Result<(), String> {
    let state = app_handle.state::<AppState>();
    if !*state.call_in_progress.lock().unwrap() {
        return Err("Look at Entire Screen is only available during an active call.".to_string());
    }

    {
        let mut in_progress_guard = state.screen_capture_in_progress.lock().unwrap();
        if *in_progress_guard {
            return Err("A screen capture is already in progress.".to_string());
        }
        *in_progress_guard = true;
    }

    refresh_tray_presentation(app_handle);
    emit_screen_capture_event(
        app_handle,
        "capturing",
        "Capturing the entire screen to attach to your next turn.",
    );

    let capture_result = run_entire_screen_capture().await;

    {
        let mut in_progress_guard = state.screen_capture_in_progress.lock().unwrap();
        *in_progress_guard = false;
    }

    if !*state.call_in_progress.lock().unwrap() {
        if let Ok(Some(path)) = &capture_result {
            remove_temp_image_file(path);
        }
        refresh_tray_presentation(app_handle);
        return Ok(());
    }

    match capture_result {
        Ok(Some(path)) => {
            attach_pending_screen_capture(
                app_handle,
                path,
                "Entire screen attached to your next turn.",
            );
            Ok(())
        }
        Ok(None) => {
            emit_screen_capture_event(app_handle, "cancelled", "Entire screen capture cancelled.");
            refresh_tray_presentation(app_handle);
            Ok(())
        }
        Err(err) => {
            let message = format!("Entire screen capture failed. {err}");
            emit_screen_capture_event(app_handle, "error", &message);
            refresh_tray_presentation(app_handle);
            Err(err)
        }
    }
}

#[cfg(target_os = "macos")]
async fn run_interactive_screen_capture(app_handle: &AppHandle) -> Result<Option<PathBuf>, String> {
    let capture_path = create_temp_screen_capture_path();

    let child = std::process::Command::new("/usr/sbin/screencapture")
        .args(["-i", "-x"])
        .arg(&capture_path)
        .spawn()
        .map_err(|err| format!("Failed to launch screencapture: {err}"))?;

    {
        let state = app_handle.state::<AppState>();
        let mut child_guard = state.screen_capture_child.lock().unwrap();
        *child_guard = Some(child);
    }

    // We need to wait for the child process to finish.
    // Since we used std::process::Command, we use a loop or spawn_blocking to wait.
    // Actually, it's better to use tokio::process::Command if we want to be truly async,
    // but here we can just use spawn_blocking to wait for the child we just spawned.
    let app_handle_clone = app_handle.clone();

    let output = tauri::async_runtime::spawn_blocking(move || {
        let state = app_handle_clone.state::<AppState>();
        // We need to get the child back to wait for it, or just wait for it if we have it.
        // But we want to be able to kill it from elsewhere.
        // std::process::Child::wait() is blocking.

        let child = {
            let mut child_guard = state.screen_capture_child.lock().unwrap();
            child_guard.take()
        };

        if let Some(mut child) = child {
            let status = child
                .wait()
                .map_err(|err| format!("Failed to wait for screencapture: {err}"))?;
            // screencapture doesn't output much to stdout/stderr in interactive mode unless it fails.
            // We just care about the status and if the file exists.
            Ok::<std::process::ExitStatus, String>(status)
        } else {
            Err("Screen capture child process was lost.".to_string())
        }
    })
    .await
    .map_err(|err| format!("Screen capture wait task failed: {err}"))??;

    if output.success() {
        if let Ok(metadata) = std::fs::metadata(&capture_path) {
            if metadata.len() > 0 {
                resize_image_file_for_context(&capture_path);
                return Ok(Some(capture_path));
            }
        }
        remove_temp_image_file(&capture_path);
        return Ok(None);
    }

    if !capture_path.exists() && (output.code() == Some(1) || output.code().is_none()) {
        return Ok(None);
    }

    remove_temp_image_file(&capture_path);

    Err(format!("screencapture exited with status {}.", output))
}

#[cfg(target_os = "macos")]
async fn run_entire_screen_capture() -> Result<Option<PathBuf>, String> {
    let capture_path = create_temp_screen_capture_path();
    let (capture_path, output) = tauri::async_runtime::spawn_blocking(move || {
        std::process::Command::new("/usr/sbin/screencapture")
            .args(["-x"])
            .arg(&capture_path)
            .output()
            .map(|output| (capture_path, output))
            .map_err(|err| format!("Failed to launch screencapture: {err}"))
    })
    .await
    .map_err(|err| format!("Screen capture task failed: {err}"))??;

    if output.status.success() {
        if let Ok(metadata) = std::fs::metadata(&capture_path) {
            if metadata.len() > 0 {
                resize_image_file_for_context(&capture_path);
                return Ok(Some(capture_path));
            }
        }
        remove_temp_image_file(&capture_path);
        return Ok(None);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    remove_temp_image_file(&capture_path);

    if stderr.is_empty() {
        Err(format!(
            "screencapture exited with status {}.",
            output.status
        ))
    } else {
        Err(format!(
            "screencapture exited with status {}: {}",
            output.status, stderr
        ))
    }
}

#[cfg(not(target_os = "macos"))]
async fn run_interactive_screen_capture(
    _app_handle: &AppHandle,
) -> Result<Option<PathBuf>, String> {
    Err("Interactive screen capture is only supported on macOS right now.".to_string())
}

#[cfg(not(target_os = "macos"))]
async fn run_entire_screen_capture() -> Result<Option<PathBuf>, String> {
    Err("Entire screen capture is only supported on macOS right now.".to_string())
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

fn reset_stt_startup_state(state: &AppState) {
    {
        let mut startup_message_guard = state.stt_startup_message.lock().unwrap();
        *startup_message_guard = None;
    }
    let mut stderr_tail_guard = state.stt_stderr_tail.lock().unwrap();
    stderr_tail_guard.clear();
}

fn update_stt_startup_message(
    app_handle: &tauri::AppHandle,
    message: Option<String>,
    emit_event: bool,
) {
    let state = app_handle.state::<AppState>();
    {
        let mut startup_message_guard = state.stt_startup_message.lock().unwrap();
        *startup_message_guard = message.clone();
    }

    if emit_event {
        if let Some(message) = message {
            emit_stt_status(app_handle, SttStatusEvent { message });
        }
    }
}

fn push_stt_stderr_line(state: &AppState, line: String) {
    let mut stderr_tail_guard = state.stt_stderr_tail.lock().unwrap();
    stderr_tail_guard.push_back(line);
    while stderr_tail_guard.len() > STT_STDERR_TAIL_LIMIT {
        stderr_tail_guard.pop_front();
    }
}

fn stt_startup_failure_message(state: &AppState, base: &str) -> String {
    let mut message = base.to_string();

    if let Some(stage) = state.stt_startup_message.lock().unwrap().clone() {
        if !stage.trim().is_empty() {
            message.push_str(&format!(". Last stage: {}", stage.trim()));
        }
    }

    let stderr_tail_guard = state.stt_stderr_tail.lock().unwrap();
    if let Some(last_stderr) = stderr_tail_guard
        .iter()
        .rev()
        .find(|line| !line.trim().is_empty())
    {
        message.push_str(&format!(". Last stderr: {}", last_stderr.trim()));
    }

    message
}

fn emit_audio_turn_processing_error(app_handle: &tauri::AppHandle, message: String) {
    emit_csm_error(
        app_handle,
        CsmErrorEvent {
            request_id: None,
            message,
        },
    );
    emit_call_stage(app_handle, "listening", "Listening");
}

fn create_temp_screen_capture_path() -> PathBuf {
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);

    let mut path = std::env::temp_dir();
    path.push("openduck-screen-captures");

    let _ = std::fs::create_dir_all(&path);

    path.push(format!("openduck-screen-{}.png", timestamp_ms));
    path
}

fn resolve_capture_sample_rate(sample_rate: Option<u32>) -> u32 {
    match sample_rate {
        Some(rate) if (MIN_CAPTURE_SAMPLE_RATE..=MAX_CAPTURE_SAMPLE_RATE).contains(&rate) => rate,
        _ => DEFAULT_SAMPLE_RATE,
    }
}

fn ensure_vad(app_handle: &tauri::AppHandle, state: &AppState) -> Result<(), String> {
    let mut vad = state.vad.lock().unwrap();
    if vad.is_none() {
        let model_path = resolve_resource_file(app_handle, "silero_vad.onnx")?;
        *vad = Some(vad::Silero::new(vad::SampleRate::SixteenkHz, model_path)?);
    }
    Ok(())
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

async fn gemma_server_is_running_on_port(port: u16) -> bool {
    let client = reqwest::Client::new();
    let url = format!("{}/v1/models", server_base_url(port));
    match client.get(url).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

async fn wait_for_gemma_server_on_port(port: u16, timeout: Duration) -> bool {
    tokio::time::timeout(timeout, async move {
        loop {
            if gemma_server_is_running_on_port(port).await {
                return;
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .is_ok()
}

fn schedule_delayed_tray_refresh(app_handle: AppHandle, delay: Duration) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(delay).await;
        refresh_tray_presentation(&app_handle);
    });
}

fn normalize_contact_export_path(output_path: &str) -> Result<PathBuf, String> {
    let trimmed_path = output_path.trim();
    if trimmed_path.is_empty() {
        return Err("No export path was provided.".to_string());
    }

    let mut path = PathBuf::from(trimmed_path);

    if path.extension().is_none() {
        path.set_extension("openduck-contact.json");
    }

    Ok(path)
}

fn huggingface_cache_root(model_dir_name: &str) -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    std::path::Path::new(&home)
        .join(".cache/huggingface/hub")
        .join(model_dir_name)
}

fn clear_huggingface_cache(model_dir_name: &str) -> Result<(), String> {
    let model_dir = huggingface_cache_root(model_dir_name);

    if !model_dir.exists() {
        return Ok(());
    }

    std::fs::remove_dir_all(&model_dir)
        .map_err(|err| format!("Failed to clear cache at {}: {err}", model_dir.display()))
}

fn clear_csm_model_cache(variant: CsmModelVariant) -> Result<(), String> {
    clear_huggingface_cache(variant.cache_dir())?;

    if variant == CsmModelVariant::CosyVoice205b
        || variant == CsmModelVariant::CosyVoice305b8bit
        || variant == CsmModelVariant::CosyVoice305b4bit
        || variant == CsmModelVariant::CosyVoice305bFp16
    {
        clear_huggingface_cache(COSYVOICE2_S3_TOKENIZER_CACHE_DIR)?;
    }

    Ok(())
}

fn huggingface_cached_files_exist(model_dir_name: &str, required_files: &[&str]) -> bool {
    if required_files.is_empty() {
        return false;
    }

    any_snapshot_matches(model_dir_name, |snapshot_dir| {
        required_files
            .iter()
            .all(|required_file| snapshot_dir.join(required_file).exists())
    })
}

fn gemma_snapshot_is_complete(snapshot_dir: &Path) -> bool {
    if !snapshot_dir.join("config.json").exists() {
        return false;
    }

    if !snapshot_dir.join("tokenizer.model").exists()
        && !snapshot_dir.join("tokenizer.json").exists()
    {
        return false;
    }

    if snapshot_dir.join("model.safetensors").exists() {
        return true;
    }

    if !snapshot_dir.join("model.safetensors.index.json").exists() {
        return false;
    }

    let Ok(entries) = std::fs::read_dir(snapshot_dir) else {
        return false;
    };

    let mut shard_numbers_by_total = std::collections::BTreeMap::<u32, HashSet<u32>>::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let Some(file_name) = file_name.strip_prefix("model-") else {
            continue;
        };
        let Some(file_name) = file_name.strip_suffix(".safetensors") else {
            continue;
        };
        let Some((shard_number, shard_total)) = file_name.split_once("-of-") else {
            continue;
        };
        let (Ok(shard_number), Ok(shard_total)) =
            (shard_number.parse::<u32>(), shard_total.parse::<u32>())
        else {
            continue;
        };

        shard_numbers_by_total
            .entry(shard_total)
            .or_default()
            .insert(shard_number);
    }

    if shard_numbers_by_total.len() != 1 {
        return false;
    }

    let Some((shard_total, shard_numbers)) = shard_numbers_by_total.into_iter().next() else {
        return false;
    };

    (1..=shard_total).all(|shard_number| shard_numbers.contains(&shard_number))
}

fn any_snapshot_matches(model_dir_name: &str, predicate: impl Fn(&Path) -> bool) -> bool {
    let snapshots_dir = huggingface_cache_root(model_dir_name).join("snapshots");
    let Ok(entries) = std::fs::read_dir(snapshots_dir) else {
        return false;
    };

    for entry in entries.flatten() {
        let snapshot_dir = entry.path();
        if !snapshot_dir.is_dir() {
            continue;
        }

        if predicate(&snapshot_dir) {
            return true;
        }
    }

    false
}

fn gemma_model_cache_exists(variant: GemmaVariant) -> bool {
    if variant.is_external() {
        return true;
    }
    variant
        .cache_dir()
        .map(|cache_dir| any_snapshot_matches(cache_dir, gemma_snapshot_is_complete))
        .unwrap_or(false)
}

fn csm_model_cache_exists(variant: CsmModelVariant) -> bool {
    let main_model_cached =
        huggingface_cached_files_exist(variant.cache_dir(), variant.required_files());
    if !main_model_cached {
        return false;
    }

    if variant == CsmModelVariant::CosyVoice205b
        || variant == CsmModelVariant::CosyVoice305b8bit
        || variant == CsmModelVariant::CosyVoice305b4bit
        || variant == CsmModelVariant::CosyVoice305bFp16
    {
        return huggingface_cached_files_exist(
            COSYVOICE2_S3_TOKENIZER_CACHE_DIR,
            &[
                COSYVOICE2_S3_TOKENIZER_CONFIG_FILE,
                COSYVOICE2_S3_TOKENIZER_MODEL_FILE,
            ],
        );
    }

    true
}

fn stt_model_cache_exists(variant: SttModelVariant) -> bool {
    match variant {
        SttModelVariant::Gemma => true,
        SttModelVariant::DistilWhisperLargeV3 => huggingface_cached_files_exist(
            STT_DISTIL_WHISPER_CACHE_DIR,
            SttModelVariant::DistilWhisperLargeV3.required_files(),
        ),
        SttModelVariant::WhisperLargeV3Turbo => huggingface_cached_files_exist(
            STT_WHISPER_CACHE_DIR,
            SttModelVariant::WhisperLargeV3Turbo.required_files(),
        ),
    }
}

fn active_download_process(state: &AppState, model: DownloadModel) -> Option<DownloadProcess> {
    match model {
        DownloadModel::Gemma => state.gemma_download_process.lock().unwrap().clone(),
        DownloadModel::Csm => state.csm_download_process.lock().unwrap().clone(),
        DownloadModel::Stt => state.stt_download_process.lock().unwrap().clone(),
    }
}

fn set_active_download_process(
    state: &AppState,
    model: DownloadModel,
    process: Option<DownloadProcess>,
) {
    match model {
        DownloadModel::Gemma => {
            *state.gemma_download_process.lock().unwrap() = process;
        }
        DownloadModel::Csm => {
            *state.csm_download_process.lock().unwrap() = process;
        }
        DownloadModel::Stt => {
            *state.stt_download_process.lock().unwrap() = process;
        }
    }
}

fn tracked_download_state(state: &AppState, model: DownloadModel) -> Option<TrackedDownloadState> {
    match model {
        DownloadModel::Gemma => state.gemma_download_state.lock().unwrap().clone(),
        DownloadModel::Csm => state.csm_download_state.lock().unwrap().clone(),
        DownloadModel::Stt => state.stt_download_state.lock().unwrap().clone(),
    }
}

fn set_tracked_download_state(
    state: &AppState,
    model: DownloadModel,
    tracked_state: Option<TrackedDownloadState>,
) {
    match model {
        DownloadModel::Gemma => {
            *state.gemma_download_state.lock().unwrap() = tracked_state;
        }
        DownloadModel::Csm => {
            *state.csm_download_state.lock().unwrap() = tracked_state;
        }
        DownloadModel::Stt => {
            *state.stt_download_state.lock().unwrap() = tracked_state;
        }
    }
}

fn update_tracked_download_event(
    state: &AppState,
    model: DownloadModel,
    latest_event: ModelDownloadEvent,
) {
    let mut tracked_state = tracked_download_state(state, model).unwrap_or(TrackedDownloadState {
        latest_event: latest_event.clone(),
        manifest: None,
    });
    tracked_state.latest_event = latest_event;
    set_tracked_download_state(state, model, Some(tracked_state));
}

fn update_tracked_download_manifest(
    state: &AppState,
    model: DownloadModel,
    manifest: DownloadManifest,
) {
    let mut tracked_state = tracked_download_state(state, model).unwrap_or(TrackedDownloadState {
        latest_event: ModelDownloadEvent {
            model: model.key().to_string(),
            phase: "progress".to_string(),
            message: "Preparing download...".to_string(),
            progress: Some(0.0),
            downloaded_bytes: Some(0),
            total_bytes: manifest.total_bytes,
            indeterminate: manifest.total_bytes.is_none(),
        },
        manifest: None,
    });
    tracked_state.manifest = Some(manifest);
    if let Some(manifest) = tracked_state.manifest.as_ref() {
        tracked_state.latest_event.total_bytes = manifest.total_bytes;
    }
    set_tracked_download_state(state, model, Some(tracked_state));
}

fn resolve_downloaded_file_bytes(file: &DownloadManifestFile) -> u64 {
    if file.local_path.exists() {
        return file.file_size;
    }

    if let Some(blob_path) = file.blob_path.as_ref() {
        if let Ok(metadata) = std::fs::metadata(blob_path) {
            return metadata.len().min(file.file_size);
        }
    }

    if let Some(incomplete_path) = file.incomplete_path.as_ref() {
        if let Ok(metadata) = std::fs::metadata(incomplete_path) {
            return metadata.len().min(file.file_size);
        }
    }

    0
}

fn poll_tracked_download_event(tracked_state: &TrackedDownloadState) -> ModelDownloadEvent {
    if tracked_state.latest_event.phase != "progress" {
        return tracked_state.latest_event.clone();
    }

    let Some(manifest) = tracked_state.manifest.as_ref() else {
        return tracked_state.latest_event.clone();
    };
    let total_bytes = manifest
        .total_bytes
        .unwrap_or_else(|| manifest.files.iter().map(|file| file.file_size).sum());

    if total_bytes == 0 {
        return tracked_state.latest_event.clone();
    }

    let scanned_bytes = manifest
        .files
        .iter()
        .map(resolve_downloaded_file_bytes)
        .sum::<u64>()
        .min(total_bytes);
    let merged_bytes = tracked_state
        .latest_event
        .downloaded_bytes
        .unwrap_or(0)
        .max(scanned_bytes)
        .min(total_bytes);
    let mut event = tracked_state.latest_event.clone();
    event.total_bytes = Some(total_bytes);
    event.downloaded_bytes = Some(merged_bytes);
    event.progress = Some((merged_bytes as f64 / total_bytes as f64 * 100.0) as f32);
    event.indeterminate = false;

    if event.message.trim().is_empty() {
        event.message = "Downloading model files...".to_string();
    }

    event
}

fn set_download_cancel_requested(state: &AppState, model: DownloadModel, requested: bool) {
    match model {
        DownloadModel::Gemma => {
            *state.gemma_download_cancel_requested.lock().unwrap() = requested;
        }
        DownloadModel::Csm => {
            *state.csm_download_cancel_requested.lock().unwrap() = requested;
        }
        DownloadModel::Stt => {
            *state.stt_download_cancel_requested.lock().unwrap() = requested;
        }
    }
}

fn take_download_cancel_requested(state: &AppState, model: DownloadModel) -> bool {
    let cancel_requested = match model {
        DownloadModel::Gemma => &state.gemma_download_cancel_requested,
        DownloadModel::Csm => &state.csm_download_cancel_requested,
        DownloadModel::Stt => &state.stt_download_cancel_requested,
    };
    let mut guard = cancel_requested.lock().unwrap();
    let was_requested = *guard;
    *guard = false;
    was_requested
}

#[tauri::command]
fn get_model_download_status(
    state: State<'_, AppState>,
    model: String,
) -> Result<Option<ModelDownloadEvent>, String> {
    let download_model = DownloadModel::from_key(&model)?;
    let Some(tracked_state) = tracked_download_state(state.inner(), download_model) else {
        return Ok(None);
    };

    Ok(Some(poll_tracked_download_event(&tracked_state)))
}

fn resolve_openduck_root() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".openduck")
}

fn resolve_openduck_config_path() -> PathBuf {
    resolve_openduck_root().join(OPENDUCK_CONFIG_FILE_NAME)
}

fn load_persisted_app_config() -> PersistedAppConfig {
    let config_path = resolve_openduck_config_path();
    let Ok(raw) = std::fs::read_to_string(&config_path) else {
        return PersistedAppConfig::default();
    };

    match serde_json::from_str::<PersistedAppConfig>(&raw) {
        Ok(config) => config,
        Err(err) => {
            warn!(
                "Failed to parse OpenDuck config at {}: {}",
                config_path.display(),
                err
            );
            PersistedAppConfig::default()
        }
    }
}

fn save_persisted_app_config(config: &PersistedAppConfig) -> Result<(), String> {
    let root = resolve_openduck_root();
    std::fs::create_dir_all(&root)
        .map_err(|err| format!("Failed to create OpenDuck config directory: {err}"))?;

    let config_path = root.join(OPENDUCK_CONFIG_FILE_NAME);
    let payload = serde_json::to_string_pretty(config)
        .map_err(|err| format!("Failed to serialize OpenDuck config: {err}"))?;

    std::fs::write(&config_path, payload).map_err(|err| {
        format!(
            "Failed to write OpenDuck config to {}: {err}",
            config_path.display()
        )
    })
}

fn snapshot_external_llm_provider_configs(state: &AppState) -> PersistedExternalLlmProvidersConfig {
    PersistedExternalLlmProvidersConfig {
        ollama: PersistedExternalLlmProviderConfig {
            base_url: state.ollama_base_url.lock().unwrap().clone(),
            api_key: state.ollama_api_key.lock().unwrap().clone(),
        },
        lmstudio: PersistedExternalLlmProviderConfig {
            base_url: state.lmstudio_base_url.lock().unwrap().clone(),
            api_key: state.lmstudio_api_key.lock().unwrap().clone(),
        },
        openai_compatible: PersistedExternalLlmProviderConfig {
            base_url: state.openai_compatible_base_url.lock().unwrap().clone(),
            api_key: state.openai_compatible_api_key.lock().unwrap().clone(),
        },
    }
}

fn persist_external_llm_provider_configs(state: &AppState) -> Result<(), String> {
    save_persisted_app_config(&PersistedAppConfig {
        version: default_persisted_app_config_version(),
        external_llm_providers: snapshot_external_llm_provider_configs(state),
    })
}

fn resolve_sessions_dir(_app_handle: &AppHandle) -> Result<PathBuf, String> {
    let mut path = resolve_openduck_root();
    path.push(SESSIONS_DIR_NAME);
    std::fs::create_dir_all(&path)
        .map_err(|err| format!("Failed to create sessions directory: {err}"))?;
    Ok(path)
}

fn resolve_session_dir(app_handle: &AppHandle, session_id: &str) -> Result<PathBuf, String> {
    let mut path = resolve_sessions_dir(app_handle)?;
    path.push(session_id);
    std::fs::create_dir_all(&path)
        .map_err(|err| format!("Failed to create session directory: {err}"))?;
    Ok(path)
}

fn resolve_session_file(app_handle: &AppHandle, session_id: &str) -> Result<PathBuf, String> {
    let mut path = resolve_session_dir(app_handle, session_id)?;
    path.push(SESSION_FILE_NAME);
    Ok(path)
}

fn resolve_session_images_dir(app_handle: &AppHandle, session_id: &str) -> Result<PathBuf, String> {
    let mut path = resolve_session_dir(app_handle, session_id)?;
    path.push(SESSION_IMAGES_DIR_NAME);
    std::fs::create_dir_all(&path)
        .map_err(|err| format!("Failed to create session images directory: {err}"))?;
    Ok(path)
}

fn save_current_session(app_handle: &AppHandle, state: &AppState) -> Result<(), String> {
    let session_id = {
        let guard = state.current_session_id.lock().unwrap();
        guard.clone()
    };

    let Some(session_id) = session_id else {
        return Ok(());
    };

    let session_file = resolve_session_file(app_handle, &session_id)?;

    let (turns, title) = {
        let turns_guard = state.conversation_turns.lock().unwrap();
        let title_guard = state.current_session_title.lock().unwrap();
        (
            turns_guard.iter().cloned().collect::<Vec<_>>(),
            title_guard.clone(),
        )
    };

    let metadata = if session_file.exists() {
        let content = std::fs::read_to_string(&session_file).map_err(|err| err.to_string())?;
        let existing_data: SessionData =
            serde_json::from_str(&content).map_err(|err| err.to_string())?;
        let mut metadata = existing_data.metadata;
        metadata.updated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if let Some(new_title) = title {
            metadata.title = new_title;
        }
        metadata
    } else {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        SessionMetadata {
            id: session_id.clone(),
            title: title.unwrap_or_else(|| {
                turns
                    .first()
                    .map(|t| t.user_text.chars().take(50).collect())
                    .unwrap_or_else(|| "New Session".to_string())
            }),
            created_at: now,
            updated_at: now,
        }
    };

    let data = SessionData { metadata, turns };

    let content = serde_json::to_string_pretty(&data).map_err(|err| err.to_string())?;
    std::fs::write(session_file, content).map_err(|err| err.to_string())?;

    Ok(())
}

#[tauri::command]
fn get_sessions(app_handle: AppHandle) -> Result<Vec<SessionMetadata>, String> {
    let sessions_dir = resolve_sessions_dir(&app_handle)?;
    let mut sessions = Vec::new();

    if let Ok(entries) = std::fs::read_dir(sessions_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let session_file = entry.path().join(SESSION_FILE_NAME);
                if session_file.exists() {
                    if let Ok(content) = std::fs::read_to_string(session_file) {
                        if let Ok(data) = serde_json::from_str::<SessionData>(&content) {
                            sessions.push(data.metadata);
                        }
                    }
                }
            }
        }
    }

    sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(sessions)
}

#[tauri::command]
async fn load_session(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<ConversationTurn>, String> {
    cancel_auto_continue_timer(state.inner());

    let session_file = resolve_session_file(&app_handle, &session_id)?;
    if !session_file.exists() {
        return Err("Session not found".to_string());
    }

    let content = std::fs::read_to_string(session_file).map_err(|err| err.to_string())?;
    let mut data: SessionData = serde_json::from_str(&content).map_err(|err| err.to_string())?;

    // Migrate old single-image turns to the new multi-image format
    for turn in &mut data.turns {
        if let Some(path) = turn.image_path.take() {
            if !turn.image_paths.contains(&path) {
                turn.image_paths.push(path);
            }
        }
        if let Some(data_url) = turn.user_image_data_url.take() {
            if !turn.user_image_data_urls.contains(&data_url) {
                turn.user_image_data_urls.push(data_url);
            }
        }
    }

    {
        let mut turns_guard = state.conversation_turns.lock().unwrap();
        turns_guard.clear();
        for turn in &data.turns {
            turns_guard.push_back(turn.clone());
        }
    }

    {
        let mut id_guard = state.current_session_id.lock().unwrap();
        *id_guard = Some(session_id);
    }

    {
        let mut title_guard = state.current_session_title.lock().unwrap();
        *title_guard = Some(data.metadata.title);
    }

    let max_id = data
        .turns
        .iter()
        .map(|t| t.user_entry_id.max(t.assistant_entry_id))
        .max()
        .unwrap_or(0);
    state
        .next_conversation_entry_id
        .store(max_id + 1, Ordering::Relaxed);
    reset_auto_continue_tracker(state.inner());

    Ok(data.turns)
}

#[tauri::command]
fn delete_session(app_handle: AppHandle, session_id: String) -> Result<(), String> {
    let session_dir = resolve_session_dir(&app_handle, &session_id)?;
    if session_dir.exists() {
        std::fs::remove_dir_all(session_dir).map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn rename_session(
    app_handle: AppHandle,
    session_id: String,
    new_title: String,
) -> Result<(), String> {
    let session_file = resolve_session_file(&app_handle, &session_id)?;
    if !session_file.exists() {
        return Err("Session not found".to_string());
    }

    let content = std::fs::read_to_string(&session_file).map_err(|err| err.to_string())?;
    let mut data: SessionData = serde_json::from_str(&content).map_err(|err| err.to_string())?;
    data.metadata.title = new_title;
    data.metadata.updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let content = serde_json::to_string_pretty(&data).map_err(|err| err.to_string())?;
    std::fs::write(session_file, content).map_err(|err| err.to_string())?;
    Ok(())
}

fn extract_snippet(text: &str, query: &str) -> String {
    let text_chars: Vec<char> = text.chars().collect();
    let query_lower = query.to_lowercase();
    let query_char_count = query.chars().count();

    if query_char_count == 0 {
        return text_chars.iter().take(100).collect();
    }

    if text_chars.len() >= query_char_count {
        for i in 0..=text_chars.len() - query_char_count {
            let chunk: String = text_chars[i..i + query_char_count].iter().collect();
            if chunk.to_lowercase() == query_lower {
                let start = i.saturating_sub(40);
                let end = std::cmp::min(text_chars.len(), i + query_char_count + 60);

                let mut snippet = if start > 0 { "..." } else { "" }.to_string();
                let content: String = text_chars[start..end].iter().collect();
                snippet.push_str(&content);
                if end < text_chars.len() {
                    snippet.push_str("...");
                }
                return snippet;
            }
        }
    }

    text_chars.iter().take(100).collect()
}

#[tauri::command]
fn search_sessions(app_handle: AppHandle, query: String) -> Result<Vec<SearchResult>, String> {
    let sessions_dir = resolve_sessions_dir(&app_handle)?;
    let mut results = Vec::new();
    let query_trimmed = query.trim();
    if query_trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let query_lower = query_trimmed.to_lowercase();

    if let Ok(entries) = std::fs::read_dir(sessions_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_dir() {
                    let session_file = entry.path().join(SESSION_FILE_NAME);
                    if let Ok(content) = std::fs::read_to_string(&session_file) {
                        if let Ok(data) = serde_json::from_str::<SessionData>(&content) {
                            let mut session_matches = Vec::new();

                            // Check title
                            if data.metadata.title.to_lowercase().contains(&query_lower) {
                                session_matches.push(SearchResult {
                                    session_id: data.metadata.id.clone(),
                                    session_title: data.metadata.title.clone(),
                                    matched_text: String::new(),
                                    updated_at: data.metadata.updated_at,
                                });
                            }

                            // Check turns
                            for turn in &data.turns {
                                if turn.user_text.to_lowercase().contains(&query_lower) {
                                    session_matches.push(SearchResult {
                                        session_id: data.metadata.id.clone(),
                                        session_title: data.metadata.title.clone(),
                                        matched_text: extract_snippet(&turn.user_text, query_trimmed),
                                        updated_at: data.metadata.updated_at,
                                    });
                                }
                                if turn.assistant_text.to_lowercase().contains(&query_lower) {
                                    session_matches.push(SearchResult {
                                        session_id: data.metadata.id.clone(),
                                        session_title: data.metadata.title.clone(),
                                        matched_text: extract_snippet(&turn.assistant_text, query_trimmed),
                                        updated_at: data.metadata.updated_at,
                                    });
                                }
                            }

                            results.extend(session_matches);
                        }
                    }
                }
            }
        }
    }

    results.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(results)
}

#[tauri::command]
async fn fork_session(
    app_handle: AppHandle,
    state: State<'_, AppState>,
    assistant_entry_id: u64,
    new_title: String,
) -> Result<SessionMetadata, String> {
    let mut forked_turns = VecDeque::new();
    {
        let turns = state.conversation_turns.lock().unwrap();
        let mut found = false;
        for turn in turns.iter() {
            forked_turns.push_back(turn.clone());
            if turn.assistant_entry_id == assistant_entry_id {
                found = true;
                break;
            }
        }
        if !found {
            return Err("Assistant entry not found in active context".to_string());
        }
    }

    let new_session_id = Uuid::new_v4().to_string();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let metadata = SessionMetadata {
        id: new_session_id.clone(),
        title: new_title,
        created_at: now,
        updated_at: now,
    };

    let session_file = resolve_session_file(&app_handle, &new_session_id)?;
    let data = SessionData {
        metadata: metadata.clone(),
        turns: forked_turns.iter().cloned().collect(),
    };
    let content = serde_json::to_string_pretty(&data).map_err(|err| err.to_string())?;
    std::fs::write(session_file, content).map_err(|err| err.to_string())?;

    {
        let mut id_guard = state.current_session_id.lock().unwrap();
        *id_guard = Some(new_session_id);
    }
    {
        let mut title_guard = state.current_session_title.lock().unwrap();
        *title_guard = Some(metadata.title.clone());
    }
    {
        let mut turns_guard = state.conversation_turns.lock().unwrap();
        *turns_guard = forked_turns;
    }

    let max_id = data
        .turns
        .iter()
        .map(|t| t.user_entry_id.max(t.assistant_entry_id))
        .max()
        .unwrap_or(0);
    state
        .next_conversation_entry_id
        .store(max_id + 1, Ordering::Relaxed);

    Ok(metadata)
}

#[tauri::command]
fn start_new_session(state: State<'_, AppState>) {
    reset_call_session_state(state.inner());
    let session_id = Uuid::new_v4().to_string();
    {
        let mut id_guard = state.current_session_id.lock().unwrap();
        *id_guard = Some(session_id);
    }
}

#[tauri::command]
fn get_current_session_id(state: State<'_, AppState>) -> Option<String> {
    state.current_session_id.lock().unwrap().clone()
}

#[tauri::command]
fn update_current_session_title(app_handle: AppHandle, state: State<'_, AppState>, title: String) {
    {
        let mut title_guard = state.current_session_title.lock().unwrap();
        *title_guard = Some(title);
    }
    if let Err(err) = save_current_session(&app_handle, state.inner()) {
        error!("Failed to save session after title update: {}", err);
    }
}

fn resolve_runtime_root(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    app_handle
        .path()
        .app_data_dir()
        .map(|path| path.join("runtime"))
        .map_err(|err| format!("Failed to resolve the OpenDuck app data directory: {err}"))
}

fn resolve_setup_script(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join("scripts").join("setup_python_env.sh"));
        if current_dir.ends_with("src-tauri") {
            candidates.push(
                current_dir
                    .join("..")
                    .join("scripts")
                    .join("setup_python_env.sh"),
            );
        }
    }

    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        candidates.push(resource_dir.join("setup_python_env.sh"));
        candidates.push(resource_dir.join("scripts").join("setup_python_env.sh"));
        candidates.push(resource_dir.join("_up_").join("setup_python_env.sh"));
        candidates.push(
            resource_dir
                .join("_up_")
                .join("scripts")
                .join("setup_python_env.sh"),
        );
        candidates.push(resource_dir.join("resources").join("setup_python_env.sh"));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| "Unable to locate setup_python_env.sh".to_string())
}

fn runtime_dependencies_available(app_handle: &tauri::AppHandle) -> bool {
    // If we have an installed runtime in the app data directory, it must be complete.
    if let Ok(runtime_root) = resolve_runtime_root(app_handle) {
        if runtime_root.exists() {
            return runtime_root.join(".complete").exists();
        }
    }

    // Fallback for development where dependencies are pre-installed in the source tree.
    resolve_gemma_python_executable(app_handle).is_ok()
        && resolve_csm_site_packages(app_handle).is_ok()
        && resolve_kokoro_site_packages(app_handle).is_ok()
        && resolve_cosyvoice_site_packages(app_handle).is_ok()
        && resolve_stt_site_packages(app_handle).is_ok()
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
    .map_err(|_| {
        "CSM dependencies are not installed yet. OpenDuck needs to finish preparing its local runtime first.".to_string()
    })
}

fn resolve_kokoro_site_packages(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    resolve_existing_path_dev_first(
        app_handle,
        Path::new("resources/kokoro_env/venv/lib/python3.11/site-packages"),
        "bundled Kokoro site-packages",
    )
    .map_err(|_| {
        "Kokoro dependencies are not installed yet. OpenDuck needs to finish preparing its local runtime first.".to_string()
    })
}

fn resolve_cosyvoice_site_packages(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    resolve_existing_path_dev_first(
        app_handle,
        Path::new("resources/cosyvoice_env/venv/lib/python3.11/site-packages"),
        "bundled CosyVoice site-packages",
    )
    .map_err(|_| {
        "CosyVoice dependencies are not installed yet. OpenDuck needs to finish preparing its local runtime first.".to_string()
    })
}

fn resolve_stt_site_packages(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    resolve_existing_path_dev_first(
        app_handle,
        Path::new("resources/stt_env/venv/lib/python3.11/site-packages"),
        "bundled STT site-packages",
    )
    .map_err(|_| {
        "Whisper STT dependencies are not installed yet. OpenDuck needs to finish preparing its local runtime first.".to_string()
    })
}

fn resolve_speech_site_packages(
    app_handle: &tauri::AppHandle,
    variant: CsmModelVariant,
) -> Result<PathBuf, String> {
    match variant {
        CsmModelVariant::Expressiva1b => resolve_csm_site_packages(app_handle),
        CsmModelVariant::Kokoro82m => resolve_kokoro_site_packages(app_handle),
        CsmModelVariant::CosyVoice205b
        | CsmModelVariant::CosyVoice305b8bit
        | CsmModelVariant::CosyVoice305b4bit
        | CsmModelVariant::CosyVoice305bFp16 => resolve_cosyvoice_site_packages(app_handle),
    }
}

fn resolve_gemma_site_packages(app_handle: &tauri::AppHandle) -> Result<PathBuf, String> {
    resolve_existing_path_dev_first(
        app_handle,
        Path::new("resources/python_env/venv/lib/python3.11/site-packages"),
        "bundled Gemma site-packages",
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

    if let Ok(runtime_root) = resolve_runtime_root(app_handle) {
        candidates.push(runtime_root.join(relative_path));
        if let Ok(stripped) = relative_path.strip_prefix("resources") {
            candidates.push(runtime_root.join(stripped));
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
    state: &AppState,
    voice: CsmVoice,
) -> Result<PathBuf, String> {
    if voice == CsmVoice::Custom {
        return state
            .csm_reference_audio_path
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| "No custom reference audio set".to_string());
    }

    let file_name = voice.file_name().unwrap();
    let mut candidates = Vec::new();

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir.join(file_name));
        candidates.push(current_dir.join("src-tauri").join("..").join(file_name));

        if current_dir.ends_with("src-tauri") {
            candidates.push(current_dir.join("..").join(file_name));
        }
    }

    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        candidates.push(resource_dir.join(file_name));
        candidates.push(resource_dir.join("_up_").join(file_name));
        candidates.push(resource_dir.join("resources").join(file_name));
    }

    candidates
        .into_iter()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| format!("Unable to locate {}", file_name))
}

fn reap_stale_model_processes(app_handle: &tauri::AppHandle) {
    for resource_name in ["patch_mlx_vlm.py", "csm_stream.py", "stt_stream.py"] {
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

#[cfg(target_os = "macos")]
fn start_download_sleep_assertion(model_key: &str, pid: u32) {
    let model_key = model_key.to_string();
    tauri::async_runtime::spawn(async move {
        let mut command = Command::new("/usr/bin/caffeinate");
        command
            .arg("-i")
            .arg("-w")
            .arg(pid.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        let mut caffeinate = match command.spawn() {
            Ok(child) => child,
            Err(err) => {
                warn!(
                    "Failed to start caffeinate for {} download (pid {}): {}",
                    model_key, pid, err
                );
                return;
            }
        };

        match caffeinate.wait().await {
            Ok(status) if status.success() => {
                debug!(
                    "Released sleep assertion for {} download (pid {})",
                    model_key, pid
                );
            }
            Ok(status) => {
                warn!(
                    "caffeinate exited with status {} while keeping {} download awake (pid {})",
                    status, model_key, pid
                );
            }
            Err(err) => {
                warn!(
                    "Failed waiting for caffeinate during {} download (pid {}): {}",
                    model_key, pid, err
                );
            }
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn start_download_sleep_assertion(_model_key: &str, _pid: u32) {}

#[tauri::command]
fn check_model_status(state: State<'_, AppState>) -> bool {
    gemma_model_cache_exists(selected_gemma_variant(state.inner()))
}

#[tauri::command]
fn check_csm_status(state: State<'_, AppState>) -> bool {
    csm_model_cache_exists(selected_csm_model(state.inner()))
}

#[tauri::command]
fn check_stt_status(state: State<'_, AppState>) -> bool {
    stt_model_cache_exists(selected_stt_model(state.inner()))
}

#[tauri::command]
fn clear_model_cache(state: State<'_, AppState>, model: String) -> Result<(), String> {
    let download_model = DownloadModel::from_key(&model)?;
    if active_download_process(state.inner(), download_model).is_some() {
        return Err(format!("{model} download already in progress"));
    }

    match download_model {
        DownloadModel::Gemma => {
            let selected_variant = selected_gemma_variant(state.inner());
            if loaded_gemma_variant(state.inner()) == Some(selected_variant) {
                return Err(format!(
                    "Unload Gemma {} before clearing its cache.",
                    selected_variant.label()
                ));
            }

            let cache_dir = selected_variant.cache_dir().ok_or_else(|| {
                format!(
                    "{} does not use a local cache directory.",
                    selected_variant.label()
                )
            })?;
            clear_huggingface_cache(cache_dir)?;
        }
        DownloadModel::Csm => {
            let selected_variant = selected_csm_model(state.inner());
            if loaded_csm_model(state.inner()) == Some(selected_variant) {
                return Err(format!(
                    "Unload {} before clearing its cache.",
                    selected_variant.label()
                ));
            }

            clear_csm_model_cache(selected_variant)?;
        }
        DownloadModel::Stt => {
            let selected_variant = selected_stt_model(state.inner());
            if !selected_variant.uses_worker() {
                return Err(format!(
                    "{} does not use a dedicated STT cache.",
                    selected_variant.label()
                ));
            }

            if loaded_stt_model(state.inner()) == Some(selected_variant) {
                return Err(format!(
                    "Unload {} before clearing its cache.",
                    selected_variant.label()
                ));
            }

            clear_huggingface_cache(
                selected_variant
                    .cache_dir()
                    .ok_or_else(|| "Missing STT cache directory.".to_string())?,
            )?;
        }
    }

    set_tracked_download_state(state.inner(), download_model, None);
    Ok(())
}

#[tauri::command]
async fn download_model(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    ensure_runtime_dependencies_inner(&app_handle, state.inner()).await?;

    let selected_variant = selected_gemma_variant(state.inner());
    let python_executable = resolve_gemma_python_executable(&app_handle)?;
    let gemma_site_packages = resolve_gemma_site_packages(&app_handle)?;
    let model_repo_id = selected_variant.repo_id().ok_or_else(|| {
        format!(
            "{} uses an external server and does not need a local download.",
            selected_variant.label()
        )
    })?;
    run_hf_download(
        &app_handle,
        python_executable,
        gemma_site_packages,
        "gemma",
        model_repo_id,
        &[],
    )
    .await
}

#[tauri::command]
async fn download_csm_model(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    ensure_runtime_dependencies_inner(&app_handle, state.inner()).await?;

    let selected_variant = selected_csm_model(state.inner());
    let python_executable = resolve_gemma_python_executable(&app_handle)?;
    let gemma_site_packages = resolve_gemma_site_packages(&app_handle)?;

    run_hf_download(
        &app_handle,
        python_executable.clone(),
        gemma_site_packages.clone(),
        "csm",
        selected_variant.repo_id(),
        selected_variant.required_files(),
    )
    .await?;

    if selected_variant == CsmModelVariant::CosyVoice205b
        || selected_variant == CsmModelVariant::CosyVoice305b8bit
        || selected_variant == CsmModelVariant::CosyVoice305b4bit
        || selected_variant == CsmModelVariant::CosyVoice305bFp16
    {
        run_hf_download(
            &app_handle,
            python_executable,
            gemma_site_packages,
            "csm",
            COSYVOICE2_S3_TOKENIZER_REPO,
            &[
                COSYVOICE2_S3_TOKENIZER_CONFIG_FILE,
                COSYVOICE2_S3_TOKENIZER_MODEL_FILE,
            ],
        )
        .await?;
    }

    Ok(())
}

#[tauri::command]
async fn download_stt_model(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    ensure_runtime_dependencies_inner(&app_handle, state.inner()).await?;

    let selected_variant = selected_stt_model(state.inner());
    let model_repo_id = selected_variant.repo_id().ok_or_else(|| {
        format!(
            "{} does not require a dedicated STT download.",
            selected_variant.label()
        )
    })?;
    let python_executable = resolve_gemma_python_executable(&app_handle)?;
    let stt_site_packages = resolve_stt_site_packages(&app_handle)?;

    run_hf_download(
        &app_handle,
        python_executable,
        stt_site_packages,
        "stt",
        model_repo_id,
        selected_variant.required_files(),
    )
    .await
}

#[tauri::command]
async fn cancel_model_download(state: State<'_, AppState>, model: String) -> Result<(), String> {
    let download_model = DownloadModel::from_key(&model)?;
    let Some(download_process) = active_download_process(state.inner(), download_model) else {
        return Ok(());
    };

    let mut child_guard = download_process.child.lock().await;
    if child_guard
        .try_wait()
        .map_err(|e| format!("Failed to inspect {} download state: {e}", model))?
        .is_none()
    {
        set_download_cancel_requested(state.inner(), download_model, true);
        child_guard
            .kill()
            .await
            .map_err(|e| format!("Failed to cancel {} download: {e}", model))?;
    }

    Ok(())
}

async fn run_hf_download(
    app_handle: &tauri::AppHandle,
    python_executable: PathBuf,
    python_site_packages: PathBuf,
    model_key: &str,
    repo_id: &str,
    allow_patterns: &[&str],
) -> Result<(), String> {
    let download_model = DownloadModel::from_key(model_key)?;
    let state = app_handle.state::<AppState>();

    if active_download_process(state.inner(), download_model).is_some() {
        return Err(format!("{model_key} download already in progress"));
    }

    set_download_cancel_requested(state.inner(), download_model, false);
    set_tracked_download_state(
        state.inner(),
        download_model,
        Some(TrackedDownloadState {
            latest_event: ModelDownloadEvent {
                model: model_key.to_string(),
                phase: "progress".to_string(),
                message: "Preparing download...".to_string(),
                progress: Some(0.0),
                downloaded_bytes: Some(0),
                total_bytes: None,
                indeterminate: true,
            },
            manifest: None,
        }),
    );

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
        .env("PYTHONPATH", python_site_packages)
        .env("HF_HUB_DISABLE_XET", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for pattern in allow_patterns {
        command.arg("--allow-pattern").arg(pattern);
    }

    let mut child = command
        .spawn()
        .map_err(|e| format!("Failed to start {model_key} downloader: {e}"))?;
    if let Some(child_pid) = child.id() {
        start_download_sleep_assertion(model_key, child_pid);
    } else {
        warn!(
            "Started {} download without a visible child pid; skipping caffeinate integration",
            model_key
        );
    }

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| format!("Failed to capture stdout for {model_key} download"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| format!("Failed to capture stderr for {model_key} download"))?;
    let child = Arc::new(AsyncMutex::new(child));
    set_active_download_process(
        state.inner(),
        download_model,
        Some(DownloadProcess {
            child: child.clone(),
        }),
    );

    let stderr_handle = tauri::async_runtime::spawn(async move {
        let mut stderr_lines = BufReader::new(stderr).lines();
        let mut collected = Vec::new();
        while let Ok(Some(line)) = stderr_lines.next_line().await {
            if !line.trim().is_empty() {
                let normalized = line.to_ascii_lowercase();
                if normalized.contains("warning") {
                    warn!("Downloader stderr: {}", line);
                } else {
                    error!("Downloader stderr: {}", line);
                }
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

        let parsed = match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(parsed) => parsed,
            Err(err) => {
                warn!(
                    "Failed to parse downloader output for {}: {} ({})",
                    model_key, err, trimmed
                );
                continue;
            }
        };
        let Some(event_type) = parsed
            .get("type")
            .and_then(|value| value.as_str())
            .map(str::to_string)
        else {
            warn!(
                "Downloader output for {} is missing a type: {}",
                model_key, trimmed
            );
            continue;
        };

        match event_type.as_str() {
            "manifest" => match serde_json::from_value::<DownloadManifestWorkerEvent>(parsed) {
                Ok(event) => {
                    update_tracked_download_manifest(
                        state.inner(),
                        download_model,
                        DownloadManifest {
                            total_bytes: event.total_bytes,
                            files: event.files,
                        },
                    );
                }
                Err(err) => {
                    warn!(
                        "Failed to parse downloader manifest for {}: {} ({})",
                        model_key, err, trimmed
                    );
                }
            },
            _ => match serde_json::from_value::<DownloadProgressWorkerEvent>(parsed) {
                Ok(event) => {
                    let phase = match event.event_type.as_str() {
                        "completed" => "completed",
                        "error" => "error",
                        _ => "progress",
                    };
                    let model_event = ModelDownloadEvent {
                        model: event.model.clone(),
                        phase: phase.to_string(),
                        message: event.message.clone(),
                        progress: event.progress,
                        downloaded_bytes: event.downloaded_bytes,
                        total_bytes: event.total_bytes,
                        indeterminate: event.indeterminate,
                    };
                    update_tracked_download_event(
                        state.inner(),
                        download_model,
                        model_event.clone(),
                    );

                    emit_model_download_event(app_handle, model_event);

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
            },
        }
    }

    let status = {
        let mut child_guard = child.lock().await;
        child_guard
            .wait()
            .await
            .map_err(|e| format!("Failed to wait for {model_key} downloader: {e}"))?
    };
    let stderr_output = stderr_handle.await.unwrap_or_default();
    let cancelled = take_download_cancel_requested(state.inner(), download_model);
    set_active_download_process(state.inner(), download_model, None);

    if cancelled {
        let cancelled_event = ModelDownloadEvent {
            model: download_model.key().to_string(),
            phase: "cancelled".to_string(),
            message: "Download cancelled.".to_string(),
            progress: None,
            downloaded_bytes: None,
            total_bytes: None,
            indeterminate: false,
        };
        update_tracked_download_event(state.inner(), download_model, cancelled_event.clone());
        emit_model_download_event(app_handle, cancelled_event);
        info!("{} download cancelled", model_key);
        return Err(format!("{model_key} download cancelled"));
    }

    if status.success() {
        update_tracked_download_event(
            state.inner(),
            download_model,
            ModelDownloadEvent {
                model: download_model.key().to_string(),
                phase: "completed".to_string(),
                message: "Download complete.".to_string(),
                progress: Some(100.0),
                downloaded_bytes: None,
                total_bytes: tracked_download_state(state.inner(), download_model)
                    .and_then(|tracked| tracked.manifest.and_then(|manifest| manifest.total_bytes)),
                indeterminate: false,
            },
        );
        info!("{} download completed successfully", model_key);
        Ok(())
    } else {
        let error_message = last_error_message
            .or_else(|| {
                let trimmed = stderr_output.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                }
            })
            .unwrap_or_else(|| format!("{model_key} download failed with status {status}"));
        update_tracked_download_event(
            state.inner(),
            download_model,
            ModelDownloadEvent {
                model: download_model.key().to_string(),
                phase: "error".to_string(),
                message: error_message.clone(),
                progress: None,
                downloaded_bytes: None,
                total_bytes: tracked_download_state(state.inner(), download_model)
                    .and_then(|tracked| tracked.manifest.and_then(|manifest| manifest.total_bytes)),
                indeterminate: true,
            },
        );
        Err(error_message)
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,openduck_lib=debug"));
    tracing_subscriber::fmt().with_env_filter(env_filter).init();

    info!("Starting OpenDuck application");
    let default_csm_quantized = std::env::var("OPEN_DUCK_CSM_QUANTIZE")
        .ok()
        .map(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(true);
    let persisted_config = load_persisted_app_config();

    tauri::Builder::default()
        .menu(build_app_menu)
        .on_menu_event(|app_handle, event| {
            handle_app_menu_event(app_handle, event.id().as_ref());
        })
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        let state = app.state::<AppState>();
                        let region_shortcut_str = state
                            .global_shortcut_look_at_screen_region
                            .lock()
                            .unwrap()
                            .clone();
                        let entire_shortcut_str = state
                            .global_shortcut_look_at_entire_screen
                            .lock()
                            .unwrap()
                            .clone();
                        let toggle_mute_shortcut_str =
                            state.global_shortcut_toggle_mute.lock().unwrap().clone();
                        let interrupt_shortcut_str =
                            state.global_shortcut_interrupt.lock().unwrap().clone();

                        if let Ok(region_shortcut) = region_shortcut_str.parse::<Shortcut>() {
                            if shortcut == &region_shortcut {
                                let app_handle = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    let _ = capture_screen_selection_inner(&app_handle).await;
                                });
                                return;
                            }
                        }

                        if let Ok(entire_shortcut) = entire_shortcut_str.parse::<Shortcut>() {
                            if shortcut == &entire_shortcut {
                                let app_handle = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    let _ = capture_entire_screen_inner(&app_handle).await;
                                });
                                return;
                            }
                        }

                        if let Ok(toggle_mute_shortcut) =
                            toggle_mute_shortcut_str.parse::<Shortcut>()
                        {
                            if shortcut == &toggle_mute_shortcut {
                                if let Err(err) = app.emit(TRAY_TOGGLE_MUTE_EVENT, ()) {
                                    error!("Failed to emit tray mute toggle event: {}", err);
                                }
                            }
                        }

                        if let Ok(interrupt_shortcut) = interrupt_shortcut_str.parse::<Shortcut>() {
                            if shortcut == &interrupt_shortcut {
                                let app_handle = app.clone();
                                tauri::async_runtime::spawn(async move {
                                    if let Err(err) = interrupt_tts(app_handle.clone()).await {
                                        error!("Failed to interrupt from global shortcut: {}", err);
                                    }
                                    emit_overlay_notification(
                                        &app_handle,
                                        OverlayNotificationEvent {
                                            message: "OpenDuck: Interrupted".to_string(),
                                        },
                                    );
                                });
                            }
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            let state = app.state::<AppState>();
            let region_shortcut_str = state
                .global_shortcut_look_at_screen_region
                .lock()
                .unwrap()
                .clone();
            if let Ok(shortcut) = region_shortcut_str.parse::<Shortcut>() {
                let _ = app.global_shortcut().register(shortcut);
            }
            let entire_shortcut_str = state
                .global_shortcut_look_at_entire_screen
                .lock()
                .unwrap()
                .clone();
            if let Ok(shortcut) = entire_shortcut_str.parse::<Shortcut>() {
                let _ = app.global_shortcut().register(shortcut);
            }
            let toggle_mute_shortcut_str =
                state.global_shortcut_toggle_mute.lock().unwrap().clone();
            if let Ok(shortcut) = toggle_mute_shortcut_str.parse::<Shortcut>() {
                let _ = app.global_shortcut().register(shortcut);
            }
            let interrupt_shortcut_str = state.global_shortcut_interrupt.lock().unwrap().clone();
            if let Ok(shortcut) = interrupt_shortcut_str.parse::<Shortcut>() {
                let _ = app.global_shortcut().register(shortcut);
            }

            reap_stale_model_processes(app.handle());
            create_tray(app.handle())?;
            Ok(())
        })
        .manage(PendingAppUpdate(Mutex::new(None)))
        .manage(AppState {
            audio_buffer: Mutex::new(Vec::new()),
            pre_audio_buffer: Mutex::new(VecDeque::new()),
            silent_chunks_count: Mutex::new(0),
            speaking_chunks_count: Mutex::new(0),
            current_utterance_voiced_samples: Mutex::new(0),
            is_speaking: Mutex::new(false),
            next_utterance_id: AtomicU64::new(1),
            live_transcription: Mutex::new(LiveTranscriptionState::default()),
            runtime_setup_lock: AsyncMutex::new(()),
            selected_gemma_variant: Mutex::new(GemmaVariant::E4b),
            loaded_gemma_variant: Mutex::new(None),
            gemma_download_process: Mutex::new(None),
            csm_download_process: Mutex::new(None),
            stt_download_process: Mutex::new(None),
            gemma_download_state: Mutex::new(None),
            csm_download_state: Mutex::new(None),
            stt_download_state: Mutex::new(None),
            gemma_download_cancel_requested: Mutex::new(false),
            csm_download_cancel_requested: Mutex::new(false),
            stt_download_cancel_requested: Mutex::new(false),
            server_process: Mutex::new(None),
            server_port: Mutex::new(None),
            csm_process: Mutex::new(None),
            csm_ready: Mutex::new(false),
            csm_startup_message: Mutex::new(None),
            csm_stderr_tail: Mutex::new(VecDeque::new()),
            stt_process: Mutex::new(None),
            stt_ready: Mutex::new(false),
            stt_startup_message: Mutex::new(None),
            stt_stderr_tail: Mutex::new(VecDeque::new()),
            selected_csm_model: Mutex::new(CsmModelVariant::Kokoro82m),
            loaded_csm_model: Mutex::new(None),
            selected_stt_model: Mutex::new(SttModelVariant::WhisperLargeV3Turbo),
            loaded_stt_model: Mutex::new(None),
            selected_csm_voice: Mutex::new(CsmVoice::Female),
            selected_csm_quantized: Mutex::new(default_csm_quantized),
            csm_reference_audio_path: Mutex::new(None),
            csm_reference_text: Mutex::new(None),
            next_csm_request_id: AtomicU64::new(1),
            next_stt_request_id: AtomicU64::new(1),
            next_generation_id: AtomicU64::new(1),
            active_generation: Mutex::new(None),
            conversation_turns: Mutex::new(VecDeque::new()),
            conversation_image_paths: Mutex::new(Vec::new()),
            pending_screen_captures: Mutex::new(Vec::new()),
            screen_capture_in_progress: Mutex::new(false),
            transient_tray_title: Mutex::new(None),
            transient_tray_icon: Mutex::new(None),
            call_stage_phase: Mutex::new("idle".to_string()),
            voice_system_prompt: Mutex::new(DEFAULT_VOICE_SYSTEM_PROMPT.to_string()),
            conversation_session_id: AtomicU64::new(1),
            current_session_id: Mutex::new(None),
            current_session_title: Mutex::new(None),
            call_started_at: Mutex::new(None),
            processing_audio_started_at: Mutex::new(None),
            processing_audio_latency_request_id: Mutex::new(None),
            tray_timer_generation: AtomicU64::new(0),
            tray_title_override_generation: AtomicU64::new(0),
            call_in_progress: Mutex::new(false),
            call_muted: Mutex::new(false),
            tts_playback_active: Mutex::new(false),
            tray_pong_playback_enabled: Mutex::new(true),
            tray_pong_playback_hydrated: Mutex::new(false),
            tray_pong_playback_modified_before_hydration: Mutex::new(false),
            end_of_utterance_silence_ms: Mutex::new(END_OF_UTTERANCE_SILENCE_MS),
            auto_continue_silence_ms: Mutex::new(DEFAULT_AUTO_CONTINUE_SILENCE_MS),
            auto_continue_max_count: Mutex::new(DEFAULT_AUTO_CONTINUE_MAX_COUNT),
            auto_continue_timer_generation: AtomicU64::new(0),
            auto_continue_tracker: Mutex::new(None),
            llm_context_turn_limit: Mutex::new(DEFAULT_LLM_CONTEXT_TURN_LIMIT),
            llm_image_history_limit: Mutex::new(DEFAULT_LLM_IMAGE_HISTORY_LIMIT),
            conversation_log_has_visible_images: Mutex::new(false),
            selected_ollama_model: Mutex::new("gemma2:2b".to_string()),
            ollama_base_url: Mutex::new(
                persisted_config
                    .external_llm_providers
                    .ollama
                    .base_url
                    .clone(),
            ),
            ollama_api_key: Mutex::new(
                persisted_config
                    .external_llm_providers
                    .ollama
                    .api_key
                    .clone(),
            ),
            selected_lmstudio_model: Mutex::new(String::new()),
            lmstudio_base_url: Mutex::new(
                persisted_config
                    .external_llm_providers
                    .lmstudio
                    .base_url
                    .clone(),
            ),
            lmstudio_api_key: Mutex::new(
                persisted_config
                    .external_llm_providers
                    .lmstudio
                    .api_key
                    .clone(),
            ),
            selected_openai_compatible_model: Mutex::new(String::new()),
            openai_compatible_base_url: Mutex::new(
                persisted_config
                    .external_llm_providers
                    .openai_compatible
                    .base_url
                    .clone(),
            ),
            openai_compatible_api_key: Mutex::new(
                persisted_config
                    .external_llm_providers
                    .openai_compatible
                    .api_key
                    .clone(),
            ),
            global_shortcut_look_at_screen_region: Mutex::new("Command+Shift+L".to_string()),
            global_shortcut_look_at_screen_region_hydrated: Mutex::new(false),
            global_shortcut_look_at_screen_region_modified_before_hydration: Mutex::new(false),
            global_shortcut_look_at_entire_screen: Mutex::new("Command+Shift+Option+L".to_string()),
            global_shortcut_look_at_entire_screen_hydrated: Mutex::new(false),
            global_shortcut_look_at_entire_screen_modified_before_hydration: Mutex::new(false),
            global_shortcut_toggle_mute: Mutex::new("Command+Shift+Option+U".to_string()),
            global_shortcut_toggle_mute_hydrated: Mutex::new(false),
            global_shortcut_toggle_mute_modified_before_hydration: Mutex::new(false),
            global_shortcut_interrupt: Mutex::new("Command+Shift+Option+I".to_string()),
            global_shortcut_interrupt_hydrated: Mutex::new(false),
            global_shortcut_interrupt_modified_before_hydration: Mutex::new(false),
            next_conversation_entry_id: AtomicU64::new(1),
            last_tray_icon_variant: Mutex::new(None),
            last_tray_menu_state: Mutex::new(None),
            last_tray_title: Mutex::new(None),
            is_quitting: Mutex::new(false),
            vad: Mutex::new(None),
            screen_capture_child: Mutex::new(None),
        })
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            capture_screen_selection,
            clear_pending_screen_capture,
            attach_pasted_screen_capture,
            is_main_window_visible_to_user,
            receive_audio_chunk,
            ping,
            get_build_info,
            check_for_app_update,
            install_app_update,
            restart_app,
            refresh_runtime_caches,
            set_voice_system_prompt,
            set_csm_reference_voice,
            export_contact_profile,
            reset_call_session,
            ensure_runtime_dependencies,
            interrupt_tts,
            start_call_timer,
            stop_call_timer,
            set_call_muted,
            set_tts_playback_active,
            set_pong_playback_enabled,
            initialize_pong_playback_preference,
            set_end_of_utterance_silence_ms,
            set_auto_continue_silence_ms,
            set_auto_continue_max_count,
            set_llm_context_turn_limit,
            set_llm_image_history_limit,
            get_global_shortcut_look_at_entire_screen,
            initialize_global_shortcut_look_at_entire_screen,
            set_global_shortcut_look_at_entire_screen,
            get_global_shortcut_look_at_screen_region,
            initialize_global_shortcut_look_at_screen_region,
            set_global_shortcut_look_at_screen_region,
            get_global_shortcut_toggle_mute,
            initialize_global_shortcut_toggle_mute,
            set_global_shortcut_toggle_mute,
            get_global_shortcut_interrupt,
            initialize_global_shortcut_interrupt,
            set_global_shortcut_interrupt,
            clear_conversation_context_images,
            sync_conversation_log_has_visible_images,
            get_gemma_variant,
            set_gemma_variant,
            get_stt_model_variant,
            set_stt_model_variant,
            check_model_status,
            check_ollama_status,
            get_ollama_models,
            get_ollama_model,
            set_ollama_model,
            get_ollama_config,
            set_ollama_config,
            check_lmstudio_status,
            get_lmstudio_models,
            get_lmstudio_model,
            set_lmstudio_model,
            get_lmstudio_config,
            set_lmstudio_config,
            check_openai_compatible_status,
            get_openai_compatible_models,
            get_openai_compatible_model,
            set_openai_compatible_model,
            get_openai_compatible_config,
            set_openai_compatible_config,
            check_csm_status,
            check_stt_status,
            clear_model_cache,
            get_model_download_status,
            download_model,
            download_csm_model,
            download_stt_model,
            cancel_model_download,
            is_server_running,
            is_csm_running,
            is_stt_running,
            get_model_memory_usage,
            start_server,
            start_csm_server,
            start_stt_server,
            get_csm_model_variant,
            get_csm_quantize,
            set_csm_model_variant,
            set_csm_quantize,
            set_csm_voice,
            update_conversation_context_entry,
            delete_conversation_context_entry,
            get_sessions,
            load_session,
            delete_session,
            rename_session,
            fork_session,
            search_sessions,
            start_new_session,
            get_current_session_id,
            update_current_session_title,
        ])
        .on_window_event(|window, event| {
            #[cfg(target_os = "macos")]
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<AppState>();
                if *state.is_quitting.lock().unwrap() {
                    return;
                }

                api.prevent_close();
                if let Err(err) = window.hide() {
                    error!("Failed to hide window to tray: {}", err);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app_handle.state::<AppState>();
                let mut quitting_guard = state.is_quitting.lock().unwrap();
                *quitting_guard = true;
                cleanup_before_app_exit(app_handle);
            }
        });
}
