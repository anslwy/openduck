// Shared frontend types for the OpenDuck home page, including Tauri events, contacts, and model selections.
export type CsmAudioStartEvent = {
    request_id: number;
    append_to_assistant_entry_id?: number | null;
};

export type CsmAudioQueuedEvent = {
    request_id: number;
    text: string;
    index: number;
};

export type CsmAudioChunkEvent = {
    request_id: number;
    audio_wav_base64: string;
    is_first_chunk: boolean;
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

export type ProcessingAudioLatencyEvent = {
    kind: "audio" | "first_message_chunk" | "first_audio_chunk";
    request_id?: number | null;
    latency_ms: number;
};

export type TranscriptEvent = {
    text: string;
    imagePaths: string[];
    imageDataUrls: string[];
};

export type TranscriptPartialEvent = {
    text: string;
};

export type AiSubtitleEvent = {
    text: string;
};

export type AiSubtitleTargetLanguage =
    | "none"
    | "ar"
    | "bn"
    | "zh"
    | "tw"
    | "en"
    | "fr"
    | "de"
    | "gu"
    | "hi"
    | "id"
    | "it"
    | "jp"
    | "ko"
    | "mr"
    | "fa"
    | "pt"
    | "pa"
    | "ru"
    | "es"
    | "ta"
    | "te"
    | "th"
    | "tr"
    | "ur"
    | "vi";

export type AssistantResponseEvent = {
    request_id: number;
    text: string;
    reasoning_text: string;
    is_final: boolean;
    append_to_assistant_entry_id: number | null;
    translations: Record<string, string>;
};

export type AssistantTranslationsEvent = {
    request_id: number;
    translations: Record<number, Record<string, string>>;
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
    attachmentCount: number;
    fileName?: string | null;
    imageDataUrls: string[];
};

export type TrayEndCallEvent = Record<string, never>;
export type TrayToggleMuteEvent = Record<string, never>;
export type TrayPongPlaybackEvent = {
    enabled: boolean;
};
export type ShowAboutModalEvent = Record<string, never>;
export type TriggerAppUpdateCheckEvent = Record<string, never>;
export type OpenDuckContactImportEvent = {
    sourcePath: string;
    rawText?: string | null;
    error?: string | null;
};
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

export type GemmaVariant =
    | "e4b"
    | "e2b"
    | "ollama"
    | "lmstudio"
    | "openai_compatible";
export type ExternalGemmaVariant = Extract<
    GemmaVariant,
    "ollama" | "lmstudio" | "openai_compatible"
>;
export type CsmModelVariant =
    | "expressiva_1b"
    | "kokoro_82m"
    | "cosyvoice2_0_5b"
    | "cosyvoice3_0_5b_8bit"
    | "cosyvoice3_0_5b_4bit"
    | "cosyvoice3_0_5b_fp16"
    | "chatterbox_turbo_8bit"
    | "chatterbox_turbo_fp16";
export type KokoroLanguage =
    | "american_english"
    | "british_english"
    | "japanese"
    | "mandarin_chinese"
    | "spanish"
    | "french"
    | "hindi"
    | "italian"
    | "brazilian_portuguese";
export type SttModelVariant = "gemma" | "distil_whisper_large_v3" | "whisper_large_v3_turbo";

export type ConversationLogEntry = {
    id: number;
    role: "user" | "assistant";
    text: string;
    reasoningText?: string;
    imageUrls: string[];
    contextEntryId: number | null;
    isContextBacked: boolean;
    translations: Record<string, string>;
};

export type StoredContactProfile = {
    id: string;
    name: string;
    prompt: string;
    hasCustomIcon: boolean;
    gender?: ContactGender | null;
    refAudio?: string | null;
    refText?: string | null;
};

export type ContactGender = "male" | "female";

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
    releaseNotesUrl?: string | null;
};

export type StoredModelPreferences = {
    version: 1;
    gemmaVariant: GemmaVariant;
    csmModel: CsmModelVariant;
    kokoroLanguage?: KokoroLanguage | null;
    sttModel: SttModelVariant;
    ollamaModel?: string | null;
    lmstudioModel?: string | null;
    openaiCompatibleModel?: string | null;
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

export type SearchResult = {
    session_id: string;
    session_title: string;
    matched_text: string;
    updated_at: number;
};

export type ConversationTurn = {
    user_entry_id: number;
    assistant_entry_id: number;
    user_text: String;
    assistant_text: String;
    image_paths: string[];
    user_image_data_urls: string[];
    translations?: Record<string, string>;
};
