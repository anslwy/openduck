<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { listen, type UnlistenFn } from "@tauri-apps/api/event";
    import { save } from "@tauri-apps/plugin-dialog";

    type CsmAudioStartEvent = {
        request_id: number;
    };

    type CsmAudioQueuedEvent = {
        request_id: number;
    };

    type CsmAudioChunkEvent = {
        request_id: number;
        audio_wav_base64: string;
    };

    type CsmAudioDoneEvent = {
        request_id: number;
    };

    type CsmAudioStopEvent = Record<string, never>;

    type CsmErrorEvent = {
        request_id?: number | null;
        message: string;
    };

    type CsmStatusEvent = {
        message: string;
    };

    type SttStatusEvent = {
        message: string;
    };

    type CallStageEvent = {
        phase: string;
        message: string;
    };

    type TranscriptEvent = {
        text: string;
        imageDataUrl?: string | null;
    };

    type AssistantResponseEvent = {
        request_id: number;
        text: string;
        is_final: boolean;
    };

    type ScreenCaptureEvent = {
        phase:
            | "capturing"
            | "ready"
            | "cancelled"
            | "cleared"
            | "consumed"
            | "error";
        message: string;
        hasPendingAttachment: boolean;
        fileName?: string | null;
    };

    type TrayEndCallEvent = Record<string, never>;
    type TrayToggleMuteEvent = Record<string, never>;

    type ModelDownloadProgressEvent = {
        model: "gemma" | "csm" | "stt";
        phase: "progress" | "completed" | "error" | "cancelled";
        message: string;
        progress?: number | null;
        downloaded_bytes?: number | null;
        total_bytes?: number | null;
        indeterminate: boolean;
    };

    type ModelMemoryUsageEntry = {
        key: "gemma" | "csm" | "stt";
        label: string;
        detail?: string | null;
        bytes: number;
        root_pid: number;
        process_count: number;
    };

    type ModelMemoryUsageSnapshot = {
        total_bytes: number;
        models: ModelMemoryUsageEntry[];
    };

    type GemmaVariant = "e4b" | "e2b";
    type CsmModelVariant = "expressiva_1b" | "kokoro_82m" | "cosyvoice2_0_5b";
    type SttModelVariant = "gemma" | "whisper_large_v3_turbo";

    type ConversationLogEntry = {
        id: number;
        role: "user" | "assistant";
        text: string;
        imageUrl: string | null;
    };

    type StoredContactProfile = {
        id: string;
        name: string;
        prompt: string;
        hasCustomIcon: boolean;
    };

    type ContactProfile = StoredContactProfile & {
        iconDataUrl: string | null;
    };

    type StoredContactsPayload = {
        version: 1;
        selectedContactId: string;
        contacts: StoredContactProfile[];
    };

    type ContactExportResult = {
        savedPath: string;
    };

    type StoredModelPreferences = {
        version: 1;
        gemmaVariant: GemmaVariant;
        csmModel: CsmModelVariant;
        sttModel: SttModelVariant;
    };

    const DEFAULT_VOICE_SYSTEM_PROMPT =
        "You are in a live voice call. Reply like a natural spoken conversation. Use plain sentences only. Never use markdown, bullets, headings, numbered lists, code fences, tables, emojis, or stage directions. Keep responses concise, direct, and easy to speak aloud. Respond with short sentences and each sentence must contain less than 20 words";
    const CONTACTS_STORAGE_KEY = "openduck.contacts.v1";
    const CONTACT_ICONS_DB_NAME = "openduck.contacts";
    const CONTACT_ICONS_STORE_NAME = "contact-icons";
    const DEFAULT_CONTACT_ID = "contact-openduck";
    const MODEL_PREFERENCES_STORAGE_KEY = "openduck.model-preferences.v1";
    const DEFAULT_GEMMA_VARIANT: GemmaVariant = "e4b";
    const DEFAULT_CSM_MODEL: CsmModelVariant = "kokoro_82m";
    const DEFAULT_STT_MODEL: SttModelVariant = "whisper_large_v3_turbo";

    let contactIconsDbPromise: Promise<IDBDatabase> | null = null;

    function createDefaultContact(): ContactProfile {
        return {
            id: DEFAULT_CONTACT_ID,
            name: "OpenDuck",
            prompt: DEFAULT_VOICE_SYSTEM_PROMPT,
            hasCustomIcon: false,
            iconDataUrl: null,
        };
    }

    function getContactDisplayName(
        contact: Pick<ContactProfile, "name"> | null | undefined,
    ) {
        const name = contact?.name.trim();
        return name ? name : "Untitled contact";
    }

    function createContactId() {
        if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
            return crypto.randomUUID();
        }

        return `contact-${Date.now()}-${Math.random().toString(16).slice(2)}`;
    }

    function createStoredContactsPayload(
        contactList: ContactProfile[],
        activeContactId: string,
    ): StoredContactsPayload {
        return {
            version: 1,
            selectedContactId: activeContactId,
            contacts: contactList.map((contact) => ({
                id: contact.id,
                name: contact.name,
                prompt: contact.prompt,
                hasCustomIcon: contact.hasCustomIcon,
            })),
        };
    }

    function createDefaultModelPreferences(): StoredModelPreferences {
        return {
            version: 1,
            gemmaVariant: DEFAULT_GEMMA_VARIANT,
            csmModel: DEFAULT_CSM_MODEL,
            sttModel: DEFAULT_STT_MODEL,
        };
    }

    function isGemmaVariant(value: unknown): value is GemmaVariant {
        return value === "e4b" || value === "e2b";
    }

    function isCsmModelVariant(value: unknown): value is CsmModelVariant {
        return (
            value === "expressiva_1b" ||
            value === "kokoro_82m" ||
            value === "cosyvoice2_0_5b"
        );
    }

    function isSttModelVariant(value: unknown): value is SttModelVariant {
        return value === "gemma" || value === "whisper_large_v3_turbo";
    }

    function loadModelPreferencesFromStorage(): StoredModelPreferences {
        const fallback = createDefaultModelPreferences();

        if (typeof window === "undefined") {
            return fallback;
        }

        const rawPayload = window.localStorage.getItem(
            MODEL_PREFERENCES_STORAGE_KEY,
        );
        if (!rawPayload) {
            return fallback;
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                version?: unknown;
                gemmaVariant?: unknown;
                csmModel?: unknown;
                sttModel?: unknown;
            };

            if (parsed.version !== 1) {
                return fallback;
            }

            return {
                version: 1,
                gemmaVariant: isGemmaVariant(parsed.gemmaVariant)
                    ? parsed.gemmaVariant
                    : DEFAULT_GEMMA_VARIANT,
                csmModel: isCsmModelVariant(parsed.csmModel)
                    ? parsed.csmModel
                    : DEFAULT_CSM_MODEL,
                sttModel: isSttModelVariant(parsed.sttModel)
                    ? parsed.sttModel
                    : DEFAULT_STT_MODEL,
            };
        } catch (err) {
            console.error("Failed to restore model preferences:", err);
            return fallback;
        }
    }

    function normalizeStoredContactProfile(
        value: unknown,
    ): StoredContactProfile | null {
        if (!value || typeof value !== "object") {
            return null;
        }

        const record = value as Record<string, unknown>;
        const id =
            typeof record.id === "string" && record.id.trim()
                ? record.id.trim()
                : null;
        if (!id) {
            return null;
        }

        return {
            id,
            name: typeof record.name === "string" ? record.name : "",
            prompt:
                typeof record.prompt === "string" &&
                record.prompt.trim().length > 0
                    ? record.prompt
                    : DEFAULT_VOICE_SYSTEM_PROMPT,
            hasCustomIcon: Boolean(record.hasCustomIcon),
        };
    }

    function slugifyContactName(name: string) {
        const slug = name
            .trim()
            .toLowerCase()
            .replace(/[^a-z0-9]+/g, "-")
            .replace(/^-+|-+$/g, "");
        return slug || "contact";
    }

    function readFileAsDataUrl(file: File) {
        return new Promise<string>((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => {
                if (typeof reader.result === "string") {
                    resolve(reader.result);
                    return;
                }

                reject(new Error("Failed to read file as a data URL."));
            };
            reader.onerror = () =>
                reject(reader.error ?? new Error("Failed to read file."));
            reader.readAsDataURL(file);
        });
    }

    function openContactIconsDatabase() {
        if (contactIconsDbPromise) {
            return contactIconsDbPromise;
        }

        contactIconsDbPromise = new Promise<IDBDatabase>((resolve, reject) => {
            const request = window.indexedDB.open(CONTACT_ICONS_DB_NAME, 1);

            request.onupgradeneeded = () => {
                const database = request.result;
                if (
                    !database.objectStoreNames.contains(
                        CONTACT_ICONS_STORE_NAME,
                    )
                ) {
                    database.createObjectStore(CONTACT_ICONS_STORE_NAME);
                }
            };

            request.onsuccess = () => resolve(request.result);
            request.onerror = () =>
                reject(
                    request.error ??
                        new Error("Failed to open the contacts icon database."),
                );
        });

        return contactIconsDbPromise;
    }

    async function runContactIconStoreRequest(
        mode: IDBTransactionMode,
        requestFactory: (store: IDBObjectStore) => IDBRequest,
    ) {
        const database = await openContactIconsDatabase();

        return new Promise<unknown>((resolve, reject) => {
            const transaction = database.transaction(
                CONTACT_ICONS_STORE_NAME,
                mode,
            );
            const store = transaction.objectStore(CONTACT_ICONS_STORE_NAME);
            const request = requestFactory(store);

            request.onsuccess = () => resolve(request.result);
            request.onerror = () =>
                reject(
                    request.error ??
                        new Error("A contacts icon storage request failed."),
                );
            transaction.onerror = () =>
                reject(
                    transaction.error ??
                        new Error("A contacts icon transaction failed."),
                );
        });
    }

    async function loadStoredContactIcon(contactId: string) {
        if (typeof window === "undefined" || !("indexedDB" in window)) {
            return null;
        }

        const result = await runContactIconStoreRequest("readonly", (store) =>
            store.get(contactId),
        );

        return typeof result === "string" ? result : null;
    }

    async function saveStoredContactIcon(contactId: string, dataUrl: string) {
        if (typeof window === "undefined" || !("indexedDB" in window)) {
            return;
        }

        await runContactIconStoreRequest("readwrite", (store) =>
            store.put(dataUrl, contactId),
        );
    }

    async function deleteStoredContactIcon(contactId: string) {
        if (typeof window === "undefined" || !("indexedDB" in window)) {
            return;
        }

        await runContactIconStoreRequest("readwrite", (store) =>
            store.delete(contactId),
        );
    }

    async function loadContactsFromStorage() {
        if (typeof window === "undefined") {
            return {
                contacts: [createDefaultContact()],
                selectedContactId: DEFAULT_CONTACT_ID,
            };
        }

        const rawPayload = window.localStorage.getItem(CONTACTS_STORAGE_KEY);
        if (!rawPayload) {
            return {
                contacts: [createDefaultContact()],
                selectedContactId: DEFAULT_CONTACT_ID,
            };
        }

        try {
            const parsed = JSON.parse(rawPayload) as {
                contacts?: unknown;
                selectedContactId?: unknown;
            };
            const storedContacts = Array.isArray(parsed.contacts)
                ? parsed.contacts
                      .map((contact) => normalizeStoredContactProfile(contact))
                      .filter(
                          (contact): contact is StoredContactProfile =>
                              contact !== null,
                      )
                : [];

            if (storedContacts.length === 0) {
                return {
                    contacts: [createDefaultContact()],
                    selectedContactId: DEFAULT_CONTACT_ID,
                };
            }

            const contactsWithIcons = await Promise.all(
                storedContacts.map(async (contact) => {
                    const iconDataUrl = contact.hasCustomIcon
                        ? await loadStoredContactIcon(contact.id)
                        : null;

                    return {
                        ...contact,
                        hasCustomIcon: Boolean(iconDataUrl),
                        iconDataUrl,
                    };
                }),
            );
            const activeContactId =
                typeof parsed.selectedContactId === "string" &&
                contactsWithIcons.some(
                    (contact) => contact.id === parsed.selectedContactId,
                )
                    ? parsed.selectedContactId
                    : contactsWithIcons[0].id;

            return {
                contacts: contactsWithIcons,
                selectedContactId: activeContactId,
            };
        } catch (err) {
            console.error("Failed to load stored contacts:", err);
            return {
                contacts: [createDefaultContact()],
                selectedContactId: DEFAULT_CONTACT_ID,
            };
        }
    }

    let calling = $state(false);
    let micMuted = $state(false);
    let time = $state(0);
    let callStartedAtMs = $state<number | null>(null);
    let isGemmaDownloaded = $state(false);
    let isGemmaLoaded = $state(false);
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
    let csmDownloadMessage = $state("Preparing download...");
    let csmDownloadProgress = $state<number | null>(null);
    let csmDownloadIndeterminate = $state(true);
    let csmDownloadError = $state<string | null>(null);
    let csmLoadMessage = $state("Starting worker...");
    let sttDownloadMessage = $state("Preparing download...");
    let sttDownloadProgress = $state<number | null>(null);
    let sttDownloadIndeterminate = $state(true);
    let sttDownloadError = $state<string | null>(null);
    let sttLoadMessage = $state("Starting worker...");
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
    let syncedTtsPlaybackActive = false;
    let pendingCompletionPongRequestId = $state<number | null>(null);
    let pendingTtsSegments = $state(0);
    let queuedPlaybackChunkCount = $state(0);
    let isQueueingCompletionPong = false;
    let cachedPongPlaybackSamples = $state<Float32Array | null>(null);
    let cachedPongPlaybackSampleRate = $state<number | null>(null);
    let activePongSource = $state<AudioBufferSourceNode | null>(null);
    let activePongGainNode = $state<GainNode | null>(null);
    let contacts = $state<ContactProfile[]>([createDefaultContact()]);
    let selectedContactId = $state(DEFAULT_CONTACT_ID);
    let showContactsPopup = $state(false);
    let showConversationPopup = $state(false);
    let conversationLogEntries = $state<ConversationLogEntry[]>([]);
    let nextConversationEntryId = 1;
    let activeAssistantResponseId: number | null = null;
    let activeAssistantConversationEntryId: number | null = null;
    let callStagePhase = $state<
        | "idle"
        | "listening"
        | "processing_audio"
        | "thinking"
        | "generating_audio"
        | "speaking"
    >("idle");
    let callStageMessage = $state("");
    let screenCapturePhase = $state<ScreenCaptureEvent["phase"] | null>(null);
    let screenCaptureMessage = $state("");
    let screenCaptureHasPendingAttachment = $state(false);
    let screenCaptureFileName = $state<string | null>(null);
    let contactsImportInput: HTMLInputElement | null = null;
    let contactIconInput: HTMLInputElement | null = null;
    let selectedContactPromptSyncTimeout: ReturnType<
        typeof window.setTimeout
    > | null = null;

    const formattedTime = $derived(
        `${Math.floor(time / 60)
            .toString()
            .padStart(2, "0")}:${(time % 60).toString().padStart(2, "0")}`,
    );
    const sttUsesGemma = $derived(selectedSttModel === "gemma");
    const effectiveSttLoaded = $derived(
        sttUsesGemma ? isGemmaLoaded : isSttLoaded,
    );
    const hasLoadedModelProcess = $derived(
        isGemmaLoaded || isCsmLoaded || isSttLoaded,
    );
    const showModelMemorySummary = $derived(
        (modelMemorySnapshot?.total_bytes ?? 0) > 0,
    );
    const showScreenCaptureCard = $derived(
        calling &&
            (screenCapturePhase === "capturing" ||
                screenCapturePhase === "error" ||
                screenCaptureHasPendingAttachment),
    );
    const modelsReady = $derived(
        isGemmaLoaded && isCsmLoaded && effectiveSttLoaded,
    );
    const gemmaVariantDisabled = $derived(
        isGemmaLoaded ||
            isDownloadingGemma ||
            isClearingGemmaCache ||
            isCancellingGemmaDownload ||
            isLoadingGemma ||
            isUnloadingGemma,
    );
    const csmVariantDisabled = $derived(
        isCsmLoaded ||
            isDownloadingCsm ||
            isClearingCsmCache ||
            isCancellingCsmDownload ||
            isLoadingCsm ||
            isUnloadingCsm,
    );
    const sttVariantDisabled = $derived(
        isDownloadingStt ||
            isClearingSttCache ||
            isCancellingSttDownload ||
            isLoadingStt ||
            isUnloadingStt ||
            (selectedSttModel === "whisper_large_v3_turbo" && isSttLoaded),
    );
    let conversationLogViewport = $state<HTMLDivElement | null>(null);
    const gemmaVariantOptions: Array<{
        value: GemmaVariant;
        label: string;
    }> = [
        { value: "e4b", label: "E4B" },
        { value: "e2b", label: "E2B" },
    ];
    const csmModelOptions: Array<{
        value: CsmModelVariant;
        label: string;
    }> = [
        { value: "expressiva_1b", label: "CSM Expressiva 1B" },
        { value: "kokoro_82m", label: "Kokoro-82M" },
        { value: "cosyvoice2_0_5b", label: "CosyVoice2-0.5B" },
    ];
    const sttModelOptions: Array<{
        value: SttModelVariant;
        label: string;
    }> = [
        { value: "gemma", label: "Gemma" },
        {
            value: "whisper_large_v3_turbo",
            label: "Whisper Large V3 Turbo",
        },
    ];
    const gemmaVariantTooltip = $derived(
        selectedGemmaVariant === "e4b"
            ? "E4B uses more RAM but is generally more capable. Recommended for Macs with 24 GB+ of unified memory."
            : "E2B uses less RAM but is generally less capable. Recommended for Macs with 16 GB+ of unified memory.",
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
              : "CosyVoice2-0.5B produces high quality audio with higher usage of memory. Quantization is not used for this model.",
    );
    const selectedSttModelLabel = $derived(
        sttModelOptions.find((option) => option.value === selectedSttModel)
            ?.label ?? "Whisper Large V3 Turbo",
    );
    const sttModelTooltip = $derived(
        selectedSttModel === "gemma"
            ? "Use the loaded Gemma model for transcription. There is no separate STT model to load."
            : "Use mlx-audio with mlx-community/whisper-large-v3-turbo-asr-fp16 for transcription. Download and load it separately from Gemma.",
    );
    const csmQuantizeAvailable = $derived(selectedCsmModel === "expressiva_1b");
    const loadAllMissingDownloads = $derived(
        (() => {
            const missingModels: string[] = [];

            if (!isGemmaDownloaded) {
                missingModels.push("Gemma");
            }

            if (!isCsmDownloaded) {
                missingModels.push(selectedCsmModelLabel);
            }

            if (
                selectedSttModel === "whisper_large_v3_turbo" &&
                !isSttDownloaded
            ) {
                missingModels.push(selectedSttModelLabel);
            }

            return missingModels;
        })(),
    );
    const loadAllNeedsAction = $derived(
        !isGemmaLoaded ||
            !isCsmLoaded ||
            (selectedSttModel === "whisper_large_v3_turbo" && !isSttLoaded),
    );
    const loadAllBusy = $derived(
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
    const loadAllDisabled = $derived(loadAllBusy || !loadAllNeedsAction);
    const loadAllButtonLabel = $derived(
        isLoadingAll
            ? "Loading All..."
            : loadAllNeedsAction
              ? "Load All"
              : "All Loaded",
    );
    const loadAllButtonTitle = $derived(
        loadAllMissingDownloads.length > 0
            ? `Download and load ${loadAllMissingDownloads.join(", ")}.`
            : loadAllNeedsAction
              ? "Load the selected models."
              : "The selected models are already loaded.",
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
        selectedContact?.prompt.trim() || DEFAULT_VOICE_SYSTEM_PROMPT,
    );
    const selectedContactIconUrl = $derived(
        selectedContact?.iconDataUrl ?? "/icon.png",
    );
    const selectedContactImageStyle = $derived(
        `background-image: url('${selectedContactIconUrl}')`,
    );
    const PLAYBACK_PREBUFFER_SAMPLES = 2048;
    const PONG_VOLUME = 0.7;
    const pongUrl = "/pong.mp3";
    const screenCaptureTitle = $derived(
        screenCapturePhase === "capturing"
            ? "Select a Screen Region"
            : screenCapturePhase === "error" &&
                !screenCaptureHasPendingAttachment
              ? "Screen Capture Failed"
              : screenCapturePhase === "error"
                ? "Screen Region Unchanged"
                : "Screen Region Attached",
    );
    const screenCaptureActionLabel = $derived(
        screenCaptureHasPendingAttachment ? "Clear" : "Dismiss",
    );
    const assistantSpeaking = $derived(
        calling && callStagePhase === "speaking",
    );

    function setCallStage(
        phase:
            | "idle"
            | "listening"
            | "processing_audio"
            | "thinking"
            | "generating_audio"
            | "speaking",
        message: string,
    ) {
        callStagePhase = phase;
        callStageMessage = message;
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
        role: ConversationLogEntry["role"],
        text: string,
        imageUrl: string | null = null,
    ) {
        const normalizedText = text.trim();
        const normalizedImageUrl = imageUrl?.trim() || null;
        if (!normalizedText && !normalizedImageUrl) {
            return null;
        }

        const entryId = nextConversationEntryId;
        conversationLogEntries = [
            ...conversationLogEntries,
            {
                id: entryId,
                role,
                text: normalizedText,
                imageUrl: normalizedImageUrl,
            },
        ];
        nextConversationEntryId += 1;
        scrollConversationLogToBottom();
        return entryId;
    }

    function upsertAssistantConversationLogEntry(
        requestId: number,
        text: string,
    ) {
        const normalizedText = text.trim();
        if (!normalizedText) {
            return;
        }

        if (
            activeAssistantResponseId !== requestId ||
            activeAssistantConversationEntryId == null
        ) {
            activeAssistantResponseId = requestId;
            activeAssistantConversationEntryId = appendConversationLogEntry(
                "assistant",
                normalizedText,
            );
            return;
        }

        conversationLogEntries = conversationLogEntries.map((entry) =>
            entry.id === activeAssistantConversationEntryId
                ? { ...entry, text: normalizedText }
                : entry,
        );
        scrollConversationLogToBottom();
    }

    function resetConversationLog() {
        conversationLogEntries = [];
        nextConversationEntryId = 1;
        activeAssistantResponseId = null;
        activeAssistantConversationEntryId = null;
    }

    function resetScreenCaptureStatus() {
        screenCapturePhase = null;
        screenCaptureMessage = "";
        screenCaptureHasPendingAttachment = false;
        screenCaptureFileName = null;
    }

    function applyScreenCaptureEvent(payload: ScreenCaptureEvent) {
        const shouldShow =
            payload.phase === "capturing" ||
            payload.phase === "error" ||
            payload.hasPendingAttachment;

        if (!shouldShow) {
            resetScreenCaptureStatus();
            return;
        }

        screenCapturePhase = payload.phase;
        screenCaptureMessage = payload.message.trim();
        screenCaptureHasPendingAttachment = payload.hasPendingAttachment;
        screenCaptureFileName = payload.fileName?.trim() || null;
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
        };
        window.localStorage.setItem(
            MODEL_PREFERENCES_STORAGE_KEY,
            JSON.stringify(payload),
        );
    }

    async function restoreModelPreferences() {
        const restoredPreferences = loadModelPreferencesFromStorage();

        selectedGemmaVariant = restoredPreferences.gemmaVariant;
        selectedCsmModel = restoredPreferences.csmModel;
        selectedSttModel = restoredPreferences.sttModel;

        try {
            await invoke("set_gemma_variant", {
                variant: restoredPreferences.gemmaVariant,
            });
        } catch (err) {
            console.error("Failed to restore Gemma variant:", err);
        }

        try {
            await invoke("set_csm_model_variant", {
                variant: restoredPreferences.csmModel,
            });
        } catch (err) {
            console.error("Failed to restore speech model:", err);
        }

        try {
            await invoke("set_stt_model_variant", {
                variant: restoredPreferences.sttModel,
            });
        } catch (err) {
            console.error("Failed to restore STT model:", err);
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

    function toggleContactsPopup() {
        showContactsPopup = !showContactsPopup;
        if (showContactsPopup) {
            closeConversationPopup();
        }
    }

    function toggleConversationPopup() {
        showConversationPopup = !showConversationPopup;
        if (showConversationPopup) {
            closeContactsPopup();
            scrollConversationLogToBottom();
        }
    }

    function selectContact(contactId: string) {
        if (!contacts.some((contact) => contact.id === contactId)) {
            return;
        }

        selectedContactId = contactId;
        persistContactsMetadata();
        queueSelectedContactPromptSync();
    }

    function createNewContact() {
        const nextContact: ContactProfile = {
            id: createContactId(),
            name: `Contact ${contacts.length + 1}`,
            prompt: selectedContact?.prompt ?? DEFAULT_VOICE_SYSTEM_PROMPT,
            hasCustomIcon: false,
            iconDataUrl: null,
        };

        contacts = [...contacts, nextContact];
        selectedContactId = nextContact.id;
        persistContactsMetadata();
        showContactsPopup = true;
        queueSelectedContactPromptSync();
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

    function triggerContactImport() {
        contactsImportInput?.click();
    }

    function triggerContactIconUpload() {
        contactIconInput?.click();
    }

    async function handleContactImportChange(event: Event) {
        const input = event.currentTarget as HTMLInputElement;
        const file = input.files?.[0];
        input.value = "";

        if (!file) {
            return;
        }

        try {
            const rawText = await file.text();
            const parsed = JSON.parse(rawText) as Record<string, unknown>;
            const nextName = typeof parsed.name === "string" ? parsed.name : "";
            const nextPrompt =
                typeof parsed.prompt === "string" ? parsed.prompt : "";
            const iconDataUrl =
                typeof parsed.iconDataUrl === "string" &&
                parsed.iconDataUrl.startsWith("data:image/")
                    ? parsed.iconDataUrl
                    : null;

            if (!nextName.trim()) {
                throw new Error("The imported contact is missing a name.");
            }

            if (!nextPrompt.trim()) {
                throw new Error("The imported contact is missing a prompt.");
            }

            const nextContact: ContactProfile = {
                id: createContactId(),
                name: nextName,
                prompt: nextPrompt,
                hasCustomIcon: Boolean(iconDataUrl),
                iconDataUrl,
            };

            if (iconDataUrl) {
                await saveStoredContactIcon(nextContact.id, iconDataUrl);
            }

            contacts = [...contacts, nextContact];
            selectedContactId = nextContact.id;
            persistContactsMetadata();
            showContactsPopup = true;
            queueSelectedContactPromptSync();
        } catch (err) {
            console.error("Failed to import contact:", err);
            alert(`Failed to import contact.\n${normalizeErrorMessage(err)}`);
        }
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
                        prompt: selectedContact.prompt,
                        iconDataUrl: selectedContact.iconDataUrl,
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

    function handleWindowKeydown(event: KeyboardEvent) {
        if (event.key !== "Escape") {
            return;
        }

        if (showContactsPopup) {
            closeContactsPopup();
            return;
        }

        if (showConversationPopup) {
            closeConversationPopup();
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

    function normalizeErrorMessage(error: unknown) {
        let message = String(error).trim();

        if (message.startsWith('"') && message.endsWith('"')) {
            try {
                const parsed = JSON.parse(message);
                if (typeof parsed === "string") {
                    message = parsed;
                }
            } catch {
                // Keep the raw string when it is not valid JSON.
            }
        }

        if (message.toLowerCase().startsWith("error: ")) {
            message = message.slice(7).trim();
        }

        return message;
    }

    function normalizeDownloadErrorMessage(error: unknown) {
        const message = normalizeErrorMessage(error);

        if (message.toLowerCase().startsWith("download failed:")) {
            return message.slice("download failed:".length).trim();
        }

        return message;
    }

    function formatDownloadPercent(progress: number) {
        if (progress >= 99.95) {
            return "100%";
        }
        if (progress < 1) {
            return `${progress.toFixed(2)}%`;
        }
        if (progress < 10) {
            return `${progress.toFixed(1)}%`;
        }
        return `${Math.round(progress)}%`;
    }

    function formatMemoryUsage(bytes: number) {
        const gib = 1024 ** 3;
        const mib = 1024 ** 2;
        const kib = 1024;

        if (bytes >= gib) {
            return `${(bytes / gib).toFixed(2)} GB`;
        }
        if (bytes >= mib) {
            return `${Math.round(bytes / mib)} MB`;
        }
        if (bytes >= kib) {
            return `${Math.round(bytes / kib)} KB`;
        }
        return `${bytes} B`;
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
        modelMemoryPollInterval = window.setInterval(() => {
            void syncModelMemoryUsage();
        }, 60000);
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

        try {
            await invoke("set_gemma_variant", { variant: nextVariant });
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
            await invoke("set_csm_model_variant", { variant: nextVariant });
            resetDownloadState("csm");
            csmLoadMessage = "Starting worker...";
            await syncModelStatus();
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
            await invoke("set_stt_model_variant", { variant: nextVariant });
            resetDownloadState("stt");
            sttLoadMessage = "Starting worker...";
            await syncModelStatus();
        } catch (err) {
            selectedSttModel = previousVariant;
            console.error("Failed to update STT model:", err);
            alert(`Failed to update the STT model.\n${String(err)}`);
        }
    }

    async function syncModelStatus() {
        try {
            const [
                gemmaVariant,
                csmModelVariant,
                sttModelVariant,
                gemmaDownloaded,
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
            isGemmaLoaded = gemmaLoaded;
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

            captureProcessor.port.onmessage = (event) => {
                if (micMuted || !calling) {
                    return;
                }

                const { inputData, playbackReferenceData, playbackActive } =
                    event.data as {
                        inputData: Float32Array;
                        playbackReferenceData?: Float32Array;
                        playbackActive?: boolean;
                    };
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
        pendingCompletionPongRequestId = null;
        pendingTtsSegments = 0;
        isQueueingCompletionPong = false;
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

        try {
            await invoke("interrupt_tts");
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
            throw new Error(`Failed to fetch pong.mp3 (${response.status})`);
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

    async function playCallStartPong() {
        if (!calling || !captureContext) {
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

        try {
            await invoke("reset_call_session");
        } catch (err) {
            console.error("Failed to reset call session:", err);
        }

        closeContactsPopup();
        closeConversationPopup();
        resetConversationLog();
        resetScreenCaptureStatus();
        calling = true;
        callStartedAtMs = Date.now();
        syncCallElapsedTime();
        activeTtsRequestId = null;
        syncTtsPlaybackState(false);
        setCallStage("listening", "Listening");

        void invoke("start_call_timer", { muted: micMuted }).catch((err) =>
            console.error("Failed to start tray call timer", err),
        );
        await startAudioCapture();
        if (calling) {
            void playCallStartPong();
        }
        void invoke("ping").catch((err) =>
            console.error("Backend ping failed", err),
        );

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
        setCallStage("idle", "");
        stopCallTimerTracking();

        try {
            await invoke("reset_call_session");
        } catch (err) {
            console.error("Failed to clear call session:", err);
        }
    }

    function toggleMic() {
        micMuted = !micMuted;
        void invoke("set_call_muted", { muted: micMuted }).catch((err) =>
            console.error("Failed to sync tray mute state", err),
        );
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
            await invoke("start_server");
            for (let i = 0; i < 30; i += 1) {
                await new Promise((resolve) => setTimeout(resolve, 1000));
                if (await invoke<boolean>("is_server_running")) {
                    isGemmaLoaded = true;
                    break;
                }
            }
        } catch (err) {
            console.error("Load model failed:", err);
            alert(`Failed to load Gemma.\n${String(err)}`);
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
            alert(`Failed to cancel ${selectedCsmModelLabel} download.\n${String(err)}`);
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
            alert(`Failed to cancel ${selectedSttModelLabel} download.\n${String(err)}`);
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

            if (selectedSttModel === "whisper_large_v3_turbo") {
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
        } finally {
            isLoadingAll = false;
            await syncModelStatus();
        }
    }

    async function handleUnloadGemma() {
        isUnloadingGemma = true;
        try {
            await invoke("stop_server");
            isGemmaLoaded = false;
        } catch (err) {
            console.error("Unload Gemma failed:", err);
            alert(`Failed to unload Gemma.\n${String(err)}`);
        } finally {
            isUnloadingGemma = false;
            await syncModelStatus();
        }
    }

    async function handleUnloadCsm() {
        isUnloadingCsm = true;
        try {
            await invoke("stop_csm_server");
            isCsmLoaded = false;
        } catch (err) {
            console.error("Unload speech model failed:", err);
            alert(`Failed to unload ${selectedCsmModelLabel}.\n${String(err)}`);
        } finally {
            isUnloadingCsm = false;
            await syncModelStatus();
        }
    }

    async function handleUnloadStt() {
        isUnloadingStt = true;
        try {
            await invoke("stop_stt_server");
            isSttLoaded = false;
        } catch (err) {
            console.error("Unload STT model failed:", err);
            alert(`Failed to unload ${selectedSttModelLabel}.\n${String(err)}`);
        } finally {
            isUnloadingStt = false;
            await syncModelStatus();
        }
    }

    onMount(() => {
        void (async () => {
            const restoredContacts = await loadContactsFromStorage();
            contacts = restoredContacts.contacts;
            selectedContactId = restoredContacts.selectedContactId;
            persistContactsMetadata();
            await syncSelectedContactPrompt();
            await restoreModelPreferences();
            await syncModelStatus();
        })();

        healthCheckInterval = window.setInterval(() => {
            void syncModelStatus();
        }, 5000);

        void (async () => {
            try {
                eventUnlisteners = await Promise.all([
                    listen<CsmAudioStartEvent>(
                        "csm-audio-start",
                        ({ payload }) => {
                            if (!calling) {
                                return;
                            }

                            if (payload.request_id !== activeTtsRequestId) {
                                stopPlayback();
                                activeTtsRequestId = payload.request_id;
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
                            updateStageAfterPlaybackStateChange();
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
                        pendingCompletionPongRequestId = null;
                        stopPlayback();
                        if (calling) {
                            updateStageAfterPlaybackStateChange();
                        }
                    }),
                    listen<CsmErrorEvent>("csm-error", ({ payload }) => {
                        console.error("CSM error:", payload.message);
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
                    }),
                    listen<CsmStatusEvent>("csm-status", ({ payload }) => {
                        if (isLoadingCsm) {
                            csmLoadMessage = payload.message;
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

                            upsertAssistantConversationLogEntry(
                                payload.request_id,
                                payload.text,
                            );
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
                    listen<TranscriptEvent>(
                        "transcript-ready",
                        ({ payload }) => {
                            if (!calling) {
                                return;
                            }

                            appendConversationLogEntry(
                                "user",
                                payload.text,
                                payload.imageDataUrl ?? null,
                            );
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
                    listen<ScreenCaptureEvent>("screen-capture", ({ payload }) => {
                        applyScreenCaptureEvent(payload);
                    }),
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
                ]);
            } catch (err) {
                console.error("Failed to register Tauri event listeners:", err);
            }
        })();
    });

    onDestroy(() => {
        if (healthCheckInterval) {
            clearInterval(healthCheckInterval);
        }
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

<svelte:window onkeydown={handleWindowKeydown} onfocus={handleWindowFocus} />

<div class="app-container" class:contacts-open={showContactsPopup}>
    <div class="background" style={selectedContactImageStyle}></div>

    {#if !calling}
        <div class="model-tags" class:dimmed={showContactsPopup}>
            <div class="model-actions">
                <button
                    type="button"
                    class="utility-btn load-all-btn"
                    disabled={loadAllDisabled}
                    title={loadAllButtonTitle}
                    onclick={handleLoadAll}
                >
                    {loadAllButtonLabel}
                </button>
            </div>
            <div
                class="download-banner"
                class:ready={isGemmaDownloaded && isGemmaLoaded}
            >
                {#if isDownloadingGemma}
                    <div class="download-content">
                        <div class="banner-heading-row">
                            <span class="banner-title">Gemma</span>
                            <div class="tooltip-shell variant-select-shell">
                                <select
                                    class="variant-select"
                                    value={selectedGemmaVariant}
                                    aria-label="Gemma variant"
                                    disabled={gemmaVariantDisabled}
                                    onchange={handleGemmaVariantChange}
                                >
                                    {#each gemmaVariantOptions as option}
                                        <option value={option.value}
                                            >{option.label}</option
                                        >
                                    {/each}
                                </select>
                                <div class="tooltip-bubble variant-tooltip">
                                    {gemmaVariantTooltip}
                                </div>
                            </div>
                        </div>
                        <div class="download-row">
                            <span
                                class="download-status-text"
                                class:failed={!!gemmaDownloadError}
                                >{gemmaDownloadMessage}</span
                            >
                            {#if gemmaDownloadProgress !== null}
                                <span class="download-percent"
                                    >{formatDownloadPercent(
                                        gemmaDownloadProgress,
                                    )}</span
                                >
                            {/if}
                        </div>
                        <div class="progress-row">
                            <button
                                type="button"
                                class="progress-cancel-btn"
                                disabled={isCancellingGemmaDownload}
                                aria-label="Cancel Gemma download"
                                title={isCancellingGemmaDownload
                                    ? "Cancelling download..."
                                    : "Cancel download"}
                                onclick={handleCancelGemmaDownload}
                            >
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    width="14"
                                    height="14"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2.2"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    ><line x1="18" y1="6" x2="6" y2="18" /><line
                                        x1="6"
                                        y1="6"
                                        x2="18"
                                        y2="18"
                                    /></svg
                                >
                            </button>
                            <div class="progress-track">
                                <div
                                    class="progress-fill"
                                    class:indeterminate={gemmaDownloadIndeterminate}
                                    class:failed={!!gemmaDownloadError}
                                    style:width={gemmaDownloadIndeterminate
                                        ? "38%"
                                        : `${gemmaDownloadProgress ?? 0}%`}
                                ></div>
                            </div>
                        </div>
                    </div>
                {:else if isGemmaDownloaded}
                    <div class="banner-row">
                        {#if isGemmaLoaded}
                            <div class="banner-status">
                                <div class="banner-copy">
                                    <div class="banner-heading-row">
                                        <span class="banner-title">Gemma</span>
                                        <div
                                            class="tooltip-shell variant-select-shell"
                                        >
                                            <select
                                                class="variant-select"
                                                value={selectedGemmaVariant}
                                                aria-label="Gemma variant"
                                                disabled={gemmaVariantDisabled}
                                                onchange={handleGemmaVariantChange}
                                            >
                                                {#each gemmaVariantOptions as option}
                                                    <option value={option.value}
                                                        >{option.label}</option
                                                    >
                                                {/each}
                                            </select>
                                            <div
                                                class="tooltip-bubble variant-tooltip"
                                            >
                                                {gemmaVariantTooltip}
                                            </div>
                                        </div>
                                    </div>
                                    <span class="banner-subtitle">Loaded</span>
                                </div>
                                <div class="loaded-actions">
                                    <button
                                        class="utility-btn"
                                        disabled={isUnloadingGemma}
                                        onclick={handleUnloadGemma}
                                    >
                                        {isUnloadingGemma
                                            ? "Unloading..."
                                            : "Unload"}
                                    </button>
                                    <div class="status-icon">
                                        <svg
                                            xmlns="http://www.w3.org/2000/svg"
                                            width="18"
                                            height="18"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="#34c759"
                                            stroke-width="3"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            ><polyline
                                                points="20 6 9 17 4 12"
                                            /></svg
                                        >
                                    </div>
                                </div>
                            </div>
                        {:else}
                            <div class="banner-copy">
                                <div class="banner-heading-row">
                                    <span class="banner-title">Gemma</span>
                                    <div
                                        class="tooltip-shell variant-select-shell"
                                    >
                                        <select
                                            class="variant-select"
                                            value={selectedGemmaVariant}
                                            aria-label="Gemma variant"
                                            disabled={gemmaVariantDisabled}
                                            onchange={handleGemmaVariantChange}
                                        >
                                            {#each gemmaVariantOptions as option}
                                                <option value={option.value}
                                                    >{option.label}</option
                                                >
                                            {/each}
                                        </select>
                                        <div
                                            class="tooltip-bubble variant-tooltip"
                                        >
                                            {gemmaVariantTooltip}
                                        </div>
                                    </div>
                                </div>
                                <div class="banner-subtitle-row">
                                    <span class="banner-subtitle"
                                        >Downloaded</span
                                    >
                                    <button
                                        type="button"
                                        class="utility-btn subtitle-action-btn"
                                        disabled={isLoadingGemma ||
                                            isUnloadingGemma ||
                                            isClearingGemmaCache}
                                        onclick={handleClearGemmaCache}
                                    >
                                        {isClearingGemmaCache
                                            ? "Clearing..."
                                            : "Clear Cache"}
                                    </button>
                                </div>
                            </div>
                            <button
                                class="download-btn"
                                disabled={isLoadingGemma ||
                                    isUnloadingGemma ||
                                    isClearingGemmaCache}
                                onclick={handleLoadGemma}
                            >
                                {isLoadingGemma ? "Loading..." : "Load Model"}
                            </button>
                        {/if}
                    </div>
                {:else}
                    <div class="banner-row">
                        <div class="banner-copy">
                            <div class="banner-heading-row">
                                <span class="banner-title">Gemma</span>
                                <div class="tooltip-shell variant-select-shell">
                                    <select
                                        class="variant-select"
                                        value={selectedGemmaVariant}
                                        aria-label="Gemma variant"
                                        disabled={gemmaVariantDisabled}
                                        onchange={handleGemmaVariantChange}
                                    >
                                        {#each gemmaVariantOptions as option}
                                            <option value={option.value}
                                                >{option.label}</option
                                            >
                                        {/each}
                                    </select>
                                    <div class="tooltip-bubble variant-tooltip">
                                        {gemmaVariantTooltip}
                                    </div>
                                </div>
                            </div>
                            {#if gemmaDownloadError}
                                <span class="banner-subtitle error"
                                    >Download failed</span
                                >
                                <span class="banner-detail error"
                                    >{gemmaDownloadError}</span
                                >
                            {:else}
                                <span class="banner-subtitle"
                                    >Model not found in cache</span
                                >
                            {/if}
                        </div>
                        <button
                            class="download-btn"
                            disabled={isDownloadingGemma}
                            onclick={handleDownloadGemma}
                        >
                            {isDownloadingGemma
                                ? "Downloading..."
                                : gemmaDownloadError
                                  ? "Retry Download"
                                  : "Download Model"}
                        </button>
                    </div>
                {/if}
            </div>

            <div
                class="download-banner"
                class:ready={sttUsesGemma
                    ? isGemmaLoaded
                    : isSttDownloaded && isSttLoaded}
            >
                {#if sttUsesGemma}
                    <div class="banner-row">
                        <div class="banner-copy">
                            <div class="banner-heading-row">
                                <span class="banner-title">STT</span>
                                <div class="tooltip-shell variant-select-shell">
                                    <select
                                        class="variant-select"
                                        value={selectedSttModel}
                                        aria-label="STT model"
                                        disabled={sttVariantDisabled}
                                        onchange={handleSttModelChange}
                                    >
                                        {#each sttModelOptions as option}
                                            <option value={option.value}
                                                >{option.label}</option
                                            >
                                        {/each}
                                    </select>
                                    <div class="tooltip-bubble variant-tooltip">
                                        {sttModelTooltip}
                                    </div>
                                </div>
                            </div>
                            <span class="banner-subtitle"
                                >{isGemmaLoaded
                                    ? "Using the loaded Gemma model"
                                    : "Loads with the Gemma model above"}</span
                            >
                        </div>
                    </div>
                {:else if isDownloadingStt}
                    <div class="download-content">
                        <div class="banner-heading-row">
                            <span class="banner-title">STT</span>
                            <div class="tooltip-shell variant-select-shell">
                                <select
                                    class="variant-select"
                                    value={selectedSttModel}
                                    aria-label="STT model"
                                    disabled={sttVariantDisabled}
                                    onchange={handleSttModelChange}
                                >
                                    {#each sttModelOptions as option}
                                        <option value={option.value}
                                            >{option.label}</option
                                        >
                                    {/each}
                                </select>
                                <div class="tooltip-bubble variant-tooltip">
                                    {sttModelTooltip}
                                </div>
                            </div>
                        </div>
                        <div class="download-row">
                            <span
                                class="download-status-text"
                                class:failed={!!sttDownloadError}
                                >{selectedSttModelLabel}: {sttDownloadMessage}</span
                            >
                            {#if sttDownloadProgress !== null}
                                <span class="download-percent"
                                    >{formatDownloadPercent(
                                        sttDownloadProgress,
                                    )}</span
                                >
                            {/if}
                        </div>
                        <div class="progress-row">
                            <button
                                type="button"
                                class="progress-cancel-btn"
                                disabled={isCancellingSttDownload}
                                aria-label="Cancel STT download"
                                title={isCancellingSttDownload
                                    ? "Cancelling download..."
                                    : "Cancel download"}
                                onclick={handleCancelSttDownload}
                            >
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    width="14"
                                    height="14"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2.2"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    ><line x1="18" y1="6" x2="6" y2="18" /><line
                                        x1="6"
                                        y1="6"
                                        x2="18"
                                        y2="18"
                                    /></svg
                                >
                            </button>
                            <div class="progress-track">
                                <div
                                    class="progress-fill"
                                    class:indeterminate={sttDownloadIndeterminate}
                                    class:failed={!!sttDownloadError}
                                    style:width={sttDownloadIndeterminate
                                        ? "38%"
                                        : `${sttDownloadProgress ?? 0}%`}
                                ></div>
                            </div>
                        </div>
                    </div>
                {:else if isSttDownloaded}
                    <div class="banner-row">
                        {#if isSttLoaded}
                            <div class="banner-status">
                                <div class="banner-copy">
                                    <div class="banner-heading-row">
                                        <span class="banner-title">STT</span>
                                        <div
                                            class="tooltip-shell variant-select-shell"
                                        >
                                            <select
                                                class="variant-select"
                                                value={selectedSttModel}
                                                aria-label="STT model"
                                                disabled={sttVariantDisabled}
                                                onchange={handleSttModelChange}
                                            >
                                                {#each sttModelOptions as option}
                                                    <option value={option.value}
                                                        >{option.label}</option
                                                    >
                                                {/each}
                                            </select>
                                            <div
                                                class="tooltip-bubble variant-tooltip"
                                            >
                                                {sttModelTooltip}
                                            </div>
                                        </div>
                                    </div>
                                    <span class="banner-subtitle">Loaded</span>
                                </div>
                                <div class="loaded-actions">
                                    <button
                                        class="utility-btn"
                                        disabled={isUnloadingStt}
                                        onclick={handleUnloadStt}
                                    >
                                        {isUnloadingStt
                                            ? "Unloading..."
                                            : "Unload"}
                                    </button>
                                    <div class="status-icon">
                                        <svg
                                            xmlns="http://www.w3.org/2000/svg"
                                            width="18"
                                            height="18"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="#34c759"
                                            stroke-width="3"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            ><polyline
                                                points="20 6 9 17 4 12"
                                            /></svg
                                        >
                                    </div>
                                </div>
                            </div>
                        {:else}
                            <div class="banner-copy">
                                <div class="banner-heading-row">
                                    <span class="banner-title">STT</span>
                                    <div
                                        class="tooltip-shell variant-select-shell"
                                    >
                                        <select
                                            class="variant-select"
                                            value={selectedSttModel}
                                            aria-label="STT model"
                                            disabled={sttVariantDisabled}
                                            onchange={handleSttModelChange}
                                        >
                                            {#each sttModelOptions as option}
                                                <option value={option.value}
                                                    >{option.label}</option
                                                >
                                            {/each}
                                        </select>
                                        <div
                                            class="tooltip-bubble variant-tooltip"
                                        >
                                            {sttModelTooltip}
                                        </div>
                                    </div>
                                </div>
                                {#if isLoadingStt}
                                    <span class="banner-subtitle"
                                        >{sttLoadMessage}</span
                                    >
                                {:else}
                                    <div class="banner-subtitle-row">
                                        <span class="banner-subtitle"
                                            >Downloaded</span
                                        >
                                        <button
                                            type="button"
                                            class="utility-btn subtitle-action-btn"
                                            disabled={isLoadingStt ||
                                                isUnloadingStt ||
                                                isClearingSttCache}
                                            onclick={handleClearSttCache}
                                        >
                                            {isClearingSttCache
                                                ? "Clearing..."
                                                : "Clear Cache"}
                                        </button>
                                    </div>
                                {/if}
                            </div>
                            <button
                                class="download-btn"
                                disabled={isLoadingStt ||
                                    isUnloadingStt ||
                                    isClearingSttCache}
                                onclick={handleLoadStt}
                            >
                                {isLoadingStt ? "Loading..." : "Load Model"}
                            </button>
                        {/if}
                    </div>
                {:else}
                    <div class="banner-row">
                        <div class="banner-copy">
                            <div class="banner-heading-row">
                                <span class="banner-title">STT</span>
                                <div class="tooltip-shell variant-select-shell">
                                    <select
                                        class="variant-select"
                                        value={selectedSttModel}
                                        aria-label="STT model"
                                        disabled={sttVariantDisabled}
                                        onchange={handleSttModelChange}
                                    >
                                        {#each sttModelOptions as option}
                                            <option value={option.value}
                                                >{option.label}</option
                                            >
                                        {/each}
                                    </select>
                                    <div class="tooltip-bubble variant-tooltip">
                                        {sttModelTooltip}
                                    </div>
                                </div>
                            </div>
                            {#if sttDownloadError}
                                <span class="banner-subtitle error"
                                    >Download failed</span
                                >
                                <span class="banner-detail error"
                                    >{sttDownloadError}</span
                                >
                            {:else}
                                <span class="banner-subtitle"
                                    >Model not found in cache</span
                                >
                            {/if}
                        </div>
                        <button
                            class="download-btn"
                            disabled={isDownloadingStt}
                            onclick={handleDownloadStt}
                        >
                            {isDownloadingStt
                                ? "Downloading..."
                                : sttDownloadError
                                  ? "Retry Download"
                                  : "Download Model"}
                        </button>
                    </div>
                {/if}
            </div>

            <div
                class="download-banner voice-config-banner"
                class:ready={isCsmDownloaded && isCsmLoaded}
            >
                {#if isDownloadingCsm}
                    <div class="download-content">
                        <div class="banner-heading-row">
                            <span class="banner-title">Speech</span>
                            <div class="tooltip-shell variant-select-shell">
                                <select
                                    class="variant-select"
                                    value={selectedCsmModel}
                                    aria-label="Speech model"
                                    disabled={csmVariantDisabled}
                                    onchange={handleCsmModelChange}
                                >
                                    {#each csmModelOptions as option}
                                        <option value={option.value}
                                            >{option.label}</option
                                        >
                                    {/each}
                                </select>
                                <div class="tooltip-bubble variant-tooltip">
                                    {csmModelTooltip}
                                </div>
                            </div>
                        </div>
                        <div class="download-row">
                            <span
                                class="download-status-text"
                                class:failed={!!csmDownloadError}
                                >{selectedCsmModelLabel}: {csmDownloadMessage}</span
                            >
                            {#if csmDownloadProgress !== null}
                                <span class="download-percent"
                                    >{formatDownloadPercent(
                                        csmDownloadProgress,
                                    )}</span
                                >
                            {/if}
                        </div>
                        <div class="progress-row">
                            <button
                                type="button"
                                class="progress-cancel-btn"
                                disabled={isCancellingCsmDownload}
                                aria-label="Cancel speech model download"
                                title={isCancellingCsmDownload
                                    ? "Cancelling download..."
                                    : "Cancel download"}
                                onclick={handleCancelCsmDownload}
                            >
                                <svg
                                    xmlns="http://www.w3.org/2000/svg"
                                    width="14"
                                    height="14"
                                    viewBox="0 0 24 24"
                                    fill="none"
                                    stroke="currentColor"
                                    stroke-width="2.2"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                    ><line x1="18" y1="6" x2="6" y2="18" /><line
                                        x1="6"
                                        y1="6"
                                        x2="18"
                                        y2="18"
                                    /></svg
                                >
                            </button>
                            <div class="progress-track">
                                <div
                                    class="progress-fill"
                                    class:indeterminate={csmDownloadIndeterminate}
                                    class:failed={!!csmDownloadError}
                                    style:width={csmDownloadIndeterminate
                                        ? "38%"
                                        : `${csmDownloadProgress ?? 0}%`}
                                ></div>
                            </div>
                        </div>
                    </div>
                {:else if isCsmDownloaded}
                    <div class="banner-row">
                        {#if isCsmLoaded}
                            <div class="banner-status">
                                <div class="banner-copy">
                                    <div class="banner-heading-row">
                                        <span class="banner-title">Speech</span>
                                        <div
                                            class="tooltip-shell variant-select-shell"
                                        >
                                            <select
                                                class="variant-select"
                                                value={selectedCsmModel}
                                                aria-label="Speech model"
                                                disabled={csmVariantDisabled}
                                                onchange={handleCsmModelChange}
                                            >
                                                {#each csmModelOptions as option}
                                                    <option value={option.value}
                                                        >{option.label}</option
                                                    >
                                                {/each}
                                            </select>
                                            <div
                                                class="tooltip-bubble variant-tooltip"
                                            >
                                                {csmModelTooltip}
                                            </div>
                                        </div>
                                    </div>
                                    <span class="banner-subtitle">Loaded</span>
                                </div>
                                <div class="loaded-actions">
                                    <button
                                        class="utility-btn"
                                        disabled={isUnloadingCsm}
                                        onclick={handleUnloadCsm}
                                    >
                                        {isUnloadingCsm
                                            ? "Unloading..."
                                            : "Unload"}
                                    </button>
                                    <div class="status-icon">
                                        <svg
                                            xmlns="http://www.w3.org/2000/svg"
                                            width="18"
                                            height="18"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="#34c759"
                                            stroke-width="3"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            ><polyline
                                                points="20 6 9 17 4 12"
                                            /></svg
                                        >
                                    </div>
                                </div>
                            </div>
                        {:else}
                            <div class="banner-copy">
                                <div class="banner-heading-row">
                                    <span class="banner-title">Speech</span>
                                    <div
                                        class="tooltip-shell variant-select-shell"
                                    >
                                        <select
                                            class="variant-select"
                                            value={selectedCsmModel}
                                            aria-label="Speech model"
                                            disabled={csmVariantDisabled}
                                            onchange={handleCsmModelChange}
                                        >
                                            {#each csmModelOptions as option}
                                                <option value={option.value}
                                                    >{option.label}</option
                                                >
                                            {/each}
                                        </select>
                                        <div
                                            class="tooltip-bubble variant-tooltip"
                                        >
                                            {csmModelTooltip}
                                        </div>
                                    </div>
                                    {#if csmQuantizeAvailable}
                                        <div class="tooltip-shell">
                                            <button
                                                type="button"
                                                class="quantize-toggle"
                                                class:active={isCsmQuantized}
                                                disabled={isLoadingCsm ||
                                                    isDownloadingCsm ||
                                                    isClearingCsmCache ||
                                                    isUpdatingCsmQuantize}
                                                onclick={handleCsmQuantizeToggle}
                                            >
                                                <span class="quantize-dot"
                                                ></span>
                                                <span>Quantize</span>
                                            </button>
                                            <div class="tooltip-bubble">
                                                This can speed up the audio
                                                generation but lose the quality.
                                            </div>
                                        </div>
                                    {/if}
                                </div>
                                {#if isLoadingCsm}
                                    <span class="banner-subtitle"
                                        >{csmLoadMessage}</span
                                    >
                                {:else}
                                    <div class="banner-subtitle-row">
                                        <span class="banner-subtitle"
                                            >Downloaded</span
                                        >
                                        <button
                                            type="button"
                                            class="utility-btn subtitle-action-btn"
                                            disabled={isLoadingCsm ||
                                                isUnloadingCsm ||
                                                isClearingCsmCache}
                                            onclick={handleClearCsmCache}
                                        >
                                            {isClearingCsmCache
                                                ? "Clearing..."
                                                : "Clear Cache"}
                                        </button>
                                    </div>
                                {/if}
                            </div>
                            <button
                                class="download-btn"
                                disabled={isLoadingCsm ||
                                    isUnloadingCsm ||
                                    isClearingCsmCache}
                                onclick={handleLoadCsm}
                            >
                                {isLoadingCsm ? "Loading..." : "Load Model"}
                            </button>
                        {/if}
                    </div>
                {:else}
                    <div class="banner-row">
                        <div class="banner-copy">
                            <div class="banner-heading-row">
                                <span class="banner-title">Speech</span>
                                <div class="tooltip-shell variant-select-shell">
                                    <select
                                        class="variant-select"
                                        value={selectedCsmModel}
                                        aria-label="Speech model"
                                        disabled={csmVariantDisabled}
                                        onchange={handleCsmModelChange}
                                    >
                                        {#each csmModelOptions as option}
                                            <option value={option.value}
                                                >{option.label}</option
                                            >
                                        {/each}
                                    </select>
                                    <div class="tooltip-bubble variant-tooltip">
                                        {csmModelTooltip}
                                    </div>
                                </div>
                                {#if csmQuantizeAvailable}
                                    <div class="tooltip-shell">
                                        <button
                                            type="button"
                                            class="quantize-toggle"
                                            class:active={isCsmQuantized}
                                            disabled={isDownloadingCsm ||
                                                isClearingCsmCache ||
                                                isUpdatingCsmQuantize}
                                            onclick={handleCsmQuantizeToggle}
                                        >
                                            <span class="quantize-dot"></span>
                                            <span>Quantize</span>
                                        </button>
                                        <div class="tooltip-bubble">
                                            This can speed up the audio
                                            generation but lose the quality.
                                        </div>
                                    </div>
                                {/if}
                            </div>
                            {#if csmDownloadError}
                                <span class="banner-subtitle error"
                                    >Download failed</span
                                >
                                <span class="banner-detail error"
                                    >{csmDownloadError}</span
                                >
                            {:else}
                                <span class="banner-subtitle"
                                    >Model not found in cache</span
                                >
                            {/if}
                        </div>
                        <button
                            class="download-btn"
                            disabled={isDownloadingCsm}
                            onclick={handleDownloadCsm}
                        >
                            {isDownloadingCsm
                                ? "Downloading..."
                                : csmDownloadError
                                  ? "Retry Download"
                                  : "Download Model"}
                        </button>
                    </div>
                {/if}
            </div>
        </div>
    {/if}

    <main class:idle-layout={!calling}>
        {#if calling && callStageMessage}
            <div class="call-stage-banner" data-phase={callStagePhase}>
                <span class="call-stage-dot"></span>
                <span>{callStageMessage}</span>
            </div>
        {/if}
        <div class="avatar-container">
            <div
                class="avatar"
                class:calling
                style={selectedContactImageStyle}
            ></div>
        </div>
    </main>

    <div class="control-bar-wrapper">
        {#if showModelMemorySummary && modelMemorySnapshot}
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
                <div class="screen-capture-copy">
                    <span class="screen-capture-title"
                        >{screenCaptureTitle}</span
                    >
                    {#if screenCaptureFileName && screenCaptureHasPendingAttachment}
                        <span class="screen-capture-file"
                            >{screenCaptureFileName}</span
                        >
                    {/if}
                    {#if screenCaptureMessage}
                        <span class="screen-capture-detail"
                            >{screenCaptureMessage}</span
                        >
                    {/if}
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
        {/if}

        {#if showContactsPopup}
            <button
                type="button"
                class="contacts-modal-backdrop"
                aria-label="Close contacts"
                onclick={closeContactsPopup}
            ></button>
            <div
                id="contacts-popup"
                class="contacts-popup"
                role="dialog"
                aria-label="Contacts"
                aria-modal="true"
            >
                <div class="contacts-popup-header">
                    <div class="contacts-popup-copy">
                        <span class="contacts-popup-title">Contacts</span>
                        <span class="contacts-popup-subtitle"
                            >Switch the active voice persona</span
                        >
                    </div>
                    <button
                        type="button"
                        class="conversation-close-btn"
                        onclick={closeContactsPopup}
                        aria-label="Close contacts"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            width="18"
                            height="18"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            ><line x1="18" y1="6" x2="6" y2="18" /><line
                                x1="6"
                                y1="6"
                                x2="18"
                                y2="18"
                            /></svg
                        >
                    </button>
                </div>

                <div class="contacts-popup-body">
                    <div class="contacts-sidebar">
                        <div class="contacts-list">
                            {#each contacts as contact (contact.id)}
                                <button
                                    type="button"
                                    class="contact-list-item"
                                    class:active={contact.id ===
                                        selectedContact?.id}
                                    onclick={() => selectContact(contact.id)}
                                >
                                    <span class="contact-list-name"
                                        >{getContactDisplayName(contact)}</span
                                    >
                                </button>
                            {/each}
                        </div>

                        <div class="contacts-sidebar-actions">
                            <button
                                type="button"
                                class="utility-btn"
                                onclick={triggerContactImport}
                            >
                                Import
                            </button>
                            <button
                                type="button"
                                class="utility-btn"
                                onclick={createNewContact}
                            >
                                Add
                            </button>
                        </div>
                    </div>

                    <div class="contacts-editor">
                        <div class="contacts-editor-scroll">
                            <div class="contacts-editor-top">
                                <button
                                    type="button"
                                    class="contact-icon-picker"
                                    onclick={triggerContactIconUpload}
                                    aria-label="Upload contact icon"
                                >
                                    <img src={selectedContactIconUrl} alt="" />
                                </button>

                                <div class="contacts-editor-copy">
                                    <span class="contacts-editor-name"
                                        >{selectedContactName}</span
                                    >
                                    <span class="contacts-editor-hint"
                                        >Click the icon to upload a contact
                                        photo. The duck icon stays as fallback.</span
                                    >
                                    {#if selectedContact?.hasCustomIcon}
                                        <button
                                            type="button"
                                            class="utility-btn subtitle-action-btn contact-inline-btn"
                                            onclick={handleResetSelectedContactIcon}
                                        >
                                            Reset Icon
                                        </button>
                                    {/if}
                                </div>
                            </div>

                            <label class="contact-field">
                                <span class="contact-field-label">Name</span>
                                <input
                                    class="contact-text-input"
                                    type="text"
                                    value={selectedContact?.name ?? ""}
                                    placeholder="Contact name"
                                    oninput={handleSelectedContactNameInput}
                                />
                            </label>

                            <label class="contact-field contact-field-grow">
                                <span class="contact-field-label">Prompt</span>
                                <textarea
                                    class="contact-textarea"
                                    rows="6"
                                    placeholder="Describe how this contact should respond."
                                    value={selectedContact?.prompt ?? ""}
                                    oninput={handleSelectedContactPromptInput}
                                ></textarea>
                            </label>
                        </div>

                        <div class="contacts-editor-actions">
                            <button
                                type="button"
                                class="utility-btn"
                                disabled={contacts.length === 1}
                                onclick={handleDeleteSelectedContact}
                            >
                                Delete
                            </button>
                            <button
                                type="button"
                                class="download-btn"
                                onclick={handleExportSelectedContact}
                            >
                                Export
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        {/if}

        {#if calling && showConversationPopup}
            <div
                id="conversation-log-popup"
                class="conversation-popup"
                role="dialog"
                aria-label="Conversation log"
                aria-modal="false"
            >
                <div class="conversation-popup-header">
                    <div class="conversation-popup-copy">
                        <span class="conversation-popup-title"
                            >Conversation</span
                        >
                        <span class="conversation-popup-subtitle"
                            >Live call transcript</span
                        >
                    </div>
                    <button
                        type="button"
                        class="conversation-close-btn"
                        onclick={closeConversationPopup}
                        aria-label="Close conversation log"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            width="18"
                            height="18"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2.2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            ><line x1="18" y1="6" x2="6" y2="18" /><line
                                x1="6"
                                y1="6"
                                x2="18"
                                y2="18"
                            /></svg
                        >
                    </button>
                </div>

                <div
                    class="conversation-log"
                    bind:this={conversationLogViewport}
                >
                    {#if conversationLogEntries.length === 0}
                        <div class="conversation-empty">
                            Start talking and the transcript will appear here.
                        </div>
                    {:else}
                        {#each conversationLogEntries as entry (entry.id)}
                            <div
                                class="conversation-entry"
                                data-role={entry.role}
                            >
                                <div
                                    class="conversation-bubble"
                                    data-role={entry.role}
                                >
                                    {#if entry.imageUrl}
                                        <img
                                            class="conversation-attachment-image"
                                            src={entry.imageUrl}
                                            alt="Attached screen capture"
                                            loading="lazy"
                                        />
                                    {/if}
                                    {#if entry.text}
                                        <div class="conversation-entry-text">
                                            {entry.text}
                                        </div>
                                    {/if}
                                </div>
                            </div>
                        {/each}
                    {/if}
                </div>
            </div>
        {/if}

        <div class="control-bar">
            <div class="info">
                <div class="username">{selectedContactName}</div>
                <div class="timer">{calling ? formattedTime : "Ready"}</div>
            </div>

            <div class="actions">
                {#if !calling}
                    <button
                        type="button"
                        class="icon-btn"
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
                {/if}
                {#if calling}
                    <button
                        type="button"
                        class="icon-btn"
                        class:active={showConversationPopup}
                        onclick={toggleConversationPopup}
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
                        class="icon-btn"
                        class:active={!micMuted}
                        type="button"
                        onclick={toggleMic}
                        aria-label={micMuted
                            ? "Unmute microphone"
                            : "Mute microphone"}
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
                                /><line x1="12" y1="19" x2="12" y2="23" /><line
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
                                /><path d="M19 10v2a7 7 0 0 1-14 0v-2" /><line
                                    x1="12"
                                    y1="19"
                                    x2="12"
                                    y2="23"
                                /><line x1="8" y1="23" x2="16" y2="23" /></svg
                            >
                        {/if}
                    </button>
                    <button
                        type="button"
                        class="icon-btn interrupt-btn"
                        disabled={!assistantSpeaking}
                        onclick={handleInterruptTts}
                        aria-label="Interrupt assistant speech"
                        title={assistantSpeaking
                            ? "Interrupt assistant speech"
                            : "Interrupt is available while the assistant is speaking"}
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
                            <path d="M15.5 11V6a1.5 1.5 0 0 1 3 0v7a6 6 0 0 1-6 6H11a5 5 0 0 1-4.64-3.14l-1.1-2.64a1.5 1.5 0 0 1 2.72-1.26L9.5 14V11" />
                        </svg>
                    </button>
                {/if}
            </div>

            {#if calling}
                <button class="end-btn" onclick={handleEndCall}>End</button>
            {:else}
                <div class="tooltip-shell start-call-tooltip-shell">
                    <button
                        class="start-btn"
                        disabled={!modelsReady}
                        onclick={handleStartCall}>Call</button
                    >
                    {#if !modelsReady}
                        <div
                            id="start-call-tooltip"
                            class="tooltip-bubble start-call-tooltip"
                        >
                            Gemma, STT, and Speech models must be all loaded to
                            start the call.
                        </div>
                    {/if}
                </div>
            {/if}
        </div>
    </div>

    <input
        class="hidden-file-input"
        type="file"
        accept="application/json,.json"
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
</div>

<style>
    :global(body) {
        margin: 0;
        padding: 0;
        overflow: hidden;
        font-family:
            -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue",
            Arial, sans-serif;
        height: 100vh;
        width: 100vw;
        background: #000;
    }

    .app-container {
        position: relative;
        width: 100%;
        height: 100vh;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
    }

    .background {
        position: absolute;
        top: 0;
        left: 0;
        width: 100%;
        height: 100%;
        background-image: url("/icon.png");
        background-size: cover;
        background-position: center;
        filter: blur(60px) brightness(0.6) saturate(1.2);
        transform: scale(1.2);
        z-index: -1;
    }

    main {
        flex: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        gap: 28px;
        width: 100%;
        box-sizing: border-box;
        padding: 0 24px;
    }

    main.idle-layout {
        padding-top: clamp(144px, 24vh, 196px);
    }

    .call-stage-banner {
        display: flex;
        align-items: center;
        gap: 10px;
        padding: 10px 16px;
        border-radius: 999px;
        background: rgba(28, 28, 30, 0.78);
        border: 1px solid rgba(255, 255, 255, 0.1);
        color: rgba(255, 255, 255, 0.92);
        backdrop-filter: blur(14px);
        font-size: 0.98rem;
        font-weight: 600;
        letter-spacing: -0.01em;
        box-shadow: 0 10px 30px rgba(0, 0, 0, 0.28);
    }

    .call-stage-dot {
        width: 10px;
        height: 10px;
        border-radius: 50%;
        background: #7fe37c;
        box-shadow: 0 0 0 0 rgba(127, 227, 124, 0.5);
        animation: callPulse 1.4s ease-out infinite;
    }

    .call-stage-banner[data-phase="processing_audio"] .call-stage-dot {
        background: #ffd25f;
        box-shadow: 0 0 0 0 rgba(255, 210, 95, 0.5);
    }

    .call-stage-banner[data-phase="thinking"] .call-stage-dot {
        background: #7cc8ff;
        box-shadow: 0 0 0 0 rgba(124, 200, 255, 0.5);
    }

    .call-stage-banner[data-phase="generating_audio"] .call-stage-dot {
        background: #ff9f68;
        box-shadow: 0 0 0 0 rgba(255, 159, 104, 0.5);
    }

    .call-stage-banner[data-phase="speaking"] .call-stage-dot {
        background: #7fe37c;
        box-shadow: 0 0 0 0 rgba(127, 227, 124, 0.5);
    }

    .avatar {
        width: 140px;
        height: 140px;
        border-radius: 50%;
        background-image: url("/icon.png");
        background-size: cover;
        background-position: center;
        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.4);
        transition: box-shadow 0.5s ease-in-out;
    }

    .avatar.calling {
        box-shadow: 0 0 60px rgba(255, 215, 0, 0.4);
    }

    .model-tags {
        position: absolute;
        top: 40px;
        display: flex;
        flex-direction: column;
        align-items: center;
        width: min(calc(100vw - 48px), 560px);
        z-index: 8;
        isolation: isolate;
        transition:
            opacity 0.22s ease,
            filter 0.22s ease,
            transform 0.22s ease;
    }

    .model-tags.dimmed {
        opacity: 0.22;
        filter: blur(6px);
        transform: translateY(-6px) scale(0.985);
        pointer-events: none;
    }

    .model-actions {
        display: flex;
        justify-content: flex-end;
        width: 100%;
        margin-bottom: 12px;
    }

    .load-all-btn {
        min-width: 112px;
        background: rgba(127, 227, 124, 0.12);
        border-color: rgba(127, 227, 124, 0.28);
        color: #c9f7c8;
    }

    .load-all-btn:hover:not(:disabled) {
        background: rgba(127, 227, 124, 0.18);
        border-color: rgba(127, 227, 124, 0.34);
    }

    .control-bar-wrapper {
        position: absolute;
        bottom: 40px;
        width: 100%;
        display: flex;
        flex-direction: column;
        justify-content: center;
        align-items: center;
        gap: 10px;
        padding: 0 20px;
        z-index: 20;
    }

    .model-memory-pill {
        display: inline-flex;
        align-items: center;
        gap: 10px;
        padding: 8px 14px;
        border-radius: 999px;
        background: rgba(32, 31, 24, 0.88);
        border: 1px solid rgba(255, 220, 102, 0.14);
        box-shadow:
            0 10px 28px rgba(0, 0, 0, 0.28),
            0 0 0 1px rgba(255, 255, 255, 0.03) inset;
        backdrop-filter: blur(16px);
    }

    .model-memory-pill-label {
        color: rgba(255, 255, 255, 0.62);
        font-size: 0.72rem;
        font-weight: 700;
        letter-spacing: 0.1em;
        text-transform: uppercase;
    }

    .model-memory-pill-value {
        color: #fff6d7;
        font-size: 0.96rem;
        font-weight: 700;
        letter-spacing: -0.02em;
    }

    .screen-capture-card {
        width: min(560px, calc(100vw - 40px));
        box-sizing: border-box;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
        padding: 14px 16px;
        border-radius: 24px;
        background: rgba(23, 31, 39, 0.9);
        border: 1px solid rgba(116, 184, 255, 0.2);
        box-shadow:
            0 12px 32px rgba(0, 0, 0, 0.28),
            0 0 0 1px rgba(255, 255, 255, 0.03) inset;
        backdrop-filter: blur(18px);
    }

    .screen-capture-card[data-phase="capturing"] {
        border-color: rgba(255, 211, 107, 0.3);
        background: rgba(39, 30, 16, 0.92);
    }

    .screen-capture-card[data-phase="error"] {
        border-color: rgba(255, 127, 127, 0.28);
        background: rgba(43, 21, 21, 0.92);
    }

    .screen-capture-copy {
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 4px;
    }

    .screen-capture-title {
        color: rgba(255, 255, 255, 0.94);
        font-size: 0.95rem;
        font-weight: 700;
        letter-spacing: -0.01em;
    }

    .screen-capture-file {
        color: rgba(187, 223, 255, 0.88);
        font-size: 0.8rem;
        font-weight: 600;
        letter-spacing: 0.01em;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .screen-capture-detail {
        color: rgba(255, 255, 255, 0.72);
        font-size: 0.82rem;
        line-height: 1.35;
    }

    .control-bar {
        background-color: #2c2c2e;
        border-radius: 32px;
        padding: 14px 28px;
        display: flex;
        align-items: center;
        gap: 36px;
        box-shadow: 0 12px 40px rgba(0, 0, 0, 0.5);
        width: auto;
        min-width: min(440px, calc(100vw - 40px));
        max-width: calc(100vw - 40px);
        border: 1px solid rgba(255, 255, 255, 0.05);
        position: relative;
        z-index: 2;
    }

    .info {
        display: flex;
        flex-direction: column;
        color: white;
        min-width: 140px;
    }

    .username {
        font-weight: 600;
        font-size: 1.15rem;
        letter-spacing: -0.01em;
    }

    .timer {
        font-size: 1rem;
        opacity: 0.6;
        margin-top: 2px;
    }

    .actions {
        display: flex;
        gap: 14px;
        flex: 1;
        justify-content: center;
        flex-wrap: wrap;
    }

    .conversation-popup {
        position: absolute;
        left: 50%;
        bottom: calc(100% + 18px);
        transform: translateX(-50%);
        width: min(calc(100vw - 32px), 420px);
        max-height: min(52vh, 420px);
        display: flex;
        flex-direction: column;
        gap: 14px;
        padding: 18px;
        border-radius: 24px;
        background: linear-gradient(
            180deg,
            rgba(34, 31, 16, 0.94) 0%,
            rgba(18, 18, 20, 0.94) 100%
        );
        border: 1px solid rgba(255, 220, 102, 0.16);
        box-shadow:
            0 28px 70px rgba(0, 0, 0, 0.42),
            0 0 0 1px rgba(255, 255, 255, 0.03) inset;
        backdrop-filter: blur(22px);
        box-sizing: border-box;
        z-index: 21;
    }

    .contacts-modal-backdrop {
        position: fixed;
        inset: 0;
        border: none;
        background: rgba(7, 7, 9, 0.48);
        backdrop-filter: blur(18px);
        cursor: pointer;
        z-index: 22;
    }

    .contacts-popup {
        position: fixed;
        top: 50%;
        left: 50%;
        transform: translate(-50%, -50%);
        width: min(1080px, calc(100vw - 56px));
        height: min(780px, calc(100vh - 56px));
        max-height: calc(100vh - 56px);
        display: flex;
        flex-direction: column;
        gap: 20px;
        padding: 24px;
        border-radius: 30px;
        background: linear-gradient(
            180deg,
            rgba(38, 34, 20, 0.96) 0%,
            rgba(17, 17, 19, 0.97) 100%
        );
        border: 1px solid rgba(255, 220, 102, 0.15);
        box-shadow:
            0 34px 80px rgba(0, 0, 0, 0.5),
            0 0 0 1px rgba(255, 255, 255, 0.03) inset;
        backdrop-filter: blur(22px);
        box-sizing: border-box;
        overflow: hidden;
        z-index: 24;
    }

    .contacts-popup-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
    }

    .contacts-popup-copy {
        display: flex;
        flex-direction: column;
        gap: 4px;
        min-width: 0;
    }

    .contacts-popup-title {
        color: rgba(255, 246, 214, 0.96);
        font-size: 1rem;
        font-weight: 700;
        letter-spacing: -0.02em;
    }

    .contacts-popup-subtitle {
        color: rgba(255, 255, 255, 0.56);
        font-size: 0.85rem;
        letter-spacing: -0.01em;
    }

    .contacts-popup-body {
        display: grid;
        grid-template-columns: minmax(240px, 300px) minmax(0, 1fr);
        gap: 18px;
        flex: 1;
        min-height: 0;
        overflow: hidden;
    }

    .contacts-sidebar {
        display: flex;
        flex-direction: column;
        gap: 14px;
        min-height: 0;
        padding: 16px;
        border-radius: 24px;
        background: rgba(255, 255, 255, 0.035);
        border: 1px solid rgba(255, 255, 255, 0.06);
    }

    .contacts-list {
        display: flex;
        flex-direction: column;
        gap: 10px;
        flex: 1;
        min-height: 0;
        overflow-y: auto;
        padding-right: 6px;
    }

    .contact-list-item {
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 16px;
        background: rgba(255, 255, 255, 0.035);
        color: rgba(255, 255, 255, 0.86);
        padding: 14px 16px;
        text-align: left;
        font-size: 0.96rem;
        font-weight: 600;
        letter-spacing: -0.015em;
        cursor: pointer;
        transition:
            background-color 0.2s ease,
            border-color 0.2s ease,
            color 0.2s ease,
            transform 0.1s ease;
    }

    .contact-list-item:hover {
        background: rgba(255, 255, 255, 0.08);
        border-color: rgba(255, 255, 255, 0.12);
    }

    .contact-list-item:active {
        transform: scale(0.98);
    }

    .contact-list-item.active {
        background: linear-gradient(135deg, #ffdf63 0%, #ffcd40 100%);
        border-color: rgba(255, 220, 102, 0.5);
        color: #2f2500;
        box-shadow: 0 10px 24px rgba(255, 205, 64, 0.18);
    }

    .contact-list-name {
        display: block;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .contacts-sidebar-actions {
        display: flex;
        gap: 10px;
        margin-top: auto;
    }

    .contacts-sidebar-actions .utility-btn {
        flex: 1;
    }

    .contacts-editor {
        display: flex;
        flex-direction: column;
        gap: 16px;
        min-width: 0;
        min-height: 0;
        padding: 18px 20px 18px;
        border-radius: 24px;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.06);
        overflow: hidden;
    }

    .contacts-editor-scroll {
        display: flex;
        flex: 1;
        flex-direction: column;
        gap: 16px;
        min-height: 0;
        overflow-y: auto;
        padding-right: 6px;
    }

    .contacts-editor-top {
        display: flex;
        align-items: center;
        gap: 18px;
    }

    .contact-icon-picker {
        width: 112px;
        height: 112px;
        border: 2px solid rgba(255, 255, 255, 0.1);
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.04);
        overflow: hidden;
        padding: 0;
        cursor: pointer;
        flex-shrink: 0;
        transition:
            transform 0.1s ease,
            border-color 0.2s ease,
            box-shadow 0.2s ease;
    }

    .contact-icon-picker:hover {
        border-color: rgba(255, 220, 102, 0.26);
        box-shadow: 0 0 0 6px rgba(255, 220, 102, 0.08);
    }

    .contact-icon-picker:active {
        transform: scale(0.97);
    }

    .contact-icon-picker img {
        width: 100%;
        height: 100%;
        object-fit: cover;
        display: block;
    }

    .contacts-editor-copy {
        display: flex;
        flex-direction: column;
        gap: 8px;
        min-width: 0;
    }

    .contacts-editor-name {
        color: rgba(255, 246, 214, 0.96);
        font-size: 1.18rem;
        font-weight: 700;
        letter-spacing: -0.02em;
    }

    .contacts-editor-hint {
        color: rgba(255, 255, 255, 0.58);
        font-size: 0.92rem;
        line-height: 1.35;
        letter-spacing: -0.01em;
        max-width: 44ch;
    }

    .contact-field {
        display: flex;
        flex-direction: column;
        gap: 8px;
        min-width: 0;
    }

    .contact-field-grow {
        flex: 1;
        min-height: 0;
    }

    .contact-field-label {
        color: rgba(255, 246, 214, 0.92);
        font-size: 1rem;
        font-weight: 600;
        letter-spacing: -0.015em;
    }

    .contact-text-input,
    .contact-textarea {
        width: 100%;
        box-sizing: border-box;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 18px;
        background: rgba(255, 255, 255, 0.05);
        color: rgba(255, 255, 255, 0.92);
        padding: 14px 16px;
        font: inherit;
        letter-spacing: -0.01em;
        transition:
            border-color 0.2s ease,
            box-shadow 0.2s ease,
            background-color 0.2s ease;
    }

    .contact-text-input::placeholder,
    .contact-textarea::placeholder {
        color: rgba(255, 255, 255, 0.34);
    }

    .contact-text-input:focus-visible,
    .contact-textarea:focus-visible {
        outline: none;
        border-color: rgba(255, 220, 102, 0.4);
        box-shadow: 0 0 0 4px rgba(255, 220, 102, 0.12);
        background: rgba(255, 255, 255, 0.08);
    }

    .contact-textarea {
        min-height: 320px;
        height: 100%;
        max-height: none;
        resize: none;
        line-height: 1.45;
        overflow-y: auto;
    }

    .contacts-editor-actions {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
        flex-shrink: 0;
        padding-top: 16px;
        border-top: 1px solid rgba(255, 255, 255, 0.08);
    }

    .contact-inline-btn {
        align-self: flex-start;
    }

    .conversation-popup-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
    }

    .conversation-popup-copy {
        display: flex;
        flex-direction: column;
        gap: 4px;
        min-width: 0;
    }

    .conversation-popup-title {
        color: rgba(255, 246, 214, 0.96);
        font-size: 1rem;
        font-weight: 700;
        letter-spacing: -0.02em;
    }

    .conversation-popup-subtitle {
        color: rgba(255, 255, 255, 0.56);
        font-size: 0.85rem;
        letter-spacing: -0.01em;
    }

    .conversation-close-btn {
        width: 34px;
        height: 34px;
        border: none;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.08);
        color: rgba(255, 255, 255, 0.8);
        display: flex;
        align-items: center;
        justify-content: center;
        cursor: pointer;
        flex-shrink: 0;
        transition:
            background-color 0.2s ease,
            color 0.2s ease,
            transform 0.1s ease;
    }

    .conversation-close-btn:hover {
        background: rgba(255, 255, 255, 0.12);
        color: #ffffff;
    }

    .conversation-close-btn:active {
        transform: scale(0.95);
    }

    .conversation-log {
        display: flex;
        flex-direction: column;
        gap: 12px;
        overflow-y: auto;
        padding-right: 4px;
    }

    .conversation-entry {
        display: flex;
        width: 100%;
    }

    .conversation-entry[data-role="user"] {
        justify-content: flex-end;
    }

    .conversation-bubble {
        max-width: 88%;
        padding: 12px 16px;
        border-radius: 22px;
        display: flex;
        flex-direction: column;
        gap: 10px;
        font-size: 0.98rem;
        line-height: 1.38;
        letter-spacing: -0.015em;
        word-break: break-word;
        box-shadow: 0 10px 28px rgba(0, 0, 0, 0.16);
    }

    .conversation-bubble[data-role="user"] {
        background: linear-gradient(135deg, #ffdf63 0%, #ffcd40 100%);
        color: #2f2500;
        border-bottom-right-radius: 10px;
    }

    .conversation-bubble[data-role="assistant"] {
        background: rgba(242, 242, 247, 0.96);
        color: #232326;
        border-bottom-left-radius: 10px;
    }

    .conversation-attachment-image {
        display: block;
        width: min(100%, 320px);
        max-width: 100%;
        border-radius: 16px;
        object-fit: cover;
        box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.08);
    }

    .conversation-entry-text {
        white-space: pre-wrap;
    }

    .conversation-empty {
        padding: 18px 16px;
        border-radius: 18px;
        background: rgba(255, 255, 255, 0.04);
        border: 1px dashed rgba(255, 255, 255, 0.08);
        color: rgba(255, 255, 255, 0.56);
        font-size: 0.94rem;
        line-height: 1.4;
        text-align: center;
    }

    .icon-btn {
        background: #444448;
        border: none;
        border-radius: 50%;
        width: 48px;
        height: 48px;
        display: flex;
        align-items: center;
        justify-content: center;
        color: white;
        cursor: pointer;
        transition:
            background-color 0.2s,
            color 0.2s,
            opacity 0.2s,
            transform 0.1s;
    }

    .icon-btn:hover:not(:disabled) {
        background: #545458;
    }

    .icon-btn:active:not(:disabled) {
        transform: scale(0.95);
    }

    .icon-btn.active {
        background: #ffffff;
        color: #1c1c1e;
    }

    .icon-btn:disabled {
        cursor: not-allowed;
        opacity: 0.46;
    }

    .icon-btn.interrupt-btn {
        background: rgba(255, 159, 104, 0.2);
        color: #ffd8c5;
    }

    .icon-btn.interrupt-btn:hover:not(:disabled) {
        background: rgba(255, 159, 104, 0.3);
    }

    .icon-btn.interrupt-btn:disabled {
        background: rgba(68, 68, 72, 0.72);
        color: rgba(255, 255, 255, 0.34);
    }

    .end-btn,
    .start-btn {
        color: white;
        border: none;
        border-radius: 24px;
        padding: 10px 30px;
        font-weight: 600;
        font-size: 1.05rem;
        cursor: pointer;
        transition:
            background-color 0.2s,
            transform 0.1s,
            opacity 0.2s;
    }

    .end-btn {
        background-color: #ff3b30;
    }

    .end-btn:hover {
        background-color: #ff453a;
    }

    .start-btn {
        background-color: #34c759;
    }

    .start-btn:hover:not(:disabled) {
        background-color: #30d158;
    }

    .end-btn:active,
    .start-btn:active:not(:disabled) {
        transform: scale(0.95);
    }

    .start-btn:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .download-banner {
        position: relative;
        background: rgba(28, 28, 30, 0.9);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 16px;
        padding: 14px 24px;
        display: flex;
        align-items: center;
        gap: 16px;
        color: white;
        backdrop-filter: blur(10px);
        animation: slideDown 0.3s ease-out;
        box-sizing: border-box;
        width: 100%;
        min-width: 0;
    }

    .download-banner:hover,
    .download-banner:focus-within {
        z-index: 2;
    }

    .download-banner.ready {
        background: rgba(28, 28, 30, 0.6);
        padding: 10px 20px;
    }

    .download-banner.voice-config-banner {
        align-items: stretch;
        flex-direction: column;
        gap: 14px;
    }

    .download-content {
        display: flex;
        flex-direction: column;
        gap: 8px;
        width: 100%;
    }

    .download-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
    }

    .progress-row {
        display: flex;
        align-items: center;
        gap: 10px;
        width: 100%;
    }

    .banner-row {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 20px;
        width: 100%;
    }

    .banner-copy {
        display: flex;
        flex-direction: column;
        gap: 4px;
        flex: 1;
        min-width: 0;
    }

    .banner-heading-row {
        display: flex;
        align-items: center;
        gap: 10px;
        flex-wrap: wrap;
    }

    .banner-title {
        font-size: 1rem;
        font-weight: 600;
        letter-spacing: -0.015em;
    }

    .banner-subtitle {
        color: rgba(255, 255, 255, 0.62);
        font-size: 0.92rem;
        letter-spacing: -0.01em;
    }

    .banner-subtitle-row {
        display: flex;
        align-items: center;
        gap: 10px;
        flex-wrap: wrap;
    }

    .banner-subtitle.error {
        color: #ffb3ad;
    }

    .banner-detail {
        color: rgba(255, 255, 255, 0.72);
        font-size: 0.84rem;
        line-height: 1.35;
        letter-spacing: -0.01em;
    }

    .banner-detail.error {
        color: rgba(255, 196, 191, 0.92);
    }

    .banner-status {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 16px;
        width: 100%;
    }

    .loaded-actions {
        display: flex;
        align-items: center;
        gap: 10px;
        flex-shrink: 0;
    }

    .download-percent {
        color: rgba(255, 255, 255, 0.75);
        font-variant-numeric: tabular-nums;
    }

    .download-status-text.failed {
        color: #ffb3ad;
    }

    .progress-track {
        position: relative;
        flex: 1;
        min-width: 0;
        width: 100%;
        height: 8px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.1);
        overflow: hidden;
    }

    .progress-cancel-btn {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        width: 26px;
        height: 26px;
        padding: 0;
        flex-shrink: 0;
        border: 1px solid rgba(255, 255, 255, 0.14);
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.08);
        color: rgba(255, 255, 255, 0.82);
        cursor: pointer;
        transition:
            background-color 0.2s ease,
            border-color 0.2s ease,
            color 0.2s ease;
    }

    .progress-cancel-btn:hover:not(:disabled) {
        background: rgba(255, 95, 87, 0.16);
        border-color: rgba(255, 95, 87, 0.32);
        color: #ffd2ce;
    }

    .progress-fill {
        height: 100%;
        border-radius: 999px;
        background: linear-gradient(90deg, #7fe37c 0%, #34c759 100%);
        transition: width 0.2s ease;
    }

    .progress-fill.failed {
        background: linear-gradient(90deg, #ff8d86 0%, #ff5f57 100%);
    }

    .progress-fill.indeterminate {
        position: relative;
        animation: indeterminateSlide 1.1s ease-in-out infinite;
    }

    .download-btn {
        background: #ffffff;
        color: #000;
        border: none;
        border-radius: 8px;
        padding: 6px 14px;
        font-weight: 600;
        cursor: pointer;
        font-size: 0.9rem;
    }

    .download-btn:disabled,
    .utility-btn:disabled,
    .progress-cancel-btn:disabled,
    .quantize-toggle:disabled,
    .variant-select:disabled {
        opacity: 0.5;
        cursor: not-allowed;
    }

    .utility-btn {
        background: rgba(255, 255, 255, 0.08);
        color: rgba(255, 255, 255, 0.9);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 10px;
        padding: 7px 12px;
        font-weight: 600;
        font-size: 0.88rem;
        cursor: pointer;
        transition:
            background-color 0.2s ease,
            border-color 0.2s ease;
    }

    .utility-btn:hover:not(:disabled) {
        background: rgba(255, 255, 255, 0.12);
        border-color: rgba(255, 255, 255, 0.12);
    }

    .subtitle-action-btn {
        border-radius: 999px;
        padding: 4px 10px;
        font-size: 0.8rem;
    }

    .hidden-file-input {
        display: none;
    }

    .tooltip-shell {
        position: relative;
        display: inline-flex;
    }

    .start-call-tooltip-shell {
        align-items: center;
    }

    .variant-select-shell {
        position: relative;
        display: inline-flex;
        align-items: center;
    }

    .variant-select-shell::after {
        content: "";
        position: absolute;
        top: 50%;
        right: 12px;
        width: 7px;
        height: 7px;
        border-right: 1.5px solid rgba(255, 255, 255, 0.66);
        border-bottom: 1.5px solid rgba(255, 255, 255, 0.66);
        transform: translateY(-60%) rotate(45deg);
        pointer-events: none;
    }

    .variant-select {
        appearance: none;
        -webkit-appearance: none;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.06);
        color: rgba(255, 255, 255, 0.82);
        padding: 5px 30px 5px 12px;
        font-size: 0.82rem;
        font-weight: 600;
        letter-spacing: -0.01em;
        cursor: pointer;
        transition:
            background-color 0.2s ease,
            border-color 0.2s ease,
            color 0.2s ease;
    }

    .variant-select:hover:not(:disabled) {
        background: rgba(255, 255, 255, 0.1);
    }

    .variant-select:focus-visible {
        outline: none;
        border-color: rgba(127, 227, 124, 0.4);
        box-shadow: 0 0 0 3px rgba(127, 227, 124, 0.14);
    }

    .quantize-toggle {
        display: inline-flex;
        align-items: center;
        gap: 8px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.06);
        color: rgba(255, 255, 255, 0.82);
        padding: 5px 12px;
        font-size: 0.82rem;
        font-weight: 600;
        letter-spacing: -0.01em;
        cursor: pointer;
        transition:
            background-color 0.2s ease,
            border-color 0.2s ease,
            color 0.2s ease;
    }

    .quantize-toggle:hover:not(:disabled) {
        background: rgba(255, 255, 255, 0.1);
    }

    .quantize-toggle.active {
        background: rgba(127, 227, 124, 0.12);
        border-color: rgba(127, 227, 124, 0.32);
        color: #9ae998;
    }

    .quantize-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        background: rgba(255, 255, 255, 0.35);
        transition:
            background-color 0.2s ease,
            box-shadow 0.2s ease;
    }

    .quantize-toggle.active .quantize-dot {
        background: #7fe37c;
        box-shadow: 0 0 0 4px rgba(127, 227, 124, 0.14);
    }

    .tooltip-bubble {
        position: absolute;
        left: 50%;
        bottom: calc(100% + 10px);
        transform: translateX(-50%) translateY(6px);
        width: 220px;
        padding: 10px 12px;
        border-radius: 12px;
        background: rgba(18, 18, 20, 0.96);
        border: 1px solid rgba(255, 255, 255, 0.08);
        box-shadow: 0 12px 36px rgba(0, 0, 0, 0.34);
        color: rgba(255, 255, 255, 0.86);
        font-size: 0.78rem;
        line-height: 1.35;
        opacity: 0;
        pointer-events: none;
        transition:
            opacity 0.18s ease,
            transform 0.18s ease;
        z-index: 20;
    }

    .tooltip-bubble.variant-tooltip {
        top: calc(100% + 10px);
        bottom: auto;
        transform: translateX(-65%) translateY(-6px);
    }

    .tooltip-bubble.start-call-tooltip {
        width: 250px;
        text-align: center;
    }

    .tooltip-bubble.variant-tooltip::after {
        top: auto;
        bottom: 100%;
        border-top: 1px solid rgba(255, 255, 255, 0.08);
        border-left: 1px solid rgba(255, 255, 255, 0.08);
        border-right: none;
        border-bottom: none;
    }

    .tooltip-bubble::after {
        content: "";
        position: absolute;
        left: 50%;
        top: 100%;
        width: 10px;
        height: 10px;
        background: rgba(18, 18, 20, 0.96);
        border-right: 1px solid rgba(255, 255, 255, 0.08);
        border-bottom: 1px solid rgba(255, 255, 255, 0.08);
        transform: translateX(-50%) rotate(45deg);
    }

    .tooltip-shell:hover .tooltip-bubble,
    .tooltip-shell:focus-within .tooltip-bubble {
        opacity: 1;
        transform: translateX(-50%) translateY(0);
    }

    .status-icon {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 28px;
        height: 28px;
        border-radius: 999px;
        background: rgba(52, 199, 89, 0.12);
        flex-shrink: 0;
    }

    .status-icon :global(svg) {
        display: block;
    }

    @keyframes slideDown {
        from {
            transform: translateY(-20px);
            opacity: 0;
        }
        to {
            transform: translateY(0);
            opacity: 1;
        }
    }

    @keyframes indeterminateSlide {
        from {
            transform: translateX(-120%);
        }
        to {
            transform: translateX(320%);
        }
    }

    @keyframes callPulse {
        0% {
            transform: scale(1);
            box-shadow: 0 0 0 0 currentColor;
        }
        70% {
            transform: scale(1.08);
            box-shadow: 0 0 0 10px rgba(255, 255, 255, 0);
        }
        100% {
            transform: scale(1);
            box-shadow: 0 0 0 0 rgba(255, 255, 255, 0);
        }
    }

    @media (max-width: 720px) {
        .control-bar-wrapper {
            bottom: 20px;
            gap: 10px;
            padding: 0 14px;
        }

        .model-memory-pill {
            max-width: calc(100vw - 28px);
            padding: 8px 12px;
        }

        .screen-capture-card {
            width: calc(100vw - 28px);
            padding: 12px 14px;
            border-radius: 20px;
        }

        .contacts-popup {
            width: calc(100vw - 28px);
            height: calc(100vh - 28px);
            max-height: calc(100vh - 28px);
            padding: 16px;
            border-radius: 24px;
        }

        .contacts-popup-body {
            grid-template-columns: 1fr;
        }

        .contacts-list {
            max-height: 220px;
        }

        .contacts-sidebar-actions {
            flex-wrap: wrap;
        }

        .contacts-editor-top {
            align-items: flex-start;
            flex-direction: column;
        }

        .contact-icon-picker {
            width: 96px;
            height: 96px;
        }

        .contact-textarea {
            min-height: 220px;
        }

        .contacts-editor-actions {
            flex-wrap: wrap;
        }
    }
</style>
