<!-- Main voice-call page that wires the app state together and delegates repeated UI sections to home components. -->
<script lang="ts">
    import "./home.css";

    import { onDestroy, onMount } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { listen, type UnlistenFn } from "@tauri-apps/api/event";
    import { save } from "@tauri-apps/plugin-dialog";
    import ContactsModal from "$lib/components/home/ContactsModal.svelte";
    import ConversationPopup from "$lib/components/home/ConversationPopup.svelte";
    import GemmaBanner from "$lib/components/home/GemmaBanner.svelte";
    import SpeechBanner from "$lib/components/home/SpeechBanner.svelte";
    import SttBanner from "$lib/components/home/SttBanner.svelte";
    import {
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
        CONTACTS_STORAGE_KEY,
        DEFAULT_CONTACT_ID,
        DEFAULT_CSM_MODEL,
        DEFAULT_GEMMA_VARIANT,
        DEFAULT_STT_MODEL,
        DEFAULT_VOICE_SYSTEM_PROMPT,
        MODEL_PREFERENCES_STORAGE_KEY,
        MODEL_PRESETS,
    } from "$lib/openduck/config";
    import {
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
        AssistantResponseEvent,
        CallStageEvent,
        CallStagePhase,
        ContactExportResult,
        ContactProfile,
        ConversationLogEntry,
        CsmAudioChunkEvent,
        CsmAudioDoneEvent,
        CsmAudioQueuedEvent,
        CsmAudioStartEvent,
        CsmAudioStopEvent,
        CsmErrorEvent,
        CsmModelVariant,
        CsmStatusEvent,
        GemmaVariant,
        ModelDownloadProgressEvent,
        ModelMemoryUsageSnapshot,
        ModelPreset,
        ModelSelection,
        RuntimeSetupStatusEvent,
        ScreenCaptureEvent,
        SelectOption,
        StoredModelPreferences,
        SttModelVariant,
        SttStatusEvent,
        TranscriptEvent,
        TrayEndCallEvent,
        TrayToggleMuteEvent,
    } from "$lib/openduck/types";

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
    let pendingModelPreset = $state<ModelPreset | null>(null);
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
    let callStagePhase = $state<CallStagePhase>("idle");
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
        (modelMemorySnapshot?.total_bytes ?? 0) > 0,
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
            (selectedSttModel === "whisper_large_v3_turbo" && isSttLoaded),
    );
    let conversationLogViewport = $state<HTMLDivElement | null>(null);
    const modelPresetOptions: Array<SelectOption<ModelPreset>> = [
        { value: "lite", label: MODEL_PRESETS.lite.label },
        { value: "normal", label: MODEL_PRESETS.normal.label },
        { value: "realistic", label: MODEL_PRESETS.realistic.label },
        { value: "custom", label: "Custom" },
    ];
    const gemmaVariantOptions: Array<SelectOption<GemmaVariant>> = [
        { value: "e4b", label: "E4B" },
        { value: "e2b", label: "E2B" },
    ];
    const csmModelOptions: Array<SelectOption<CsmModelVariant>> = [
        { value: "expressiva_1b", label: "CSM Expressiva 1B" },
        { value: "kokoro_82m", label: "Kokoro-82M" },
        { value: "cosyvoice2_0_5b", label: "CosyVoice2-0.5B" },
    ];
    const sttModelOptions: Array<SelectOption<SttModelVariant>> = [
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
    const loadAllDisabled = $derived(loadAllBusy || !loadAllNeedsAction);
    const presetSelectDisabled = $derived(
        loadAllBusy || isUpdatingCsmQuantize || pendingModelPreset !== null,
    );
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
    const runtimeSetupTitle = $derived(
        isPreparingRuntime ? "Preparing Local Runtime" : "Runtime Setup Failed",
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

    function setCallStage(phase: CallStagePhase, message: string) {
        callStagePhase = phase;
        callStageMessage = message;
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

    function getCurrentModelSelection(): ModelSelection {
        return {
            gemmaVariant: selectedGemmaVariant,
            csmModel: selectedCsmModel,
            sttModel: selectedSttModel,
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
        selectedSttModel = restoredPreferences.sttModel;

        try {
            await setGemmaVariantSelection(restoredPreferences.gemmaVariant);
        } catch (err) {
            console.error("Failed to restore Gemma variant:", err);
        }

        try {
            await setCsmModelSelection(restoredPreferences.csmModel);
        } catch (err) {
            console.error("Failed to restore speech model:", err);
        }

        try {
            await setSttModelSelection(restoredPreferences.sttModel);
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
        } catch (err) {
            selectedGemmaVariant = previousSelection.gemmaVariant;
            selectedCsmModel = previousSelection.csmModel;
            selectedSttModel = previousSelection.sttModel;
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

            captureProcessor.port.onmessage = (event) => {
                if (!calling) {
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
        syncCaptureMutedState(micMuted);
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
            isGemmaLoaded = true;
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
        isUnloadingCsm = true;
        try {
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
        void (async () => {
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
                ]);
            } catch (err) {
                console.error("Failed to register Tauri event listeners:", err);
            }

            const restoredContacts = await loadContactsFromStorage();
            contacts = restoredContacts.contacts;
            selectedContactId = restoredContacts.selectedContactId;
            persistContactsMetadata();
            await syncSelectedContactPrompt();
            await restoreModelPreferences();
            await ensureRuntimeDependencies();
            await syncModelStatus();
        })();

        healthCheckInterval = window.setInterval(() => {
            void syncModelStatus();
        }, 5000);
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
                            <option value={option.value}>{option.label}</option>
                        {/each}
                    </select>
                </div>
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
            />
            <SttBanner
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
                {handleResetSelectedContactIcon}
                {handleSelectedContactNameInput}
                {handleSelectedContactPromptInput}
                {handleDeleteSelectedContact}
                {handleExportSelectedContact}
            />
        {/if}

        {#if calling && showConversationPopup}
            <ConversationPopup
                {conversationLogEntries}
                {closeConversationPopup}
                setConversationLogViewport={(element) => {
                    conversationLogViewport = element;
                }}
            />
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
                    <div class="tooltip-shell control-tooltip-shell">
                        <button
                            class="icon-btn"
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
                                    /><path d="M19 10v2a7 7 0 0 1-14 0v-2" /><line
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
                            <path
                                d="M15.5 11V6a1.5 1.5 0 0 1 3 0v7a6 6 0 0 1-6 6H11a5 5 0 0 1-4.64-3.14l-1.1-2.64a1.5 1.5 0 0 1 2.72-1.26L9.5 14V11"
                            />
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
