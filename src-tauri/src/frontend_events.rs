// Serializable payloads and emit helpers for events sent from the Tauri backend to the frontend.
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tracing::error;

use crate::{constants::*, pending_screen_capture_file_name, AppState};

#[derive(Clone, Serialize)]
pub(crate) struct CsmAudioStartEvent {
    pub(crate) request_id: u64,
    pub(crate) append_to_assistant_entry_id: Option<u64>,
}

#[derive(Clone, Serialize)]
pub(crate) struct CsmAudioQueuedEvent {
    pub(crate) request_id: u64,
    pub(crate) text: String,
    pub(crate) index: usize,
}

#[derive(Clone, Serialize)]
pub(crate) struct AssistantTranslationsEvent {
    pub(crate) request_id: u64,
    pub(crate) translations:
        std::collections::HashMap<usize, std::collections::HashMap<String, String>>,
}

#[derive(Clone, Serialize)]
pub(crate) struct AssistantResponseEvent {
    pub(crate) request_id: u64,
    pub(crate) text: String,
    pub(crate) reasoning_text: String,
    pub(crate) is_final: bool,
    pub(crate) append_to_assistant_entry_id: Option<u64>,
    pub(crate) translations: std::collections::HashMap<String, String>,
}

pub(crate) const ASSISTANT_TRANSLATIONS_EVENT: &str = "assistant-translations";

pub(crate) fn emit_assistant_translations(
    app_handle: &AppHandle,
    payload: AssistantTranslationsEvent,
) {
    if let Err(err) = app_handle.emit(ASSISTANT_TRANSLATIONS_EVENT, payload) {
        error!("Failed to emit assistant translations event: {}", err);
    }
}

#[derive(Clone, Serialize)]
pub(crate) struct CsmAudioChunkEvent {
    pub(crate) request_id: u64,
    pub(crate) audio_wav_base64: String,
    pub(crate) is_first_chunk: bool,
}

#[derive(Clone, Serialize)]
pub(crate) struct CsmAudioDoneEvent {
    pub(crate) request_id: u64,
}

#[derive(Clone, Serialize)]
pub(crate) struct CsmAudioStopEvent {}

#[derive(Clone, Serialize)]
pub(crate) struct CsmErrorEvent {
    pub(crate) request_id: Option<u64>,
    pub(crate) message: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct CsmStatusEvent {
    pub(crate) message: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct SttStatusEvent {
    pub(crate) message: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct CallStageEvent {
    pub(crate) phase: String,
    pub(crate) message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProcessingAudioLatencyKind {
    Audio,
    FirstMessageChunk,
    FirstAudioChunk,
}

#[derive(Clone, Serialize)]
pub(crate) struct ProcessingAudioLatencyEvent {
    pub(crate) kind: ProcessingAudioLatencyKind,
    pub(crate) request_id: Option<u64>,
    pub(crate) latency_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscriptEvent {
    pub(crate) text: String,
    pub(crate) image_paths: Vec<String>,
    pub(crate) image_data_urls: Vec<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct TranscriptPartialEvent {
    pub(crate) text: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct OverlayNotificationEvent {
    pub(crate) message: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScreenCaptureEvent {
    pub(crate) phase: String,
    pub(crate) message: String,
    pub(crate) has_pending_attachment: bool,
    pub(crate) attachment_count: usize,
    pub(crate) file_name: Option<String>,
    pub(crate) image_data_urls: Vec<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct ModelDownloadEvent {
    pub(crate) model: String,
    pub(crate) phase: String,
    pub(crate) message: String,
    pub(crate) progress: Option<f32>,
    pub(crate) downloaded_bytes: Option<u64>,
    pub(crate) total_bytes: Option<u64>,
    pub(crate) indeterminate: bool,
}

#[derive(Clone, Serialize)]
pub(crate) struct RuntimeSetupStatusEvent {
    pub(crate) phase: String,
    pub(crate) message: String,
}

#[derive(Clone, Serialize)]
pub(crate) struct TrayPongPlaybackEvent {
    pub(crate) enabled: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ConversationContextCommittedEvent {
    pub(crate) request_id: u64,
    pub(crate) user_entry_id: u64,
    pub(crate) assistant_entry_id: u64,
    pub(crate) user_text: String,
    pub(crate) assistant_text: String,
    pub(crate) session_title: Option<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct ConversationImageHistoryClearedEvent {}

#[derive(Clone, Serialize)]
pub(crate) struct ShowAboutModalEvent {}

#[derive(Clone, Serialize)]
pub(crate) struct TriggerAppUpdateCheckEvent {}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OpenDuckContactImportEvent {
    pub(crate) source_path: String,
    pub(crate) raw_text: Option<String>,
    pub(crate) error: Option<String>,
}

pub(crate) fn emit_csm_error(app_handle: &AppHandle, payload: CsmErrorEvent) {
    if let Err(err) = app_handle.emit(CSM_ERROR_EVENT, payload) {
        error!("Failed to emit CSM error event: {}", err);
    }
}

pub(crate) fn emit_csm_audio_start(app_handle: &AppHandle, payload: CsmAudioStartEvent) {
    if let Err(err) = app_handle.emit(CSM_AUDIO_START_EVENT, payload) {
        error!("Failed to emit CSM audio start event: {}", err);
    }
}

pub(crate) fn emit_csm_audio_queued(app_handle: &AppHandle, payload: CsmAudioQueuedEvent) {
    if let Err(err) = app_handle.emit(CSM_AUDIO_QUEUED_EVENT, payload) {
        error!("Failed to emit CSM audio queued event: {}", err);
    }
}

pub(crate) fn emit_csm_audio_stop(app_handle: &AppHandle) {
    if let Err(err) = app_handle.emit(CSM_AUDIO_STOP_EVENT, CsmAudioStopEvent {}) {
        error!("Failed to emit CSM audio stop event: {}", err);
    }
}

pub(crate) fn emit_csm_status(app_handle: &AppHandle, payload: CsmStatusEvent) {
    if let Err(err) = app_handle.emit(CSM_STATUS_EVENT, payload) {
        error!("Failed to emit CSM status event: {}", err);
    }
}

pub(crate) fn emit_stt_status(app_handle: &AppHandle, payload: SttStatusEvent) {
    if let Err(err) = app_handle.emit(STT_STATUS_EVENT, payload) {
        error!("Failed to emit STT status event: {}", err);
    }
}

pub(crate) fn emit_call_stage(app_handle: &AppHandle, phase: &str, message: &str) {
    {
        let state = app_handle.state::<crate::AppState>();
        let mut phase_guard = state.call_stage_phase.lock().unwrap();
        *phase_guard = phase.to_string();

        match phase {
            "processing_audio" => {
                *state.processing_audio_started_at.lock().unwrap() =
                    Some(std::time::Instant::now());
                *state.processing_audio_latency_request_id.lock().unwrap() = None;
            }
            "listening" => {
                *state.processing_audio_started_at.lock().unwrap() = None;
                *state.processing_audio_latency_request_id.lock().unwrap() = None;
            }
            _ => {}
        }
    }
    crate::refresh_tray_presentation(app_handle);

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

pub(crate) fn emit_transcript_event(app_handle: &AppHandle, payload: TranscriptEvent) {
    if let Err(err) = app_handle.emit(TRANSCRIPT_EVENT, payload) {
        error!("Failed to emit transcript event: {}", err);
    }
}

pub(crate) fn emit_transcript_partial_event(
    app_handle: &AppHandle,
    payload: TranscriptPartialEvent,
) {
    if let Err(err) = app_handle.emit(TRANSCRIPT_PARTIAL_EVENT, payload) {
        error!("Failed to emit transcript partial event: {}", err);
    }
}

pub(crate) fn emit_processing_audio_latency(
    app_handle: &AppHandle,
    payload: ProcessingAudioLatencyEvent,
) {
    if let Err(err) = app_handle.emit(PROCESSING_AUDIO_LATENCY_EVENT, payload) {
        error!("Failed to emit processing-audio-latency event: {}", err);
    }
}

pub(crate) fn emit_assistant_response(app_handle: &AppHandle, payload: AssistantResponseEvent) {
    if let Err(err) = app_handle.emit(ASSISTANT_RESPONSE_EVENT, payload) {
        error!("Failed to emit assistant response event: {}", err);
    }
}

pub(crate) fn emit_overlay_notification(app_handle: &AppHandle, payload: OverlayNotificationEvent) {
    if let Err(err) = app_handle.emit(OVERLAY_NOTIFICATION_EVENT, payload) {
        error!("Failed to emit overlay-notification event: {}", err);
    }
}

pub(crate) fn emit_screen_capture_event(app_handle: &AppHandle, phase: &str, message: &str) {
    let state = app_handle.state::<AppState>();
    let file_name = pending_screen_capture_file_name(state.inner());
    let attachment_count = crate::pending_screen_capture_count(state.inner());
    let image_data_urls = {
        let captures = state.pending_screen_captures.lock().unwrap();
        captures
            .iter()
            .filter_map(|path| crate::load_image_data_url(path))
            .collect::<Vec<_>>()
    };

    if let Err(err) = app_handle.emit(
        SCREEN_CAPTURE_EVENT,
        ScreenCaptureEvent {
            phase: phase.to_string(),
            message: message.to_string(),
            has_pending_attachment: attachment_count > 0,
            attachment_count,
            file_name,
            image_data_urls,
        },
    ) {
        error!("Failed to emit screen capture event: {}", err);
    }
}

pub(crate) fn emit_model_download_event(app_handle: &AppHandle, payload: ModelDownloadEvent) {
    if let Err(err) = app_handle.emit(MODEL_DOWNLOAD_EVENT, payload) {
        error!("Failed to emit model download event: {}", err);
    }
}

pub(crate) fn emit_runtime_setup_status(app_handle: &AppHandle, payload: RuntimeSetupStatusEvent) {
    if let Err(err) = app_handle.emit(RUNTIME_SETUP_EVENT, payload) {
        error!("Failed to emit runtime setup status event: {}", err);
    }
}

pub(crate) fn emit_tray_pong_playback(app_handle: &AppHandle, enabled: bool) {
    if let Err(err) = app_handle.emit(TRAY_PONG_PLAYBACK_EVENT, TrayPongPlaybackEvent { enabled }) {
        error!("Failed to emit tray pong playback event: {}", err);
    }
}

pub(crate) fn emit_play_tray_pong(app_handle: &AppHandle) {
    if let Err(err) = app_handle.emit(PLAY_TRAY_PONG_EVENT, ()) {
        error!("Failed to emit play tray pong event: {}", err);
    }
}

pub(crate) fn emit_conversation_context_committed(
    app_handle: &AppHandle,
    payload: ConversationContextCommittedEvent,
) {
    if let Err(err) = app_handle.emit(CONVERSATION_CONTEXT_COMMITTED_EVENT, payload) {
        error!(
            "Failed to emit conversation context committed event: {}",
            err
        );
    }
}

pub(crate) fn emit_conversation_image_history_cleared(app_handle: &AppHandle) {
    if let Err(err) = app_handle.emit(
        CONVERSATION_IMAGE_HISTORY_CLEARED_EVENT,
        ConversationImageHistoryClearedEvent {},
    ) {
        error!(
            "Failed to emit conversation image history cleared event: {}",
            err
        );
    }
}

pub(crate) fn emit_show_about_modal(app_handle: &AppHandle) {
    if let Err(err) = app_handle.emit(SHOW_ABOUT_MODAL_EVENT, ShowAboutModalEvent {}) {
        error!("Failed to emit show-about-modal event: {}", err);
    }
}

pub(crate) fn emit_trigger_app_update_check(app_handle: &AppHandle) {
    if let Err(err) = app_handle.emit(
        TRIGGER_APP_UPDATE_CHECK_EVENT,
        TriggerAppUpdateCheckEvent {},
    ) {
        error!("Failed to emit trigger-app-update-check event: {}", err);
    }
}

pub(crate) fn emit_openduck_contact_import(
    app_handle: &AppHandle,
    payload: OpenDuckContactImportEvent,
) {
    if let Err(err) = app_handle.emit(OPENDUCK_CONTACT_IMPORT_EVENT, payload) {
        error!("Failed to emit OpenDuck contact import event: {}", err);
    }
}
