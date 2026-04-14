// Shared frontend types for the OpenDuck home page, including Tauri events, contacts, and model selections.
export type CsmAudioStartEvent = {
    request_id: number;
};

export type CsmAudioQueuedEvent = {
    request_id: number;
};

export type CsmAudioChunkEvent = {
    request_id: number;
    audio_wav_base64: string;
};

export type CsmAudioDoneEvent = {
    request_id: number;
};

export type CsmAudioStopEvent = Record<string, never>;

export type CsmErrorEvent = {
    request_id?: number | null;
    message: string;
};

export type CsmStatusEvent = {
    message: string;
};

export type SttStatusEvent = {
    message: string;
};

export type CallStagePhase =
    | "idle"
    | "listening"
    | "processing_audio"
    | "thinking"
    | "generating_audio"
    | "speaking";

export type CallStageEvent = {
    phase: CallStagePhase;
    message: string;
};

export type TranscriptEvent = {
    text: string;
    imageDataUrl?: string | null;
};

export type AssistantResponseEvent = {
    request_id: number;
    text: string;
    is_final: boolean;
};

export type ScreenCapturePhase =
    | "capturing"
    | "ready"
    | "cancelled"
    | "cleared"
    | "consumed"
    | "error";

export type ScreenCaptureEvent = {
    phase: ScreenCapturePhase;
    message: string;
    hasPendingAttachment: boolean;
    fileName?: string | null;
};

export type TrayEndCallEvent = Record<string, never>;
export type TrayToggleMuteEvent = Record<string, never>;
export type TrayPongPlaybackEvent = {
    enabled: boolean;
};
export type ShowAboutModalEvent = Record<string, never>;
export type TriggerAppUpdateCheckEvent = Record<string, never>;
export type ConversationContextCommittedEvent = {
    requestId: number;
    userEntryId: number;
    assistantEntryId: number;
    userText: string;
    assistantText: string;
    sessionTitle?: string | null;
};
export type ConversationImageHistoryClearedEvent = Record<string, never>;

export type DownloadModelKey = "gemma" | "csm" | "stt";

export type ModelDownloadProgressEvent = {
    model: DownloadModelKey;
    phase: "progress" | "completed" | "error" | "cancelled";
    message: string;
    progress?: number | null;
    downloaded_bytes?: number | null;
    total_bytes?: number | null;
    indeterminate: boolean;
};

export type RuntimeSetupStatusEvent = {
    phase: "starting" | "progress" | "completed" | "error";
    message: string;
};

export type ModelMemoryUsageEntry = {
    key: DownloadModelKey;
    label: string;
    detail?: string | null;
    bytes: number;
    root_pid: number;
    process_count: number;
};

export type ModelMemoryUsageSnapshot = {
    total_bytes: number;
    models: ModelMemoryUsageEntry[];
};

export type GemmaVariant = "e4b" | "e2b" | "ollama";
export type CsmModelVariant =
    | "expressiva_1b"
    | "kokoro_82m"
    | "cosyvoice2_0_5b"
    | "cosyvoice3_0_5b_8bit"
    | "cosyvoice3_0_5b_4bit"
    | "cosyvoice3_0_5b_fp16";
export type SttModelVariant = "gemma" | "distil_whisper_large_v3" | "whisper_large_v3_turbo";

export type ConversationLogEntry = {
    id: number;
    role: "user" | "assistant";
    text: string;
    imageUrl: string | null;
    contextEntryId: number | null;
};

export type StoredContactProfile = {
    id: string;
    name: string;
    prompt: string;
    hasCustomIcon: boolean;
    refAudio?: string | null;
    refText?: string | null;
};

export type ContactProfile = StoredContactProfile & {
    iconDataUrl: string | null;
};

export type StoredContactsPayload = {
    version: 1;
    selectedContactId: string;
    contacts: StoredContactProfile[];
};

export type ContactExportResult = {
    savedPath: string;
};

export type BuildInfo = {
    app_name: string;
    version: string;
    version_label?: string | null;
    build_channel?: string | null;
    build_number?: string | null;
    git_sha?: string | null;
    git_short_sha?: string | null;
    build_id?: string | null;
    is_dirty: boolean;
    dirty_files?: string[] | null;
    copy_text: string;
};

export type AppUpdateStatus =
    | "idle"
    | "checking"
    | "available"
    | "up_to_date"
    | "installing"
    | "installed"
    | "error";

export type AppUpdateInfo = {
    version: string;
    currentVersion: string;
    notes?: string | null;
    publishedAt?: string | null;
    target: string;
};

export type StoredModelPreferences = {
    version: 1;
    gemmaVariant: GemmaVariant;
    csmModel: CsmModelVariant;
    sttModel: SttModelVariant;
    ollamaModel?: string | null;
};

export type ModelSelection = Omit<StoredModelPreferences, "version">;
export type ModelPreset = "lite" | "normal" | "realistic" | "custom";

export type SelectOption<T extends string> = {
    value: T;
    label: string;
    disabled?: boolean;
};

export type SessionMetadata = {
    id: string;
    title: string;
    created_at: number;
    updated_at: number;
};

export type ConversationTurn = {
    user_entry_id: number;
    assistant_entry_id: number;
    user_text: string;
    assistant_text: string;
    image_path?: string | null;
    user_image_data_url?: string | null;
};
