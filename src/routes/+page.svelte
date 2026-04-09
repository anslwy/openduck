<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { listen, type UnlistenFn } from "@tauri-apps/api/event";

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

    type CallStageEvent = {
        phase: string;
        message: string;
    };

    type TranscriptEvent = {
        text: string;
    };

    type AssistantResponseEvent = {
        request_id: number;
        text: string;
        is_final: boolean;
    };

    type TrayEndCallEvent = Record<string, never>;
    type TrayToggleMuteEvent = Record<string, never>;

    type ModelDownloadProgressEvent = {
        model: "gemma" | "csm";
        phase: "progress" | "completed" | "error" | "cancelled";
        message: string;
        progress?: number | null;
        downloaded_bytes?: number | null;
        total_bytes?: number | null;
        indeterminate: boolean;
    };

    type GemmaVariant = "e4b" | "e2b";
    type CsmModelVariant = "expressiva_1b" | "kokoro_82m";

    type ConversationLogEntry = {
        id: number;
        role: "user" | "assistant";
        text: string;
    };

    let calling = $state(false);
    let micMuted = $state(false);
    let time = $state(0);
    let callStartedAtMs = $state<number | null>(null);
    let isGemmaDownloaded = $state(false);
    let isGemmaLoaded = $state(false);
    let isCsmDownloaded = $state(false);
    let isCsmLoaded = $state(false);
    let isDownloadingGemma = $state(false);
    let isClearingGemmaCache = $state(false);
    let isCancellingGemmaDownload = $state(false);
    let isLoadingGemma = $state(false);
    let isDownloadingCsm = $state(false);
    let isClearingCsmCache = $state(false);
    let isLoadingCsm = $state(false);
    let isUnloadingGemma = $state(false);
    let isUnloadingCsm = $state(false);
    let isUpdatingCsmQuantize = $state(false);
    let gemmaDownloadMessage = $state("Preparing download...");
    let gemmaDownloadProgress = $state<number | null>(null);
    let gemmaDownloadIndeterminate = $state(true);
    let gemmaDownloadError = $state<string | null>(null);
    let selectedGemmaVariant = $state<GemmaVariant>("e4b");
    let selectedCsmModel = $state<CsmModelVariant>("expressiva_1b");
    let csmDownloadMessage = $state("Preparing download...");
    let csmDownloadProgress = $state<number | null>(null);
    let csmDownloadIndeterminate = $state(true);
    let csmDownloadError = $state<string | null>(null);
    let csmLoadMessage = $state("Starting worker...");
    let isCsmQuantized = $state(true);

    let captureContext: AudioContext | null = null;
    let mediaStream: MediaStream | null = null;
    let captureSource: MediaStreamAudioSourceNode | null = null;
    let captureProcessor: AudioWorkletNode | null = null;
    let playbackProcessor: AudioWorkletNode | null = null;
    let silentCaptureSink: GainNode | null = null;
    let healthCheckInterval: ReturnType<typeof window.setInterval> | null =
        null;
    let downloadStatusPollInterval:
        | ReturnType<typeof window.setInterval>
        | null = null;
    let callTimerInterval: ReturnType<typeof window.setInterval> | null = null;
    let playbackIdleTimeout: ReturnType<typeof window.setTimeout> | null = null;
    let eventUnlisteners: UnlistenFn[] = [];
    let activeTtsRequestId: number | null = null;
    let pendingTtsSegments = $state(0);
    let queuedPlaybackChunkCount = $state(0);
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

    const formattedTime = $derived(
        `${Math.floor(time / 60)
            .toString()
            .padStart(2, "0")}:${(time % 60).toString().padStart(2, "0")}`,
    );
    const modelsReady = $derived(isGemmaLoaded && isCsmLoaded);
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
            isLoadingCsm ||
            isUnloadingCsm,
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
    ];
    const gemmaVariantTooltip = $derived(
        selectedGemmaVariant === "e4b"
            ? "E4B uses more RAM but is generally more capable. Recommended for Macs with 24 GB+ of unified memory."
            : "E2B uses less RAM but is generally less capable. Recommended for Macs with 16 GB+ of unified memory.",
    );
    const selectedCsmModelLabel = $derived(
        csmModelOptions.find((option) => option.value === selectedCsmModel)
            ?.label ?? "CSM Expressiva 1B",
    );
    const csmModelTooltip = $derived(
        selectedCsmModel === "expressiva_1b"
            ? "CSM Expressiva 1B supports voice conditioning and optional quantization."
            : "Kokoro-82M is a lighter English TTS backend. Quantization is not used for this model.",
    );
    const csmQuantizeAvailable = $derived(selectedCsmModel === "expressiva_1b");
    const PLAYBACK_PREBUFFER_SAMPLES = 2048;

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
    ) {
        const normalizedText = text.trim();
        if (!normalizedText) {
            return null;
        }

        const entryId = nextConversationEntryId;
        conversationLogEntries = [
            ...conversationLogEntries,
            {
                id: entryId,
                role,
                text: normalizedText,
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

    function closeConversationPopup() {
        showConversationPopup = false;
    }

    function toggleConversationPopup() {
        showConversationPopup = !showConversationPopup;
        if (showConversationPopup) {
            scrollConversationLogToBottom();
        }
    }

    function handleWindowKeydown(event: KeyboardEvent) {
        if (event.key === "Escape" && showConversationPopup) {
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

    function resetDownloadState(model: "gemma" | "csm") {
        if (model === "gemma") {
            gemmaDownloadMessage = "Preparing download...";
            gemmaDownloadProgress = null;
            gemmaDownloadIndeterminate = true;
            gemmaDownloadError = null;
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
    }

    function stopDownloadStatusPolling() {
        if (downloadStatusPollInterval) {
            clearInterval(downloadStatusPollInterval);
            downloadStatusPollInterval = null;
        }
    }

    async function pollActiveDownloadStatuses() {
        if (!isDownloadingGemma && !isDownloadingCsm) {
            stopDownloadStatusPolling();
            return;
        }

        try {
            const [gemmaStatus, csmStatus] = await Promise.all([
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
            ]);

            if (
                gemmaStatus &&
                (!isCancellingGemmaDownload || gemmaStatus.phase !== "progress")
            ) {
                applyDownloadEvent(gemmaStatus);
            }
            if (csmStatus) {
                applyDownloadEvent(csmStatus);
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

    async function syncModelStatus() {
        try {
            const [
                gemmaVariant,
                csmModelVariant,
                gemmaDownloaded,
                gemmaLoaded,
                csmDownloaded,
                csmLoaded,
                csmQuantized,
            ] = await Promise.all([
                invoke<GemmaVariant>("get_gemma_variant"),
                invoke<CsmModelVariant>("get_csm_model_variant"),
                invoke<boolean>("check_model_status"),
                invoke<boolean>("is_server_running"),
                invoke<boolean>("check_csm_status"),
                invoke<boolean>("is_csm_running"),
                invoke<boolean>("get_csm_quantize"),
            ]);

            selectedGemmaVariant = gemmaVariant;
            selectedCsmModel = csmModelVariant;
            isGemmaDownloaded = gemmaDownloaded;
            isGemmaLoaded = gemmaLoaded;
            isCsmDownloaded = csmDownloaded;
            isCsmLoaded = csmLoaded;
            isCsmQuantized = csmQuantized;
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

                const {
                    inputData,
                    playbackReferenceData,
                    playbackActive,
                } = event.data as {
                    inputData: Float32Array;
                    playbackReferenceData?: Float32Array;
                    playbackActive?: boolean;
                };
                void invoke("receive_audio_chunk", {
                    payload: {
                        data: Array.from(inputData),
                        sample_rate: Math.round(captureContext?.sampleRate ?? 0),
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
                                updateStageAfterPlaybackStateChange();
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
        pendingTtsSegments = 0;
    }

    function updateStageAfterPlaybackStateChange() {
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

        for (let channelIndex = 0; channelIndex < audioBuffer.numberOfChannels; channelIndex += 1) {
            const channelData = audioBuffer.getChannelData(channelIndex);
            for (let sampleIndex = 0; sampleIndex < channelData.length; sampleIndex += 1) {
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

    async function handleStartCall() {
        if (!modelsReady) {
            return;
        }

        try {
            await invoke("reset_call_session");
        } catch (err) {
            console.error("Failed to reset call session:", err);
        }

        closeConversationPopup();
        resetConversationLog();
        calling = true;
        callStartedAtMs = Date.now();
        syncCallElapsedTime();
        activeTtsRequestId = null;
        setCallStage("listening", "Listening");

        void invoke("start_call_timer", { muted: micMuted }).catch((err) =>
            console.error("Failed to start tray call timer", err),
        );
        void startAudioCapture();
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
        closeConversationPopup();
        resetConversationLog();
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
        try {
            await invoke("download_model");
            await syncModelStatus();
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
            if (!isDownloadingCsm) {
                stopDownloadStatusPolling();
            }
            await syncModelStatus();
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
        }
        finally {
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
        resetDownloadState("csm");
        ensureDownloadStatusPolling();
        try {
            await invoke("download_csm_model");
            await syncModelStatus();
        } catch (err) {
            console.error("Download speech model failed:", err);
            const message = normalizeDownloadErrorMessage(err);
            csmDownloadError = message;
            csmDownloadMessage = message;
            csmDownloadProgress = null;
            csmDownloadIndeterminate = true;
        } finally {
            isDownloadingCsm = false;
            if (!isDownloadingGemma) {
                stopDownloadStatusPolling();
            }
            await syncModelStatus();
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
        }
        finally {
            isClearingCsmCache = false;
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

    onMount(() => {
        void syncModelStatus();

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
                        },
                    ),
                    listen<CsmAudioQueuedEvent>(
                        "csm-audio-queued",
                        ({ payload }) => {
                            if (!calling || payload.request_id !== activeTtsRequestId) {
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
                                    updateStageAfterPlaybackStateChange();
                                }
                                console.log("Finished streaming CSM response");
                            }
                        },
                    ),
                    listen<CsmAudioStopEvent>("csm-audio-stop", () => {
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

                            appendConversationLogEntry("user", payload.text);
                        },
                    ),
                    listen<ModelDownloadProgressEvent>(
                        "model-download-progress",
                        ({ payload }) => {
                            applyDownloadEvent(payload);
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
        })();
    });

    onDestroy(() => {
        if (healthCheckInterval) {
            clearInterval(healthCheckInterval);
        }
        stopDownloadStatusPolling();
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

<div class="app-container">
    <div class="background"></div>

    {#if !calling}
        <div class="model-tags">
            <div
                class="download-banner"
                class:ready={isGemmaDownloaded && isGemmaLoaded}
            >
                {#if isDownloadingGemma}
                    <div class="download-content">
                        <div class="banner-row">
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
                            <button
                                class="utility-btn"
                                disabled={isCancellingGemmaDownload}
                                onclick={handleCancelGemmaDownload}
                            >
                                {isCancellingGemmaDownload
                                    ? "Cancelling..."
                                    : "Cancel"}
                            </button>
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
                                                disabled={isLoadingCsm ||
                                                    isDownloadingCsm ||
                                                    isClearingCsmCache ||
                                                    isUpdatingCsmQuantize}
                                                onclick={handleCsmQuantizeToggle}
                                            >
                                                <span
                                                    class="quantize-dot"
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
            <div class="avatar" class:calling></div>
        </div>
    </main>

    <div class="control-bar-wrapper">
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
                                    {entry.text}
                                </div>
                            </div>
                        {/each}
                    {/if}
                </div>
            </div>
        {/if}

        <div class="control-bar">
            <div class="info">
                <div class="username">openduck</div>
                <div class="timer">{calling ? formattedTime : "Ready"}</div>
            </div>

            <div class="actions">
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
                {/if}
            </div>

            {#if calling}
                <button class="end-btn" onclick={handleEndCall}>End</button>
            {:else}
                <button
                    class="start-btn"
                    disabled={!modelsReady}
                    onclick={handleStartCall}>Call</button
                >
            {/if}
        </div>
    </div>
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
        z-index: 10;
        isolation: isolate;
    }

    .control-bar-wrapper {
        position: absolute;
        bottom: 40px;
        width: 100%;
        display: flex;
        justify-content: center;
        align-items: center;
        padding: 0 20px;
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
        min-width: 440px;
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
        z-index: 1;
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
            transform 0.1s;
    }

    .icon-btn:hover {
        background: #545458;
    }

    .icon-btn:active {
        transform: scale(0.95);
    }

    .icon-btn.active {
        background: #ffffff;
        color: #1c1c1e;
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

    .download-banner.voice-config-banner.ready {
        padding: 14px 24px;
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
        width: 100%;
        height: 8px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.1);
        overflow: hidden;
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

    .tooltip-shell {
        position: relative;
        display: inline-flex;
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
</style>
