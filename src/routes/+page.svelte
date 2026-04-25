<!-- Main voice-call page that wires the app state together and delegates repeated UI sections to home components. -->
<script lang="ts">
    import "./home.css";

    import { onDestroy, onMount } from "svelte";
    import { fade } from "svelte/transition";
    import { invoke, convertFileSrc } from "@tauri-apps/api/core";
    import { check, type Update } from "@tauri-apps/plugin-updater";
    import { relaunch } from "@tauri-apps/plugin-process";
    import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
    import { save } from "@tauri-apps/plugin-dialog";
    import AboutModal from "$lib/components/home/AboutModal.svelte";
    import ContactsModal from "$lib/components/home/ContactsModal.svelte";
    import ConversationPopup from "$lib/components/home/ConversationPopup.svelte";
    import SessionsPopup from "$lib/components/home/SessionsPopup.svelte";
    import UpdatePromptModal from "$lib/components/home/UpdatePromptModal.svelte";
    import AppHeader from "$lib/components/home/AppHeader.svelte";
    import SearchModal from "$lib/components/home/SearchModal.svelte";
    import ExternalLlmConfigModal from "$lib/components/home/ExternalLlmConfigModal.svelte";
    import SubtitleTranslationLlmConfigModal from "$lib/components/home/SubtitleTranslationLlmConfigModal.svelte";
    import GemmaBanner from "$lib/components/home/GemmaBanner.svelte";
    import SpeechBanner from "$lib/components/home/SpeechBanner.svelte";
    import SttBanner from "$lib/components/home/SttBanner.svelte";
    import {
        createImportedContactFromRawText,
        createContactId,
        createDefaultContact,
        createStoredContactsPayload,
        deleteStoredContactIcon,
        getContactDisplayName,
        loadContactsFromStorage,
        readFileAsDataUrl,
        saveStoredContactIcon,
        slugifyContactName,
    } from "$lib/openduck/contacts";
    import {
        AUTO_CONTINUE_MAX_COUNT_STORAGE_KEY,
        AUTO_CONTINUE_SILENCE_STEP_MS,
        AUTO_CONTINUE_SILENCE_STORAGE_KEY,
        APP_UPDATE_PREFERENCES_STORAGE_KEY,
        DEFAULT_CONTACT_PROMPT,
        CONTACTS_STORAGE_KEY,
        DEFAULT_AUTO_CONTINUE_MAX_COUNT,
        DEFAULT_AUTO_CONTINUE_SILENCE_MS,
        DEFAULT_CONTACT_ID,
        DEFAULT_CSM_MODEL,
        DEFAULT_END_OF_UTTERANCE_SILENCE_MS,
        DEFAULT_GEMMA_VARIANT,
        DEFAULT_LLM_CONTEXT_TURN_LIMIT,
        DEFAULT_LLM_IMAGE_HISTORY_LIMIT,
        DEFAULT_LMSTUDIO_MODEL,
        DEFAULT_OPENAI_COMPATIBLE_MODEL,
        DEFAULT_OLLAMA_MODEL,
        DEFAULT_STT_MODEL,
        END_OF_UTTERANCE_SILENCE_STEP_MS,
        END_OF_UTTERANCE_SILENCE_STORAGE_KEY,
        LLM_CONTEXT_TURN_LIMIT_STORAGE_KEY,
        LLM_IMAGE_HISTORY_LIMIT_STORAGE_KEY,
        MODEL_PREFERENCES_STORAGE_KEY,
        MAX_END_OF_UTTERANCE_SILENCE_MS,
        MAX_LLM_CONTEXT_TURN_LIMIT,
        MAX_LLM_IMAGE_HISTORY_LIMIT,
        MAX_AUTO_CONTINUE_MAX_COUNT,
        MAX_AUTO_CONTINUE_SILENCE_MS,
        MIN_END_OF_UTTERANCE_SILENCE_MS,
        MIN_LLM_CONTEXT_TURN_LIMIT,
        MIN_LLM_IMAGE_HISTORY_LIMIT,
        MIN_AUTO_CONTINUE_MAX_COUNT,
        MIN_AUTO_CONTINUE_SILENCE_MS,
        MODEL_PRESETS,
        PONG_PLAYBACK_STORAGE_KEY,
        SELECT_LAST_SESSION_STORAGE_KEY,
        AUTO_LOAD_MODELS_ON_STARTUP_STORAGE_KEY,
        SHOW_STAT_STORAGE_KEY,
        SHOW_SUBTITLE_STORAGE_KEY,
        SHOW_AI_SUBTITLE_STORAGE_KEY,
        AI_SUBTITLE_TARGET_LANGUAGE_STORAGE_KEY,
        SHOW_CALL_TIMER_STORAGE_KEY,
        SHOW_HIDDEN_WINDOW_OVERLAY_STORAGE_KEY,
        AUTO_UNMUTE_ON_PASTED_SCREENSHOT_STORAGE_KEY,
        GLOBAL_SHORTCUT_STORAGE_KEY,
        GLOBAL_SHORTCUT_ENTIRE_SCREEN_STORAGE_KEY,
        GLOBAL_SHORTCUT_TOGGLE_MUTE_STORAGE_KEY,
        GLOBAL_SHORTCUT_INTERRUPT_STORAGE_KEY,
        DEFAULT_AUTO_UNMUTE_ON_PASTED_SCREENSHOT,
        DEFAULT_SHOW_AI_SUBTITLE,
        DEFAULT_AI_SUBTITLE_TARGET_LANGUAGE,
        DEFAULT_SHOW_CALL_TIMER,
        DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY,
        DEFAULT_AUTO_LOAD_MODELS_ON_STARTUP,
        DEFAULT_AUTO_CHECK_APP_UPDATES,
        DEFAULT_GLOBAL_SHORTCUT,
        DEFAULT_GLOBAL_SHORTCUT_ENTIRE_SCREEN,
        DEFAULT_GLOBAL_SHORTCUT_TOGGLE_MUTE,
        DEFAULT_GLOBAL_SHORTCUT_INTERRUPT,
    } from "$lib/openduck/config";
    import {
        createReleaseNotesPreview,
        formatAppUpdateInstallError,
        formatDownloadPercent,
        formatMemoryUsage,
        normalizeDownloadErrorMessage,
        normalizeErrorMessage,
    } from "$lib/openduck/format";
    import {
        getModelPresetDescription,
        loadModelPreferencesFromStorage,
        resolveModelPreset,
    } from "$lib/openduck/model-preferences";
    import type {
        AppUpdateInfo,
        AppUpdateStatus,
        AiSubtitleEvent,
        AiSubtitleTargetLanguage,
        AssistantResponseEvent,
        AssistantTranslationsEvent,
        BuildInfo,
        CallStageEvent,
        CallStagePhase,
        ContactGender,
        ContactExportResult,
        ContactProfile,
        ConversationContextCommittedEvent,
        ConversationImageHistoryClearedEvent,
        ConversationLogEntry,
        CsmAudioChunkEvent,
        CsmAudioDoneEvent,
        CsmAudioQueuedEvent,
        CsmAudioStartEvent,
        CsmAudioStopEvent,
        CsmErrorEvent,
        CsmModelVariant,
        CsmStatusEvent,
        ExternalGemmaVariant,
        GemmaVariant,
        ModelDownloadProgressEvent,
        ModelMemoryUsageSnapshot,
        ModelPreset,
        ModelSelection,
        ProcessingAudioLatencyEvent,
        RuntimeSetupStatusEvent,
        ScreenCaptureEvent,
        SelectOption,
        OpenDuckContactImportEvent,
        ShowAboutModalEvent,
        StoredModelPreferences,
        TriggerAppUpdateCheckEvent,
        SttModelVariant,
        SttStatusEvent,
        TranscriptEvent,
        TranscriptPartialEvent,
        TrayEndCallEvent,
        TrayPongPlaybackEvent,
        TrayToggleMuteEvent,
    } from "$lib/openduck/types";

    type StoredPongPlaybackPreference = {
        version: 1;
        enabled: boolean;
    };

    type StoredSelectLastSessionPreference = {
        version: 1;
        enabled: boolean;
    };

    type StoredAppUpdatePreference = {
        version: 1;
        skippedVersion: string | null;
        autoCheckEnabled?: boolean;
    };

    type StoredShowStatPreference = {
        version: 1;
        enabled: boolean;
    };

    type StoredShowSubtitlePreference = {
        version: 1;
        enabled: boolean;
    };

    type StoredShowAiSubtitlePreference = {
        version: 1;
        enabled: boolean;
    };

    type StoredAiSubtitleTargetLanguagePreference = {
        version: 1;
        targetLanguage: AiSubtitleTargetLanguage;
    };

    type StoredShowHiddenWindowOverlayPreference = {
        version: 1;
        enabled: boolean;
    };

    type StoredAutoUnmuteOnPastedScreenshotPreference = {
        version: 1;
        enabled: boolean;
    };

    type StoredEndOfUtteranceSilencePreference = {
        version: 1;
        milliseconds: number;
    };

    type StoredAutoContinueSilencePreference = {
        version: 1;
        milliseconds: number | null;
    };

    type StoredAutoContinueMaxCountPreference = {
        version: 1;
        count: number | null;
    };

    type ProviderConfig = {
        baseUrl: string;
        hasApiKey: boolean;
    };

    type SubtitleTranslationLlmConfig = ProviderConfig & {
        modelId: string;
    };

    type StoredLlmContextTurnLimitPreference = {
        version: 1;
        limit: number | null;
    };

    type StoredLlmImageHistoryLimitPreference = {
        version: 1;
        limit: number | null;
    };

    const OVERLAY_WINDOW_LABEL = "overlay";
    const OVERLAY_WINDOW_ROUTE = "/overlay";
    const AI_SUBTITLE_EVENT = "ai-subtitle";
    const LIVE_TRANSCRIPT_SUBTITLE_DURATION_MS = 5_000;
    const GITHUB_LATEST_RELEASE_API_URL =
        "https://api.github.com/repos/anslwy/openduck/releases/latest";
    const RELEASE_NOTES_URL =
        "https://github.com/anslwy/openduck/releases/latest";

    type GithubLatestReleasePayload = {
        body?: unknown;
        html_url?: unknown;
    };

    let calling = $state(false);
    let micMuted = $state(false);
    let time = $state(0);
    let callStartedAtMs = $state<number | null>(null);
    let isGemmaDownloaded = $state(false);
    let isGemmaLoaded = $state(false);
    let isOllamaSupported = $state(false);
    let isLmStudioSupported = $state(false);
    let isOpenAiCompatibleSupported = $state(false);
    let ollamaModels = $state<string[]>([]);
    let selectedOllamaModel = $state<string>("");
    let lmStudioModels = $state<string[]>([]);
    let selectedLmStudioModel = $state<string>(DEFAULT_LMSTUDIO_MODEL);
    let openAiCompatibleModels = $state<string[]>([]);
    let selectedOpenAiCompatibleModel = $state<string>(
        DEFAULT_OPENAI_COMPATIBLE_MODEL,
    );
    let isCsmDownloaded = $state(false);
    let isCsmLoaded = $state(false);
    let isSttDownloaded = $state(false);
    let isSttLoaded = $state(false);
    let isDownloadingGemma = $state(false);
    let isClearingGemmaCache = $state(false);
    let isCancellingGemmaDownload = $state(false);
    let isLoadingGemma = $state(false);
    let isDownloadingCsm = $state(false);
    let isClearingCsmCache = $state(false);
    let isCancellingCsmDownload = $state(false);
    let isLoadingCsm = $state(false);
    let isDownloadingStt = $state(false);
    let isClearingSttCache = $state(false);
    let isCancellingSttDownload = $state(false);
    let isLoadingStt = $state(false);
    let isUnloadingGemma = $state(false);
    let isUnloadingCsm = $state(false);
    let isUnloadingStt = $state(false);
    let isLoadingAll = $state(false);
    let isUpdatingCsmQuantize = $state(false);
    let gemmaDownloadMessage = $state("Preparing download...");
    let gemmaDownloadProgress = $state<number | null>(null);
    let gemmaDownloadIndeterminate = $state(true);
    let gemmaDownloadError = $state<string | null>(null);
    let selectedGemmaVariant = $state<GemmaVariant>(DEFAULT_GEMMA_VARIANT);
    let selectedCsmModel = $state<CsmModelVariant>(DEFAULT_CSM_MODEL);
    let selectedSttModel = $state<SttModelVariant>(DEFAULT_STT_MODEL);
    let pendingModelPreset = $state<ModelPreset | null>(null);
    let csmDownloadMessage = $state("Preparing download...");
    let csmDownloadProgress = $state<number | null>(null);
    let csmDownloadIndeterminate = $state(true);
    let csmDownloadError = $state<string | null>(null);
    let csmLoadMessage = $state("Starting worker...");
    let csmNotificationMessage = $state<string | null>(null);
    let sttDownloadMessage = $state("Preparing download...");
    let sttDownloadProgress = $state<number | null>(null);
    let sttDownloadIndeterminate = $state(true);
    let sttDownloadError = $state<string | null>(null);
    let sttLoadMessage = $state("Starting worker...");
    let isPreparingRuntime = $state(false);
    let runtimeSetupMessage = $state(
        "Preparing local Python runtime. [Time Needed: ~2mins]",
    );
    let runtimeSetupError = $state<string | null>(null);
    let isCsmQuantized = $state(true);
    let modelMemorySnapshot = $state<ModelMemoryUsageSnapshot | null>(null);

    let captureContext: AudioContext | null = null;
    let mediaStream: MediaStream | null = null;
    let captureSource: MediaStreamAudioSourceNode | null = null;
    let captureProcessor: AudioWorkletNode | null = null;
    let playbackProcessor: AudioWorkletNode | null = null;
    let silentCaptureSink: GainNode | null = null;
    let healthCheckInterval: ReturnType<typeof window.setInterval> | null =
        null;
    let downloadStatusPollInterval: ReturnType<
        typeof window.setInterval
    > | null = null;
    let modelMemoryPollInterval: ReturnType<typeof window.setInterval> | null =
        null;
    let callTimerInterval: ReturnType<typeof window.setInterval> | null = null;
    let playbackIdleTimeout: ReturnType<typeof window.setTimeout> | null = null;
    let eventUnlisteners: UnlistenFn[] = [];
    let activeTtsRequestId: number | null = null;
    let ttsSegmentTextMap = new Map<
        number,
        { text: string; index: number }[]
    >();
    let assistantSegmentTranslationsMap = new Map<
        number,
        Map<number, Record<string, string>>
    >();
    let syncedTtsPlaybackActive = false;
    let pendingCompletionPongRequestId = $state<number | null>(null);
    let pendingTtsSegments = $state(0);
    let queuedPlaybackChunkCount = $state(0);
    let isQueueingCompletionPong = false;
    let cachedPongPlaybackSamples = $state<Float32Array | null>(null);
    let cachedPongPlaybackSampleRate = $state<number | null>(null);
    let activePongSource = $state<AudioBufferSourceNode | null>(null);
    let activePongGainNode = $state<GainNode | null>(null);
    let pongPlaybackEnabled = $state(true);
    let autoUnmuteOnPastedScreenshotEnabled = $state(
        DEFAULT_AUTO_UNMUTE_ON_PASTED_SCREENSHOT,
    );
    let selectLastSessionEnabled = $state(false);
    let autoLoadModelsOnStartupEnabled = $state(false);
    let showStatEnabled = $state(false);
    let showSubtitleEnabled = $state(true);
    let showAiSubtitleEnabled = $state(DEFAULT_SHOW_AI_SUBTITLE);
    let aiSubtitleTargetLanguage = $state<AiSubtitleTargetLanguage>(
        DEFAULT_AI_SUBTITLE_TARGET_LANGUAGE,
    );
    let showCallTimerEnabled = $state(DEFAULT_SHOW_CALL_TIMER);
    let showHiddenWindowOverlayEnabled = $state(
        DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY,
    );
    let endOfUtteranceSilenceMs = $state(DEFAULT_END_OF_UTTERANCE_SILENCE_MS);
    let autoContinueSilenceMs = $state<number | null>(
        DEFAULT_AUTO_CONTINUE_SILENCE_MS,
    );
    let autoContinueMaxCount = $state<number | null>(
        DEFAULT_AUTO_CONTINUE_MAX_COUNT,
    );
    let llmContextTurnLimit = $state<number | null>(
        DEFAULT_LLM_CONTEXT_TURN_LIMIT,
    );
    let llmImageHistoryLimit = $state<number | null>(
        DEFAULT_LLM_IMAGE_HISTORY_LIMIT,
    );
    let globalShortcut = $state(DEFAULT_GLOBAL_SHORTCUT);
    let globalShortcutEntireScreen = $state(
        DEFAULT_GLOBAL_SHORTCUT_ENTIRE_SCREEN,
    );
    let globalShortcutToggleMute = $state(DEFAULT_GLOBAL_SHORTCUT_TOGGLE_MUTE);
    let globalShortcutInterrupt = $state(DEFAULT_GLOBAL_SHORTCUT_INTERRUPT);
    let contacts = $state<ContactProfile[]>([createDefaultContact()]);
    let selectedContactId = $state(DEFAULT_CONTACT_ID);
    let showContactsPopup = $state(false);
    let contactIdOnPopupOpen = $state<string | null>(null);
    let showExternalLlmConfig = $state(false);
    let ollamaBaseUrl = $state("http://127.0.0.1:11434");
    let ollamaHasApiKey = $state(false);
    let lmstudioBaseUrl = $state("http://127.0.0.1:1234");
    let lmstudioHasApiKey = $state(false);
    let openAiCompatibleBaseUrl = $state("");
    let openAiCompatibleHasApiKey = $state(false);
    let showSubtitleTranslationLlmConfig = $state(false);
    let subtitleTranslationBaseUrl = $state("");
    let subtitleTranslationHasApiKey = $state(false);
    let subtitleTranslationModelId = $state("");
    let showConversationPopup = $state(false);
    let showSessionsPopup = $state(false);
    let sessions = $state<SessionMetadata[]>([]);
    const hasHistory = $derived(sessions.length > 0);
    let currentSessionTitle = $state<string | null>(null);
    let currentSessionId = $state<string | null>(null);
    let showAboutPopup = $state(false);
    let showUpdatePrompt = $state(false);
    let showSearchModal = $state(false);
    let conversationLogEntries = $state<ConversationLogEntry[]>([]);
    let buildInfo = $state<BuildInfo | null>(null);
    let buildInfoError = $state<string | null>(null);
    let availableAppUpdate = $state<AppUpdateInfo | null>(null);
    let appUpdateStatus = $state<AppUpdateStatus>("idle");
    let appUpdateError = $state<string | null>(null);
    let skippedAppUpdateVersion = $state<string | null>(null);
    let autoCheckAppUpdatesEnabled = $state(DEFAULT_AUTO_CHECK_APP_UPDATES);
    let suppressAutoUpdatePromptUntilNextCheck = $state(false);
    let pendingAutomaticUpdatePromptVersion = $state<string | null>(null);
    let updateObject: Update | null = null;
    let nextConversationEntryId = 1;
    let pendingConversationUserLogEntryId: number | null = null;
    let activeAssistantResponseId: number | null = null;
    let activeAssistantConversationEntryId: number | null = null;
    let currentSpokenResponse = $state("");
    let isSavingConversationLogEntryEdit = $state(false);
    let isClearingConversationLogImages = $state(false);

    const popupActionsBusy = $derived(
        isSavingConversationLogEntryEdit || isClearingConversationLogImages,
    );

    $effect(() => {
        const pendingVersion = pendingAutomaticUpdatePromptVersion;
        if (!pendingVersion) {
            return;
        }

        if (calling || showAboutPopup) {
            return;
        }

        if (availableAppUpdate?.version !== pendingVersion) {
            pendingAutomaticUpdatePromptVersion = null;
            return;
        }

        showUpdatePrompt = true;
        pendingAutomaticUpdatePromptVersion = null;
    });

    let callStagePhase = $state<CallStagePhase>("idle");
    let callStageMessage = $state("");
    let reasoningText = $state("");
    let showReasoningPopup = $state(false);
    let lastReasoningRequestId = $state<number | null>(null);
    let processingAudioToAudioLatencyMs = $state<number | null>(null);
    let processingAudioToLlmLatencyMs = $state<number | null>(null);
    let processingAudioLatencyMs = $state<number | null>(null);
    let liveTranscriptSubtitle = $state("");
    let currentAiSubtitle = $state("");
    let liveTranscriptSubtitleTimeout: ReturnType<
        typeof window.setTimeout
    > | null = null;
    let screenCapturePhase = $state<ScreenCaptureEvent["phase"] | null>(null);
    let screenCaptureMessage = $state("");
    let screenCaptureHasPendingAttachment = $state(false);
    let screenCaptureFileName = $state<string | null>(null);
    let screenCaptureImageDataUrls = $state<string[]>([]);
    let contactsImportInput: HTMLInputElement | null = null;
    let contactIconInput: HTMLInputElement | null = null;
    let contactRefAudioInput: HTMLInputElement | null = null;
    let refAudioPlaying = $state(false);
    let refAudioEl: HTMLAudioElement | null = null;
    let selectedContactPromptSyncTimeout: ReturnType<
        typeof window.setTimeout
    > | null = null;
    let selectedContactVoiceReferenceSyncTimeout: ReturnType<
        typeof window.setTimeout
    > | null = null;
    let syncedConversationLogHasVisibleImages: boolean | null = null;
    let previewImageUrl = $state<string | null>(null);

    const formattedTime = $derived(
        `${Math.floor(time / 60)
            .toString()
            .padStart(2, "0")}:${(time % 60).toString().padStart(2, "0")}`,
    );
    const sttUsesGemma = $derived(selectedSttModel === "gemma");
    const resolvedModelPreset = $derived(
        resolveModelPreset({
            gemmaVariant: selectedGemmaVariant,
            csmModel: selectedCsmModel,
            sttModel: selectedSttModel,
        }),
    );
    const activeModelPreset = $derived(
        pendingModelPreset ?? resolvedModelPreset,
    );
    const effectiveSttLoaded = $derived(
        sttUsesGemma ? isGemmaLoaded : isSttLoaded,
    );
    const hasLoadedModelProcess = $derived(
        isGemmaLoaded || isCsmLoaded || isSttLoaded,
    );
    const showModelMemorySummary = $derived(
        showStatEnabled && (modelMemorySnapshot?.total_bytes ?? 0) > 0,
    );
    const showScreenCaptureCard = $derived(
        calling &&
            (screenCapturePhase === "capturing" ||
                screenCapturePhase === "error" ||
                screenCaptureHasPendingAttachment),
    );
    const showRuntimeSetupBanner = $derived(
        isPreparingRuntime || runtimeSetupError !== null,
    );
    const conversationLogHasVisibleImages = $derived(
        conversationLogEntries.some(
            (entry) => (entry.imageUrls?.length ?? 0) > 0,
        ),
    );
    const modelsReady = $derived(
        isGemmaLoaded && isCsmLoaded && effectiveSttLoaded,
    );
    const gemmaVariantDisabled = $derived(
        isPreparingRuntime ||
            isGemmaLoaded ||
            isDownloadingGemma ||
            isClearingGemmaCache ||
            isCancellingGemmaDownload ||
            isLoadingGemma ||
            isUnloadingGemma,
    );
    const externalModelDisabled = $derived(
        isPreparingRuntime || isLoadingGemma || isUnloadingGemma,
    );
    const csmVariantDisabled = $derived(
        isPreparingRuntime ||
            isCsmLoaded ||
            isDownloadingCsm ||
            isClearingCsmCache ||
            isCancellingCsmDownload ||
            isLoadingCsm ||
            isUnloadingCsm,
    );
    const sttVariantDisabled = $derived(
        isPreparingRuntime ||
            isDownloadingStt ||
            isClearingSttCache ||
            isCancellingSttDownload ||
            isLoadingStt ||
            isUnloadingStt ||
            (selectedSttModel !== "gemma" && isSttLoaded),
    );
    let conversationLogViewport = $state<HTMLDivElement | null>(null);
    let contactsPopupEl = $state<HTMLElement | null>(null);
    let conversationPopupEl = $state<HTMLElement | null>(null);
    let aboutPopupEl = $state<HTMLElement | null>(null);
    let sessionsPopupEl = $state<HTMLElement | null>(null);

    function setConversationLogViewport(element: HTMLDivElement | null) {
        conversationLogViewport = element;
    }

    function isExternalGemmaVariant(
        variant: GemmaVariant,
    ): variant is ExternalGemmaVariant {
        return (
            variant === "ollama" ||
            variant === "lmstudio" ||
            variant === "openai_compatible"
        );
    }

    function getExternalProviderLabel(variant: GemmaVariant) {
        if (variant === "lmstudio") {
            return "LM Studio";
        }

        if (variant === "openai_compatible") {
            return "OpenAI-compatible API";
        }

        return "Ollama";
    }

    function getExternalProviderGuideText(variant: GemmaVariant) {
        if (variant === "lmstudio") {
            return "Start LM Studio's local server and load a model so it appears in the dropdown. The default URL is http://127.0.0.1:1234.";
        }

        if (variant === "openai_compatible") {
            return "Point this at a service that exposes /v1/models and /v1/chat/completions. Vision requests are sent with image_url content, so use a model and endpoint that support images.";
        }

        return 'If your model does not show, run "ollama run {your_model}" in Terminal once so it appears here.';
    }

    function getExternalProviderUrlPlaceholder(variant: GemmaVariant) {
        if (variant === "lmstudio") {
            return "http://127.0.0.1:1234";
        }

        if (variant === "openai_compatible") {
            return "https://api.openai.com";
        }

        return "http://127.0.0.1:11434";
    }

    function getExternalProviderBaseUrl(variant: GemmaVariant) {
        if (variant === "lmstudio") {
            return lmstudioBaseUrl;
        }

        if (variant === "openai_compatible") {
            return openAiCompatibleBaseUrl;
        }

        return ollamaBaseUrl;
    }

    function getExternalProviderHasApiKey(variant: GemmaVariant) {
        if (variant === "lmstudio") {
            return lmstudioHasApiKey;
        }

        if (variant === "openai_compatible") {
            return openAiCompatibleHasApiKey;
        }

        return ollamaHasApiKey;
    }

    const subtitleTranslationLlmConfigured = $derived(
        subtitleTranslationBaseUrl.trim() !== "" &&
            subtitleTranslationModelId.trim() !== "",
    );

    async function syncExternalModelsForVariant(variant: ExternalGemmaVariant) {
        if (variant === "ollama") {
            await syncOllamaModels();
            return;
        }

        if (variant === "lmstudio") {
            await syncLmStudioModels();
            return;
        }

        await syncOpenAiCompatibleModels();
    }

    const modelPresetOptions: Array<SelectOption<ModelPreset>> = [
        { value: "lite", label: MODEL_PRESETS.lite.label },
        { value: "normal", label: MODEL_PRESETS.normal.label },
        { value: "realistic", label: MODEL_PRESETS.realistic.label },
        { value: "custom", label: "Custom" },
    ];
    const gemmaVariantOptions: Array<SelectOption<GemmaVariant>> = $derived([
        { value: "e4b", label: "Gemma-4-E4B" },
        { value: "e2b", label: "Gemma-4-E2B" },
        {
            value: "ollama",
            label: "Ollama",
        },
        {
            value: "lmstudio",
            label: "LM Studio",
        },
        {
            value: "openai_compatible",
            label: "OpenAI-compatible API",
        },
    ]);
    const csmModelOptions: Array<SelectOption<CsmModelVariant>> = [
        { value: "kokoro_82m", label: "Kokoro-82M" },
        {
            value: "chatterbox_turbo_8bit",
            label: "Chatterbox Turbo-350M (8-bit)",
        },
        {
            value: "chatterbox_turbo_fp16",
            label: "Chatterbox Turbo-350M (fp16)",
        },
        { value: "cosyvoice3_0_5b_4bit", label: "Fun-CosyVoice3-0.5B (4-bit)" },
        { value: "cosyvoice3_0_5b_8bit", label: "Fun-CosyVoice3-0.5B (8-bit)" },
        { value: "cosyvoice3_0_5b_fp16", label: "Fun-CosyVoice3-0.5B (fp16)" },
        { value: "expressiva_1b", label: "CSM Expressiva 1B" },
    ];
    const sttModelOptions: Array<SelectOption<SttModelVariant>> = $derived([
        {
            value: "gemma",
            label: "Gemma",
            disabled: isExternalGemmaVariant(selectedGemmaVariant),
        },
        {
            value: "distil_whisper_large_v3",
            label: "Distil-Whisper",
        },
        {
            value: "whisper_large_v3_turbo",
            label: "Whisper Large V3 Turbo",
        },
    ]);
    const gemmaVariantTooltip = $derived(
        selectedGemmaVariant === "e4b"
            ? "E4B uses more RAM but is generally more capable. Recommended for Macs with 24 GB+ of unified memory."
            : selectedGemmaVariant === "e2b"
              ? "E2B uses less RAM but is generally less capable. Recommended for Macs with 16 GB+ of unified memory."
              : selectedGemmaVariant === "lmstudio"
                ? "Gemma STT is not supported with LM Studio. Start LM Studio's local server and load a model so it appears here."
                : selectedGemmaVariant === "openai_compatible"
                  ? "Gemma STT is not supported with OpenAI-compatible APIs. Use a text-and-image chat model here and switch STT to Whisper."
                  : 'Gemma STT is not supported with Ollama. If your model does not show, run "ollama run {your_model}" in Terminal (One-time only).',
    );
    const selectedGemmaUsesExternalProvider = $derived(
        isExternalGemmaVariant(selectedGemmaVariant),
    );
    const selectedExternalProviderLabel = $derived(
        getExternalProviderLabel(selectedGemmaVariant),
    );
    const selectedExternalProviderSupported = $derived(
        selectedGemmaVariant === "lmstudio"
            ? isLmStudioSupported
            : selectedGemmaVariant === "openai_compatible"
              ? isOpenAiCompatibleSupported
              : selectedGemmaVariant === "ollama"
                ? isOllamaSupported
                : false,
    );
    const selectedExternalProviderGuideText = $derived(
        selectedGemmaUsesExternalProvider && !selectedExternalProviderSupported
            ? getExternalProviderGuideText(selectedGemmaVariant)
            : null,
    );
    const selectedExternalModels = $derived(
        selectedGemmaVariant === "lmstudio"
            ? lmStudioModels
            : selectedGemmaVariant === "openai_compatible"
              ? openAiCompatibleModels
              : ollamaModels,
    );
    const selectedExternalModel = $derived(
        selectedGemmaVariant === "lmstudio"
            ? selectedLmStudioModel
            : selectedGemmaVariant === "openai_compatible"
              ? selectedOpenAiCompatibleModel
              : selectedOllamaModel,
    );
    const selectedCsmModelLabel = $derived(
        csmModelOptions.find((option) => option.value === selectedCsmModel)
            ?.label ?? "Kokoro-82M",
    );
    const csmModelTooltip = $derived(
        selectedCsmModel === "expressiva_1b"
            ? "CSM Expressiva 1B supports voice conditioning and optional quantization."
            : selectedCsmModel === "kokoro_82m"
              ? "Kokoro-82M is a lighter English TTS backend. Quantization is not used for this model."
              : selectedCsmModel === "cosyvoice2_0_5b"
                ? "CosyVoice2-0.5B produces high quality audio with higher usage of memory."
                : selectedCsmModel === "cosyvoice3_0_5b_8bit"
                  ? "Fun-CosyVoice3-0.5B (8-bit) provides a balance between realistic voice quality and RAM usage."
                  : selectedCsmModel === "cosyvoice3_0_5b_fp16"
                    ? "Fun-CosyVoice3-0.5B (fp16) provides the highest possible voice quality."
                    : selectedCsmModel === "chatterbox_turbo_8bit"
                      ? "Chatterbox Turbo (8-bit) provides a fast and realistic voice with lower RAM usage."
                      : selectedCsmModel === "chatterbox_turbo_fp16"
                        ? "Chatterbox Turbo (fp16) provides the highest possible voice quality with higher RAM usage."
                        : "Fun-CosyVoice3-0.5B (4-bit) provides a realistic voice while using significantly less VRAM.",
    );
    const selectedSttModelLabel = $derived(
        sttModelOptions.find((option) => option.value === selectedSttModel)
            ?.label ?? "Whisper Large V3 Turbo",
    );
    const sttModelTooltip = $derived(
        selectedSttModel === "gemma"
            ? selectedGemmaUsesExternalProvider
                ? `Gemma STT is not supported when using ${selectedExternalProviderLabel}.`
                : "Use the loaded Gemma model for transcription. There is no separate STT model to load."
            : selectedSttModel === "distil_whisper_large_v3"
              ? "Use mlx-audio with distil-whisper/distil-large-v3 for transcription. This model has lower RAM usage but supports only English."
              : "Use mlx-audio with mlx-community/whisper-large-v3-turbo-asr-fp16 for transcription. This model has higher RAM usage with 99+ languages support.",
    );
    const modelPresetTooltip = $derived(
        getModelPresetDescription(activeModelPreset),
    );
    const muteButtonLabel = $derived(
        micMuted ? "Unmute microphone" : "Mute microphone",
    );
    const muteButtonTitle = $derived(`${muteButtonLabel} (Space)`);
    const csmQuantizeAvailable = $derived(selectedCsmModel === "expressiva_1b");
    const loadAllMissingDownloads = $derived(
        (() => {
            const missingModels: string[] = [];

            if (!isGemmaDownloaded) {
                missingModels.push("LLM");
            }

            if (!isCsmDownloaded) {
                missingModels.push(selectedCsmModelLabel);
            }

            if (selectedSttModel !== "gemma" && !isSttDownloaded) {
                missingModels.push(selectedSttModelLabel);
            }

            return missingModels;
        })(),
    );
    const loadAllNeedsAction = $derived(
        !isGemmaLoaded ||
            !isCsmLoaded ||
            (selectedSttModel !== "gemma" && !isSttLoaded),
    );
    const loadAllBusy = $derived(
        isPreparingRuntime ||
            isLoadingAll ||
            isDownloadingGemma ||
            isClearingGemmaCache ||
            isCancellingGemmaDownload ||
            isLoadingGemma ||
            isUnloadingGemma ||
            isDownloadingCsm ||
            isClearingCsmCache ||
            isCancellingCsmDownload ||
            isLoadingCsm ||
            isUnloadingCsm ||
            isDownloadingStt ||
            isClearingSttCache ||
            isCancellingSttDownload ||
            isLoadingStt ||
            isUnloadingStt,
    );
    const loadAllDisabled = $derived(loadAllBusy);
    let lastLoadAllDisabled = $state(true);
    $effect(() => {
        if (lastLoadAllDisabled && !loadAllDisabled && loadAllNeedsAction) {
            const loadAllBtn = document.querySelector(".load-all-btn");
            if (loadAllBtn) {
                loadAllBtn.classList.add("pulse-animation");
                setTimeout(
                    () => loadAllBtn.classList.remove("pulse-animation"),
                    1500,
                );
            }
        }
        lastLoadAllDisabled = loadAllDisabled;
    });
    const presetSelectDisabled = $derived(
        loadAllBusy || isUpdatingCsmQuantize || pendingModelPreset !== null,
    );
    const loadAllButtonLabel = $derived(
        isLoadingAll
            ? "Processing..."
            : loadAllNeedsAction
              ? "Load All"
              : "Unload All",
    );
    const loadAllButtonTitle = $derived(
        loadAllMissingDownloads.length > 0
            ? `Download and load ${loadAllMissingDownloads.join(", ")}.`
            : loadAllNeedsAction
              ? "Load the selected models."
              : "Unload all loaded models.",
    );
    const selectedContact = $derived(
        contacts.find((contact) => contact.id === selectedContactId) ??
            contacts[0] ??
            null,
    );
    const selectedContactName = $derived(
        getContactDisplayName(selectedContact),
    );
    const selectedContactPrompt = $derived(
        selectedContact?.prompt.trim() || DEFAULT_CONTACT_PROMPT,
    );

    $effect(() => {
        if (showContactsPopup) {
            contactIdOnPopupOpen = selectedContactId;
        } else if (contactIdOnPopupOpen !== null) {
            contactIdOnPopupOpen = null;

            // Do not unload / reload voice clone models for now, seems no difference for the quality
            // if (
            //     isCsmLoaded &&
            //     (selectedCsmModel === "cosyvoice3_0_5b_8bit" ||
            //         selectedCsmModel === "cosyvoice3_0_5b_4bit" ||
            //         selectedCsmModel === "cosyvoice3_0_5b_fp16" ||
            //         selectedCsmModel === "chatterbox_turbo_8bit" ||
            //         selectedCsmModel === "chatterbox_turbo_fp16")
            // ) {
            //     void (async () => {
            //         try {
            //             await handleUnloadCsm({ suppressAlert: true });
            //             await handleLoadCsm();
            //         } catch (err) {
            //             console.error(
            //                 "Failed to automatically reload the speech model:",
            //                 err,
            //             );
            //         }
            //     })();
            // }
        }
    });
    const selectedContactIconUrl = $derived(
        selectedContact?.iconDataUrl ?? "/icon.png",
    );
    const selectedContactImageStyle = $derived(
        `background-image: url('${selectedContactIconUrl}')`,
    );
    const runtimeSetupTitle = $derived(
        isPreparingRuntime
            ? "Preparing Local Runtime [One-Time Setup]"
            : "Runtime Setup Failed",
    );
    const PLAYBACK_PREBUFFER_SAMPLES = 2048;
    const MAX_CONTEXT_BACKED_CONVERSATION_LOG_ENTRIES = 48;
    const PONG_VOLUME = 0.7;
    const pongUrl = "/pop.mp3";
    const screenCaptureTitle = $derived(
        screenCapturePhase === "capturing"
            ? "Select a Screen Region"
            : screenCapturePhase === "error" &&
                !screenCaptureHasPendingAttachment
              ? "Screen Capture Failed"
              : screenCapturePhase === "error"
                ? "Screen Region Unchanged"
                : "Image(s) Attached to Next Turn",
    );
    const screenCaptureActionLabel = $derived(
        screenCaptureHasPendingAttachment ? "Clear" : "Dismiss",
    );
    const assistantSpeaking = $derived(
        calling && callStagePhase === "speaking",
    );
    let userVolume = $state(0);
    const userSpeaking = $derived(calling && !micMuted && userVolume > 0.005);
    const activeAvatarSubtitle = $derived(
        showAiSubtitleEnabled && currentAiSubtitle.trim()
            ? currentAiSubtitle.trim()
            : showSubtitleEnabled &&
                liveTranscriptSubtitle.trim() &&
                (!assistantSpeaking || userSpeaking)
              ? liveTranscriptSubtitle.trim()
              : "",
    );
    let themeRgb = $state("127, 227, 124");

    $effect(() => {
        if (!selectedContactIconUrl) return;
        const img = new Image();
        img.src = selectedContactIconUrl;
        img.onload = () => {
            const canvas = document.createElement("canvas");
            canvas.width = 1;
            canvas.height = 1;
            const ctx = canvas.getContext("2d");
            if (ctx) {
                ctx.drawImage(img, 0, 0, 1, 1);
                const [r, g, b] = ctx.getImageData(0, 0, 1, 1).data;
                themeRgb = `${r}, ${g}, ${b}`;
            }
        };
    });

    $effect(() => {
        const window = getCurrentWindow();
        void window.setTitle(currentSessionTitle || "");
    });

    function clearLiveTranscriptSubtitleTimeout() {
        if (liveTranscriptSubtitleTimeout) {
            clearTimeout(liveTranscriptSubtitleTimeout);
            liveTranscriptSubtitleTimeout = null;
        }
    }

    function clearLiveTranscriptSubtitle() {
        clearLiveTranscriptSubtitleTimeout();
        liveTranscriptSubtitle = "";
    }

    function syncAiSubtitle(text: string) {
        void emit<AiSubtitleEvent>(AI_SUBTITLE_EVENT, {
            text,
        }).catch((err) => {
            console.error("Failed to sync AI subtitle:", err);
        });
    }

    async function setCurrentAiSubtitle(
        text: string,
        requestId?: number,
        segmentIndex?: number,
    ) {
        const nextText = text.trim();
        if (nextText && showAiSubtitleEnabled) {
            clearLiveTranscriptSubtitle();
        }
        if (currentAiSubtitle === nextText) {
            return;
        }

        if (nextText && aiSubtitleTargetLanguage !== "none") {
            let translated: string | undefined;

            if (requestId != null && segmentIndex != null) {
                const requestMap =
                    assistantSegmentTranslationsMap.get(requestId);
                if (requestMap) {
                    const translations = requestMap.get(segmentIndex);
                    if (translations) {
                        translated = translations[aiSubtitleTargetLanguage];
                    }
                }
            }

            if (!translated) {
                try {
                    translated = await invoke<string>("translate_text", {
                        text: nextText,
                        targetLang: aiSubtitleTargetLanguage,
                    });
                } catch (err) {
                    console.error("Failed to translate subtitle:", err);
                }
            }

            if (translated) {
                currentAiSubtitle = `${translated}\n${nextText}`;
            } else {
                currentAiSubtitle = nextText;
            }
        } else {
            currentAiSubtitle = nextText;
        }

        syncAiSubtitle(currentAiSubtitle);
    }

    function updateLiveTranscriptSubtitle(text: string) {
        const nextText = text.trim();
        if (!nextText) {
            return;
        }

        liveTranscriptSubtitle = nextText;
        clearLiveTranscriptSubtitleTimeout();
        liveTranscriptSubtitleTimeout = window.setTimeout(() => {
            liveTranscriptSubtitle = "";
            liveTranscriptSubtitleTimeout = null;
        }, LIVE_TRANSCRIPT_SUBTITLE_DURATION_MS);
    }

    function setCallStage(phase: CallStagePhase, message: string) {
        if (callStagePhase === phase && callStageMessage === message) {
            return;
        }

        if (phase === "thinking") {
            reasoningText = "";
            showReasoningPopup = false;
        }

        if (phase === "speaking") {
            clearLiveTranscriptSubtitle();
        } else {
            setCurrentAiSubtitle("");
        }

        callStagePhase = phase;
        callStageMessage = message;
    }

    function formatProcessingAudioLatency(latencyMs: number) {
        if (latencyMs >= 10_000) {
            return `${(latencyMs / 1000).toFixed(1)} s`;
        }

        if (latencyMs >= 1000) {
            return `${(latencyMs / 1000).toFixed(2)} s`;
        }

        return `${latencyMs} ms`;
    }

    function formatLatencyStat(label: string, latencyMs: number | null) {
        return `${label} ${
            latencyMs == null ? "--" : formatProcessingAudioLatency(latencyMs)
        }`;
    }

    function formatLatencySummary() {
        if (
            processingAudioToAudioLatencyMs == null &&
            processingAudioToLlmLatencyMs == null &&
            processingAudioLatencyMs == null
        ) {
            return "--";
        }

        return `${formatLatencyStat("STT", processingAudioToAudioLatencyMs)} / ${formatLatencyStat("LLM", processingAudioToLlmLatencyMs)} / ${formatLatencyStat("1st audio", processingAudioLatencyMs)}`;
    }

    function resetProcessingAudioLatencies() {
        processingAudioToAudioLatencyMs = null;
        processingAudioToLlmLatencyMs = null;
        processingAudioLatencyMs = null;
    }

    function syncCaptureMutedState(muted: boolean) {
        if (!captureProcessor) {
            return;
        }

        captureProcessor.port.postMessage({ type: "set-muted", muted });
    }

    function syncTtsPlaybackState(active: boolean) {
        if (syncedTtsPlaybackActive === active) {
            return;
        }

        syncedTtsPlaybackActive = active;
        void invoke("set_tts_playback_active", { active }).catch((err) =>
            console.error("Failed to sync TTS playback state:", err),
        );
    }

    function syncCallElapsedTime() {
        if (callStartedAtMs == null) {
            time = 0;
            return;
        }

        time = Math.max(0, Math.floor((Date.now() - callStartedAtMs) / 1000));
    }

    function scrollConversationLogToBottom() {
        window.requestAnimationFrame(() => {
            if (conversationLogViewport) {
                conversationLogViewport.scrollTop =
                    conversationLogViewport.scrollHeight;
            }
        });
    }

    function appendConversationLogEntry(
        role: "user" | "assistant",
        text: string,
        imageUrls: string[],
        translations: Record<string, string> = {},
    ): number {
        const id = nextConversationEntryId++;
        conversationLogEntries = [
            ...conversationLogEntries,
            {
                id,
                role,
                text,
                imageUrls,
                contextEntryId: null,
                isContextBacked: false,
                translations,
            },
        ];
        scrollConversationLogToBottom();
        return id;
    }

    function upsertAssistantConversationLogEntry(
        requestId: number,
        text: string,
        appendToAssistantEntryId: number | null = null,
        translations: Record<string, string> = {},
    ) {
        const normalizedText = text.trim();
        if (!normalizedText && Object.keys(translations).length === 0) {
            return;
        }

        if (appendToAssistantEntryId != null) {
            const existingEntry = conversationLogEntries.find(
                (entry) => entry.id === appendToAssistantEntryId,
            );
            if (existingEntry) {
                activeAssistantResponseId = requestId;
                activeAssistantConversationEntryId = appendToAssistantEntryId;
                conversationLogEntries = conversationLogEntries.map((entry) =>
                    entry.id === appendToAssistantEntryId
                        ? {
                              ...entry,
                              text: normalizedText,
                              translations: {
                                  ...(entry.translations || {}),
                                  ...translations,
                              },
                          }
                        : entry,
                );
                scrollConversationLogToBottom();
                return;
            }
        }

        if (
            activeAssistantResponseId !== requestId ||
            activeAssistantConversationEntryId == null
        ) {
            activeAssistantResponseId = requestId;
            activeAssistantConversationEntryId = appendConversationLogEntry(
                "assistant",
                normalizedText,
                [],
                translations,
            );
            return;
        }

        conversationLogEntries = conversationLogEntries.map((entry) =>
            entry.id === activeAssistantConversationEntryId
                ? {
                      ...entry,
                      text: normalizedText,
                      translations: {
                          ...(entry.translations || {}),
                          ...translations,
                      },
                  }
                : entry,
        );
        scrollConversationLogToBottom();
    }

    function pruneConversationLogContextBackedEntries() {
        const contextBackedEntries = conversationLogEntries.filter(
            (entry) => entry.contextEntryId !== null,
        );
        const removableEntries = contextBackedEntries.slice(
            0,
            Math.max(
                0,
                contextBackedEntries.length -
                    MAX_CONTEXT_BACKED_CONVERSATION_LOG_ENTRIES,
            ),
        );
        if (removableEntries.length === 0) {
            return;
        }

        const removableEntryIds = new Set(
            removableEntries.map((entry) => entry.id),
        );
        conversationLogEntries = conversationLogEntries.map((entry) =>
            removableEntryIds.has(entry.id)
                ? { ...entry, contextEntryId: null }
                : entry,
        );
    }

    function markConversationTurnAsContextBacked(
        payload: ConversationContextCommittedEvent,
    ) {
        if (payload.sessionTitle) {
            currentSessionTitle = payload.sessionTitle;
        }

        const userEntryIdInLog = pendingConversationUserLogEntryId;
        const assistantEntryIdInLog = activeAssistantConversationEntryId;

        if (userEntryIdInLog == null || assistantEntryIdInLog == null) {
            return;
        }

        const userEntry = conversationLogEntries.find(
            (e) => e.id === userEntryIdInLog,
        );
        const assistantEntry = conversationLogEntries.find(
            (e) => e.id === assistantEntryIdInLog,
        );

        if (!userEntry || !assistantEntry) {
            // One or both entries were deleted from the UI before they could be context-backed.
            // Still update the IDs so we can delete them from the backend.
            void invoke("delete_conversation_context_entry", {
                entryId: payload.userEntryId,
            });
        } else {
            // Sync any edits that happened while the turn was being processed.
            if (userEntry.text !== payload.userText) {
                console.log(
                    "Syncing edited user text to backend",
                    userEntry.text,
                );
                void invoke("update_conversation_context_entry", {
                    entryId: payload.userEntryId,
                    text: userEntry.text,
                    clearImages: false,
                });
            }
            if (assistantEntry.text !== payload.assistantText) {
                console.log(
                    "Syncing edited assistant text to backend",
                    assistantEntry.text,
                );
                void invoke("update_conversation_context_entry", {
                    entryId: payload.assistantEntryId,
                    text: assistantEntry.text,
                    clearImages: false,
                });
            }
        }

        conversationLogEntries = conversationLogEntries.map((entry) => {
            if (entry.id === userEntryIdInLog) {
                return {
                    ...entry,
                    contextEntryId: payload.userEntryId,
                    isContextBacked: true,
                };
            }

            if (entry.id === assistantEntryIdInLog) {
                return {
                    ...entry,
                    contextEntryId: payload.assistantEntryId,
                    isContextBacked: true,
                };
            }

            return entry;
        });

        pendingConversationUserLogEntryId = null;
        pruneConversationLogContextBackedEntries();
    }

    function resetConversationLog() {
        conversationLogEntries = [];
        nextConversationEntryId = 1;
        pendingConversationUserLogEntryId = null;
        activeAssistantResponseId = null;
        activeAssistantConversationEntryId = null;
        currentSpokenResponse = "";
        setCurrentAiSubtitle("");
    }

    function resetScreenCaptureStatus() {
        screenCapturePhase = null;
        screenCaptureMessage = "";
        screenCaptureHasPendingAttachment = false;
        screenCaptureFileName = null;
        screenCaptureImageDataUrls = [];
    }

    function applyScreenCaptureEvent(payload: ScreenCaptureEvent) {
        screenCapturePhase = payload.phase;
        screenCaptureMessage = payload.message.trim();
        screenCaptureHasPendingAttachment = payload.hasPendingAttachment;
        if (payload.attachmentCount > 1) {
            screenCaptureFileName = `${payload.attachmentCount} attachments`;
        } else {
            screenCaptureFileName = payload.fileName?.trim() || null;
        }
        screenCaptureImageDataUrls = payload.imageDataUrls || [];

        if (
            payload.phase === "ready" &&
            payload.hasPendingAttachment &&
            autoUnmuteOnPastedScreenshotEnabled &&
            micMuted
        ) {
            setMicMuted(false);
        }
    }

    async function handleClearPendingScreenCapture() {
        if (!screenCaptureHasPendingAttachment) {
            resetScreenCaptureStatus();
            return;
        }

        try {
            await invoke("clear_pending_screen_capture");
        } catch (err) {
            console.error("Failed to clear pending screen capture:", err);
            alert(`Failed to clear the screen attachment.\n${String(err)}`);
        }
    }

    async function handleRemoveScreenCaptureAt(index: number) {
        try {
            await invoke("remove_pending_screen_capture_at", { index });
        } catch (err) {
            console.error("Failed to remove screen capture:", err);
        }
    }

    function persistContactsMetadata() {
        if (typeof window === "undefined") {
            return;
        }

        const payload = createStoredContactsPayload(
            contacts,
            selectedContactId,
        );
        window.localStorage.setItem(
            CONTACTS_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function persistModelPreferences() {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredModelPreferences = {
            version: 1,
            gemmaVariant: selectedGemmaVariant,
            csmModel: selectedCsmModel,
            sttModel: selectedSttModel,
            ollamaModel: selectedOllamaModel,
            lmstudioModel: selectedLmStudioModel,
            openaiCompatibleModel: selectedOpenAiCompatibleModel,
        };
        window.localStorage.setItem(
            MODEL_PREFERENCES_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function persistAppUpdatePreference(options: {
        skippedVersion: string | null;
        autoCheckEnabled: boolean;
    }) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredAppUpdatePreference = {
            version: 1,
            skippedVersion: options.skippedVersion,
            autoCheckEnabled: options.autoCheckEnabled,
        };
        window.localStorage.setItem(
            APP_UPDATE_PREFERENCES_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function persistPongPlaybackPreference(enabled: boolean) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredPongPlaybackPreference = {
            version: 1,
            enabled,
        };
        window.localStorage.setItem(
            PONG_PLAYBACK_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function persistAutoUnmuteOnPastedScreenshotPreference(enabled: boolean) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredAutoUnmuteOnPastedScreenshotPreference = {
            version: 1,
            enabled,
        };
        window.localStorage.setItem(
            AUTO_UNMUTE_ON_PASTED_SCREENSHOT_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function persistSelectLastSessionPreference(enabled: boolean) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredSelectLastSessionPreference = {
            version: 1,
            enabled,
        };
        window.localStorage.setItem(
            SELECT_LAST_SESSION_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function persistShowStatPreference(enabled: boolean) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredShowStatPreference = {
            version: 1,
            enabled,
        };
        window.localStorage.setItem(
            SHOW_STAT_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function persistShowSubtitlePreference(enabled: boolean) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredShowSubtitlePreference = {
            version: 1,
            enabled,
        };
        window.localStorage.setItem(
            SHOW_SUBTITLE_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function persistShowAiSubtitlePreference(enabled: boolean) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredShowAiSubtitlePreference = {
            version: 1,
            enabled,
        };
        window.localStorage.setItem(
            SHOW_AI_SUBTITLE_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function persistAiSubtitleTargetLanguagePreference(
        targetLanguage: AiSubtitleTargetLanguage,
    ) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredAiSubtitleTargetLanguagePreference = {
            version: 1,
            targetLanguage,
        };
        window.localStorage.setItem(
            AI_SUBTITLE_TARGET_LANGUAGE_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function persistShowHiddenWindowOverlayPreference(enabled: boolean) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredShowHiddenWindowOverlayPreference = {
            version: 1,
            enabled,
        };
        window.localStorage.setItem(
            SHOW_HIDDEN_WINDOW_OVERLAY_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function clampEndOfUtteranceSilenceMs(milliseconds: number) {
        if (!Number.isFinite(milliseconds)) {
            return DEFAULT_END_OF_UTTERANCE_SILENCE_MS;
        }

        const roundedMilliseconds = Math.round(milliseconds);
        const steppedMilliseconds =
            Math.round(roundedMilliseconds / END_OF_UTTERANCE_SILENCE_STEP_MS) *
            END_OF_UTTERANCE_SILENCE_STEP_MS;

        return Math.min(
            MAX_END_OF_UTTERANCE_SILENCE_MS,
            Math.max(MIN_END_OF_UTTERANCE_SILENCE_MS, steppedMilliseconds),
        );
    }

    function persistEndOfUtteranceSilencePreference(milliseconds: number) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredEndOfUtteranceSilencePreference = {
            version: 1,
            milliseconds,
        };
        window.localStorage.setItem(
            END_OF_UTTERANCE_SILENCE_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function clampAutoContinueSilenceMs(
        milliseconds: number | null,
    ): number | null {
        if (milliseconds === null) {
            return DEFAULT_AUTO_CONTINUE_SILENCE_MS;
        }

        if (!Number.isFinite(milliseconds)) {
            return DEFAULT_AUTO_CONTINUE_SILENCE_MS;
        }

        const roundedMilliseconds = Math.round(milliseconds);
        const steppedMilliseconds =
            Math.round(roundedMilliseconds / AUTO_CONTINUE_SILENCE_STEP_MS) *
            AUTO_CONTINUE_SILENCE_STEP_MS;

        return Math.min(
            MAX_AUTO_CONTINUE_SILENCE_MS,
            Math.max(MIN_AUTO_CONTINUE_SILENCE_MS, steppedMilliseconds),
        );
    }

    function persistAutoContinueSilencePreference(milliseconds: number | null) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredAutoContinueSilencePreference = {
            version: 1,
            milliseconds,
        };
        window.localStorage.setItem(
            AUTO_CONTINUE_SILENCE_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function clampAutoContinueMaxCount(count: number | null): number | null {
        if (count === null) {
            return null;
        }

        if (!Number.isFinite(count)) {
            return DEFAULT_AUTO_CONTINUE_MAX_COUNT;
        }

        const roundedCount = Math.round(count);
        return Math.min(
            MAX_AUTO_CONTINUE_MAX_COUNT,
            Math.max(MIN_AUTO_CONTINUE_MAX_COUNT, roundedCount),
        );
    }

    function persistAutoContinueMaxCountPreference(count: number | null) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredAutoContinueMaxCountPreference = {
            version: 1,
            count,
        };
        window.localStorage.setItem(
            AUTO_CONTINUE_MAX_COUNT_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function clampLlmContextTurnLimit(limit: number | null): number | null {
        if (limit === null) {
            return null;
        }

        if (!Number.isFinite(limit)) {
            return DEFAULT_LLM_CONTEXT_TURN_LIMIT;
        }

        const roundedLimit = Math.round(limit);
        return Math.min(
            MAX_LLM_CONTEXT_TURN_LIMIT,
            Math.max(MIN_LLM_CONTEXT_TURN_LIMIT, roundedLimit),
        );
    }

    function persistLlmContextTurnLimitPreference(limit: number | null) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredLlmContextTurnLimitPreference = {
            version: 1,
            limit,
        };
        window.localStorage.setItem(
            LLM_CONTEXT_TURN_LIMIT_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function clampLlmImageHistoryLimit(limit: number | null): number | null {
        if (limit === null) {
            return DEFAULT_LLM_IMAGE_HISTORY_LIMIT;
        }

        if (!Number.isFinite(limit)) {
            return DEFAULT_LLM_IMAGE_HISTORY_LIMIT;
        }

        const roundedLimit = Math.round(limit);
        return Math.min(
            MAX_LLM_IMAGE_HISTORY_LIMIT,
            Math.max(MIN_LLM_IMAGE_HISTORY_LIMIT, roundedLimit),
        );
    }

    function persistLlmImageHistoryLimitPreference(limit: number | null) {
        if (typeof window === "undefined") {
            return;
        }

        const payload: StoredLlmImageHistoryLimitPreference = {
            version: 1,
            limit,
        };
        window.localStorage.setItem(
            LLM_IMAGE_HISTORY_LIMIT_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function loadPongPlaybackPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return true;
        }

        const rawPayload = window.localStorage.getItem(
            PONG_PLAYBACK_STORAGE_KEY,
        );
        if (!rawPayload) {
            return true;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                enabled?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.enabled !== "boolean") {
                return true;
            }

            return parsed.enabled;
        } catch (err) {
            console.error("Failed to restore pong playback preference:", err);
            return true;
        }
    }

    function loadAppUpdatePreferenceFromStorage() {
        if (typeof window === "undefined") {
            return {
                skippedVersion: null,
                autoCheckEnabled: DEFAULT_AUTO_CHECK_APP_UPDATES,
            };
        }

        const rawPayload = window.localStorage.getItem(
            APP_UPDATE_PREFERENCES_STORAGE_KEY,
        );
        if (!rawPayload) {
            return {
                skippedVersion: null,
                autoCheckEnabled: DEFAULT_AUTO_CHECK_APP_UPDATES,
            };
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                skippedVersion?: unknown;
                autoCheckEnabled?: unknown;
            };
            if (
                parsed.version !== 1 ||
                (parsed.skippedVersion !== null &&
                    typeof parsed.skippedVersion !== "string") ||
                (parsed.autoCheckEnabled !== undefined &&
                    typeof parsed.autoCheckEnabled !== "boolean")
            ) {
                return {
                    skippedVersion: null,
                    autoCheckEnabled: DEFAULT_AUTO_CHECK_APP_UPDATES,
                };
            }

            return {
                skippedVersion: parsed.skippedVersion ?? null,
                autoCheckEnabled:
                    parsed.autoCheckEnabled ?? DEFAULT_AUTO_CHECK_APP_UPDATES,
            };
        } catch (err) {
            console.error("Failed to restore app update preference:", err);
            return {
                skippedVersion: null,
                autoCheckEnabled: DEFAULT_AUTO_CHECK_APP_UPDATES,
            };
        }
    }

    function loadAutoUnmuteOnPastedScreenshotPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_AUTO_UNMUTE_ON_PASTED_SCREENSHOT;
        }

        const rawPayload = window.localStorage.getItem(
            AUTO_UNMUTE_ON_PASTED_SCREENSHOT_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_AUTO_UNMUTE_ON_PASTED_SCREENSHOT;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                enabled?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.enabled !== "boolean") {
                return DEFAULT_AUTO_UNMUTE_ON_PASTED_SCREENSHOT;
            }

            return parsed.enabled;
        } catch (err) {
            console.error(
                "Failed to restore auto-unmute-on-pasted-screenshot preference:",
                err,
            );
            return DEFAULT_AUTO_UNMUTE_ON_PASTED_SCREENSHOT;
        }
    }

    function loadSelectLastSessionPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return false;
        }

        const rawPayload = window.localStorage.getItem(
            SELECT_LAST_SESSION_STORAGE_KEY,
        );
        if (!rawPayload) {
            return false;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                enabled?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.enabled !== "boolean") {
                return false;
            }

            return parsed.enabled;
        } catch (err) {
            console.error(
                "Failed to restore select last session preference:",
                err,
            );
            return false;
        }
    }

    function loadAutoLoadModelsOnStartupPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_AUTO_LOAD_MODELS_ON_STARTUP;
        }

        const rawPayload = window.localStorage.getItem(
            AUTO_LOAD_MODELS_ON_STARTUP_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_AUTO_LOAD_MODELS_ON_STARTUP;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                enabled?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.enabled !== "boolean") {
                return DEFAULT_AUTO_LOAD_MODELS_ON_STARTUP;
            }

            return parsed.enabled;
        } catch (err) {
            console.error(
                "Failed to restore auto load models on startup preference:",
                err,
            );
            return DEFAULT_AUTO_LOAD_MODELS_ON_STARTUP;
        }
    }

    function loadShowStatPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return false;
        }

        const rawPayload = window.localStorage.getItem(SHOW_STAT_STORAGE_KEY);
        if (!rawPayload) {
            return false;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                enabled?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.enabled !== "boolean") {
                return false;
            }

            return parsed.enabled;
        } catch (err) {
            console.error("Failed to restore show stat preference:", err);
            return false;
        }
    }

    function loadShowSubtitlePreferenceFromStorage() {
        if (typeof window === "undefined") {
            return true;
        }

        const rawPayload = window.localStorage.getItem(
            SHOW_SUBTITLE_STORAGE_KEY,
        );
        if (!rawPayload) {
            return true;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                enabled?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.enabled !== "boolean") {
                return true;
            }

            return parsed.enabled;
        } catch (err) {
            console.error("Failed to restore show subtitle preference:", err);
            return true;
        }
    }

    function loadShowAiSubtitlePreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_SHOW_AI_SUBTITLE;
        }

        const rawPayload = window.localStorage.getItem(
            SHOW_AI_SUBTITLE_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_SHOW_AI_SUBTITLE;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                enabled?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.enabled !== "boolean") {
                return DEFAULT_SHOW_AI_SUBTITLE;
            }

            return parsed.enabled;
        } catch (err) {
            console.error(
                "Failed to restore show AI subtitle preference:",
                err,
            );
            return DEFAULT_SHOW_AI_SUBTITLE;
        }
    }

    function loadShowHiddenWindowOverlayPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY;
        }

        const rawPayload = window.localStorage.getItem(
            SHOW_HIDDEN_WINDOW_OVERLAY_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                enabled?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.enabled !== "boolean") {
                return DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY;
            }

            return parsed.enabled;
        } catch (err) {
            console.error(
                "Failed to restore hidden-window overlay preference:",
                err,
            );
            return DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY;
        }
    }

    function loadEndOfUtteranceSilencePreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_END_OF_UTTERANCE_SILENCE_MS;
        }

        const rawPayload = window.localStorage.getItem(
            END_OF_UTTERANCE_SILENCE_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_END_OF_UTTERANCE_SILENCE_MS;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                milliseconds?: unknown;
            };
            if (
                parsed.version !== 1 ||
                typeof parsed.milliseconds !== "number"
            ) {
                return DEFAULT_END_OF_UTTERANCE_SILENCE_MS;
            }

            return clampEndOfUtteranceSilenceMs(parsed.milliseconds);
        } catch (err) {
            console.error(
                "Failed to restore end-of-utterance silence preference:",
                err,
            );
            return DEFAULT_END_OF_UTTERANCE_SILENCE_MS;
        }
    }

    function loadAutoContinueSilencePreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_AUTO_CONTINUE_SILENCE_MS;
        }

        const rawPayload = window.localStorage.getItem(
            AUTO_CONTINUE_SILENCE_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_AUTO_CONTINUE_SILENCE_MS;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                milliseconds?: unknown;
            };
            if (
                parsed.version !== 1 ||
                (parsed.milliseconds !== null &&
                    typeof parsed.milliseconds !== "number")
            ) {
                return DEFAULT_AUTO_CONTINUE_SILENCE_MS;
            }

            return clampAutoContinueSilenceMs(parsed.milliseconds ?? null);
        } catch (err) {
            console.error(
                "Failed to restore auto-continue silence preference:",
                err,
            );
            return DEFAULT_AUTO_CONTINUE_SILENCE_MS;
        }
    }

    function loadAutoContinueMaxCountPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_AUTO_CONTINUE_MAX_COUNT;
        }

        const rawPayload = window.localStorage.getItem(
            AUTO_CONTINUE_MAX_COUNT_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_AUTO_CONTINUE_MAX_COUNT;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                count?: unknown;
            };
            if (
                parsed.version !== 1 ||
                (parsed.count !== null && typeof parsed.count !== "number")
            ) {
                return DEFAULT_AUTO_CONTINUE_MAX_COUNT;
            }

            return clampAutoContinueMaxCount(parsed.count ?? null);
        } catch (err) {
            console.error(
                "Failed to restore auto-continue max count preference:",
                err,
            );
            return DEFAULT_AUTO_CONTINUE_MAX_COUNT;
        }
    }

    function loadLlmContextTurnLimitPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_LLM_CONTEXT_TURN_LIMIT;
        }

        const rawPayload = window.localStorage.getItem(
            LLM_CONTEXT_TURN_LIMIT_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_LLM_CONTEXT_TURN_LIMIT;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                limit?: unknown;
            };
            if (
                parsed.version !== 1 ||
                (parsed.limit !== null && typeof parsed.limit !== "number")
            ) {
                return DEFAULT_LLM_CONTEXT_TURN_LIMIT;
            }

            return clampLlmContextTurnLimit(parsed.limit ?? null);
        } catch (err) {
            console.error(
                "Failed to restore LLM context turn limit preference:",
                err,
            );
            return DEFAULT_LLM_CONTEXT_TURN_LIMIT;
        }
    }

    function loadLlmImageHistoryLimitPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_LLM_IMAGE_HISTORY_LIMIT;
        }

        const rawPayload = window.localStorage.getItem(
            LLM_IMAGE_HISTORY_LIMIT_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_LLM_IMAGE_HISTORY_LIMIT;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                limit?: unknown;
            };
            if (
                parsed.version !== 1 ||
                (parsed.limit !== null && typeof parsed.limit !== "number")
            ) {
                return DEFAULT_LLM_IMAGE_HISTORY_LIMIT;
            }

            return clampLlmImageHistoryLimit(parsed.limit ?? null);
        } catch (err) {
            console.error(
                "Failed to restore LLM image history limit preference:",
                err,
            );
            return DEFAULT_LLM_IMAGE_HISTORY_LIMIT;
        }
    }

    function syncEndOfUtteranceSilenceWithCaptureProcessor(
        milliseconds: number,
    ) {
        if (!captureProcessor) {
            return;
        }

        captureProcessor.port.postMessage({
            type: "set-end-of-utterance-silence-ms",
            milliseconds,
        });
    }

    function applyPongPlaybackPreference(enabled: boolean) {
        pongPlaybackEnabled = enabled;
        persistPongPlaybackPreference(enabled);

        invoke("set_pong_playback_enabled", { enabled }).catch((err) => {
            console.error("Failed to update pong playback preference:", err);
        });

        if (!enabled) {
            pendingCompletionPongRequestId = null;
            stopActivePongPlayback();
            if (calling) {
                updateStageAfterPlaybackStateChange();
            }
        }
    }

    function applyAutoUnmuteOnPastedScreenshotPreference(enabled: boolean) {
        autoUnmuteOnPastedScreenshotEnabled = enabled;
        persistAutoUnmuteOnPastedScreenshotPreference(enabled);
    }

    function applyAutoCheckAppUpdatesPreference(enabled: boolean) {
        autoCheckAppUpdatesEnabled = enabled;
        persistAppUpdatePreference({
            skippedVersion: skippedAppUpdateVersion,
            autoCheckEnabled: enabled,
        });
    }

    function applySelectLastSessionPreference(enabled: boolean) {
        selectLastSessionEnabled = enabled;
        window.localStorage.setItem(
            SELECT_LAST_SESSION_STORAGE_KEY,
            JSON.stringify({ version: 1, enabled }),
        );
    }

    function applyAutoLoadModelsOnStartupPreference(enabled: boolean) {
        autoLoadModelsOnStartupEnabled = enabled;
        window.localStorage.setItem(
            AUTO_LOAD_MODELS_ON_STARTUP_STORAGE_KEY,
            JSON.stringify({ version: 1, enabled }),
        );
    }

    function applyShowStatPreference(enabled: boolean) {
        showStatEnabled = enabled;
        persistShowStatPreference(enabled);
    }

    function applyShowSubtitlePreference(enabled: boolean) {
        showSubtitleEnabled = enabled;
        persistShowSubtitlePreference(enabled);
    }

    function applyShowAiSubtitlePreference(enabled: boolean) {
        showAiSubtitleEnabled = enabled;
        persistShowAiSubtitlePreference(enabled);
    }

    function applyAiSubtitleTargetLanguagePreference(
        targetLanguage: AiSubtitleTargetLanguage,
    ) {
        aiSubtitleTargetLanguage = targetLanguage;
        void invoke("set_ai_subtitle_target_language", {
            targetLang: targetLanguage,
        });
        persistAiSubtitleTargetLanguagePreference(targetLanguage);
    }

    function applyShowCallTimerPreference(enabled: boolean) {
        showCallTimerEnabled = enabled;
        localStorage.setItem(
            SHOW_CALL_TIMER_STORAGE_KEY,
            JSON.stringify(enabled),
        );
    }

    function applyShowHiddenWindowOverlayPreference(enabled: boolean) {
        showHiddenWindowOverlayEnabled = enabled;
        persistShowHiddenWindowOverlayPreference(enabled);
        if (enabled) {
            void ensureOverlayWindow();
        }
    }

    function applyEndOfUtteranceSilencePreference(milliseconds: number) {
        const normalizedMilliseconds =
            clampEndOfUtteranceSilenceMs(milliseconds);
        endOfUtteranceSilenceMs = normalizedMilliseconds;
        persistEndOfUtteranceSilencePreference(normalizedMilliseconds);
        syncEndOfUtteranceSilenceWithCaptureProcessor(normalizedMilliseconds);

        void invoke("set_end_of_utterance_silence_ms", {
            milliseconds: normalizedMilliseconds,
        }).catch((err) => {
            console.error(
                "Failed to update end-of-utterance silence preference:",
                err,
            );
        });
    }

    function applyAutoContinueSilencePreference(milliseconds: number | null) {
        const normalizedMilliseconds = clampAutoContinueSilenceMs(milliseconds);
        autoContinueSilenceMs = normalizedMilliseconds;
        persistAutoContinueSilencePreference(normalizedMilliseconds);

        void invoke("set_auto_continue_silence_ms", {
            milliseconds: normalizedMilliseconds,
        }).catch((err) => {
            console.error(
                "Failed to update auto-continue silence preference:",
                err,
            );
        });
    }

    function applyAutoContinueMaxCountPreference(count: number | null) {
        const normalizedCount = clampAutoContinueMaxCount(count);
        autoContinueMaxCount = normalizedCount;
        persistAutoContinueMaxCountPreference(normalizedCount);

        void invoke("set_auto_continue_max_count", {
            count: normalizedCount,
        }).catch((err) => {
            console.error(
                "Failed to update auto-continue max count preference:",
                err,
            );
        });
    }

    function applyLlmContextTurnLimitPreference(limit: number | null) {
        const normalizedLimit = clampLlmContextTurnLimit(limit);
        llmContextTurnLimit = normalizedLimit;
        persistLlmContextTurnLimitPreference(normalizedLimit);

        void invoke("set_llm_context_turn_limit", {
            limit: normalizedLimit,
        }).catch((err) => {
            console.error("Failed to update LLM context turn limit:", err);
        });
    }

    function applyLlmImageHistoryLimitPreference(limit: number | null) {
        const normalizedLimit = clampLlmImageHistoryLimit(limit);
        llmImageHistoryLimit = normalizedLimit;
        persistLlmImageHistoryLimitPreference(normalizedLimit);

        void invoke("set_llm_image_history_limit", {
            limit: normalizedLimit,
        }).catch((err) => {
            console.error("Failed to update LLM image history limit:", err);
        });
    }

    async function initializePongPlaybackPreference() {
        const storedEnabled = loadPongPlaybackPreferenceFromStorage();
        let effectiveEnabled = storedEnabled;

        try {
            effectiveEnabled = await invoke<boolean>(
                "initialize_pong_playback_preference",
                { enabled: storedEnabled },
            );
        } catch (err) {
            console.error(
                "Failed to initialize pong playback preference:",
                err,
            );
        }

        applyPongPlaybackPreference(effectiveEnabled);
    }

    async function initializeAppUpdatePreference() {
        const storedPreference = loadAppUpdatePreferenceFromStorage();
        skippedAppUpdateVersion = storedPreference.skippedVersion;
        autoCheckAppUpdatesEnabled = storedPreference.autoCheckEnabled;
    }

    async function initializeAutoUnmuteOnPastedScreenshotPreference() {
        const storedEnabled =
            loadAutoUnmuteOnPastedScreenshotPreferenceFromStorage();
        applyAutoUnmuteOnPastedScreenshotPreference(storedEnabled);
    }

    async function initializeSelectLastSessionPreference() {
        const storedEnabled = loadSelectLastSessionPreferenceFromStorage();
        applySelectLastSessionPreference(storedEnabled);
    }

    async function initializeAutoLoadModelsOnStartupPreference() {
        const storedEnabled =
            loadAutoLoadModelsOnStartupPreferenceFromStorage();
        applyAutoLoadModelsOnStartupPreference(storedEnabled);
    }

    async function initializeShowStatPreference() {
        const storedEnabled = loadShowStatPreferenceFromStorage();
        applyShowStatPreference(storedEnabled);
    }

    async function initializeShowSubtitlePreference() {
        const storedEnabled = loadShowSubtitlePreferenceFromStorage();
        applyShowSubtitlePreference(storedEnabled);
    }

    async function initializeShowAiSubtitlePreference() {
        const stored = localStorage.getItem(SHOW_AI_SUBTITLE_STORAGE_KEY);
        const storedEnabled =
            stored !== null
                ? (JSON.parse(stored) as StoredShowAiSubtitlePreference).enabled
                : DEFAULT_SHOW_AI_SUBTITLE;
        applyShowAiSubtitlePreference(storedEnabled);
    }

    async function initializeAiSubtitleTargetLanguagePreference() {
        const stored = localStorage.getItem(
            AI_SUBTITLE_TARGET_LANGUAGE_STORAGE_KEY,
        );
        const storedTargetLanguage =
            stored !== null
                ? (
                      JSON.parse(
                          stored,
                      ) as StoredAiSubtitleTargetLanguagePreference
                  ).targetLanguage
                : DEFAULT_AI_SUBTITLE_TARGET_LANGUAGE;
        applyAiSubtitleTargetLanguagePreference(storedTargetLanguage);
    }

    async function initializeShowCallTimerPreference() {
        const stored = localStorage.getItem(SHOW_CALL_TIMER_STORAGE_KEY);
        const storedEnabled =
            stored !== null
                ? (JSON.parse(stored) as boolean)
                : DEFAULT_SHOW_CALL_TIMER;
        applyShowCallTimerPreference(storedEnabled);
    }

    async function initializeShowHiddenWindowOverlayPreference() {
        const storedEnabled =
            loadShowHiddenWindowOverlayPreferenceFromStorage();
        applyShowHiddenWindowOverlayPreference(storedEnabled);
    }

    async function initializeEndOfUtteranceSilencePreference() {
        const storedMilliseconds =
            loadEndOfUtteranceSilencePreferenceFromStorage();
        applyEndOfUtteranceSilencePreference(storedMilliseconds);
    }

    async function initializeAutoContinueSilencePreference() {
        const storedMilliseconds =
            loadAutoContinueSilencePreferenceFromStorage();
        applyAutoContinueSilencePreference(storedMilliseconds);
    }

    async function initializeAutoContinueMaxCountPreference() {
        const storedCount = loadAutoContinueMaxCountPreferenceFromStorage();
        applyAutoContinueMaxCountPreference(storedCount);
    }

    async function initializeLlmContextTurnLimitPreference() {
        const storedLimit = loadLlmContextTurnLimitPreferenceFromStorage();
        applyLlmContextTurnLimitPreference(storedLimit);
    }

    async function initializeLlmImageHistoryLimitPreference() {
        const storedLimit = loadLlmImageHistoryLimitPreferenceFromStorage();
        applyLlmImageHistoryLimitPreference(storedLimit);
    }

    function persistGlobalShortcutPreference(shortcut: string) {
        if (typeof window === "undefined") {
            return;
        }

        const payload = {
            version: 1,
            shortcut,
        };
        window.localStorage.setItem(
            GLOBAL_SHORTCUT_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function loadGlobalShortcutPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_GLOBAL_SHORTCUT;
        }

        const rawPayload = window.localStorage.getItem(
            GLOBAL_SHORTCUT_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_GLOBAL_SHORTCUT;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                shortcut?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.shortcut !== "string") {
                return DEFAULT_GLOBAL_SHORTCUT;
            }

            return parsed.shortcut;
        } catch (err) {
            console.error("Failed to restore global shortcut preference:", err);
            return DEFAULT_GLOBAL_SHORTCUT;
        }
    }

    async function applyGlobalShortcutPreference(shortcut: string) {
        try {
            const effectiveShortcut = await invoke<string>(
                "set_global_shortcut_look_at_screen_region",
                { shortcutStr: shortcut },
            );
            globalShortcut = effectiveShortcut;
            persistGlobalShortcutPreference(effectiveShortcut);
        } catch (err) {
            console.error("Failed to apply global shortcut preference:", err);
            // If failed to apply, we don't update the state or persist
            alert(`Failed to set shortcut: ${err}`);
        }
    }

    async function initializeGlobalShortcutPreference() {
        const storedShortcut = loadGlobalShortcutPreferenceFromStorage();
        try {
            const effectiveShortcut = await invoke<string>(
                "initialize_global_shortcut_look_at_screen_region",
                { shortcutStr: storedShortcut },
            );
            globalShortcut = effectiveShortcut;
        } catch (err) {
            console.error(
                "Failed to initialize global shortcut preference:",
                err,
            );
        }
    }

    function persistGlobalShortcutEntireScreenPreference(shortcut: string) {
        if (typeof window === "undefined") {
            return;
        }

        const payload = {
            version: 1,
            shortcut,
        };
        window.localStorage.setItem(
            GLOBAL_SHORTCUT_ENTIRE_SCREEN_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function loadGlobalShortcutEntireScreenPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_GLOBAL_SHORTCUT_ENTIRE_SCREEN;
        }

        const rawPayload = window.localStorage.getItem(
            GLOBAL_SHORTCUT_ENTIRE_SCREEN_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_GLOBAL_SHORTCUT_ENTIRE_SCREEN;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                shortcut?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.shortcut !== "string") {
                return DEFAULT_GLOBAL_SHORTCUT_ENTIRE_SCREEN;
            }

            return parsed.shortcut;
        } catch (err) {
            console.error(
                "Failed to restore global shortcut (entire screen) preference:",
                err,
            );
            return DEFAULT_GLOBAL_SHORTCUT_ENTIRE_SCREEN;
        }
    }

    async function applyGlobalShortcutEntireScreenPreference(shortcut: string) {
        try {
            const effectiveShortcut = await invoke<string>(
                "set_global_shortcut_look_at_entire_screen",
                { shortcutStr: shortcut },
            );
            globalShortcutEntireScreen = effectiveShortcut;
            persistGlobalShortcutEntireScreenPreference(effectiveShortcut);
        } catch (err) {
            console.error(
                "Failed to apply global shortcut (entire screen) preference:",
                err,
            );
            alert(`Failed to set shortcut: ${err}`);
        }
    }

    async function initializeGlobalShortcutEntireScreenPreference() {
        const storedShortcut =
            loadGlobalShortcutEntireScreenPreferenceFromStorage();
        try {
            const effectiveShortcut = await invoke<string>(
                "initialize_global_shortcut_look_at_entire_screen",
                { shortcutStr: storedShortcut },
            );
            globalShortcutEntireScreen = effectiveShortcut;
        } catch (err) {
            console.error(
                "Failed to initialize global shortcut (entire screen) preference:",
                err,
            );
        }
    }

    function persistGlobalShortcutToggleMutePreference(shortcut: string) {
        if (typeof window === "undefined") {
            return;
        }

        const payload = {
            version: 1,
            shortcut,
        };
        window.localStorage.setItem(
            GLOBAL_SHORTCUT_TOGGLE_MUTE_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function loadGlobalShortcutToggleMutePreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_GLOBAL_SHORTCUT_TOGGLE_MUTE;
        }

        const rawPayload = window.localStorage.getItem(
            GLOBAL_SHORTCUT_TOGGLE_MUTE_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_GLOBAL_SHORTCUT_TOGGLE_MUTE;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                shortcut?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.shortcut !== "string") {
                return DEFAULT_GLOBAL_SHORTCUT_TOGGLE_MUTE;
            }

            return parsed.shortcut;
        } catch (err) {
            console.error(
                "Failed to restore global shortcut (toggle mute) preference:",
                err,
            );
            return DEFAULT_GLOBAL_SHORTCUT_TOGGLE_MUTE;
        }
    }

    async function applyGlobalShortcutToggleMutePreference(shortcut: string) {
        try {
            const effectiveShortcut = await invoke<string>(
                "set_global_shortcut_toggle_mute",
                { shortcutStr: shortcut },
            );
            globalShortcutToggleMute = effectiveShortcut;
            persistGlobalShortcutToggleMutePreference(effectiveShortcut);
        } catch (err) {
            console.error(
                "Failed to apply global shortcut (toggle mute) preference:",
                err,
            );
            alert(`Failed to set shortcut: ${err}`);
        }
    }

    async function initializeGlobalShortcutToggleMutePreference() {
        const storedShortcut =
            loadGlobalShortcutToggleMutePreferenceFromStorage();
        try {
            const effectiveShortcut = await invoke<string>(
                "initialize_global_shortcut_toggle_mute",
                { shortcutStr: storedShortcut },
            );
            globalShortcutToggleMute = effectiveShortcut;
        } catch (err) {
            console.error(
                "Failed to initialize global shortcut (toggle mute) preference:",
                err,
            );
        }
    }

    function persistGlobalShortcutInterruptPreference(shortcut: string) {
        if (typeof window === "undefined") {
            return;
        }

        const payload = {
            version: 1,
            shortcut,
        };
        window.localStorage.setItem(
            GLOBAL_SHORTCUT_INTERRUPT_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    function loadGlobalShortcutInterruptPreferenceFromStorage() {
        if (typeof window === "undefined") {
            return DEFAULT_GLOBAL_SHORTCUT_INTERRUPT;
        }

        const rawPayload = window.localStorage.getItem(
            GLOBAL_SHORTCUT_INTERRUPT_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_GLOBAL_SHORTCUT_INTERRUPT;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                shortcut?: unknown;
            };
            if (parsed.version !== 1 || typeof parsed.shortcut !== "string") {
                return DEFAULT_GLOBAL_SHORTCUT_INTERRUPT;
            }

            return parsed.shortcut;
        } catch (err) {
            console.error(
                "Failed to restore global shortcut (interrupt) preference:",
                err,
            );
            return DEFAULT_GLOBAL_SHORTCUT_INTERRUPT;
        }
    }

    async function applyGlobalShortcutInterruptPreference(shortcut: string) {
        try {
            const effectiveShortcut = await invoke<string>(
                "set_global_shortcut_interrupt",
                { shortcutStr: shortcut },
            );
            globalShortcutInterrupt = effectiveShortcut;
            persistGlobalShortcutInterruptPreference(effectiveShortcut);
        } catch (err) {
            console.error(
                "Failed to apply global shortcut (interrupt) preference:",
                err,
            );
            alert(`Failed to set shortcut: ${err}`);
        }
    }

    async function initializeGlobalShortcutInterruptPreference() {
        const storedShortcut =
            loadGlobalShortcutInterruptPreferenceFromStorage();
        try {
            const effectiveShortcut = await invoke<string>(
                "initialize_global_shortcut_interrupt",
                { shortcutStr: storedShortcut },
            );
            globalShortcutInterrupt = effectiveShortcut;
        } catch (err) {
            console.error(
                "Failed to initialize global shortcut (interrupt) preference:",
                err,
            );
        }
    }

    async function deleteConversationLogEntry(entryId: number) {
        const entryIndex = conversationLogEntries.findIndex(
            (candidate) => candidate.id === entryId,
        );
        if (entryIndex === -1) {
            return false;
        }
        const entry = conversationLogEntries[entryIndex];

        if (entry.contextEntryId == null) {
            conversationLogEntries = conversationLogEntries.filter(
                (candidate) => candidate.id !== entryId,
            );
            return true;
        }

        isSavingConversationLogEntryEdit = true;

        try {
            console.log("Deleting context entry", entry.contextEntryId);
            const otherContextEntryId: number | null = await invoke(
                "delete_conversation_context_entry",
                {
                    entryId: entry.contextEntryId,
                },
            );

            const contextEntryIdsToRemove = [entry.contextEntryId];
            if (otherContextEntryId !== null) {
                contextEntryIdsToRemove.push(otherContextEntryId);
            }

            conversationLogEntries = conversationLogEntries.filter(
                (candidate) =>
                    candidate.contextEntryId === null ||
                    !contextEntryIdsToRemove.includes(candidate.contextEntryId),
            );

            // Also clear the pending IDs if we just deleted the hot messages
            if (
                pendingConversationUserLogEntryId === entry.id ||
                pendingConversationUserLogEntryId === otherContextEntryId
            ) {
                pendingConversationUserLogEntryId = null;
            }
            if (
                activeAssistantConversationEntryId === entry.id ||
                activeAssistantConversationEntryId === otherContextEntryId
            ) {
                activeAssistantConversationEntryId = null;
            }

            return true;
        } catch (err) {
            const errorMsg = String(err);
            if (errorMsg.includes("no longer part of the active context")) {
                conversationLogEntries = conversationLogEntries.filter(
                    (candidate) => candidate.id !== entryId,
                );
                return true;
            }

            console.error("Failed to delete conversation entry:", err);
            alert(`Failed to delete the conversation entry.\n${errorMsg}`);
            return false;
        } finally {
            isSavingConversationLogEntryEdit = false;
        }
    }

    async function saveConversationLogEntryEdit(
        entryId: number,
        nextText: string,
        clearImage: boolean,
    ) {
        const entry = conversationLogEntries.find(
            (candidate) => candidate.id === entryId,
        );
        if (!entry) {
            return false;
        }

        const normalizedText = nextText.trim();
        const nextImageUrls = clearImage ? [] : entry.imageUrls || [];
        const nextTranslations = { ...(entry.translations || {}) };

        if (
            normalizedText === entry.text &&
            nextImageUrls === entry.imageUrls
        ) {
            return true;
        }

        // If it's a context-backed entry, we must update the backend.
        if (entry.contextEntryId !== null) {
            isSavingConversationLogEntryEdit = true;

            try {
                await invoke("update_conversation_context_entry", {
                    entryId: entry.contextEntryId,
                    text: normalizedText,
                    clearImages: clearImage,
                });

                // Success, update local state
                conversationLogEntries = conversationLogEntries.map(
                    (candidate) =>
                        candidate.id === entryId
                            ? {
                                  ...candidate,
                                  text: normalizedText,
                                  imageUrls: nextImageUrls,
                                  translations: nextTranslations,
                              }
                            : candidate,
                );
                return true;
            } catch (err) {
                const errorMsg = String(err);
                if (errorMsg.includes("no longer part of the active context")) {
                    // It's gone from the backend, so just update locally and clear the ID
                    conversationLogEntries = conversationLogEntries.map(
                        (candidate) =>
                            candidate.id === entryId
                                ? {
                                      ...candidate,
                                      text: normalizedText,
                                      imageUrls: nextImageUrls,
                                      contextEntryId: null,
                                      translations: nextTranslations,
                                  }
                                : candidate,
                    );
                    return true;
                }

                console.error("Failed to update conversation entry:", err);
                alert(`Failed to update the conversation entry.\n${errorMsg}`);
                return false;
            } finally {
                isSavingConversationLogEntryEdit = false;
            }
        } else {
            // No context ID. Just update locally.
            conversationLogEntries = conversationLogEntries.map((candidate) =>
                candidate.id === entryId
                    ? {
                          ...candidate,
                          text: normalizedText,
                          imageUrls: nextImageUrls,
                          translations: nextTranslations,
                      }
                    : candidate,
            );
            return true;
        }
    }

    async function clearConversationLogImages() {
        const hasVisibleImages = conversationLogEntries.some(
            (entry) => (entry.imageUrls?.length ?? 0) > 0,
        );
        if (!hasVisibleImages) {
            return;
        }

        const hasContextImages = conversationLogEntries.some(
            (entry) =>
                entry.contextEntryId !== null &&
                (entry.imageUrls?.length ?? 0) > 0,
        );

        isClearingConversationLogImages = true;

        try {
            if (hasContextImages) {
                await invoke("clear_conversation_context_images");
            }
            clearConversationLogImageHistoryEntries();
        } catch (err) {
            console.error("Failed to clear conversation images:", err);
            alert(`Failed to clear conversation images.\n${String(err)}`);
        } finally {
            isClearingConversationLogImages = false;
        }
    }

    function clearConversationLogImageHistoryEntries() {
        conversationLogEntries = conversationLogEntries.map((entry) => ({
            ...entry,
            imageUrls: [],
        }));
    }

    $effect(() => {
        if (typeof window === "undefined") {
            return;
        }

        if (
            syncedConversationLogHasVisibleImages ===
            conversationLogHasVisibleImages
        ) {
            return;
        }

        syncedConversationLogHasVisibleImages = conversationLogHasVisibleImages;
        void invoke("sync_conversation_log_has_visible_images", {
            visible: conversationLogHasVisibleImages,
        }).catch((err) =>
            console.error(
                "Failed to sync visible conversation image history:",
                err,
            ),
        );
    });

    function getCurrentModelSelection(): ModelSelection {
        return {
            gemmaVariant: selectedGemmaVariant,
            csmModel: selectedCsmModel,
            sttModel: selectedSttModel,
            ollamaModel: selectedOllamaModel,
            lmstudioModel: selectedLmStudioModel,
            openaiCompatibleModel: selectedOpenAiCompatibleModel,
        };
    }

    async function setGemmaVariantSelection(nextVariant: GemmaVariant) {
        await invoke("set_gemma_variant", { variant: nextVariant });
    }

    async function setCsmModelSelection(nextVariant: CsmModelVariant) {
        await invoke("set_csm_model_variant", { variant: nextVariant });
        resetDownloadState("csm");
        csmLoadMessage = "Starting worker...";
    }

    async function setSttModelSelection(nextVariant: SttModelVariant) {
        await invoke("set_stt_model_variant", { variant: nextVariant });
        resetDownloadState("stt");
        sttLoadMessage = "Starting worker...";
    }

    async function restoreModelPreferences() {
        const restoredPreferences = loadModelPreferencesFromStorage();

        selectedGemmaVariant = restoredPreferences.gemmaVariant;
        selectedCsmModel = restoredPreferences.csmModel;
        selectedSttModel =
            restoredPreferences.sttModel === "gemma" &&
            isExternalGemmaVariant(restoredPreferences.gemmaVariant)
                ? DEFAULT_STT_MODEL
                : restoredPreferences.sttModel;
        if (restoredPreferences.ollamaModel) {
            selectedOllamaModel = restoredPreferences.ollamaModel;
        }
        if (restoredPreferences.lmstudioModel) {
            selectedLmStudioModel = restoredPreferences.lmstudioModel;
        }
        if (restoredPreferences.openaiCompatibleModel) {
            selectedOpenAiCompatibleModel =
                restoredPreferences.openaiCompatibleModel;
        }

        try {
            await setGemmaVariantSelection(restoredPreferences.gemmaVariant);
        } catch (err) {
            console.error("Failed to restore Gemma variant:", err);
        }

        if (restoredPreferences.ollamaModel) {
            try {
                await invoke("set_ollama_model", {
                    model: restoredPreferences.ollamaModel,
                });
            } catch (err) {
                console.error("Failed to restore Ollama model:", err);
            }
        }

        if (restoredPreferences.lmstudioModel) {
            try {
                await invoke("set_lmstudio_model", {
                    model: restoredPreferences.lmstudioModel,
                });
            } catch (err) {
                console.error("Failed to restore LM Studio model:", err);
            }
        }

        if (restoredPreferences.openaiCompatibleModel) {
            try {
                await invoke("set_openai_compatible_model", {
                    model: restoredPreferences.openaiCompatibleModel,
                });
            } catch (err) {
                console.error(
                    "Failed to restore OpenAI-compatible model:",
                    err,
                );
            }
        }

        try {
            await setCsmModelSelection(restoredPreferences.csmModel);
        } catch (err) {
            console.error("Failed to restore speech model:", err);
        }

        try {
            await setSttModelSelection(selectedSttModel);
        } catch (err) {
            console.error("Failed to restore STT model:", err);
        }

        if (isOllamaSupported) {
            await syncOllamaModels();
        }
        if (isLmStudioSupported) {
            await syncLmStudioModels();
        }
        if (isOpenAiCompatibleSupported) {
            await syncOpenAiCompatibleModels();
        }
    }

    async function syncSelectedContactPrompt() {
        try {
            await invoke("set_voice_system_prompt", {
                prompt: selectedContactPrompt,
            });
        } catch (err) {
            console.error("Failed to sync the selected contact prompt:", err);
        }
    }

    function queueSelectedContactPromptSync() {
        if (selectedContactPromptSyncTimeout) {
            clearTimeout(selectedContactPromptSyncTimeout);
        }

        selectedContactPromptSyncTimeout = window.setTimeout(() => {
            selectedContactPromptSyncTimeout = null;
            void syncSelectedContactPrompt();
        }, 160);
    }

    function resolveContactVoicePreset(
        contact: Pick<ContactProfile, "gender"> | null | undefined,
    ): ContactGender {
        return contact?.gender === "male" ? "male" : "female";
    }

    async function syncSelectedContactVoiceReference() {
        const voicePreset = resolveContactVoicePreset(selectedContact);

        if (selectedCsmModel === "kokoro_82m") {
            try {
                await invoke("set_csm_voice", { voice: voicePreset });
            } catch (err) {
                console.error("Failed to sync the Kokoro voice:", err);
            }
            return;
        }

        if (!selectedContact?.refAudio) {
            try {
                await invoke("set_csm_voice", { voice: voicePreset });
            } catch (err) {
                console.error("Failed to reset the voice reference:", err);
            }
            return;
        }

        try {
            await invoke("set_csm_reference_voice", {
                refAudioDataUrl: selectedContact.refAudio,
                refText: selectedContact.refText,
            });
        } catch (err) {
            console.error(
                "Failed to sync the selected contact voice reference:",
                err,
            );
        }
    }

    function queueSelectedContactVoiceReferenceSync() {
        if (selectedContactVoiceReferenceSyncTimeout) {
            clearTimeout(selectedContactVoiceReferenceSyncTimeout);
        }

        selectedContactVoiceReferenceSyncTimeout = window.setTimeout(() => {
            selectedContactVoiceReferenceSyncTimeout = null;
            void syncSelectedContactVoiceReference();
        }, 320);
    }

    function updateContactById(
        contactId: string,
        updater: (contact: ContactProfile) => ContactProfile,
    ) {
        let didUpdate = false;
        contacts = contacts.map((contact) => {
            if (contact.id !== contactId) {
                return contact;
            }

            didUpdate = true;
            return updater(contact);
        });

        if (didUpdate) {
            persistContactsMetadata();
        }
    }

    function closeContactsPopup() {
        showContactsPopup = false;
    }

    function closeConversationPopup() {
        showConversationPopup = false;
    }

    function closeAboutPopup() {
        closeSubtitleTranslationLlmConfig();
        showAboutPopup = false;
    }

    function closeUpdatePrompt() {
        showUpdatePrompt = false;
        pendingAutomaticUpdatePromptVersion = null;
    }

    function queueAutomaticUpdatePrompt(version: string) {
        if (calling || showAboutPopup) {
            pendingAutomaticUpdatePromptVersion = version;
            return;
        }

        showUpdatePrompt = true;
        pendingAutomaticUpdatePromptVersion = null;
    }

    function shouldPromptForAutomaticUpdate(
        version: string,
        suppressPrompt: boolean,
    ) {
        return !suppressPrompt && version !== skippedAppUpdateVersion;
    }

    function skipAvailableAppUpdateVersion() {
        const version = availableAppUpdate?.version?.trim();
        if (version) {
            skippedAppUpdateVersion = version;
            persistAppUpdatePreference({
                skippedVersion: version,
                autoCheckEnabled: autoCheckAppUpdatesEnabled,
            });
        }

        suppressAutoUpdatePromptUntilNextCheck = false;
        closeUpdatePrompt();
    }

    function remindAboutAppUpdateLater() {
        suppressAutoUpdatePromptUntilNextCheck = true;
        closeUpdatePrompt();
    }

    async function triggerAutomaticAppUpdateCheck() {
        if (
            typeof navigator === "undefined" ||
            !navigator.onLine ||
            !autoCheckAppUpdatesEnabled
        ) {
            return;
        }

        if (
            appUpdateStatus === "checking" ||
            appUpdateStatus === "installing" ||
            appUpdateStatus === "installed"
        ) {
            return;
        }

        const suppressPrompt = suppressAutoUpdatePromptUntilNextCheck;
        suppressAutoUpdatePromptUntilNextCheck = false;

        await checkForAppUpdates({
            source: "automatic",
            suppressAutoPrompt: suppressPrompt,
        });
    }

    async function checkForAppUpdates(options: {
        source?: "manual" | "automatic";
        suppressAutoPrompt?: boolean;
    } = {}) {
        const source = options.source ?? "manual";
        const suppressAutoPrompt = options.suppressAutoPrompt ?? false;
        if (
            appUpdateStatus === "checking" ||
            appUpdateStatus === "installing"
        ) {
            return;
        }

        const previousAvailableAppUpdate = availableAppUpdate;
        const previousAppUpdateStatus = appUpdateStatus;
        const previousAppUpdateError = appUpdateError;
        const previousUpdateObject = updateObject;

        appUpdateStatus = "checking";
        appUpdateError = null;
        availableAppUpdate = null;
        updateObject = null;

        try {
            updateObject = await check();
            if (updateObject) {
                const rawTarget = updateObject.rawJson.target;
                const releaseNotes = await resolveAppUpdateReleaseNotes(
                    updateObject,
                );
                availableAppUpdate = {
                    version: updateObject.version,
                    currentVersion:
                        updateObject.currentVersion || buildInfo?.version || "",
                    notes: releaseNotes.notesPreview,
                    publishedAt: updateObject.date,
                    target: typeof rawTarget === "string" ? rawTarget : "",
                    releaseNotesUrl: releaseNotes.releaseNotesUrl,
                };
                appUpdateStatus = "available";

                if (
                    source === "automatic" &&
                    shouldPromptForAutomaticUpdate(
                        updateObject.version,
                        suppressAutoPrompt,
                    )
                ) {
                    queueAutomaticUpdatePrompt(updateObject.version);
                } else if (source === "automatic") {
                    closeUpdatePrompt();
                }
            } else {
                appUpdateStatus = "up_to_date";
                closeUpdatePrompt();
            }
        } catch (err) {
            console.error("Failed to check for app updates:", err);
            if (source === "automatic") {
                availableAppUpdate = previousAvailableAppUpdate;
                appUpdateStatus = previousAppUpdateStatus;
                appUpdateError = previousAppUpdateError;
                updateObject = previousUpdateObject;
                return;
            }

            availableAppUpdate = null;
            updateObject = null;
            appUpdateStatus = "error";
            appUpdateError = normalizeErrorMessage(err);
        }
    }

    async function installAppUpdate() {
        if (appUpdateStatus !== "available" || !updateObject) {
            return;
        }

        appUpdateStatus = "installing";
        appUpdateError = null;

        try {
            let downloaded = 0;
            let contentLength = 0;
            await updateObject.downloadAndInstall((event) => {
                switch (event.event) {
                    case "Started":
                        contentLength = event.data.contentLength || 0;
                        console.log(
                            `started downloading ${contentLength} bytes`,
                        );
                        break;
                    case "Progress":
                        downloaded += event.data.chunkLength;
                        if (contentLength > 0) {
                            console.log(
                                `downloaded ${downloaded} from ${contentLength}`,
                            );
                        }
                        break;
                    case "Finished":
                        console.log("download finished");
                        break;
                }
            });
            appUpdateStatus = "installed";
        } catch (err) {
            console.error("Failed to install app update:", err);
            appUpdateStatus = "error";
            appUpdateError = formatAppUpdateInstallError(err);
        }
    }

    async function restartToApplyUpdate() {
        try {
            await relaunch();
        } catch (err) {
            console.error("Failed to restart after installing update:", err);
            appUpdateError = normalizeErrorMessage(err);
        }
    }

    async function loadBuildInfo() {
        try {
            buildInfo = await invoke<BuildInfo>("get_build_info");
            buildInfoError = null;
        } catch (err) {
            console.error("Failed to load build info:", err);
            buildInfoError = normalizeErrorMessage(err);
        }
    }

    function openAboutPopup(options: { checkForUpdates?: boolean } = {}) {
        showAboutPopup = true;
        closeContactsPopup();
        closeConversationPopup();

        if (!buildInfo) {
            void loadBuildInfo();
        }

        if (options.checkForUpdates) {
            void checkForAppUpdates();
        }
    }

    async function fetchGithubReleaseNotesMetadata() {
        const response = await fetch(GITHUB_LATEST_RELEASE_API_URL, {
            headers: {
                Accept: "application/vnd.github+json",
            },
        });

        if (!response.ok) {
            throw new Error(
                `GitHub release notes request failed with status ${response.status}.`,
            );
        }

        const payload = (await response.json()) as GithubLatestReleasePayload;
        const notes =
            typeof payload.body === "string" ? payload.body : undefined;
        const releaseNotesUrl =
            typeof payload.html_url === "string" && payload.html_url.trim()
                ? payload.html_url
                : RELEASE_NOTES_URL;

        return {
            notesPreview: createReleaseNotesPreview(notes, 150),
            releaseNotesUrl,
        };
    }

    async function resolveAppUpdateReleaseNotes(update: Update) {
        const fallbackNotesPreview = createReleaseNotesPreview(update.body, 150);

        try {
            return await fetchGithubReleaseNotesMetadata();
        } catch (err) {
            console.warn("Failed to fetch GitHub release notes preview:", err);
            return {
                notesPreview: fallbackNotesPreview,
                releaseNotesUrl: RELEASE_NOTES_URL,
            };
        }
    }

    function toggleContactsPopup() {
        showContactsPopup = !showContactsPopup;
        if (showContactsPopup) {
            closeConversationPopup();
            closeAboutPopup();
        }
    }

    function toggleConversationPopup() {
        showConversationPopup = !showConversationPopup;
        if (showConversationPopup) {
            closeContactsPopup();
            closeAboutPopup();
            scrollConversationLogToBottom();
        }
    }

    async function loadSessions() {
        try {
            sessions = await invoke<SessionMetadata[]>("get_sessions");
            currentSessionId = await invoke<string | null>(
                "get_current_session_id",
            );
            if (currentSessionId) {
                const session = sessions.find((s) => s.id === currentSessionId);
                if (session) {
                    currentSessionTitle = session.title;
                }
            }
        } catch (err) {
            console.error("Failed to load sessions:", err);
        }
    }

    async function handleSelectSession(session: SessionMetadata) {
        try {
            const turns = await invoke<ConversationTurn[]>("load_session", {
                sessionId: session.id,
            });
            currentSessionId = session.id;
            resetConversationLog();
            for (const turn of turns) {
                // Prefer data URLs if available, otherwise fall back to paths.
                // We avoid combining them to prevent duplicate images in the UI.
                let imageUrls = [...turn.user_image_data_urls];
                if (imageUrls.length === 0 && turn.image_paths.length > 0) {
                    for (const path of turn.image_paths) {
                        imageUrls.push(convertFileSrc(path));
                    }
                }
                const userEntryId = appendConversationLogEntry(
                    "user",
                    turn.user_text,
                    imageUrls,
                );
                const assistantEntryId = appendConversationLogEntry(
                    "assistant",
                    turn.assistant_text,
                    [],
                    turn.translations,
                );
                // Mark them as context backed immediately
                conversationLogEntries = conversationLogEntries.map((entry) => {
                    if (entry.id === userEntryId)
                        return { ...entry, contextEntryId: turn.user_entry_id };
                    if (entry.id === assistantEntryId)
                        return {
                            ...entry,
                            contextEntryId: turn.assistant_entry_id,
                        };
                    return entry;
                });
            }
            currentSessionTitle = session.title;
            showSessionsPopup = false;

            // Pulse the conversation log button if the session has content
            if (turns.length > 0) {
                const logBtn = document.querySelector(".conversation-log-btn");
                if (logBtn) {
                    logBtn.classList.add("pulse-animation");
                    setTimeout(
                        () => logBtn.classList.remove("pulse-animation"),
                        1000,
                    );
                }
            }
        } catch (err) {
            console.error("Failed to load session:", err);
            alert(`Failed to load session.\n${String(err)}`);
        }
    }

    async function handleDeleteSession(session: SessionMetadata) {
        try {
            await invoke("delete_session", { sessionId: session.id });
            if (session.id === currentSessionId) {
                await handleNewChat();
            } else {
                await loadSessions();
            }
        } catch (err) {
            console.error("Failed to delete session:", err);
            alert(`Failed to delete session.\n${String(err)}`);
        }
    }

    async function handleRenameSession(
        session: SessionMetadata,
        newTitle: string,
    ) {
        if (!newTitle || newTitle === session.title) return;
        try {
            await invoke("rename_session", {
                sessionId: session.id,
                newTitle: newTitle,
            });
            if (
                session.id ===
                (await invoke<string | null>("get_current_session_id"))
            ) {
                currentSessionTitle = newTitle;
            }
            await loadSessions();
        } catch (err) {
            console.error("Failed to rename session:", err);
            alert(`Failed to rename session.\n${String(err)}`);
        }
    }

    async function handleForkSession(entry: ConversationLogEntry) {
        if (entry.contextEntryId === null) {
            alert("This message is not yet saved to the conversation history.");
            return;
        }

        const originalTitle = currentSessionTitle || "Conversation";
        const newTitle = `${originalTitle} - Fork`;

        try {
            const newMetadata = await invoke<SessionMetadata>("fork_session", {
                assistantEntryId: entry.contextEntryId,
                newTitle: newTitle,
            });
            await loadSessions();
            await handleSelectSession(newMetadata);
        } catch (err) {
            console.error("Failed to fork session:", err);
            alert(`Failed to fork session.\n${String(err)}`);
        }
    }

    async function handleNewChat() {
        try {
            await invoke("reset_call_session");
            resetConversationLog();
            currentSessionId = await invoke<string | null>(
                "get_current_session_id",
            );
            currentSessionTitle = null;
            showSessionsPopup = false;
        } catch (err) {
            console.error("Failed to reset session:", err);
        }
    }

    function toggleSessionsPopup() {
        showSessionsPopup = !showSessionsPopup;
    }

    function openSearchModal() {
        showSearchModal = true;
        showSessionsPopup = false;
    }

    function closeSearchModal() {
        showSearchModal = false;
    }

    async function handleSearchSelect(sessionId: string) {
        showSearchModal = false;
        const session = sessions.find((s) => s.id === sessionId);
        if (session) {
            await handleSelectSession(session);
        } else {
            // If session not in memory list, we might need to load it by ID
            // but sessions list is usually updated.
            console.warn("Session not found in memory list:", sessionId);
            // Fallback: we could invoke a load_session directly
            try {
                const turns = await invoke<ConversationTurn[]>("load_session", {
                    sessionId,
                });
                applyLoadedSession(sessionId, turns);
            } catch (err) {
                console.error("Failed to load session from search:", err);
            }
        }
    }

    function applyLoadedSession(sessionId: string, turns: ConversationTurn[]) {
        currentSessionId = sessionId;
        const session = sessions.find((s) => s.id === sessionId);
        currentSessionTitle = session?.title || null;

        resetConversationLog();
        for (const turn of turns) {
            appendConversationLogEntry("user", turn.user_text as string, [
                ...turn.image_paths,
            ]);
            upsertAssistantConversationLogEntry(
                0,
                turn.assistant_text as string,
            );
        }
        scrollConversationLogToBottom();
    }

    function toggleAboutPopup() {
        if (showAboutPopup) {
            closeAboutPopup();
            return;
        }

        openAboutPopup();
    }

    function selectContact(contactId: string) {
        if (!contacts.some((contact) => contact.id === contactId)) {
            return;
        }

        selectedContactId = contactId;
        persistContactsMetadata();
        queueSelectedContactPromptSync();
        queueSelectedContactVoiceReferenceSync();
    }

    function createNewContact() {
        const nextContact: ContactProfile = {
            id: createContactId(),
            name: `Contact ${contacts.length + 1}`,
            prompt: selectedContact?.prompt ?? DEFAULT_CONTACT_PROMPT,
            hasCustomIcon: false,
            iconDataUrl: null,
            gender: selectedContact?.gender ?? null,
            refAudio: null,
            refText: null,
        };

        contacts = [...contacts, nextContact];
        selectedContactId = nextContact.id;
        persistContactsMetadata();
        showContactsPopup = true;
        queueSelectedContactPromptSync();
        queueSelectedContactVoiceReferenceSync();
    }

    async function handleDeleteSelectedContact() {
        if (!selectedContact || contacts.length <= 1) {
            return;
        }

        const contactId = selectedContact.id;
        try {
            await deleteStoredContactIcon(contactId);
        } catch (err) {
            console.error("Failed to remove the contact icon:", err);
        }

        const remainingContacts = contacts.filter(
            (contact) => contact.id !== contactId,
        );
        contacts = remainingContacts;
        selectedContactId = remainingContacts[0]?.id ?? DEFAULT_CONTACT_ID;
        persistContactsMetadata();
        queueSelectedContactPromptSync();
        queueSelectedContactVoiceReferenceSync();
    }

    function handleSelectedContactNameInput(event: Event) {
        if (!selectedContact) {
            return;
        }

        const nextName = (event.currentTarget as HTMLInputElement).value;
        updateContactById(selectedContact.id, (contact) => ({
            ...contact,
            name: nextName,
        }));
    }

    function handleSelectedContactPromptInput(event: Event) {
        if (!selectedContact) {
            return;
        }

        const nextPrompt = (event.currentTarget as HTMLTextAreaElement).value;
        updateContactById(selectedContact.id, (contact) => ({
            ...contact,
            prompt: nextPrompt,
        }));
        queueSelectedContactPromptSync();
    }

    function handleSelectedContactGenderInput(event: Event) {
        if (!selectedContact) {
            return;
        }

        const nextGender = (event.currentTarget as HTMLSelectElement)
            .value as ContactGender;
        updateContactById(selectedContact.id, (contact) => ({
            ...contact,
            gender: nextGender,
        }));
        queueSelectedContactVoiceReferenceSync();
    }

    function triggerContactImport() {
        contactsImportInput?.click();
    }

    function triggerContactIconUpload() {
        contactIconInput?.click();
    }

    async function importContactFromRawText(
        rawText: string,
        sourceLabel?: string | null,
    ) {
        try {
            const nextContact = createImportedContactFromRawText(rawText);

            if (nextContact.iconDataUrl) {
                await saveStoredContactIcon(nextContact.id, nextContact.iconDataUrl);
            }

            contacts = [...contacts, nextContact];
            persistContactsMetadata();

            if (calling) {
                alert(
                    `${nextContact.name} has been added. Please check contacts page after finishing the current conversation`,
                );
                return;
            }

            selectedContactId = nextContact.id;
            showContactsPopup = true;
            queueSelectedContactPromptSync();
            queueSelectedContactVoiceReferenceSync();
        } catch (err) {
            console.error("Failed to import contact:", err);
            const message = sourceLabel
                ? `Failed to import contact from ${sourceLabel}.\n${normalizeErrorMessage(err)}`
                : `Failed to import contact.\n${normalizeErrorMessage(err)}`;
            alert(message);
        }
    }

    async function handleOpenDuckContactImportEvent(
        payload: OpenDuckContactImportEvent,
    ) {
        if (payload.error) {
            alert(
                `Failed to import contact from ${payload.sourcePath}.\n${payload.error}`,
            );
            return;
        }

        if (!payload.rawText) {
            alert(
                `Failed to import contact from ${payload.sourcePath}.\nThe file did not contain any import data.`,
            );
            return;
        }

        await importContactFromRawText(payload.rawText, payload.sourcePath);
    }

    async function initializeOpenDuckContactImports() {
        try {
            const pendingImports = await invoke<OpenDuckContactImportEvent[]>(
                "initialize_openduck_contact_imports",
            );

            for (const pendingImport of pendingImports) {
                await handleOpenDuckContactImportEvent(pendingImport);
            }
        } catch (err) {
            console.error(
                "Failed to initialize OpenDuck contact imports:",
                err,
            );
        }
    }

    async function handleContactImportChange(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        input.value = "";

        if (!file) {
            return;
        }

        await importContactFromRawText(await file.text(), file.name);
    }

    async function handleContactIconChange(event: Event) {
        if (!selectedContact) {
            return;
        }

        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        input.value = "";

        if (!file) {
            return;
        }

        try {
            const iconDataUrl = await readFileAsDataUrl(file);
            await saveStoredContactIcon(selectedContact.id, iconDataUrl);
            updateContactById(selectedContact.id, (contact) => ({
                ...contact,
                hasCustomIcon: true,
                iconDataUrl,
            }));
        } catch (err) {
            console.error("Failed to save the selected contact icon:", err);
            alert(
                `Failed to save contact icon.\n${normalizeErrorMessage(err)}`,
            );
        }
    }

    async function handleResetSelectedContactIcon() {
        if (!selectedContact?.hasCustomIcon) {
            return;
        }

        try {
            await deleteStoredContactIcon(selectedContact.id);
            updateContactById(selectedContact.id, (contact) => ({
                ...contact,
                hasCustomIcon: false,
                iconDataUrl: null,
            }));
        } catch (err) {
            console.error("Failed to clear the contact icon:", err);
            alert(
                `Failed to clear contact icon.\n${normalizeErrorMessage(err)}`,
            );
        }
    }

    function triggerContactRefAudioUpload() {
        contactRefAudioInput?.click();
    }

    async function handleContactRefAudioChange(event: Event) {
        if (!selectedContact) {
            return;
        }

        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        input.value = "";

        if (!file) {
            return;
        }

        try {
            const refAudio = await readFileAsDataUrl(file);
            updateContactById(selectedContact.id, (contact) => ({
                ...contact,
                refAudio,
            }));
            persistContactsMetadata();
            queueSelectedContactVoiceReferenceSync();
        } catch (err) {
            console.error("Failed to save the voice reference audio:", err);
            alert(
                `Failed to save voice reference audio.\n${normalizeErrorMessage(err)}`,
            );
        }
    }

    function handleResetSelectedContactRefAudio() {
        if (!selectedContact) {
            return;
        }

        updateContactById(selectedContact.id, (contact) => ({
            ...contact,
            refAudio: null,
        }));
        persistContactsMetadata();
        queueSelectedContactVoiceReferenceSync();
    }

    function handlePlaySelectedContactRefAudio() {
        if (!selectedContact?.refAudio) {
            return;
        }

        if (refAudioPlaying && refAudioEl) {
            refAudioEl.pause();
            refAudioEl = null;
            refAudioPlaying = false;
            return;
        }

        if (refAudioEl) {
            refAudioEl.pause();
        }

        refAudioEl = new Audio(selectedContact.refAudio);
        refAudioPlaying = true;

        refAudioEl.onended = () => {
            refAudioPlaying = false;
            refAudioEl = null;
        };

        refAudioEl.onerror = () => {
            refAudioPlaying = false;
            refAudioEl = null;
        };

        refAudioEl.play().catch((err) => {
            console.error("Failed to play voice reference audio:", err);
            refAudioPlaying = false;
            refAudioEl = null;
        });
    }

    function handleSelectedContactRefTextInput(event: Event) {
        if (!selectedContact) {
            return;
        }

        const nextText = (event.currentTarget as HTMLInputElement).value;
        updateContactById(selectedContact.id, (contact) => ({
            ...contact,
            refText: nextText,
        }));
        persistContactsMetadata();
        queueSelectedContactVoiceReferenceSync();
    }

    async function handleExportSelectedContact() {
        if (!selectedContact) {
            return;
        }

        try {
            const outputPath = await save({
                title: "Export Contact",
                defaultPath: `${slugifyContactName(selectedContact.name)}.openduck`,
                filters: [
                    {
                        name: "OpenDuck Contact",
                        extensions: ["openduck"],
                    },
                ],
            });
            if (!outputPath) {
                return;
            }

            const result = await invoke<ContactExportResult>(
                "export_contact_profile",
                {
                    payload: {
                        name: getContactDisplayName(selectedContact),
                        prompt:
                            selectedContact.prompt.trim() ||
                            DEFAULT_CONTACT_PROMPT,
                        iconDataUrl: selectedContact.iconDataUrl,
                        gender: selectedContact.gender,
                        refAudio: selectedContact.refAudio,
                        refText: selectedContact.refText,
                        outputPath,
                    },
                },
            );
            alert(`Exported contact to:\n${result.savedPath}`);
        } catch (err) {
            console.error("Failed to export contact:", err);
            alert(`Failed to export contact.\n${normalizeErrorMessage(err)}`);
        }
    }

    function shouldIgnoreGlobalShortcutTarget(target: EventTarget | null) {
        if (!(target instanceof HTMLElement)) {
            return false;
        }

        return (
            target.isContentEditable ||
            target.closest(
                "input, textarea, select, [contenteditable]:not([contenteditable='false'])",
            ) !== null
        );
    }

    async function handleWindowPaste(event: ClipboardEvent) {
        if (!event.clipboardData) {
            return;
        }

        const imageItem = Array.from(event.clipboardData.items).find(
            (item) => item.kind === "file" && item.type.startsWith("image/"),
        );
        const file = imageItem?.getAsFile();
        if (!file) {
            return;
        }

        if (
            !calling ||
            (shouldIgnoreGlobalShortcutTarget(event.target) &&
                !imageItem.type.startsWith("image/"))
        ) {
            return;
        }

        event.preventDefault();

        try {
            const dataUrl = await readFileAsDataUrl(file);
            await invoke("attach_pasted_screen_capture", { dataUrl });
        } catch (err) {
            console.error("Failed to attach pasted screenshot:", err);
            alert(`Failed to paste screenshot.\n${String(err)}`);
        }
    }

    function handleWindowKeydown(event: KeyboardEvent) {
        if (event.defaultPrevented) {
            return;
        }

        if (
            calling &&
            event.code === "Space" &&
            !event.repeat &&
            !event.altKey &&
            !event.ctrlKey &&
            !event.metaKey &&
            !event.shiftKey &&
            !shouldIgnoreGlobalShortcutTarget(event.target)
        ) {
            event.preventDefault();
            toggleMic();
            return;
        }

        if (event.key !== "Escape") {
            return;
        }

        if (assistantSpeaking) {
            void handleInterruptTts();
            return;
        }

        if (previewImageUrl) {
            previewImageUrl = null;
            return;
        }

        if (showUpdatePrompt) {
            if (appUpdateStatus === "installing") {
                return;
            }

            remindAboutAppUpdateLater();
            return;
        }

        if (showSubtitleTranslationLlmConfig) {
            closeSubtitleTranslationLlmConfig();
            return;
        }

        if (showAboutPopup) {
            closeAboutPopup();
            return;
        }

        if (showContactsPopup) {
            closeContactsPopup();
            return;
        }

        if (showConversationPopup) {
            closeConversationPopup();
            return;
        }

        if (showSessionsPopup) {
            showSessionsPopup = false;
            return;
        }

        if (showExternalLlmConfig) {
            closeExternalConfig();
        }
    }

    function handleWindowFocus() {
        if (calling) {
            syncCallElapsedTime();
        }
    }

    function stopCallTimerTracking() {
        callStartedAtMs = null;
        time = 0;
        resetProcessingAudioLatencies();

        if (callTimerInterval) {
            clearInterval(callTimerInterval);
            callTimerInterval = null;
        }

        void invoke("stop_call_timer").catch((err) =>
            console.error("Failed to stop tray call timer", err),
        );
    }

    function resetDownloadState(model: "gemma" | "csm" | "stt") {
        if (model === "gemma") {
            gemmaDownloadMessage = "Preparing download...";
            gemmaDownloadProgress = null;
            gemmaDownloadIndeterminate = true;
            gemmaDownloadError = null;
            return;
        }

        if (model === "stt") {
            sttDownloadMessage = "Preparing download...";
            sttDownloadProgress = null;
            sttDownloadIndeterminate = true;
            sttDownloadError = null;
            return;
        }

        csmDownloadMessage = "Preparing download...";
        csmDownloadProgress = null;
        csmDownloadIndeterminate = true;
        csmDownloadError = null;
    }

    function stopModelMemoryPolling() {
        if (modelMemoryPollInterval) {
            clearInterval(modelMemoryPollInterval);
            modelMemoryPollInterval = null;
        }
    }

    async function syncModelMemoryUsage() {
        if (!hasLoadedModelProcess) {
            modelMemorySnapshot = null;
            stopModelMemoryPolling();
            return;
        }

        try {
            const snapshot = await invoke<ModelMemoryUsageSnapshot>(
                "get_model_memory_usage",
            );

            if (snapshot.models.length === 0) {
                modelMemorySnapshot = null;
                return;
            }

            modelMemorySnapshot = snapshot;
        } catch (err) {
            console.error("Failed to sync model memory usage:", err);
        }
    }

    function ensureModelMemoryPolling() {
        if (modelMemoryPollInterval || !hasLoadedModelProcess) {
            return;
        }

        void syncModelMemoryUsage();
        // Polling every 60 seconds
        // Disable polling for now since it's not accurate
        // modelMemoryPollInterval = window.setInterval(() => {
        //     void syncModelMemoryUsage();
        // }, 60000);
    }

    function applyDownloadEvent(payload: ModelDownloadProgressEvent) {
        if (payload.model === "gemma") {
            gemmaDownloadMessage = payload.message;
            gemmaDownloadProgress = payload.progress ?? null;
            gemmaDownloadIndeterminate = payload.indeterminate;
            if (payload.phase === "error") {
                gemmaDownloadError = normalizeDownloadErrorMessage(
                    payload.message,
                );
            } else if (
                payload.phase === "completed" ||
                payload.phase === "cancelled"
            ) {
                gemmaDownloadError = null;
            }
            if (payload.phase === "cancelled") {
                isCancellingGemmaDownload = false;
            }
            return;
        }

        if (payload.model === "stt") {
            sttDownloadMessage = payload.message;
            sttDownloadProgress = payload.progress ?? null;
            sttDownloadIndeterminate = payload.indeterminate;
            if (payload.phase === "error") {
                sttDownloadError = normalizeDownloadErrorMessage(
                    payload.message,
                );
            } else if (
                payload.phase === "completed" ||
                payload.phase === "cancelled"
            ) {
                sttDownloadError = null;
            }
            if (payload.phase === "cancelled") {
                isCancellingSttDownload = false;
            }
            return;
        }

        csmDownloadMessage = payload.message;
        csmDownloadProgress = payload.progress ?? null;
        csmDownloadIndeterminate = payload.indeterminate;
        if (payload.phase === "error") {
            csmDownloadError = normalizeDownloadErrorMessage(payload.message);
        } else if (
            payload.phase === "completed" ||
            payload.phase === "cancelled"
        ) {
            csmDownloadError = null;
        }
        if (payload.phase === "cancelled") {
            isCancellingCsmDownload = false;
        }
    }

    function applyRuntimeSetupEvent(payload: RuntimeSetupStatusEvent) {
        runtimeSetupMessage = payload.message.trim();

        if (payload.phase === "error") {
            isPreparingRuntime = false;
            runtimeSetupError = payload.message.trim();
            return;
        }

        if (payload.phase === "completed") {
            isPreparingRuntime = false;
            runtimeSetupError = null;
            return;
        }

        isPreparingRuntime = true;
        runtimeSetupError = null;
    }

    function shouldApplyDownloadEvent(payload: ModelDownloadProgressEvent) {
        if (payload.phase !== "progress") {
            return true;
        }

        if (payload.model === "gemma") {
            return !isCancellingGemmaDownload;
        }

        if (payload.model === "csm") {
            return !isCancellingCsmDownload;
        }

        return !isCancellingSttDownload;
    }

    function stopDownloadStatusPolling() {
        if (downloadStatusPollInterval) {
            clearInterval(downloadStatusPollInterval);
            downloadStatusPollInterval = null;
        }
    }

    async function pollActiveDownloadStatuses() {
        if (!isDownloadingGemma && !isDownloadingCsm && !isDownloadingStt) {
            stopDownloadStatusPolling();
            return;
        }

        try {
            const [gemmaStatus, csmStatus, sttStatus] = await Promise.all([
                isDownloadingGemma
                    ? invoke<ModelDownloadProgressEvent | null>(
                          "get_model_download_status",
                          { model: "gemma" },
                      )
                    : Promise.resolve(null),
                isDownloadingCsm
                    ? invoke<ModelDownloadProgressEvent | null>(
                          "get_model_download_status",
                          { model: "csm" },
                      )
                    : Promise.resolve(null),
                isDownloadingStt
                    ? invoke<ModelDownloadProgressEvent | null>(
                          "get_model_download_status",
                          { model: "stt" },
                      )
                    : Promise.resolve(null),
            ]);

            if (gemmaStatus && shouldApplyDownloadEvent(gemmaStatus)) {
                applyDownloadEvent(gemmaStatus);
            }
            if (csmStatus && shouldApplyDownloadEvent(csmStatus)) {
                applyDownloadEvent(csmStatus);
            }
            if (sttStatus && shouldApplyDownloadEvent(sttStatus)) {
                applyDownloadEvent(sttStatus);
            }
        } catch (err) {
            console.error("Failed to poll download status:", err);
        }
    }

    function ensureDownloadStatusPolling() {
        if (downloadStatusPollInterval) {
            return;
        }

        void pollActiveDownloadStatuses();
        downloadStatusPollInterval = window.setInterval(() => {
            void pollActiveDownloadStatuses();
        }, 1000);
    }

    async function applyCsmQuantizeSelection() {
        await invoke("set_csm_quantize", { enabled: isCsmQuantized });
    }

    async function handleCsmQuantizeToggle() {
        const previousValue = isCsmQuantized;
        isCsmQuantized = !isCsmQuantized;
        isUpdatingCsmQuantize = true;

        try {
            await applyCsmQuantizeSelection();
        } catch (err) {
            isCsmQuantized = previousValue;
            console.error("Failed to update CSM quantize setting:", err);
            alert(`Failed to update CSM quantize setting.\n${String(err)}`);
        } finally {
            isUpdatingCsmQuantize = false;
        }
    }

    async function handleGemmaVariantChange(event: Event) {
        const target = event.currentTarget as HTMLSelectElement;
        const nextVariant = target.value as GemmaVariant;

        const previousVariant = selectedGemmaVariant;

        selectedGemmaVariant = nextVariant;

        if (isExternalGemmaVariant(nextVariant)) {
            void syncExternalModelsForVariant(nextVariant);
        }

        if (
            isExternalGemmaVariant(nextVariant) &&
            selectedSttModel === "gemma"
        ) {
            try {
                selectedSttModel = DEFAULT_STT_MODEL;
                await setSttModelSelection(DEFAULT_STT_MODEL);
            } catch (err) {
                console.error("Failed to auto-switch STT model:", err);
            }
        }

        try {
            await setGemmaVariantSelection(nextVariant);
            await syncModelStatus();
        } catch (err) {
            selectedGemmaVariant = previousVariant;
            console.error("Failed to update Gemma variant:", err);
            alert(`Failed to update Gemma variant.\n${String(err)}`);
        }
    }

    async function handleCsmModelChange(event: Event) {
        const target = event.currentTarget as HTMLSelectElement;
        const nextVariant = target.value as CsmModelVariant;
        const previousVariant = selectedCsmModel;

        selectedCsmModel = nextVariant;

        try {
            await setCsmModelSelection(nextVariant);
            await syncModelStatus();
            queueSelectedContactVoiceReferenceSync();
        } catch (err) {
            selectedCsmModel = previousVariant;
            console.error("Failed to update speech model:", err);
            alert(`Failed to update the speech model.\n${String(err)}`);
        }
    }

    async function handleSttModelChange(event: Event) {
        const target = event.currentTarget as HTMLSelectElement;
        const nextVariant = target.value as SttModelVariant;
        const previousVariant = selectedSttModel;

        selectedSttModel = nextVariant;

        try {
            await setSttModelSelection(nextVariant);
            await syncModelStatus();
        } catch (err) {
            selectedSttModel = previousVariant;
            console.error("Failed to update STT model:", err);
            alert(`Failed to update the STT model.\n${String(err)}`);
        }
    }

    async function handleModelPresetChange(event: Event) {
        const target = event.currentTarget as HTMLSelectElement;
        const nextPreset = target.value as ModelPreset;

        if (nextPreset === "custom" || pendingModelPreset) {
            return;
        }

        const nextSelection = MODEL_PRESETS[nextPreset].selection;
        const previousSelection = getCurrentModelSelection();
        pendingModelPreset = nextPreset;

        try {
            await syncModelStatus();

            const currentSelection = getCurrentModelSelection();
            const shouldUnloadStt =
                isSttLoaded &&
                (currentSelection.sttModel !== nextSelection.sttModel ||
                    nextSelection.sttModel === "gemma");
            const shouldUnloadCsm =
                isCsmLoaded &&
                currentSelection.csmModel !== nextSelection.csmModel;
            const shouldUnloadGemma =
                isGemmaLoaded &&
                currentSelection.gemmaVariant !== nextSelection.gemmaVariant;

            if (shouldUnloadStt) {
                await handleUnloadStt({ suppressAlert: true });
            }

            if (shouldUnloadCsm) {
                await handleUnloadCsm({ suppressAlert: true });
            }

            if (shouldUnloadGemma) {
                await handleUnloadGemma({ suppressAlert: true });
            }

            if (currentSelection.gemmaVariant !== nextSelection.gemmaVariant) {
                selectedGemmaVariant = nextSelection.gemmaVariant;
                await setGemmaVariantSelection(nextSelection.gemmaVariant);
            }

            if (currentSelection.csmModel !== nextSelection.csmModel) {
                selectedCsmModel = nextSelection.csmModel;
                await setCsmModelSelection(nextSelection.csmModel);
            }

            if (currentSelection.sttModel !== nextSelection.sttModel) {
                selectedSttModel = nextSelection.sttModel;
                await setSttModelSelection(nextSelection.sttModel);
            }

            if (currentSelection.ollamaModel !== nextSelection.ollamaModel) {
                selectedOllamaModel = nextSelection.ollamaModel;
                await invoke("set_ollama_model", {
                    model: nextSelection.ollamaModel,
                });
            }

            if (
                currentSelection.lmstudioModel !== nextSelection.lmstudioModel
            ) {
                selectedLmStudioModel = nextSelection.lmstudioModel;
                await invoke("set_lmstudio_model", {
                    model: nextSelection.lmstudioModel,
                });
            }

            if (
                currentSelection.openaiCompatibleModel !==
                nextSelection.openaiCompatibleModel
            ) {
                selectedOpenAiCompatibleModel =
                    nextSelection.openaiCompatibleModel ??
                    DEFAULT_OPENAI_COMPATIBLE_MODEL;
                await invoke("set_openai_compatible_model", {
                    model: selectedOpenAiCompatibleModel,
                });
            }
        } catch (err) {
            selectedGemmaVariant = previousSelection.gemmaVariant;
            selectedCsmModel = previousSelection.csmModel;
            selectedSttModel = previousSelection.sttModel;
            selectedOllamaModel =
                previousSelection.ollamaModel ?? DEFAULT_OLLAMA_MODEL;
            selectedLmStudioModel =
                previousSelection.lmstudioModel ?? DEFAULT_LMSTUDIO_MODEL;
            selectedOpenAiCompatibleModel =
                previousSelection.openaiCompatibleModel ??
                DEFAULT_OPENAI_COMPATIBLE_MODEL;
            console.error("Failed to apply model preset:", err);
            alert(
                `Failed to apply the ${MODEL_PRESETS[nextPreset].label} preset.\n${normalizeErrorMessage(err)}`,
            );
        } finally {
            pendingModelPreset = null;
            await syncModelStatus();
        }
    }

    async function syncModelStatus() {
        try {
            const [
                gemmaVariant,
                csmModelVariant,
                sttModelVariant,
                gemmaDownloaded,
                ollamaSupported,
                lmstudioSupported,
                openaiCompatibleSupported,
                gemmaLoaded,
                csmDownloaded,
                csmLoaded,
                sttDownloaded,
                sttLoaded,
                csmQuantized,
            ] = await Promise.all([
                invoke<GemmaVariant>("get_gemma_variant"),
                invoke<CsmModelVariant>("get_csm_model_variant"),
                invoke<SttModelVariant>("get_stt_model_variant"),
                invoke<boolean>("check_model_status"),
                invoke<boolean>("check_ollama_status"),
                invoke<boolean>("check_lmstudio_status"),
                invoke<boolean>("check_openai_compatible_status"),
                invoke<boolean>("is_server_running"),
                invoke<boolean>("check_csm_status"),
                invoke<boolean>("is_csm_running"),
                invoke<boolean>("check_stt_status"),
                invoke<boolean>("is_stt_running"),
                invoke<boolean>("get_csm_quantize"),
            ]);

            selectedGemmaVariant = gemmaVariant;
            selectedCsmModel = csmModelVariant;
            selectedSttModel = sttModelVariant;
            isGemmaDownloaded = gemmaDownloaded;
            const previousOllamaSupported = isOllamaSupported;
            const previousLmStudioSupported = isLmStudioSupported;
            const previousOpenAiCompatibleSupported =
                isOpenAiCompatibleSupported;
            isOllamaSupported = ollamaSupported;
            isLmStudioSupported = lmstudioSupported;
            isOpenAiCompatibleSupported = openaiCompatibleSupported;
            isGemmaLoaded = gemmaLoaded;

            if (
                ollamaSupported &&
                (!previousOllamaSupported || ollamaModels.length === 0)
            ) {
                void syncOllamaModels();
            } else if (!ollamaSupported) {
                ollamaModels = [];
            }

            if (
                lmstudioSupported &&
                (!previousLmStudioSupported || lmStudioModels.length === 0)
            ) {
                void syncLmStudioModels();
            } else if (!lmstudioSupported) {
                lmStudioModels = [];
            }

            if (
                openaiCompatibleSupported &&
                (!previousOpenAiCompatibleSupported ||
                    openAiCompatibleModels.length === 0)
            ) {
                void syncOpenAiCompatibleModels();
            } else if (!openaiCompatibleSupported) {
                openAiCompatibleModels = [];
            }

            isCsmDownloaded = csmDownloaded;
            isCsmLoaded = csmLoaded;
            isSttDownloaded = sttDownloaded;
            isSttLoaded = sttLoaded;
            isCsmQuantized = csmQuantized;
            persistModelPreferences();

            if (gemmaLoaded || csmLoaded || sttLoaded) {
                ensureModelMemoryPolling();
                await syncModelMemoryUsage();
            } else {
                modelMemorySnapshot = null;
                stopModelMemoryPolling();
            }
        } catch (err) {
            console.error("Failed to sync model status:", err);
        }
    }

    async function syncOllamaModels() {
        if (!isOllamaSupported) {
            ollamaModels = [];
            return;
        }
        try {
            const models = await invoke<string[]>("get_ollama_models");
            ollamaModels = models;
            const current = await invoke<string>("get_ollama_model");

            if (selectedOllamaModel === "") {
                selectedOllamaModel = current;
            }

            if (
                ollamaModels.length > 0 &&
                selectedOllamaModel !== "" &&
                !ollamaModels.includes(selectedOllamaModel)
            ) {
                // If the selected model is not in the list, but there's a match without tag, use it
                const matchWithoutTag = ollamaModels.find(
                    (m) =>
                        m.split(":")[0] === selectedOllamaModel.split(":")[0],
                );
                if (matchWithoutTag) {
                    selectedOllamaModel = matchWithoutTag;
                    await invoke("set_ollama_model", {
                        model: selectedOllamaModel,
                    });
                }
            } else if (ollamaModels.length > 0 && selectedOllamaModel === "") {
                selectedOllamaModel = ollamaModels[0];
                await invoke("set_ollama_model", {
                    model: selectedOllamaModel,
                });
            }
        } catch (err) {
            console.error("Failed to fetch Ollama models:", err);
        }
    }

    async function handleOllamaModelChange(event: Event) {
        const target = event.currentTarget as HTMLSelectElement;
        const nextModel = target.value;
        selectedOllamaModel = nextModel;
        await invoke("set_ollama_model", { model: nextModel });
        persistModelPreferences();
    }

    async function syncLmStudioModels() {
        if (!isLmStudioSupported) {
            lmStudioModels = [];
            return;
        }
        try {
            const models = await invoke<string[]>("get_lmstudio_models");
            lmStudioModels = models;
            const current = await invoke<string>("get_lmstudio_model");

            if (selectedLmStudioModel === "") {
                selectedLmStudioModel = current;
            }

            if (
                lmStudioModels.length > 0 &&
                selectedLmStudioModel !== "" &&
                !lmStudioModels.includes(selectedLmStudioModel)
            ) {
                selectedLmStudioModel = lmStudioModels[0];
                await invoke("set_lmstudio_model", {
                    model: selectedLmStudioModel,
                });
            } else if (
                lmStudioModels.length > 0 &&
                selectedLmStudioModel === ""
            ) {
                selectedLmStudioModel = lmStudioModels[0];
                await invoke("set_lmstudio_model", {
                    model: selectedLmStudioModel,
                });
            }
        } catch (err) {
            console.error("Failed to fetch LM Studio models:", err);
        }
    }

    async function handleLmStudioModelChange(event: Event) {
        const target = event.currentTarget as HTMLSelectElement;
        const nextModel = target.value;
        selectedLmStudioModel = nextModel;
        await invoke("set_lmstudio_model", { model: nextModel });
        persistModelPreferences();
    }

    async function syncOpenAiCompatibleModels() {
        if (!isOpenAiCompatibleSupported) {
            openAiCompatibleModels = [];
            return;
        }
        try {
            const models = await invoke<string[]>(
                "get_openai_compatible_models",
            );
            openAiCompatibleModels = models;
            const current = await invoke<string>("get_openai_compatible_model");

            if (selectedOpenAiCompatibleModel === "") {
                selectedOpenAiCompatibleModel = current;
            }

            if (
                openAiCompatibleModels.length > 0 &&
                selectedOpenAiCompatibleModel !== "" &&
                !openAiCompatibleModels.includes(selectedOpenAiCompatibleModel)
            ) {
                selectedOpenAiCompatibleModel = openAiCompatibleModels[0];
                await invoke("set_openai_compatible_model", {
                    model: selectedOpenAiCompatibleModel,
                });
            } else if (
                openAiCompatibleModels.length > 0 &&
                selectedOpenAiCompatibleModel === ""
            ) {
                selectedOpenAiCompatibleModel = openAiCompatibleModels[0];
                await invoke("set_openai_compatible_model", {
                    model: selectedOpenAiCompatibleModel,
                });
            }
        } catch (err) {
            console.error("Failed to fetch OpenAI-compatible models:", err);
        }
    }

    async function handleOpenAiCompatibleModelChange(event: Event) {
        const target = event.currentTarget as HTMLSelectElement;
        const nextModel = target.value;
        selectedOpenAiCompatibleModel = nextModel;
        await invoke("set_openai_compatible_model", { model: nextModel });
        persistModelPreferences();
    }

    async function syncOllamaConfig() {
        try {
            const { baseUrl, hasApiKey } =
                await invoke<ProviderConfig>("get_ollama_config");
            ollamaBaseUrl = baseUrl;
            ollamaHasApiKey = hasApiKey;
        } catch (err) {
            console.error("Failed to sync Ollama config:", err);
        }
    }

    async function syncLmStudioConfig() {
        try {
            const { baseUrl, hasApiKey } =
                await invoke<ProviderConfig>("get_lmstudio_config");
            lmstudioBaseUrl = baseUrl;
            lmstudioHasApiKey = hasApiKey;
        } catch (err) {
            console.error("Failed to sync LM Studio config:", err);
        }
    }

    async function syncOpenAiCompatibleConfig() {
        try {
            const { baseUrl, hasApiKey } = await invoke<ProviderConfig>(
                "get_openai_compatible_config",
            );
            openAiCompatibleBaseUrl = baseUrl;
            openAiCompatibleHasApiKey = hasApiKey;
        } catch (err) {
            console.error("Failed to sync OpenAI-compatible config:", err);
        }
    }

    async function syncSubtitleTranslationLlmConfig() {
        try {
            const { baseUrl, hasApiKey, modelId } =
                await invoke<SubtitleTranslationLlmConfig>(
                    "get_subtitle_translation_llm_config",
                );
            subtitleTranslationBaseUrl = baseUrl;
            subtitleTranslationHasApiKey = hasApiKey;
            subtitleTranslationModelId = modelId;
        } catch (err) {
            console.error(
                "Failed to sync subtitle translation LLM config:",
                err,
            );
        }
    }

    async function saveExternalLlmConfig(
        url: string,
        key: string,
        clearKey: boolean,
    ) {
        const normalizedUrl = url.trim();
        const normalizedKey = key.trim();
        const nextHasApiKey = clearKey
            ? false
            : normalizedKey !== ""
              ? true
              : getExternalProviderHasApiKey(selectedGemmaVariant);

        try {
            if (selectedGemmaVariant === "lmstudio") {
                await invoke("set_lmstudio_config", {
                    url: normalizedUrl,
                    key: normalizedKey || null,
                    clearKey,
                });
                lmstudioBaseUrl = normalizedUrl;
                lmstudioHasApiKey = nextHasApiKey;
            } else if (selectedGemmaVariant === "openai_compatible") {
                await invoke("set_openai_compatible_config", {
                    url: normalizedUrl,
                    key: normalizedKey || null,
                    clearKey,
                });
                openAiCompatibleBaseUrl = normalizedUrl;
                openAiCompatibleHasApiKey = nextHasApiKey;
            } else {
                await invoke("set_ollama_config", {
                    url: normalizedUrl,
                    key: normalizedKey || null,
                    clearKey,
                });
                ollamaBaseUrl = normalizedUrl;
                ollamaHasApiKey = nextHasApiKey;
            }

            await syncModelStatus();
            if (isExternalGemmaVariant(selectedGemmaVariant)) {
                await syncExternalModelsForVariant(selectedGemmaVariant);
            }
        } catch (err) {
            console.error("Failed to save external LLM config:", err);
            throw err;
        }
    }

    async function testSubtitleTranslationConnection(
        url: string,
        key: string,
        useSavedKey: boolean,
    ) {
        return await invoke<string[]>("test_subtitle_translation_connection", {
            url,
            key: key || null,
            useSavedKey,
        });
    }

    async function saveSubtitleTranslationLlmConfig(
        url: string,
        key: string,
        clearKey: boolean,
        modelId: string,
    ) {
        try {
            await invoke("set_subtitle_translation_llm_config", {
                url,
                key: key || null,
                clearKey,
                modelId,
            });
            await syncSubtitleTranslationLlmConfig();
        } catch (err) {
            console.error(
                "Failed to save subtitle translation LLM config:",
                err,
            );
            throw err;
        }
    }

    function openExternalConfig() {
        showExternalLlmConfig = true;
    }

    function closeExternalConfig() {
        showExternalLlmConfig = false;
    }

    function openSubtitleTranslationLlmConfig() {
        showSubtitleTranslationLlmConfig = true;
    }

    function closeSubtitleTranslationLlmConfig() {
        showSubtitleTranslationLlmConfig = false;
    }

    async function ensureRuntimeDependencies() {
        isPreparingRuntime = true;
        runtimeSetupError = null;
        runtimeSetupMessage =
            "Preparing local Python runtime. This can take several minutes on first launch.";

        try {
            await invoke("ensure_runtime_dependencies");
            runtimeSetupMessage = "Local Python runtime is ready.";
            return true;
        } catch (err) {
            const message = normalizeErrorMessage(err);
            runtimeSetupError = message;
            runtimeSetupMessage = message;
            console.error("Failed to prepare the local runtime:", err);
            return false;
        } finally {
            isPreparingRuntime = false;
        }
    }

    async function handleRetryRuntimeSetup() {
        const succeeded = await ensureRuntimeDependencies();
        if (succeeded) {
            await syncModelStatus();
        }
    }

    async function startAudioCapture() {
        console.log("Starting audio capture...");

        try {
            mediaStream = await navigator.mediaDevices.getUserMedia({
                audio: {
                    echoCancellation: true,
                    noiseSuppression: true,
                },
            });
            captureContext = new AudioContext();

            await captureContext.audioWorklet.addModule("/audio-processor.js");
            await captureContext.audioWorklet.addModule(
                "/playback-processor.js",
            );

            captureSource = captureContext.createMediaStreamSource(mediaStream);
            captureProcessor = new AudioWorkletNode(
                captureContext,
                "audio-processor",
            );
            playbackProcessor = new AudioWorkletNode(
                captureContext,
                "playback-processor",
            );
            silentCaptureSink = captureContext.createGain();
            silentCaptureSink.gain.value = 0;
            syncCaptureMutedState(micMuted);
            syncEndOfUtteranceSilenceWithCaptureProcessor(
                endOfUtteranceSilenceMs,
            );

            captureProcessor.port.onmessage = (event) => {
                if (!calling) {
                    userVolume = 0;
                    return;
                }

                const { inputData, playbackReferenceData, playbackActive } =
                    event.data as {
                        inputData: Float32Array;
                        playbackReferenceData?: Float32Array;
                        playbackActive?: boolean;
                    };

                // Calculate RMS volume for voice activity visualization
                let sum = 0;
                for (let i = 0; i < inputData.length; i++) {
                    sum += inputData[i] * inputData[i];
                }
                userVolume = Math.sqrt(sum / (inputData.length || 1));

                void invoke("receive_audio_chunk", {
                    payload: {
                        data: Array.from(inputData),
                        sample_rate: Math.round(
                            captureContext?.sampleRate ?? 0,
                        ),
                        playback_reference:
                            playbackActive && playbackReferenceData
                                ? Array.from(playbackReferenceData)
                                : null,
                        playback_active: Boolean(playbackActive),
                    },
                }).catch((err) => console.error("Invoke error:", err));
            };

            playbackProcessor.port.onmessage = (event) => {
                const { type, requestId } = event.data as {
                    type?: string;
                    requestId?: number;
                };

                if (type === "chunk-started") {
                    if (requestId != null) {
                        const segments = ttsSegmentTextMap.get(requestId);
                        if (segments && segments.length > 0) {
                            const segment = segments.shift();
                            if (segment) {
                                currentSpokenResponse = (
                                    currentSpokenResponse + segment.text
                                ).trim();
                                setCurrentAiSubtitle(
                                    segment.text,
                                    requestId,
                                    segment.index,
                                );
                            }
                        }
                    }
                    return;
                }

                if (type !== "chunk-finished") {
                    return;
                }

                if (requestId === activeTtsRequestId) {
                    queuedPlaybackChunkCount = Math.max(
                        0,
                        queuedPlaybackChunkCount - 1,
                    );
                    if (queuedPlaybackChunkCount === 0 && calling) {
                        playbackIdleTimeout = window.setTimeout(() => {
                            if (queuedPlaybackChunkCount === 0 && calling) {
                                void queueCompletionPongIfReady(
                                    requestId ?? null,
                                ).then((didQueuePong) => {
                                    if (!didQueuePong) {
                                        updateStageAfterPlaybackStateChange();
                                    }
                                });
                            }
                            playbackIdleTimeout = null;
                        }, 450);
                    } else {
                        updateStageAfterPlaybackStateChange();
                    }
                }
            };

            captureSource.connect(captureProcessor);
            captureProcessor.connect(silentCaptureSink);
            silentCaptureSink.connect(captureContext.destination);
            playbackProcessor.connect(captureContext.destination);

            if (captureContext.state === "suspended") {
                await captureContext.resume();
            }
        } catch (err) {
            console.error("Failed to start audio capture:", err);
            alert("Failed to start audio capture: " + String(err));
            calling = false;
            stopCallTimerTracking();
        }
    }

    function stopAudioCapture() {
        if (captureProcessor) {
            captureProcessor.disconnect();
            captureProcessor = null;
        }
        if (playbackProcessor) {
            playbackProcessor.disconnect();
            playbackProcessor = null;
        }
        if (silentCaptureSink) {
            silentCaptureSink.disconnect();
            silentCaptureSink = null;
        }
        if (captureSource) {
            captureSource.disconnect();
            captureSource = null;
        }
        if (captureContext) {
            void captureContext.close();
            captureContext = null;
        }
        userVolume = 0;
        if (mediaStream) {
            mediaStream.getTracks().forEach((track) => track.stop());
            mediaStream = null;
        }

        stopActivePongPlayback();
    }

    function stopPlayback(_closeContext = false) {
        if (playbackIdleTimeout) {
            clearTimeout(playbackIdleTimeout);
            playbackIdleTimeout = null;
        }
        if (playbackProcessor) {
            playbackProcessor.port.postMessage({ type: "stop" });
        }
        queuedPlaybackChunkCount = 0;
        activeTtsRequestId = null;
        ttsSegmentTextMap.clear();
        pendingCompletionPongRequestId = null;
        pendingTtsSegments = 0;
        isQueueingCompletionPong = false;
        setCurrentAiSubtitle("");
        stopActivePongPlayback();
        syncTtsPlaybackState(false);
    }

    function updateStageAfterPlaybackStateChange() {
        syncTtsPlaybackState(calling && queuedPlaybackChunkCount > 0);

        if (!calling) {
            return;
        }

        if (queuedPlaybackChunkCount > 0) {
            setCallStage("speaking", "Speaking");
            return;
        }

        if (pendingTtsSegments > 0) {
            setCallStage("generating_audio", "Generating Audio");
            return;
        }

        setCallStage("listening", "Listening");
    }

    async function handleInterruptTts() {
        if (!assistantSpeaking) {
            return;
        }

        const entryId = activeAssistantConversationEntryId;
        const text = currentSpokenResponse;

        try {
            await invoke("interrupt_tts");
            if (entryId != null && text) {
                void saveConversationLogEntryEdit(entryId, text, false);
            }
        } catch (err) {
            console.error("Failed to interrupt TTS:", err);
        }
    }

    function decodeBase64Bytes(audioBase64: string) {
        const binary = atob(audioBase64);
        const bytes = new Uint8Array(binary.length);

        for (let i = 0; i < binary.length; i += 1) {
            bytes[i] = binary.charCodeAt(i);
        }

        return bytes;
    }

    function mixAudioBufferToMono(audioBuffer: AudioBuffer) {
        if (audioBuffer.numberOfChannels === 0 || audioBuffer.length === 0) {
            return new Float32Array();
        }

        if (audioBuffer.numberOfChannels === 1) {
            return new Float32Array(audioBuffer.getChannelData(0));
        }

        const mono = new Float32Array(audioBuffer.length);
        const mixScale = 1 / audioBuffer.numberOfChannels;

        for (
            let channelIndex = 0;
            channelIndex < audioBuffer.numberOfChannels;
            channelIndex += 1
        ) {
            const channelData = audioBuffer.getChannelData(channelIndex);
            for (
                let sampleIndex = 0;
                sampleIndex < channelData.length;
                sampleIndex += 1
            ) {
                mono[sampleIndex] += channelData[sampleIndex] * mixScale;
            }
        }

        return mono;
    }

    async function queuePlaybackChunk(payload: CsmAudioChunkEvent) {
        if (!calling) {
            return;
        }
        if (activeTtsRequestId !== payload.request_id) {
            return;
        }
        if (!captureContext || !playbackProcessor) {
            return;
        }

        const audioBytes = decodeBase64Bytes(payload.audio_wav_base64);
        if (audioBytes.length === 0) {
            return;
        }

        try {
            const decodedAudio = await captureContext.decodeAudioData(
                audioBytes.slice().buffer,
            );
            if (!calling || activeTtsRequestId !== payload.request_id) {
                return;
            }

            const playbackSamples = mixAudioBufferToMono(decodedAudio);
            if (playbackSamples.length === 0) {
                return;
            }

            if (playbackIdleTimeout) {
                clearTimeout(playbackIdleTimeout);
                playbackIdleTimeout = null;
            }

            queuedPlaybackChunkCount += 1;
            updateStageAfterPlaybackStateChange();
            playbackProcessor.port.postMessage(
                {
                    type: "push",
                    requestId: payload.request_id,
                    samples: playbackSamples,
                    prebufferSamples: PLAYBACK_PREBUFFER_SAMPLES,
                    isNewSegment: payload.is_first_chunk,
                },
                [playbackSamples.buffer],
            );
        } catch (err) {
            console.error("Failed to decode queued CSM audio:", err);
        }
    }

    async function getPongPlaybackSamples() {
        if (!captureContext) {
            return null;
        }

        if (
            cachedPongPlaybackSamples &&
            cachedPongPlaybackSampleRate === captureContext.sampleRate
        ) {
            return cachedPongPlaybackSamples;
        }

        const response = await fetch(pongUrl);
        if (!response.ok) {
            throw new Error(`Failed to fetch pop.mp3 (${response.status})`);
        }

        const audioBytes = await response.arrayBuffer();
        const decodedAudio = await captureContext.decodeAudioData(
            audioBytes.slice(0),
        );
        const playbackSamples = mixAudioBufferToMono(decodedAudio);

        cachedPongPlaybackSamples = playbackSamples;
        cachedPongPlaybackSampleRate = captureContext.sampleRate;
        return playbackSamples;
    }

    function stopActivePongPlayback() {
        if (activePongSource) {
            try {
                activePongSource.stop();
            } catch {
                // Ignore stop errors when the source already finished.
            }
            activePongSource.disconnect();
            activePongSource = null;
        }

        if (activePongGainNode) {
            activePongGainNode.disconnect();
            activePongGainNode = null;
        }
    }

    async function playTrayPong() {
        if (!pongPlaybackEnabled || !captureContext) {
            return false;
        }

        try {
            if (captureContext.state === "suspended") {
                await captureContext.resume();
            }

            const pongSamples = await getPongPlaybackSamples();
            if (!pongSamples || pongSamples.length === 0 || !captureContext) {
                return false;
            }

            stopActivePongPlayback();

            const audioBuffer = captureContext.createBuffer(
                1,
                pongSamples.length,
                captureContext.sampleRate,
            );
            audioBuffer.copyToChannel(pongSamples, 0);

            const source = captureContext.createBufferSource();
            const gainNode = captureContext.createGain();
            gainNode.gain.value = PONG_VOLUME;
            source.buffer = audioBuffer;
            source.connect(gainNode);
            gainNode.connect(captureContext.destination);
            source.onended = () => {
                if (activePongSource === source) {
                    activePongSource = null;
                }
                if (activePongGainNode === gainNode) {
                    activePongGainNode = null;
                }
                source.disconnect();
                gainNode.disconnect();
            };

            activePongSource = source;
            activePongGainNode = gainNode;
            source.start();
            return true;
        } catch (err) {
            console.error("Failed to play tray pong:", err);
            stopActivePongPlayback();
            return false;
        }
    }

    async function playCallStartPong() {
        if (!pongPlaybackEnabled || !calling || !captureContext) {
            return false;
        }

        try {
            if (captureContext.state === "suspended") {
                await captureContext.resume();
            }

            const pongSamples = await getPongPlaybackSamples();
            if (!pongSamples || pongSamples.length === 0 || !captureContext) {
                return false;
            }

            stopActivePongPlayback();

            const audioBuffer = captureContext.createBuffer(
                1,
                pongSamples.length,
                captureContext.sampleRate,
            );
            audioBuffer.copyToChannel(pongSamples, 0);

            const source = captureContext.createBufferSource();
            const gainNode = captureContext.createGain();
            gainNode.gain.value = PONG_VOLUME;
            source.buffer = audioBuffer;
            source.connect(gainNode);
            gainNode.connect(captureContext.destination);
            source.onended = () => {
                if (activePongSource === source) {
                    activePongSource = null;
                }
                if (activePongGainNode === gainNode) {
                    activePongGainNode = null;
                }
                source.disconnect();
                gainNode.disconnect();
            };

            activePongSource = source;
            activePongGainNode = gainNode;
            source.start();
            return true;
        } catch (err) {
            console.error("Failed to play call start pong:", err);
            stopActivePongPlayback();
            return false;
        }
    }

    async function queueCompletionPongIfReady(requestId: number | null) {
        if (
            requestId == null ||
            requestId !== activeTtsRequestId ||
            requestId !== pendingCompletionPongRequestId ||
            !pongPlaybackEnabled ||
            !calling ||
            !captureContext ||
            !playbackProcessor ||
            pendingTtsSegments > 0 ||
            queuedPlaybackChunkCount > 0 ||
            isQueueingCompletionPong
        ) {
            return false;
        }

        isQueueingCompletionPong = true;
        pendingCompletionPongRequestId = null;

        try {
            if (playbackIdleTimeout) {
                clearTimeout(playbackIdleTimeout);
                playbackIdleTimeout = null;
            }

            const pongSamples = await getPongPlaybackSamples();
            if (
                !pongSamples ||
                pongSamples.length === 0 ||
                !calling ||
                requestId !== activeTtsRequestId ||
                !playbackProcessor
            ) {
                return false;
            }

            const playbackSamples = new Float32Array(pongSamples.length);
            for (let index = 0; index < pongSamples.length; index += 1) {
                playbackSamples[index] = pongSamples[index] * PONG_VOLUME;
            }
            queuedPlaybackChunkCount += 1;
            updateStageAfterPlaybackStateChange();
            playbackProcessor.port.postMessage(
                {
                    type: "push",
                    requestId,
                    samples: playbackSamples,
                    prebufferSamples: 0,
                },
                [playbackSamples.buffer],
            );
            return true;
        } catch (err) {
            console.error("Failed to queue completion pong:", err);
            return false;
        } finally {
            isQueueingCompletionPong = false;
        }
    }

    async function handleStartCall() {
        if (!modelsReady) {
            return;
        }

        await syncSelectedContactPrompt();
        await syncSelectedContactVoiceReference();

        try {
            await invoke("start_new_session");
        } catch (err) {
            console.error("Failed to start new session:", err);
            alert("Failed to start new session: " + String(err));
        }

        closeContactsPopup();
        closeConversationPopup();
        showSessionsPopup = false;
        resetConversationLog();
        resetScreenCaptureStatus();
        calling = true;
        callStartedAtMs = Date.now();
        resetProcessingAudioLatencies();
        clearLiveTranscriptSubtitle();
        syncCallElapsedTime();
        activeTtsRequestId = null;
        syncTtsPlaybackState(false);
        setCallStage("listening", "Listening");

        void invoke("start_call_timer", { muted: micMuted }).catch((err) => {
            console.error("Failed to start tray call timer", err);
            alert("Failed to start tray call timer: " + String(err));
        });
        await startAudioCapture();
        if (calling) {
            void playCallStartPong();
        }
        void invoke("ping").catch((err) => {
            console.error("Backend ping failed", err);
            alert("Backend ping failed: " + String(err));
        });

        if (callTimerInterval) {
            clearInterval(callTimerInterval);
        }

        callTimerInterval = window.setInterval(() => {
            if (!calling || callStartedAtMs == null) {
                if (callTimerInterval) {
                    clearInterval(callTimerInterval);
                    callTimerInterval = null;
                }
                return;
            }
            syncCallElapsedTime();
        }, 1000);
    }

    async function handleResumeCall() {
        if (!modelsReady) {
            return;
        }

        await syncSelectedContactPrompt();
        await syncSelectedContactVoiceReference();

        closeContactsPopup();
        closeConversationPopup();
        showSessionsPopup = false;
        resetScreenCaptureStatus();
        calling = true;
        callStartedAtMs = Date.now();
        resetProcessingAudioLatencies();
        clearLiveTranscriptSubtitle();
        syncCallElapsedTime();
        activeTtsRequestId = null;
        syncTtsPlaybackState(false);
        setCallStage("listening", "Listening");

        void invoke("start_call_timer", { muted: micMuted }).catch((err) => {
            console.error("Failed to start tray call timer", err);
            alert("Failed to start tray call timer: " + String(err));
        });
        await startAudioCapture();
        if (calling) {
            void playCallStartPong();
        }
        void invoke("ping").catch((err) => {
            console.error("Backend ping failed", err);
            alert("Backend ping failed: " + String(err));
        });

        if (callTimerInterval) {
            clearInterval(callTimerInterval);
        }

        callTimerInterval = window.setInterval(() => {
            if (!calling || callStartedAtMs == null) {
                if (callTimerInterval) {
                    clearInterval(callTimerInterval);
                    callTimerInterval = null;
                }
                return;
            }
            syncCallElapsedTime();
        }, 1000);
    }

    async function handleEndCall() {
        calling = false;
        stopPlayback();
        stopAudioCapture();
        closeContactsPopup();
        closeConversationPopup();
        resetConversationLog();
        resetScreenCaptureStatus();
        resetProcessingAudioLatencies();
        clearLiveTranscriptSubtitle();
        setCallStage("idle", "");
        stopCallTimerTracking();

        try {
            await invoke("reset_call_session");
            currentSessionId = await invoke<string | null>(
                "get_current_session_id",
            );
            currentSessionTitle = null;
            await loadSessions();

            if (
                !currentSessionId &&
                selectLastSessionEnabled &&
                sessions.length > 0
            ) {
                void handleSelectSession(sessions[0]);
            }
        } catch (err) {
            console.error("Failed to clear call session:", err);
        }
    }

    function setMicMuted(muted: boolean) {
        if (micMuted === muted) {
            return;
        }

        micMuted = muted;
        syncCaptureMutedState(micMuted);
        void invoke("set_call_muted", { muted: micMuted }).catch((err) =>
            console.error("Failed to sync tray mute state", err),
        );
    }

    function toggleMic() {
        setMicMuted(!micMuted);
    }

    async function ensureOverlayWindow() {
        const existingWindow =
            await WebviewWindow.getByLabel(OVERLAY_WINDOW_LABEL);
        if (existingWindow) {
            return;
        }

        try {
            const overlayWindow = new WebviewWindow(OVERLAY_WINDOW_LABEL, {
                url: OVERLAY_WINDOW_ROUTE,
                title: "",
                width: 960,
                height: 260,
                visible: true,
                resizable: false,
                decorations: false,
                transparent: true,
                focus: false,
                focusable: false,
                alwaysOnTop: true,
                skipTaskbar: true,
                shadow: false,
                visibleOnAllWorkspaces: true,
            });

            void overlayWindow.once("tauri://error", (event) => {
                console.error("Failed to create overlay window:", event);
            });
        } catch (err) {
            console.error("Failed to create overlay window:", err);
        }
    }

    async function handleDownloadGemma() {
        isDownloadingGemma = true;
        isCancellingGemmaDownload = false;
        resetDownloadState("gemma");
        ensureDownloadStatusPolling();
        let shouldAutoLoad = false;
        try {
            await invoke("download_model");
            shouldAutoLoad = true;
        } catch (err) {
            console.error("Download model failed:", err);
            if (!String(err).toLowerCase().includes("cancelled")) {
                const message = normalizeDownloadErrorMessage(err);
                gemmaDownloadError = message;
                gemmaDownloadMessage = message;
                gemmaDownloadProgress = null;
                gemmaDownloadIndeterminate = true;
            }
        } finally {
            isDownloadingGemma = false;
            isCancellingGemmaDownload = false;
            if (!isDownloadingCsm && !isDownloadingStt) {
                stopDownloadStatusPolling();
            }
            await syncModelStatus();
        }

        if (shouldAutoLoad && !isGemmaLoaded) {
            await handleLoadGemma();
        }
    }

    async function handleClearGemmaCache() {
        if (isClearingGemmaCache || isLoadingGemma || isUnloadingGemma) {
            return;
        }

        isClearingGemmaCache = true;
        gemmaDownloadError = null;
        resetDownloadState("gemma");

        try {
            await invoke("clear_model_cache", { model: "gemma" });
        } catch (err) {
            const message = normalizeErrorMessage(err);
            console.error("Failed to clear Gemma cache:", err);
            gemmaDownloadError = message;
            alert(`Failed to clear Gemma cache.\n${message}`);
            return;
        } finally {
            isClearingGemmaCache = false;
            await syncModelStatus();
        }
    }

    async function handleCancelGemmaDownload() {
        if (!isDownloadingGemma) {
            return;
        }

        isCancellingGemmaDownload = true;
        gemmaDownloadMessage = "Cancelling download...";
        gemmaDownloadProgress = null;
        gemmaDownloadIndeterminate = true;

        try {
            await invoke("cancel_model_download", { model: "gemma" });
        } catch (err) {
            isCancellingGemmaDownload = false;
            console.error("Failed to cancel Gemma download:", err);
            alert(`Failed to cancel Gemma download.\n${String(err)}`);
        }
    }

    async function handleLoadGemma() {
        isLoadingGemma = true;
        try {
            if (isExternalGemmaVariant(selectedGemmaVariant)) {
                await syncExternalModelsForVariant(selectedGemmaVariant);
            }
            await invoke("start_server");
            isGemmaLoaded = true;
        } catch (err) {
            console.error("Load model failed:", err);
            alert(`Failed to load LLM.\n${String(err)}`);
        } finally {
            isLoadingGemma = false;
            await syncModelStatus();
        }
    }

    async function handleDownloadCsm() {
        isDownloadingCsm = true;
        isCancellingCsmDownload = false;
        resetDownloadState("csm");
        ensureDownloadStatusPolling();
        let shouldAutoLoad = false;
        try {
            await invoke("download_csm_model");
            shouldAutoLoad = true;
        } catch (err) {
            console.error("Download speech model failed:", err);
            if (!String(err).toLowerCase().includes("cancelled")) {
                const message = normalizeDownloadErrorMessage(err);
                csmDownloadError = message;
                csmDownloadMessage = message;
                csmDownloadProgress = null;
                csmDownloadIndeterminate = true;
            }
        } finally {
            isDownloadingCsm = false;
            isCancellingCsmDownload = false;
            if (!isDownloadingGemma && !isDownloadingStt) {
                stopDownloadStatusPolling();
            }
            await syncModelStatus();
        }

        if (shouldAutoLoad && !isCsmLoaded) {
            await handleLoadCsm();
        }
    }

    async function handleDownloadStt() {
        isDownloadingStt = true;
        isCancellingSttDownload = false;
        resetDownloadState("stt");
        ensureDownloadStatusPolling();
        let shouldAutoLoad = false;
        try {
            await invoke("download_stt_model");
            shouldAutoLoad = true;
        } catch (err) {
            console.error("Download STT model failed:", err);
            if (!String(err).toLowerCase().includes("cancelled")) {
                const message = normalizeDownloadErrorMessage(err);
                sttDownloadError = message;
                sttDownloadMessage = message;
                sttDownloadProgress = null;
                sttDownloadIndeterminate = true;
            }
        } finally {
            isDownloadingStt = false;
            isCancellingSttDownload = false;
            if (!isDownloadingGemma && !isDownloadingCsm) {
                stopDownloadStatusPolling();
            }
            await syncModelStatus();
        }

        if (shouldAutoLoad && !isSttLoaded) {
            await handleLoadStt();
        }
    }

    async function handleCancelCsmDownload() {
        if (!isDownloadingCsm) {
            return;
        }

        isCancellingCsmDownload = true;
        csmDownloadMessage = "Cancelling download...";
        csmDownloadProgress = null;
        csmDownloadIndeterminate = true;

        try {
            await invoke("cancel_model_download", { model: "csm" });
        } catch (err) {
            isCancellingCsmDownload = false;
            console.error("Failed to cancel speech model download:", err);
            alert(
                `Failed to cancel ${selectedCsmModelLabel} download.\n${String(err)}`,
            );
        }
    }

    async function handleCancelSttDownload() {
        if (!isDownloadingStt) {
            return;
        }

        isCancellingSttDownload = true;
        sttDownloadMessage = "Cancelling download...";
        sttDownloadProgress = null;
        sttDownloadIndeterminate = true;

        try {
            await invoke("cancel_model_download", { model: "stt" });
        } catch (err) {
            isCancellingSttDownload = false;
            console.error("Failed to cancel STT model download:", err);
            alert(
                `Failed to cancel ${selectedSttModelLabel} download.\n${String(err)}`,
            );
        }
    }

    async function handleClearCsmCache() {
        if (isClearingCsmCache || isLoadingCsm || isUnloadingCsm) {
            return;
        }

        isClearingCsmCache = true;
        csmDownloadError = null;
        resetDownloadState("csm");

        try {
            await invoke("clear_model_cache", { model: "csm" });
        } catch (err) {
            const message = normalizeErrorMessage(err);
            console.error("Failed to clear speech model cache:", err);
            csmDownloadError = message;
            alert(
                `Failed to clear ${selectedCsmModelLabel} cache.\n${message}`,
            );
            return;
        } finally {
            isClearingCsmCache = false;
            await syncModelStatus();
        }
    }

    async function handleClearSttCache() {
        if (isClearingSttCache || isLoadingStt || isUnloadingStt) {
            return;
        }

        isClearingSttCache = true;
        sttDownloadError = null;
        resetDownloadState("stt");

        try {
            await invoke("clear_model_cache", { model: "stt" });
        } catch (err) {
            const message = normalizeErrorMessage(err);
            console.error("Failed to clear STT model cache:", err);
            sttDownloadError = message;
            alert(
                `Failed to clear ${selectedSttModelLabel} cache.\n${message}`,
            );
            return;
        } finally {
            isClearingSttCache = false;
            await syncModelStatus();
        }
    }

    async function handleLoadCsm() {
        isLoadingCsm = true;
        csmLoadMessage = "Starting worker...";
        try {
            if (selectedCsmModel === "expressiva_1b") {
                await applyCsmQuantizeSelection();
            }
            await invoke("start_csm_server");
            isCsmLoaded = true;
        } catch (err) {
            console.error("Load speech model failed:", err);
            alert(`Failed to load ${selectedCsmModelLabel}.\n${String(err)}`);
        } finally {
            isLoadingCsm = false;
            await syncModelStatus();
        }
    }

    async function handleLoadStt() {
        isLoadingStt = true;
        sttLoadMessage = "Starting worker...";
        try {
            await invoke("start_stt_server");
            isSttLoaded = true;
        } catch (err) {
            console.error("Load STT model failed:", err);
            alert(`Failed to load ${selectedSttModelLabel}.\n${String(err)}`);
        } finally {
            isLoadingStt = false;
            await syncModelStatus();
        }
    }

    async function handleLoadAll() {
        if (isLoadingAll) {
            return;
        }

        isLoadingAll = true;

        try {
            await syncModelStatus();

            const sttNeeded = selectedSttModel !== "gemma";
            const allDownloaded =
                isGemmaDownloaded &&
                isCsmDownloaded &&
                (!sttNeeded || isSttDownloaded);

            if (allDownloaded) {
                await Promise.all([
                    !isGemmaLoaded ? handleLoadGemma() : Promise.resolve(),
                    sttNeeded && !isSttLoaded
                        ? handleLoadStt()
                        : Promise.resolve(),
                    !isCsmLoaded ? handleLoadCsm() : Promise.resolve(),
                ]);
            } else {
                if (!isGemmaDownloaded) {
                    await handleDownloadGemma();
                    if (!isGemmaDownloaded || !isGemmaLoaded) {
                        return;
                    }
                } else if (!isGemmaLoaded) {
                    await handleLoadGemma();
                    if (!isGemmaLoaded) {
                        return;
                    }
                }

                if (selectedSttModel !== "gemma") {
                    if (!isSttDownloaded) {
                        await handleDownloadStt();
                        if (!isSttDownloaded || !isSttLoaded) {
                            return;
                        }
                    } else if (!isSttLoaded) {
                        await handleLoadStt();
                        if (!isSttLoaded) {
                            return;
                        }
                    }
                }

                if (!isCsmDownloaded) {
                    await handleDownloadCsm();
                    if (!isCsmDownloaded || !isCsmLoaded) {
                        return;
                    }
                } else if (!isCsmLoaded) {
                    await handleLoadCsm();
                    if (!isCsmLoaded) {
                        return;
                    }
                }
            }
        } finally {
            isLoadingAll = false;
            await syncModelStatus();
        }
    }

    async function handleUnloadAll() {
        if (isLoadingAll) {
            return;
        }

        isLoadingAll = true;

        try {
            await handleUnloadGemma({ suppressAlert: true });
            await handleUnloadCsm({ suppressAlert: true });
            await handleUnloadStt({ suppressAlert: true });
        } finally {
            isLoadingAll = false;
            await syncModelStatus();
        }
    }

    async function handleUnloadGemma(options?: { suppressAlert?: boolean }) {
        isUnloadingGemma = true;
        try {
            await invoke("stop_server");
            isGemmaLoaded = false;
        } catch (err) {
            console.error("Unload Gemma failed:", err);
            if (!options?.suppressAlert) {
                alert(`Failed to unload Gemma.\n${String(err)}`);
            } else {
                throw err;
            }
        } finally {
            isUnloadingGemma = false;
            await syncModelStatus();
        }
    }

    async function handleUnloadCsm(options?: { suppressAlert?: boolean }) {
        try {
            isUnloadingCsm = true;
            csmNotificationMessage = null;
            await invoke("stop_csm_server");
            isCsmLoaded = false;
        } catch (err) {
            console.error("Unload speech model failed:", err);
            if (!options?.suppressAlert) {
                alert(
                    `Failed to unload ${selectedCsmModelLabel}.\n${String(err)}`,
                );
            } else {
                throw err;
            }
        } finally {
            isUnloadingCsm = false;
            await syncModelStatus();
        }
    }

    async function handleUnloadStt(options?: { suppressAlert?: boolean }) {
        isUnloadingStt = true;
        try {
            await invoke("stop_stt_server");
            isSttLoaded = false;
        } catch (err) {
            console.error("Unload STT model failed:", err);
            if (!options?.suppressAlert) {
                alert(
                    `Failed to unload ${selectedSttModelLabel}.\n${String(err)}`,
                );
            } else {
                throw err;
            }
        } finally {
            isUnloadingStt = false;
            await syncModelStatus();
        }
    }

    onMount(() => {
        window.addEventListener("error", (event) => {
            alert(
                `JS Error: ${event.message}\nAt: ${event.filename}:${event.lineno}`,
            );
        });
        window.addEventListener("unhandledrejection", (event) => {
            alert(`Unhandled Promise Rejection: ${String(event.reason)}`);
        });

        void (async () => {
            await ensureOverlayWindow();

            try {
                eventUnlisteners = await Promise.all([
                    listen<RuntimeSetupStatusEvent>(
                        "runtime-setup-status",
                        ({ payload }) => {
                            applyRuntimeSetupEvent(payload);
                        },
                    ),
                    listen<CsmAudioStartEvent>(
                        "csm-audio-start",
                        ({ payload }) => {
                            if (!calling) {
                                return;
                            }

                            const shouldPreserveSpokenResponse =
                                payload.append_to_assistant_entry_id != null &&
                                payload.append_to_assistant_entry_id ===
                                    activeAssistantConversationEntryId;
                            if (payload.request_id !== activeTtsRequestId) {
                                stopPlayback();
                                activeTtsRequestId = payload.request_id;
                                if (!shouldPreserveSpokenResponse) {
                                    currentSpokenResponse = "";
                                }
                            }
                            pendingCompletionPongRequestId = payload.request_id;
                        },
                    ),
                    listen<CsmAudioQueuedEvent>(
                        "csm-audio-queued",
                        ({ payload }) => {
                            if (
                                !calling ||
                                payload.request_id !== activeTtsRequestId
                            ) {
                                return;
                            }

                            pendingTtsSegments += 1;
                            const segments =
                                ttsSegmentTextMap.get(payload.request_id) ?? [];
                            segments.push({
                                text: payload.text,
                                index: payload.index,
                            });
                            ttsSegmentTextMap.set(payload.request_id, segments);
                            updateStageAfterPlaybackStateChange();
                        },
                    ),
                    listen<AssistantTranslationsEvent>(
                        "assistant-translations",
                        ({ payload }) => {
                            let requestMap =
                                assistantSegmentTranslationsMap.get(
                                    payload.request_id,
                                );
                            if (!requestMap) {
                                requestMap = new Map();
                                assistantSegmentTranslationsMap.set(
                                    payload.request_id,
                                    requestMap,
                                );
                            }

                            for (const [
                                indexStr,
                                translations,
                            ] of Object.entries(payload.translations)) {
                                requestMap.set(
                                    parseInt(indexStr),
                                    translations,
                                );
                            }
                        },
                    ),
                    listen<CsmAudioChunkEvent>(
                        "csm-audio-chunk",
                        ({ payload }) => {
                            void queuePlaybackChunk(payload);
                        },
                    ),
                    listen<CsmAudioDoneEvent>(
                        "csm-audio-done",
                        ({ payload }) => {
                            if (payload.request_id === activeTtsRequestId) {
                                pendingTtsSegments = Math.max(
                                    0,
                                    pendingTtsSegments - 1,
                                );
                                if (queuedPlaybackChunkCount === 0) {
                                    void queueCompletionPongIfReady(
                                        payload.request_id,
                                    ).then((didQueuePong) => {
                                        if (!didQueuePong) {
                                            updateStageAfterPlaybackStateChange();
                                        }
                                    });
                                }
                                console.log("Finished streaming CSM response");
                            }
                        },
                    ),
                    listen<CsmAudioStopEvent>("csm-audio-stop", () => {
                        if (assistantSpeaking) {
                            const entryId = activeAssistantConversationEntryId;
                            const text = currentSpokenResponse;
                            if (entryId != null && text) {
                                void saveConversationLogEntryEdit(
                                    entryId,
                                    text,
                                    false,
                                );
                            }
                        }
                        pendingCompletionPongRequestId = null;
                        stopPlayback();
                        if (calling) {
                            updateStageAfterPlaybackStateChange();
                        }
                    }),
                    listen<CsmErrorEvent>("csm-error", ({ payload }) => {
                        const message = normalizeErrorMessage(payload.message);
                        console.error("CSM error:", message);
                        void syncModelStatus();
                        if (
                            payload.request_id == null ||
                            payload.request_id === activeTtsRequestId
                        ) {
                            pendingCompletionPongRequestId = null;
                            stopPlayback();
                            if (calling) {
                                updateStageAfterPlaybackStateChange();
                            }
                        }
                        if (calling && payload.request_id == null) {
                            alert(`OpenDuck error.\n${message}`);
                        }
                    }),
                    listen<CsmStatusEvent>("csm-status", ({ payload }) => {
                        if (isLoadingCsm) {
                            csmLoadMessage = payload.message;
                        }
                        if (payload.message.startsWith("WARNING: ")) {
                            csmNotificationMessage = payload.message.replace(
                                "WARNING: ",
                                "",
                            );
                        }
                    }),
                    listen<SttStatusEvent>("stt-status", ({ payload }) => {
                        if (isLoadingStt) {
                            sttLoadMessage = payload.message;
                        }
                    }),
                    listen<AssistantResponseEvent>(
                        "assistant-response",
                        ({ payload }) => {
                            if (!calling) {
                                return;
                            }

                            if (lastReasoningRequestId !== payload.request_id) {
                                lastReasoningRequestId = payload.request_id;
                                reasoningText = payload.reasoning_text;
                            } else if (
                                payload.reasoning_text.length >
                                reasoningText.length
                            ) {
                                reasoningText = payload.reasoning_text;
                            }

                            upsertAssistantConversationLogEntry(
                                payload.request_id,
                                payload.text,
                                payload.append_to_assistant_entry_id ?? null,
                                payload.translations,
                            );
                        },
                    ),
                    listen<ConversationContextCommittedEvent>(
                        "conversation-context-committed",
                        ({ payload }) => {
                            if (
                                !calling ||
                                payload.requestId !== activeAssistantResponseId
                            ) {
                                return;
                            }

                            markConversationTurnAsContextBacked(payload);
                        },
                    ),
                    listen<ConversationImageHistoryClearedEvent>(
                        "conversation-image-history-cleared",
                        () => {
                            clearConversationLogImageHistoryEntries();
                        },
                    ),
                    listen<CallStageEvent>("call-stage", ({ payload }) => {
                        if (!calling) {
                            return;
                        }

                        if (
                            payload.phase === "processing_audio" ||
                            payload.phase === "thinking" ||
                            payload.phase === "generating_audio"
                        ) {
                            setCallStage(payload.phase, payload.message);
                        }
                    }),
                    listen<ProcessingAudioLatencyEvent>(
                        "processing-audio-latency",
                        ({ payload }) => {
                            if (!calling) {
                                return;
                            }

                            if (payload.kind === "audio") {
                                processingAudioToAudioLatencyMs =
                                    payload.latency_ms;
                                return;
                            }

                            if (payload.kind === "first_message_chunk") {
                                processingAudioToLlmLatencyMs =
                                    payload.latency_ms;
                                return;
                            }

                            processingAudioLatencyMs = payload.latency_ms;
                        },
                    ),
                    listen<TranscriptPartialEvent>(
                        "transcript-partial",
                        ({ payload }) => {
                            if (!calling) {
                                return;
                            }

                            const nextText = payload.text.trim();
                            if (!nextText) {
                                return;
                            }

                            updateLiveTranscriptSubtitle(nextText);
                            if (pendingConversationUserLogEntryId == null) {
                                pendingConversationUserLogEntryId =
                                    appendConversationLogEntry(
                                        "user",
                                        nextText,
                                        [],
                                    );
                                return;
                            }

                            const entryId = pendingConversationUserLogEntryId;
                            if (
                                !conversationLogEntries.some(
                                    (entry) => entry.id === entryId,
                                )
                            ) {
                                pendingConversationUserLogEntryId =
                                    appendConversationLogEntry(
                                        "user",
                                        nextText,
                                        [],
                                    );
                                return;
                            }

                            conversationLogEntries = conversationLogEntries.map(
                                (entry) =>
                                    entry.id === entryId
                                        ? { ...entry, text: nextText }
                                        : entry,
                            );
                            scrollConversationLogToBottom();
                        },
                    ),
                    listen<TranscriptEvent>(
                        "transcript-ready",
                        ({ payload }) => {
                            if (!calling) {
                                return;
                            }

                            // Use data URLs if available for immediate display,
                            // as they are more reliable during file move operations.
                            let imageUrls = [...payload.imageDataUrls];
                            if (
                                imageUrls.length === 0 &&
                                payload.imagePaths.length > 0
                            ) {
                                for (const path of payload.imagePaths) {
                                    imageUrls.push(convertFileSrc(path));
                                }
                            }

                            const nextText = payload.text.trim();
                            if (!nextText && imageUrls.length === 0) {
                                return;
                            }

                            if (nextText) {
                                updateLiveTranscriptSubtitle(nextText);
                            }
                            if (pendingConversationUserLogEntryId == null) {
                                pendingConversationUserLogEntryId =
                                    appendConversationLogEntry(
                                        "user",
                                        nextText,
                                        imageUrls,
                                    );
                                return;
                            }

                            const entryId = pendingConversationUserLogEntryId;
                            if (
                                !conversationLogEntries.some(
                                    (entry) => entry.id === entryId,
                                )
                            ) {
                                pendingConversationUserLogEntryId =
                                    appendConversationLogEntry(
                                        "user",
                                        nextText,
                                        imageUrls,
                                    );
                                return;
                            }

                            conversationLogEntries = conversationLogEntries.map(
                                (entry) =>
                                    entry.id === entryId
                                        ? {
                                              ...entry,
                                              text: nextText,
                                              imageUrls: imageUrls,
                                          }
                                        : entry,
                            );
                            scrollConversationLogToBottom();
                        },
                    ),
                    listen<ModelDownloadProgressEvent>(
                        "model-download-progress",
                        ({ payload }) => {
                            if (shouldApplyDownloadEvent(payload)) {
                                applyDownloadEvent(payload);
                            }
                        },
                    ),
                    listen<ScreenCaptureEvent>(
                        "screen-capture",
                        ({ payload }) => {
                            applyScreenCaptureEvent(payload);
                        },
                    ),
                    listen<TrayEndCallEvent>("tray-end-call", () => {
                        if (calling) {
                            void handleEndCall();
                        }
                    }),
                    listen<TrayToggleMuteEvent>("tray-toggle-mute", () => {
                        if (calling) {
                            toggleMic();
                        }
                    }),
                    listen<TrayPongPlaybackEvent>(
                        "tray-pong-playback",
                        ({ payload }) => {
                            applyPongPlaybackPreference(payload.enabled);
                        },
                    ),
                    listen<ShowAboutModalEvent>("show-about-modal", () => {
                        openAboutPopup();
                    }),
                    listen<TriggerAppUpdateCheckEvent>(
                        "trigger-app-update-check",
                        () => {
                            openAboutPopup({ checkForUpdates: true });
                        },
                    ),
                    listen<OpenDuckContactImportEvent>(
                        "openduck-contact-import",
                        ({ payload }) => {
                            void handleOpenDuckContactImportEvent(payload);
                        },
                    ),
                    listen("play-tray-pong", () => {
                        void playTrayPong();
                    }),
                ]);

                window.addEventListener("rename-session", (async (
                    e: CustomEvent<string>,
                ) => {
                    try {
                        await invoke("update_current_session_title", {
                            title: e.detail,
                        });
                        currentSessionTitle = e.detail;
                        await loadSessions();
                    } catch (err) {
                        console.error(
                            "Failed to update current session title:",
                            err,
                        );
                    }
                }) as any);
            } catch (err) {
                console.error("Failed to register Tauri event listeners:", err);
            }

            await loadBuildInfo();
            await initializeAppUpdatePreference();
            const restoredContacts = await loadContactsFromStorage();
            contacts = restoredContacts.contacts;
            selectedContactId = restoredContacts.selectedContactId;
            persistContactsMetadata();
            await initializeOpenDuckContactImports();
            await syncSelectedContactPrompt();
            await syncSelectedContactVoiceReference();
            await restoreModelPreferences();
            await syncOllamaConfig();
            await syncLmStudioConfig();
            await syncOpenAiCompatibleConfig();
            await syncSubtitleTranslationLlmConfig();
            await initializePongPlaybackPreference();
            await initializeAutoUnmuteOnPastedScreenshotPreference();
            await initializeSelectLastSessionPreference();
            await initializeAutoLoadModelsOnStartupPreference();
            await initializeShowStatPreference();
            await initializeShowSubtitlePreference();
            await initializeShowAiSubtitlePreference();
            await initializeAiSubtitleTargetLanguagePreference();
            await initializeShowCallTimerPreference();
            await initializeShowHiddenWindowOverlayPreference();
            await initializeEndOfUtteranceSilencePreference();
            await initializeAutoContinueSilencePreference();
            await initializeAutoContinueMaxCountPreference();
            await initializeLlmContextTurnLimitPreference();
            await initializeLlmImageHistoryLimitPreference();
            await initializeGlobalShortcutPreference();
            await initializeGlobalShortcutEntireScreenPreference();
            await initializeGlobalShortcutToggleMutePreference();
            await initializeGlobalShortcutInterruptPreference();

            if (navigator.onLine) {
                void triggerAutomaticAppUpdateCheck();
            }

            await loadSessions();
            if (
                !currentSessionId &&
                selectLastSessionEnabled &&
                sessions.length > 0
            ) {
                void handleSelectSession(sessions[0]);
            }
            await ensureRuntimeDependencies();
            await syncModelStatus();
            await syncOllamaModels();
            await syncLmStudioModels();
            await syncOpenAiCompatibleModels();

            if (autoLoadModelsOnStartupEnabled) {
                const sttNeeded = selectedSttModel !== "gemma";
                const allDownloaded =
                    isGemmaDownloaded &&
                    isCsmDownloaded &&
                    (!sttNeeded || isSttDownloaded);
                if (allDownloaded) {
                    void handleLoadAll();
                }
            }
        })();

        healthCheckInterval = window.setInterval(() => {
            void syncModelStatus();
        }, 5000);

        const handleWindowOnline = () => {
            void triggerAutomaticAppUpdateCheck();
        };

        const handleClickOutside = (event: MouseEvent) => {
            const target = event.target as HTMLElement;
            const isInsideSubtitleTranslationModal =
                !!target.closest(".subtitle-translation-config-modal") ||
                !!target.closest(".subtitle-translation-config-backdrop");

            // If the image preview is open, don't close other popups
            if (previewImageUrl) {
                return;
            }

            if (showUpdatePrompt) {
                return;
            }

            // Check Sessions Popup
            if (
                showSessionsPopup &&
                sessionsPopupEl &&
                !sessionsPopupEl.contains(target)
            ) {
                if (
                    !document
                        .querySelector(".sessions-dropdown-btn")
                        ?.contains(target)
                ) {
                    showSessionsPopup = false;
                }
            }

            // Check Conversation Popup
            if (
                showConversationPopup &&
                conversationPopupEl &&
                !conversationPopupEl.contains(target)
            ) {
                let shouldNotClose = false;
                document
                    .querySelectorAll(".conversation-log-btn, .mute-btn")
                    .forEach((btn) => {
                        if (btn.contains(target)) shouldNotClose = true;
                    });
                if (!shouldNotClose) {
                    showConversationPopup = false;
                }
            }

            // Check Contacts Popup
            if (
                showContactsPopup &&
                contactsPopupEl &&
                !contactsPopupEl.contains(target)
            ) {
                if (
                    !document.querySelector(".contacts-btn")?.contains(target)
                ) {
                    closeContactsPopup();
                }
            }

            // Check About Popup
            if (
                showAboutPopup &&
                aboutPopupEl &&
                !aboutPopupEl.contains(target) &&
                !isInsideSubtitleTranslationModal
            ) {
                if (!document.querySelector(".about-btn")?.contains(target)) {
                    closeAboutPopup();
                }
            }
        };

        window.addEventListener("online", handleWindowOnline);
        window.addEventListener("mousedown", handleClickOutside);

        return () => {
            window.removeEventListener("online", handleWindowOnline);
            window.removeEventListener("mousedown", handleClickOutside);
        };
    });

    onDestroy(() => {
        if (healthCheckInterval) {
            clearInterval(healthCheckInterval);
        }
        clearLiveTranscriptSubtitleTimeout();
        stopDownloadStatusPolling();
        stopModelMemoryPolling();
        if (selectedContactPromptSyncTimeout) {
            clearTimeout(selectedContactPromptSyncTimeout);
        }
        stopCallTimerTracking();
        if (playbackIdleTimeout) {
            clearTimeout(playbackIdleTimeout);
        }

        stopPlayback(true);
        stopAudioCapture();

        for (const unlisten of eventUnlisteners) {
            unlisten();
        }
        eventUnlisteners = [];
    });
</script>

<svelte:window
    onkeydown={handleWindowKeydown}
    onfocus={handleWindowFocus}
    onpaste={handleWindowPaste}
/>

<div class="app-container" class:contacts-open={showContactsPopup}>
    <div class="background" style={selectedContactImageStyle}></div>

    {#if !isPreparingRuntime && !showContactsPopup && !showAboutPopup && !showConversationPopup && !showUpdatePrompt}
        <AppHeader
            {currentSessionTitle}
            {showSessionsPopup}
            {calling}
            onToggleSessions={toggleSessionsPopup}
        />
    {/if}

    {#if !calling}
        <div
            class="model-tags"
            class:dimmed={showContactsPopup || showAboutPopup || showUpdatePrompt}
        >
            {#if showRuntimeSetupBanner}
                <div
                    class="runtime-setup-banner"
                    class:error={!!runtimeSetupError}
                >
                    <div class="runtime-setup-copy">
                        <span class="runtime-setup-title"
                            >{runtimeSetupTitle}</span
                        >
                        <span class="runtime-setup-detail"
                            >{runtimeSetupError ?? runtimeSetupMessage}</span
                        >
                    </div>
                    {#if runtimeSetupError}
                        <button
                            type="button"
                            class="utility-btn runtime-setup-action"
                            onclick={handleRetryRuntimeSetup}
                        >
                            Retry
                        </button>
                    {/if}
                </div>
            {/if}
            {#if !isPreparingRuntime}
                <div class="model-actions">
                    <div
                        class="tooltip-shell variant-select-shell model-preset-shell"
                    >
                        <select
                            class="variant-select model-preset-select"
                            value={activeModelPreset}
                            aria-label="Model preset"
                            title={modelPresetTooltip}
                            disabled={presetSelectDisabled}
                            onchange={handleModelPresetChange}
                        >
                            {#each modelPresetOptions as option}
                                <option value={option.value}
                                    >{option.label}</option
                                >
                            {/each}
                        </select>
                    </div>
                    <button
                        type="button"
                        class="utility-btn load-all-btn"
                        class:load-all-btn-primary={loadAllNeedsAction}
                        disabled={loadAllDisabled}
                        title={loadAllButtonTitle}
                        onclick={loadAllNeedsAction
                            ? handleLoadAll
                            : handleUnloadAll}
                    >
                        {loadAllButtonLabel}
                    </button>
                </div>
                <GemmaBanner
                    {isDownloadingGemma}
                    {isGemmaDownloaded}
                    {isGemmaLoaded}
                    {selectedGemmaVariant}
                    {gemmaVariantOptions}
                    {gemmaVariantDisabled}
                    {gemmaVariantTooltip}
                    {gemmaDownloadError}
                    {gemmaDownloadMessage}
                    {gemmaDownloadProgress}
                    {gemmaDownloadIndeterminate}
                    {isCancellingGemmaDownload}
                    {isUnloadingGemma}
                    {isLoadingGemma}
                    {isClearingGemmaCache}
                    {formatDownloadPercent}
                    {handleGemmaVariantChange}
                    {handleCancelGemmaDownload}
                    {handleUnloadGemma}
                    {handleClearGemmaCache}
                    {handleDownloadGemma}
                    {handleLoadGemma}
                    isExternalGemmaVariant={selectedGemmaUsesExternalProvider}
                    externalProviderLabel={selectedExternalProviderLabel}
                    externalProviderSupported={selectedExternalProviderSupported}
                    externalProviderGuideText={selectedExternalProviderGuideText}
                    externalModels={selectedExternalModels}
                    {selectedExternalModel}
                    handleExternalModelChange={selectedGemmaVariant ===
                    "lmstudio"
                        ? handleLmStudioModelChange
                        : selectedGemmaVariant === "openai_compatible"
                          ? handleOpenAiCompatibleModelChange
                          : handleOllamaModelChange}
                    {externalModelDisabled}
                    {openExternalConfig}
                />
                <SttBanner
                    {selectedGemmaVariant}
                    {sttUsesGemma}
                    {isGemmaLoaded}
                    {isDownloadingStt}
                    {isSttDownloaded}
                    {isSttLoaded}
                    {selectedSttModel}
                    {sttModelOptions}
                    {sttVariantDisabled}
                    {sttModelTooltip}
                    {selectedSttModelLabel}
                    {sttDownloadError}
                    {sttDownloadMessage}
                    {sttDownloadProgress}
                    {sttDownloadIndeterminate}
                    {sttLoadMessage}
                    {isCancellingSttDownload}
                    {isUnloadingStt}
                    {isLoadingStt}
                    {isClearingSttCache}
                    {formatDownloadPercent}
                    {handleSttModelChange}
                    {handleCancelSttDownload}
                    {handleUnloadStt}
                    {handleClearSttCache}
                    {handleDownloadStt}
                    {handleLoadStt}
                    isExternalGemmaVariant={selectedGemmaUsesExternalProvider}
                    externalProviderLabel={selectedExternalProviderLabel}
                />
                <SpeechBanner
                    {isDownloadingCsm}
                    {isCsmDownloaded}
                    {isCsmLoaded}
                    {selectedCsmModel}
                    {csmModelOptions}
                    {csmVariantDisabled}
                    {csmModelTooltip}
                    {csmQuantizeAvailable}
                    {isCsmQuantized}
                    {isUpdatingCsmQuantize}
                    {selectedCsmModelLabel}
                    {csmDownloadError}
                    {csmDownloadMessage}
                    {csmDownloadProgress}
                    {csmDownloadIndeterminate}
                    {csmLoadMessage}
                    {csmNotificationMessage}
                    {isCancellingCsmDownload}
                    {isUnloadingCsm}
                    {isLoadingCsm}
                    {isClearingCsmCache}
                    {formatDownloadPercent}
                    {handleCsmModelChange}
                    {handleCancelCsmDownload}
                    {handleUnloadCsm}
                    {handleClearCsmCache}
                    {handleDownloadCsm}
                    {handleLoadCsm}
                    {handleCsmQuantizeToggle}
                />
            {/if}
        </div>
    {/if}

    <main class:idle-layout={!calling}>
        {#if calling && callStageMessage}
            <div class="call-stage-banner-wrapper">
                <div
                    class="call-stage-banner"
                    class:interactive={callStagePhase === "thinking" &&
                        reasoningText}
                    data-phase={callStagePhase}
                    onclick={() => {
                        if (callStagePhase === "thinking" && reasoningText) {
                            showReasoningPopup = !showReasoningPopup;
                        }
                    }}
                >
                    <span class="call-stage-dot"></span>
                    <span>{callStageMessage}</span>
                    {#if callStagePhase === "thinking" && reasoningText}
                        <div class="tooltip-shell">
                            <div class="thinking-spinner-shell">
                                <svg
                                    class="spinner"
                                    width="12"
                                    height="12"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="3"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    ><path
                                        d="M21 12a9 9 0 1 1-6.219-8.56"
                                    /></svg
                                >
                            </div>
                            <div class="tooltip-bubble control-tooltip">
                                Click to show thinking process
                            </div>
                        </div>
                    {/if}
                </div>

                {#if showReasoningPopup && callStagePhase === "thinking" && reasoningText}
                    <div
                        class="reasoning-popup"
                        transition:fade={{ duration: 150 }}
                    >
                        <div class="reasoning-popup-header">
                            <span class="reasoning-popup-title"
                                >Thinking Process</span
                            >
                            <button
                                class="reasoning-popup-close"
                                onclick={() => (showReasoningPopup = false)}
                            >
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    width="14"
                                    height="14"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2.5"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    ><line x1="18" y1="6" x2="6" y2="18"
                                    ></line><line x1="6" y1="6" x2="18" y2="18"
                                    ></line></svg
                                >
                            </button>
                        </div>
                        <div class="reasoning-popup-content">
                            {reasoningText}
                        </div>
                    </div>
                {/if}
            </div>
        {/if}
        <div class="avatar-shell" style="--theme-rgb: {themeRgb}">
            <div class="avatar-container">
                {#if assistantSpeaking}
                    <div class="avatar-wave" out:fade={{ duration: 400 }}></div>
                    <div class="avatar-wave" out:fade={{ duration: 400 }}></div>
                    <div class="avatar-wave" out:fade={{ duration: 400 }}></div>
                {/if}
                <div
                    class="avatar"
                    class:calling
                    class:user-speaking={userSpeaking}
                    style={selectedContactImageStyle}
                ></div>
            </div>
            {#if calling && showStatEnabled}
                <div class="avatar-latency" aria-live="polite">
                    <span class="avatar-latency-label">Latency</span>
                    <span class="avatar-latency-value"
                        >{formatLatencySummary()}</span
                    >
                </div>
            {/if}
            {#if calling}
                <div
                    class="avatar-subtitle"
                    class:display={activeAvatarSubtitle.length > 0}
                    aria-live="polite"
                    aria-atomic="true"
                >
                    <span class="avatar-subtitle-text"
                        >{activeAvatarSubtitle}</span
                    >
                </div>
            {/if}
        </div>
    </main>

    <div class="control-bar-wrapper">
        {#if showModelMemorySummary && modelMemorySnapshot && !calling}
            <div class="model-memory-pill" aria-live="polite">
                <span class="model-memory-pill-label">Memory</span>
                <span class="model-memory-pill-value"
                    >{formatMemoryUsage(modelMemorySnapshot.total_bytes)}</span
                >
            </div>
        {/if}

        {#if showScreenCaptureCard}
            <div
                class="screen-capture-card"
                data-phase={screenCapturePhase ?? "ready"}
                aria-live="polite"
            >
                <div class="screen-capture-card-header">
                    <div class="screen-capture-copy">
                        <span class="screen-capture-title"
                            >{screenCaptureTitle}</span
                        >
                    </div>
                    {#if screenCapturePhase !== "capturing"}
                        <button
                            type="button"
                            class="utility-btn subtitle-action-btn"
                            onclick={handleClearPendingScreenCapture}
                        >
                            {screenCaptureActionLabel}
                        </button>
                    {/if}
                </div>

                {#if screenCaptureImageDataUrls.length > 0}
                    <div class="screen-capture-thumbnails">
                        {#each screenCaptureImageDataUrls as url, i}
                            <div class="screen-capture-thumbnail-wrapper">
                                <!-- svelte-ignore a11y_click_events_have_key_events -->
                                <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                                <div
                                    class="screen-capture-thumbnail"
                                    onclick={() => (previewImageUrl = url)}
                                    role="button"
                                    tabindex="0"
                                >
                                    <img src={url} alt="Screen Capture" />
                                </div>
                                <button
                                    type="button"
                                    class="screen-capture-remove-btn"
                                    onclick={() =>
                                        handleRemoveScreenCaptureAt(i)}
                                    aria-label="Remove attachment"
                                >
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="3"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        ><line x1="18" y1="6" x2="6" y2="18"
                                        ></line><line
                                            x1="6"
                                            y1="6"
                                            x2="18"
                                            y2="18"
                                        ></line></svg
                                    >
                                </button>
                            </div>
                        {/each}
                    </div>
                {/if}
            </div>
        {/if}

        {#if showContactsPopup}
            <div class="popup-wrapper" bind:this={contactsPopupEl}>
                <ContactsModal
                    {contacts}
                    {selectedContact}
                    {selectedContactName}
                    {selectedContactIconUrl}
                    {getContactDisplayName}
                    {closeContactsPopup}
                    {selectContact}
                    {triggerContactImport}
                    {createNewContact}
                    {triggerContactIconUpload}
                    {triggerContactRefAudioUpload}
                    {handleResetSelectedContactIcon}
                    {handleResetSelectedContactRefAudio}
                    {handlePlaySelectedContactRefAudio}
                    {handleSelectedContactNameInput}
                    {handleSelectedContactPromptInput}
                    {handleSelectedContactGenderInput}
                    {handleSelectedContactRefTextInput}
                    {handleDeleteSelectedContact}
                    {handleExportSelectedContact}
                    {refAudioPlaying}
                />
            </div>
        {/if}

        {#if showExternalLlmConfig}
            <ExternalLlmConfigModal
                providerName={selectedExternalProviderLabel}
                baseUrl={getExternalProviderBaseUrl(selectedGemmaVariant)}
                hasApiKey={getExternalProviderHasApiKey(selectedGemmaVariant)}
                urlPlaceholder={getExternalProviderUrlPlaceholder(
                    selectedGemmaVariant,
                )}
                onSave={saveExternalLlmConfig}
                onClose={closeExternalConfig}
            />
        {/if}

        {#if showConversationPopup}
            <div class="popup-wrapper" bind:this={conversationPopupEl}>
                <ConversationPopup
                    {conversationLogEntries}
                    sessionTitle={currentSessionTitle}
                    {aiSubtitleTargetLanguage}
                    {popupActionsBusy}
                    {calling}
                    onClearHistory={clearConversationLogImages}
                    onClose={closeConversationPopup}
                    onFork={handleForkSession}
                    onPreviewImage={(url) => (previewImageUrl = url)}
                    {saveConversationLogEntryEdit}
                    {deleteConversationLogEntry}
                    {setConversationLogViewport}
                />
            </div>
        {/if}

        {#if previewImageUrl}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div
                class="image-preview-overlay"
                onclick={() => (previewImageUrl = null)}
                role="presentation"
            >
                <button
                    type="button"
                    class="image-preview-close-btn"
                    onclick={() => (previewImageUrl = null)}
                    aria-label="Close preview"
                >
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        width="24"
                        height="24"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        ><line x1="18" y1="6" x2="6" y2="18"></line><line
                            x1="6"
                            y1="6"
                            x2="18"
                            y2="18"
                        ></line></svg
                    >
                </button>

                <div
                    class="image-preview-content"
                    onclick={(e) => e.stopPropagation()}
                >
                    <img
                        src={previewImageUrl}
                        alt="Preview"
                        class="image-preview-img"
                    />
                </div>
            </div>
        {/if}

        {#if showAboutPopup}
            <div class="popup-wrapper" bind:this={aboutPopupEl}>
                <AboutModal
                    {calling}
                    {buildInfo}
                    {buildInfoError}
                    {availableAppUpdate}
                    {appUpdateStatus}
                    {appUpdateError}
                    checkForUpdates={checkForAppUpdates}
                    installAvailableUpdate={installAppUpdate}
                    {restartToApplyUpdate}
                    {closeAboutPopup}
                    {globalShortcut}
                    {globalShortcutEntireScreen}
                    {globalShortcutToggleMute}
                    {globalShortcutInterrupt}
                    {pongPlaybackEnabled}
                    {autoUnmuteOnPastedScreenshotEnabled}
                    {autoCheckAppUpdatesEnabled}
                    {selectLastSessionEnabled}
                    {autoLoadModelsOnStartupEnabled}
                    {showStatEnabled}
                    {showSubtitleEnabled}
                    {showAiSubtitleEnabled}
                    {aiSubtitleTargetLanguage}
                    {subtitleTranslationLlmConfigured}
                    {showCallTimerEnabled}
                    {showHiddenWindowOverlayEnabled}
                    {endOfUtteranceSilenceMs}
                    {autoContinueSilenceMs}
                    {autoContinueMaxCount}
                    {llmContextTurnLimit}
                    {llmImageHistoryLimit}
                    onUpdateGlobalShortcut={applyGlobalShortcutPreference}
                    onUpdateGlobalShortcutEntireScreen={applyGlobalShortcutEntireScreenPreference}
                    onUpdateGlobalShortcutToggleMute={applyGlobalShortcutToggleMutePreference}
                    onUpdateGlobalShortcutInterrupt={applyGlobalShortcutInterruptPreference}
                    onUpdatePongPlayback={applyPongPlaybackPreference}
                    onUpdateAutoUnmuteOnPastedScreenshot={applyAutoUnmuteOnPastedScreenshotPreference}
                    onUpdateAutoCheckAppUpdates={applyAutoCheckAppUpdatesPreference}
                    onUpdateSelectLastSession={applySelectLastSessionPreference}
                    onUpdateAutoLoadModelsOnStartup={applyAutoLoadModelsOnStartupPreference}
                    onUpdateShowStat={applyShowStatPreference}
                    onUpdateShowSubtitle={applyShowSubtitlePreference}
                    onUpdateShowAiSubtitle={applyShowAiSubtitlePreference}
                    onUpdateAiSubtitleTargetLanguage={applyAiSubtitleTargetLanguagePreference}
                    onOpenSubtitleTranslationLlmConfig={openSubtitleTranslationLlmConfig}
                    onUpdateShowCallTimer={applyShowCallTimerPreference}
                    onUpdateShowHiddenWindowOverlay={applyShowHiddenWindowOverlayPreference}
                    onUpdateEndOfUtteranceSilenceMs={applyEndOfUtteranceSilencePreference}
                    onUpdateAutoContinueSilenceMs={applyAutoContinueSilencePreference}
                    onUpdateAutoContinueMaxCount={applyAutoContinueMaxCountPreference}
                    onUpdateLlmContextTurnLimit={applyLlmContextTurnLimitPreference}
                    onUpdateLlmImageHistoryLimit={applyLlmImageHistoryLimitPreference}
                />
            </div>
        {/if}

        {#if showUpdatePrompt && availableAppUpdate}
            <UpdatePromptModal
                {availableAppUpdate}
                {appUpdateStatus}
                {appUpdateError}
                onInstall={appUpdateStatus === "installed"
                    ? restartToApplyUpdate
                    : installAppUpdate}
                onSkipVersion={skipAvailableAppUpdateVersion}
                onRemindLater={remindAboutAppUpdateLater}
            />
        {/if}

        {#if showSubtitleTranslationLlmConfig}
            <SubtitleTranslationLlmConfigModal
                baseUrl={subtitleTranslationBaseUrl}
                hasApiKey={subtitleTranslationHasApiKey}
                modelId={subtitleTranslationModelId}
                onSave={saveSubtitleTranslationLlmConfig}
                onTestConnection={testSubtitleTranslationConnection}
                onClose={closeSubtitleTranslationLlmConfig}
            />
        {/if}

        <div
            class="control-bar"
            class:dimmed={showContactsPopup || showAboutPopup || showUpdatePrompt}
        >
            <div class="info">
                <span class="username"
                    >{selectedContact?.name.trim() || "OpenDuck"}</span
                >
                {#if showCallTimerEnabled}
                    <span class="timer"
                        >{calling
                            ? formattedTime
                            : modelsReady
                              ? "Ready"
                              : "Pending"}</span
                    >
                {/if}
            </div>

            <div class="actions">
                {#if !calling}
                    <button
                        type="button"
                        class="icon-btn conversation-log-btn"
                        class:active={showConversationPopup}
                        onclick={toggleConversationPopup}
                        disabled={!conversationLogEntries.length}
                        aria-label="Toggle conversation log"
                        aria-controls="conversation-log-popup"
                        aria-expanded={showConversationPopup}
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            width="22"
                            height="22"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            ><path d="M7 10h8" /><path d="M7 14h5" /><path
                                d="M21 12a8 8 0 0 1-8 8H5l-2 2V12a8 8 0 0 1 8-8h2a8 8 0 0 1 8 8z"
                            /></svg
                        >
                    </button>
                    <button
                        type="button"
                        class="icon-btn contacts-btn"
                        class:active={showContactsPopup}
                        onclick={toggleContactsPopup}
                        aria-label="Toggle contacts"
                        aria-controls="contacts-popup"
                        aria-expanded={showContactsPopup}
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            width="22"
                            height="22"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            ><path
                                d="M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2"
                            /><circle cx="12" cy="7" r="4" /></svg
                        >
                    </button>
                    <button
                        type="button"
                        class="icon-btn about-btn"
                        class:active={showAboutPopup}
                        onclick={toggleAboutPopup}
                        aria-label="Settings"
                        aria-controls="about-popup"
                        aria-expanded={showAboutPopup}
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            width="22"
                            height="22"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            aria-hidden="true"
                        >
                            <path
                                d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"
                            />
                            <circle cx="12" cy="12" r="3" />
                        </svg>
                    </button>
                {/if}
                {#if calling}
                    <div class="tooltip-shell control-tooltip-shell">
                        <button
                            type="button"
                            class="icon-btn conversation-log-btn"
                            class:active={showConversationPopup}
                            onclick={toggleConversationPopup}
                            aria-label="Toggle conversation log"
                            aria-controls="conversation-log-popup"
                            aria-expanded={showConversationPopup}
                            title="Conversation Log"
                        >
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                width="22"
                                height="22"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2.2"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                ><path d="M7 10h8" /><path d="M7 14h5" /><path
                                    d="M21 12a8 8 0 0 1-8 8H5l-2 2V12a8 8 0 0 1 8-8h2a8 8 0 0 1 8 8z"
                                /></svg
                            >
                        </button>
                        <div class="tooltip-bubble control-tooltip">
                            <span>Conversation Log</span>
                        </div>
                    </div>
                    <div class="tooltip-shell control-tooltip-shell">
                        <button
                            class="icon-btn mute-btn"
                            class:active={!micMuted}
                            class:muted={micMuted}
                            type="button"
                            onclick={toggleMic}
                            aria-label={muteButtonLabel}
                            aria-keyshortcuts="Space"
                            title={muteButtonTitle}
                        >
                            {#if micMuted}
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    width="22"
                                    height="22"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2.5"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    ><line x1="1" y1="1" x2="23" y2="23" /><path
                                        d="M9 9v3a3 3 0 0 0 5.12 2.12M15 9.34V4a3 3 0 0 0-5.94-.6"
                                    /><path
                                        d="M17 16.95A7 7 0 0 1 5 12v-2m14 0v2a7 7 0 0 1-.11 1.23"
                                    /><line
                                        x1="12"
                                        y1="19"
                                        x2="12"
                                        y2="23"
                                    /><line
                                        x1="8"
                                        y1="23"
                                        x2="16"
                                        y2="23"
                                    /></svg
                                >
                            {:else}
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    width="22"
                                    height="22"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2.5"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    ><path
                                        d="M12 1a3 3 0 0 0-3 3v8a3 3 0 0 0 6 0V4a3 3 0 0 0-3-3z"
                                    /><path
                                        d="M19 10v2a7 7 0 0 1-14 0v-2"
                                    /><line
                                        x1="12"
                                        y1="19"
                                        x2="12"
                                        y2="23"
                                    /><line
                                        x1="8"
                                        y1="23"
                                        x2="16"
                                        y2="23"
                                    /></svg
                                >
                            {/if}
                        </button>
                        <div class="tooltip-bubble control-tooltip">
                            <span>{muteButtonLabel}</span>
                            <span class="tooltip-shortcut">Space</span>
                        </div>
                    </div>
                    <div class="tooltip-shell control-tooltip-shell">
                        <button
                            type="button"
                            class="icon-btn interrupt-btn"
                            disabled={!assistantSpeaking}
                            onclick={handleInterruptTts}
                            aria-label="Interrupt assistant speech"
                            title="Interrupt assistant speech (ESC)"
                        >
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                width="22"
                                height="22"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2.2"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                aria-hidden="true"
                            >
                                <path d="M6.5 11V6.5a1.5 1.5 0 0 1 3 0V11" />
                                <path d="M9.5 11V5a1.5 1.5 0 0 1 3 0v6" />
                                <path d="M12.5 11V4.5a1.5 1.5 0 0 1 3 0V11" />
                                <path
                                    d="M15.5 11V6a1.5 1.5 0 0 1 3 0v7a6 6 0 0 1-6 6H11a5 5 0 0 1-4.64-3.14l-1.1-2.64a1.5 1.5 0 0 1 2.72-1.26L9.5 14V11"
                                />
                            </svg>
                        </button>
                        <div class="tooltip-bubble control-tooltip">
                            <span>Interrupt</span>
                            <span class="tooltip-shortcut">ESC</span>
                        </div>
                    </div>
                    <div class="tooltip-shell control-tooltip-shell">
                        <button
                            type="button"
                            class="icon-btn about-btn"
                            class:active={showAboutPopup}
                            onclick={toggleAboutPopup}
                            aria-label="Settings"
                            aria-controls="about-popup"
                            aria-expanded={showAboutPopup}
                        >
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                width="22"
                                height="22"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2.2"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                aria-hidden="true"
                            >
                                <path
                                    d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"
                                />
                                <circle cx="12" cy="12" r="3" />
                            </svg>
                        </button>
                        <div class="tooltip-bubble control-tooltip">
                            <span>Settings</span>
                        </div>
                    </div>
                {/if}
            </div>

            {#if calling}
                <button class="end-btn" onclick={handleEndCall}>End</button>
            {:else}
                <div class="tooltip-shell start-call-tooltip-shell">
                    {#if conversationLogEntries.length > 0}
                        <button
                            class="start-btn"
                            disabled={!modelsReady}
                            onclick={handleResumeCall}>Resume</button
                        >
                    {:else}
                        <button
                            class="start-btn"
                            disabled={!modelsReady}
                            onclick={handleStartCall}>Call</button
                        >
                    {/if}
                    {#if !modelsReady}
                        <div
                            id="start-call-tooltip"
                            class="tooltip-bubble start-call-tooltip"
                        >
                            LLM, STT, and TTS models must be all loaded to start
                            the call.
                        </div>
                    {/if}
                </div>
            {/if}
        </div>
    </div>

    {#if showSessionsPopup}
        <div class="popup-wrapper" bind:this={sessionsPopupEl}>
            <SessionsPopup
                {sessions}
                activeSessionId={currentSessionId}
                onSelect={handleSelectSession}
                onDelete={handleDeleteSession}
                onRename={handleRenameSession}
                onNewChat={handleNewChat}
                onOpenSearch={openSearchModal}
                onClose={() => (showSessionsPopup = false)}
            />
        </div>
    {/if}

    {#if showSearchModal}
        <SearchModal
            onClose={closeSearchModal}
            onSelect={handleSearchSelect}
            onNewChat={() => {
                showSearchModal = false;
                handleNewChat();
            }}
        />
    {/if}

    <input
        class="hidden-file-input"
        type="file"
        accept=".openduck,application/json,.json"
        bind:this={contactsImportInput}
        onchange={handleContactImportChange}
    />
    <input
        class="hidden-file-input"
        type="file"
        accept="image/*"
        bind:this={contactIconInput}
        onchange={handleContactIconChange}
    />
    <input
        class="hidden-file-input"
        type="file"
        accept="audio/*"
        bind:this={contactRefAudioInput}
        onchange={handleContactRefAudioChange}
    />
</div>
