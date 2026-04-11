// Serializable payloads and emit helpers for events sent from the Tauri backend to the frontend.
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tracing::error;

use crate::{constants::*, pending_screen_capture_file_name, AppState};

#[derive(Clone, Serialize)]
pub(crate) struct CsmAudioStartEvent {
    pub(crate) request_id: u64,
}

#[derive(Clone, Serialize)]
pub(crate) struct CsmAudioQueuedEvent {
    pub(crate) request_id: u64,
}

#[derive(Clone, Serialize)]
pub(crate) struct CsmAudioChunkEvent {
    pub(crate) request_id: u64,
    pub(crate) audio_wav_base64: String,
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
#[serde(rename_all = "camelCase")]
pub(crate) struct TranscriptEvent {
    pub(crate) text: String,
    pub(crate) image_path: Option<String>,
    pub(crate) image_data_url: Option<String>,
}

#[derive(Clone, Serialize)]
pub(crate) struct AssistantResponseEvent {
    pub(crate) request_id: u64,
    pub(crate) text: String,
    pub(crate) is_final: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScreenCaptureEvent {
    pub(crate) phase: String,
    pub(crate) message: String,
    pub(crate) has_pending_attachment: bool,
    pub(crate) file_name: Option<String>,
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

pub(crate) fn emit_assistant_response(app_handle: &AppHandle, payload: AssistantResponseEvent) {
    if let Err(err) = app_handle.emit(ASSISTANT_RESPONSE_EVENT, payload) {
        error!("Failed to emit assistant response event: {}", err);
    }
}

pub(crate) fn emit_screen_capture_event(app_handle: &AppHandle, phase: &str, message: &str) {
    let state = app_handle.state::<AppState>();
    let file_name = pending_screen_capture_file_name(state.inner());
    if let Err(err) = app_handle.emit(
        SCREEN_CAPTURE_EVENT,
        ScreenCaptureEvent {
            phase: phase.to_string(),
            message: message.to_string(),
            has_pending_attachment: file_name.is_some(),
            file_name,
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
