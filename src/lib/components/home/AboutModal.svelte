<!-- Custom About modal that shows build metadata such as version labels and commit hashes. -->
<script lang="ts">
    import { onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { ask } from "@tauri-apps/plugin-dialog";

    import {
        AUTO_CONTINUE_MAX_COUNT_CONTINUOUS_SLIDER_VALUE,
        AUTO_CONTINUE_NEVER_SLIDER_VALUE,
        AUTO_CONTINUE_SILENCE_STEP_MS,
        DEFAULT_AUTO_CONTINUE_SILENCE_MS,
        DEFAULT_LLM_IMAGE_HISTORY_LIMIT,
        LLM_CONTEXT_TURN_LIMIT_UNLIMITED_SLIDER_VALUE,
        END_OF_UTTERANCE_SILENCE_STEP_MS,
        LLM_IMAGE_HISTORY_UNLIMITED_SLIDER_VALUE,
        MAX_AUTO_CONTINUE_MAX_COUNT,
        MAX_LLM_CONTEXT_TURN_LIMIT,
        MAX_END_OF_UTTERANCE_SILENCE_MS,
        MAX_LLM_IMAGE_HISTORY_LIMIT,
        MIN_AUTO_CONTINUE_MAX_COUNT,
        MIN_AUTO_CONTINUE_SILENCE_MS,
        MIN_LLM_CONTEXT_TURN_LIMIT,
        MIN_END_OF_UTTERANCE_SILENCE_MS,
        MIN_LLM_IMAGE_HISTORY_LIMIT,
        DEFAULT_GLOBAL_SHORTCUT,
        DEFAULT_GLOBAL_SHORTCUT_ENTIRE_SCREEN,
        DEFAULT_GLOBAL_SHORTCUT_TOGGLE_MUTE,
        DEFAULT_GLOBAL_SHORTCUT_INTERRUPT,
        NO_GLOBAL_SHORTCUT,
    } from "$lib/openduck/config";
    import type {
        AppUpdateInfo,
        AppUpdateStatus,
        BuildInfo,
    } from "$lib/openduck/types";
    import ShortcutCapture from "./ShortcutCapture.svelte";

    let {
        calling,
        buildInfo,
        buildInfoError,
        availableAppUpdate,
        appUpdateStatus,
        appUpdateError,
        checkForUpdates,
        installAvailableUpdate,
        restartToApplyUpdate,
        closeAboutPopup,
        globalShortcut,
        globalShortcutEntireScreen,
        globalShortcutToggleMute,
        globalShortcutInterrupt,
        pongPlaybackEnabled,
        autoUnmuteOnPastedScreenshotEnabled,
        selectLastSessionEnabled,
        showStatEnabled,
        showSubtitleEnabled,
        showAiSubtitleEnabled,
        showCallTimerEnabled,
        showHiddenWindowOverlayEnabled,
        endOfUtteranceSilenceMs,
        autoContinueSilenceMs,
        autoContinueMaxCount,
        llmContextTurnLimit,
        llmImageHistoryLimit,
        onUpdateGlobalShortcut,
        onUpdateGlobalShortcutEntireScreen,
        onUpdateGlobalShortcutToggleMute,
        onUpdateGlobalShortcutInterrupt,
        onUpdatePongPlayback,
        onUpdateAutoUnmuteOnPastedScreenshot,
        onUpdateSelectLastSession,
        onUpdateShowStat,
        onUpdateShowSubtitle,
        onUpdateShowAiSubtitle,
        onUpdateShowCallTimer,
        onUpdateShowHiddenWindowOverlay,
        onUpdateEndOfUtteranceSilenceMs,
        onUpdateAutoContinueSilenceMs,
        onUpdateAutoContinueMaxCount,
        onUpdateLlmContextTurnLimit,
        onUpdateLlmImageHistoryLimit,
    } = $props<{
        calling: boolean;
        buildInfo: BuildInfo | null;
        buildInfoError: string | null;
        availableAppUpdate: AppUpdateInfo | null;
        appUpdateStatus: AppUpdateStatus;
        appUpdateError: string | null;
        checkForUpdates: () => void;
        installAvailableUpdate: () => void;
        restartToApplyUpdate: () => void;
        closeAboutPopup: () => void;
        globalShortcut: string;
        globalShortcutEntireScreen: string;
        globalShortcutToggleMute: string;
        globalShortcutInterrupt: string;
        pongPlaybackEnabled: boolean;
        autoUnmuteOnPastedScreenshotEnabled: boolean;
        selectLastSessionEnabled: boolean;
        showStatEnabled: boolean;
        showSubtitleEnabled: boolean;
        showAiSubtitleEnabled: boolean;
        showCallTimerEnabled: boolean;
        showHiddenWindowOverlayEnabled: boolean;
        endOfUtteranceSilenceMs: number;
        autoContinueSilenceMs: number | null;
        autoContinueMaxCount: number | null;
        llmContextTurnLimit: number | null;
        llmImageHistoryLimit: number | null;
        onUpdateGlobalShortcut: (shortcut: string) => void;
        onUpdateGlobalShortcutEntireScreen: (shortcut: string) => void;
        onUpdateGlobalShortcutToggleMute: (shortcut: string) => void;
        onUpdateGlobalShortcutInterrupt: (shortcut: string) => void;
        onUpdatePongPlayback: (enabled: boolean) => void;
        onUpdateAutoUnmuteOnPastedScreenshot: (enabled: boolean) => void;
        onUpdateSelectLastSession: (enabled: boolean) => void;
        onUpdateShowStat: (enabled: boolean) => void;
        onUpdateShowSubtitle: (enabled: boolean) => void;
        onUpdateShowAiSubtitle: (enabled: boolean) => void;
        onUpdateShowCallTimer: (enabled: boolean) => void;
        onUpdateShowHiddenWindowOverlay: (enabled: boolean) => void;
        onUpdateEndOfUtteranceSilenceMs: (milliseconds: number) => void;
        onUpdateAutoContinueSilenceMs: (milliseconds: number | null) => void;
        onUpdateAutoContinueMaxCount: (count: number | null) => void;
        onUpdateLlmContextTurnLimit: (limit: number | null) => void;
        onUpdateLlmImageHistoryLimit: (limit: number | null) => void;
    }>();

    let copyState = $state<"idle" | "copied" | "failed">("idle");
    let copyResetTimeout: ReturnType<typeof window.setTimeout> | null = null;
    let isRefreshing = $state(false);
    let searchQuery = $state("");

    function matchesSearch(text: string, detail?: string) {
        const query = searchQuery.toLowerCase().trim();
        if (!query) return true;
        return (
            text.toLowerCase().includes(query) ||
            (detail?.toLowerCase().includes(query) ?? false)
        );
    }

    let editedShortcut = $state("");
    let editedShortcutEntireScreen = $state("");
    let editedShortcutToggleMute = $state("");
    let editedShortcutInterrupt = $state("");

    $effect(() => {
        editedShortcut = globalShortcut;
    });

    $effect(() => {
        editedShortcutEntireScreen = globalShortcutEntireScreen;
    });

    $effect(() => {
        editedShortcutToggleMute = globalShortcutToggleMute;
    });

    $effect(() => {
        editedShortcutInterrupt = globalShortcutInterrupt;
    });

    function clearCopyFeedback() {
        if (copyResetTimeout) {
            clearTimeout(copyResetTimeout);
            copyResetTimeout = null;
        }
    }

    function queueCopyFeedbackReset() {
        clearCopyFeedback();
        copyResetTimeout = window.setTimeout(() => {
            copyState = "idle";
            copyResetTimeout = null;
        }, 1800);
    }

    async function handleDownloadFromGithub() {
        try {
            await invoke("plugin:shell|open", {
                path: "https://github.com/anslwy/openduck/releases",
            });
        } catch (error) {
            console.error("Failed to open GitHub releases:", error);
        }
    }

    async function refreshRuntimeCaches() {
        const confirmed = await ask(
            "This will clear the local Python runtime and bootstrap caches, then restart OpenDuck. Are you sure?",
            {
                title: "Refresh Runtime Caches",
                kind: "warning",
            },
        );

        if (!confirmed) return;

        isRefreshing = true;
        try {
            await invoke("refresh_runtime_caches");
        } catch (error) {
            console.error("Failed to refresh runtime caches:", error);
            isRefreshing = false;
        }
    }

    const copyButtonLabel = $derived(
        copyState === "copied"
            ? "Copied"
            : copyState === "failed"
              ? "Copy Failed"
              : "Copy",
    );
    const checkButtonLabel = $derived(
        appUpdateStatus === "checking"
            ? "Checking..."
            : appUpdateStatus === "installing"
              ? "Installing..."
              : "Check for Updates",
    );
    const formattedPublishedAt = $derived.by(() => {
        const publishedAt = availableAppUpdate?.publishedAt;
        if (!publishedAt) {
            return null;
        }

        const parsedDate = new Date(publishedAt);
        if (Number.isNaN(parsedDate.valueOf())) {
            return publishedAt;
        }

        return parsedDate.toLocaleString(undefined, {
            dateStyle: "medium",
            timeStyle: "short",
        });
    });
    const updateStatusDetail = $derived.by(() => {
        switch (appUpdateStatus) {
            case "checking":
                return "Checking GitHub Releases for a newer build.";
            case "available":
                return availableAppUpdate
                    ? `Version ${availableAppUpdate.version} is ready to download.`
                    : "A newer build is available.";
            case "up_to_date":
                return "This build is already up to date.";
            case "installing":
                return availableAppUpdate
                    ? `Installing version ${availableAppUpdate.version}.`
                    : "Installing the latest update.";
            case "installed":
                return availableAppUpdate
                    ? `Version ${availableAppUpdate.version} is installed and ready after restart.`
                    : "The update is installed and ready after restart.";
            case "error":
                return appUpdateError ?? "The update check failed.";
            default:
                return "Check GitHub for a newer build.";
        }
    });
    const updateActionDisabled = $derived(
        appUpdateStatus === "checking" || appUpdateStatus === "installing",
    );
    const formattedEndOfUtteranceSilence = $derived.by(() => {
        const seconds = endOfUtteranceSilenceMs / 1000;
        return Number.isInteger(seconds)
            ? `${seconds}s`
            : `${seconds.toFixed(1)}s`;
    });
    const minimumEndOfUtteranceSilenceLabel = $derived.by(() => {
        const seconds = MIN_END_OF_UTTERANCE_SILENCE_MS / 1000;
        return Number.isInteger(seconds)
            ? `${seconds.toFixed(0)}s`
            : `${seconds.toFixed(1)}s`;
    });
    const maximumEndOfUtteranceSilenceLabel = $derived.by(() => {
        const seconds = MAX_END_OF_UTTERANCE_SILENCE_MS / 1000;
        return Number.isInteger(seconds)
            ? `${seconds.toFixed(0)}s`
            : `${seconds.toFixed(1)}s`;
    });
    const endOfUtteranceSilenceProgress = $derived.by(() => {
        const range =
            MAX_END_OF_UTTERANCE_SILENCE_MS - MIN_END_OF_UTTERANCE_SILENCE_MS;
        if (range <= 0) {
            return 0;
        }

        return (
            ((endOfUtteranceSilenceMs - MIN_END_OF_UTTERANCE_SILENCE_MS) /
                range) *
            100
        );
    });
    const autoContinueSilenceSliderValue = $derived.by(() => {
        if (autoContinueSilenceMs === DEFAULT_AUTO_CONTINUE_SILENCE_MS) {
            return AUTO_CONTINUE_NEVER_SLIDER_VALUE;
        }

        return autoContinueSilenceMs;
    });
    const formattedAutoContinueSilence = $derived.by(() => {
        if (autoContinueSilenceMs === DEFAULT_AUTO_CONTINUE_SILENCE_MS) {
            return "Never auto continue";
        }

        return `${autoContinueSilenceMs / 1000}s`;
    });
    const autoContinueSilenceProgress = $derived.by(() => {
        const range =
            AUTO_CONTINUE_NEVER_SLIDER_VALUE - MIN_AUTO_CONTINUE_SILENCE_MS;
        if (range <= 0) {
            return 0;
        }

        return (
            ((autoContinueSilenceSliderValue - MIN_AUTO_CONTINUE_SILENCE_MS) /
                range) *
            100
        );
    });
    const minimumAutoContinueSilenceLabel = `${
        MIN_AUTO_CONTINUE_SILENCE_MS / 1000
    }s`;
    const maximumAutoContinueSilenceLabel = "Never";
    const autoContinueMaxCountDisabled = $derived(
        autoContinueSilenceMs === DEFAULT_AUTO_CONTINUE_SILENCE_MS,
    );
    const autoContinueMaxCountSliderValue = $derived.by(() => {
        if (autoContinueMaxCount === null) {
            return AUTO_CONTINUE_MAX_COUNT_CONTINUOUS_SLIDER_VALUE;
        }

        return autoContinueMaxCount;
    });
    const formattedAutoContinueMaxCount = $derived.by(() => {
        if (autoContinueMaxCount === null) {
            return "Continuous";
        }

        return autoContinueMaxCount === 1
            ? "1 time"
            : `${autoContinueMaxCount} times`;
    });
    const autoContinueMaxCountProgress = $derived.by(() => {
        const range =
            AUTO_CONTINUE_MAX_COUNT_CONTINUOUS_SLIDER_VALUE -
            MIN_AUTO_CONTINUE_MAX_COUNT;
        if (range <= 0) {
            return 0;
        }

        return (
            ((autoContinueMaxCountSliderValue - MIN_AUTO_CONTINUE_MAX_COUNT) /
                range) *
            100
        );
    });
    const minimumAutoContinueMaxCountLabel = `${MIN_AUTO_CONTINUE_MAX_COUNT}`;
    const maximumAutoContinueMaxCountLabel = "Continuous";
    const showContinuousAutoContinueWarning = $derived(
        !autoContinueMaxCountDisabled && autoContinueMaxCount === null,
    );
    const llmContextTurnSliderValue = $derived.by(() => {
        if (llmContextTurnLimit === null) {
            return LLM_CONTEXT_TURN_LIMIT_UNLIMITED_SLIDER_VALUE;
        }

        return llmContextTurnLimit;
    });
    const formattedLlmContextTurnLimit = $derived.by(() => {
        if (llmContextTurnLimit === null) {
            return "Unlimited";
        }

        return `${llmContextTurnLimit} turns`;
    });
    const llmContextTurnProgress = $derived.by(() => {
        const range =
            LLM_CONTEXT_TURN_LIMIT_UNLIMITED_SLIDER_VALUE -
            MIN_LLM_CONTEXT_TURN_LIMIT;
        if (range <= 0) {
            return 0;
        }

        return (
            ((llmContextTurnSliderValue - MIN_LLM_CONTEXT_TURN_LIMIT) / range) *
            100
        );
    });
    const minimumLlmContextTurnLabel = `${MIN_LLM_CONTEXT_TURN_LIMIT}`;
    const maximumLlmContextTurnLabel = "Unlimited";
    const llmImageHistorySliderValue = $derived.by(() => {
        if (llmImageHistoryLimit === DEFAULT_LLM_IMAGE_HISTORY_LIMIT) {
            return LLM_IMAGE_HISTORY_UNLIMITED_SLIDER_VALUE;
        }

        return llmImageHistoryLimit;
    });
    const formattedLlmImageHistoryLimit = $derived.by(() => {
        if (llmImageHistoryLimit === DEFAULT_LLM_IMAGE_HISTORY_LIMIT) {
            return "Unlimited";
        }

        return llmImageHistoryLimit === 1
            ? "1 image"
            : `${llmImageHistoryLimit} images`;
    });
    const llmImageHistoryProgress = $derived.by(() => {
        const range =
            LLM_IMAGE_HISTORY_UNLIMITED_SLIDER_VALUE -
            MIN_LLM_IMAGE_HISTORY_LIMIT;
        if (range <= 0) {
            return 0;
        }

        return (
            ((llmImageHistorySliderValue - MIN_LLM_IMAGE_HISTORY_LIMIT) /
                range) *
            100
        );
    });
    const minimumLlmImageHistoryLabel = `${MIN_LLM_IMAGE_HISTORY_LIMIT}`;
    const maximumLlmImageHistoryLabel = "Unlimited";

    onDestroy(() => {
        clearCopyFeedback();
    });

    const settingsData = $derived([
        {
            id: "pop-sound",
            type: "toggle",
            label: "Enable Pop Sound (Screenshots / Processing Audio / Finished Response)",
            value: pongPlaybackEnabled,
            onUpdate: onUpdatePongPlayback,
        },
        {
            id: "auto-unmute",
            type: "toggle",
            label: "Auto-Unmute After Attaching Screenshot",
            value: autoUnmuteOnPastedScreenshotEnabled,
            onUpdate: onUpdateAutoUnmuteOnPastedScreenshot,
        },
        {
            id: "last-session",
            type: "toggle",
            label: "Select Last Session When Startup",
            value: selectLastSessionEnabled,
            onUpdate: onUpdateSelectLastSession,
        },
        {
            id: "show-stats",
            type: "toggle",
            label: "Show Stats (Latency, Memory Usage) [Experimental]",
            value: showStatEnabled,
            onUpdate: onUpdateShowStat,
        },
        {
            id: "subtitles",
            type: "toggle",
            label: "Show Subtitles (Live Transcript)",
            value: showSubtitleEnabled,
            onUpdate: onUpdateShowSubtitle,
        },
        {
            id: "ai-subtitle",
            type: "toggle",
            label: "Show AI Subtitle",
            value: showAiSubtitleEnabled,
            onUpdate: onUpdateShowAiSubtitle,
        },
        {
            id: "call-timer",
            type: "toggle",
            label: "Show Call Timer",
            value: showCallTimerEnabled,
            onUpdate: onUpdateShowCallTimer,
        },
        {
            id: "overlay",
            type: "toggle",
            label: "Show Hidden-Window Overlay (Toasts, Live Transcript, AI Subtitle)",
            value: showHiddenWindowOverlayEnabled,
            onUpdate: onUpdateShowHiddenWindowOverlay,
        },
        {
            id: "stt-silence",
            type: "slider",
            label: "Silence Before Sending Audio to STT",
            detail: "Longer waits capture more pause-heavy speech before transcription starts. The minimum stays conservative to avoid mid-sentence cutoffs.",
            value: endOfUtteranceSilenceMs,
            min: MIN_END_OF_UTTERANCE_SILENCE_MS,
            max: MAX_END_OF_UTTERANCE_SILENCE_MS,
            step: END_OF_UTTERANCE_SILENCE_STEP_MS,
            displayValue: formattedEndOfUtteranceSilence,
            progress: endOfUtteranceSilenceProgress,
            minLabel: minimumEndOfUtteranceSilenceLabel,
            maxLabel: maximumEndOfUtteranceSilenceLabel,
            onUpdate: onUpdateEndOfUtteranceSilenceMs,
        },
        {
            id: "auto-continue",
            type: "slider",
            label: "AI Auto-Continue After Silence",
            detail: "After the assistant finishes speaking, wait this long with no user speech before it adds a short continuation to the same assistant message.",
            value: autoContinueSilenceSliderValue,
            min: MIN_AUTO_CONTINUE_SILENCE_MS,
            max: AUTO_CONTINUE_NEVER_SLIDER_VALUE,
            step: AUTO_CONTINUE_SILENCE_STEP_MS,
            displayValue: formattedAutoContinueSilence,
            progress: autoContinueSilenceProgress,
            minLabel: minimumAutoContinueSilenceLabel,
            maxLabel: maximumAutoContinueSilenceLabel,
            onUpdate: (val: number) =>
                onUpdateAutoContinueSilenceMs(
                    val >= AUTO_CONTINUE_NEVER_SLIDER_VALUE
                        ? DEFAULT_AUTO_CONTINUE_SILENCE_MS
                        : val,
                ),
        },
        {
            id: "max-continues",
            type: "slider",
            label: "Max Auto-Continues Per Reply",
            detail: "Limits how many extra follow-up bursts the assistant can add to the same reply before waiting for you to speak.",
            value: autoContinueMaxCountSliderValue,
            min: MIN_AUTO_CONTINUE_MAX_COUNT,
            max: AUTO_CONTINUE_MAX_COUNT_CONTINUOUS_SLIDER_VALUE,
            step: 1,
            displayValue: autoContinueMaxCountDisabled
                ? "Disabled"
                : formattedAutoContinueMaxCount,
            progress: autoContinueMaxCountProgress,
            minLabel: minimumAutoContinueMaxCountLabel,
            maxLabel: maximumAutoContinueMaxCountLabel,
            disabled: autoContinueMaxCountDisabled,
            warning: showContinuousAutoContinueWarning
                ? "Warning: Continuous can keep the assistant talking indefinitely until you interrupt it or speak."
                : null,
            onUpdate: (val: number) =>
                onUpdateAutoContinueMaxCount(
                    val >= AUTO_CONTINUE_MAX_COUNT_CONTINUOUS_SLIDER_VALUE
                        ? null
                        : Math.min(
                              MAX_AUTO_CONTINUE_MAX_COUNT,
                              Math.max(MIN_AUTO_CONTINUE_MAX_COUNT, val),
                          ),
                ),
        },
        {
            id: "context-turns",
            type: "slider",
            label: "Last Conversation Turns Visible to AI",
            detail: "Caps how many recent back-and-forth turns the model can inspect across the active conversation context. Move it to Unlimited to keep the full conversation history.",
            value: llmContextTurnSliderValue,
            min: MIN_LLM_CONTEXT_TURN_LIMIT,
            max: LLM_CONTEXT_TURN_LIMIT_UNLIMITED_SLIDER_VALUE,
            step: 1,
            displayValue: formattedLlmContextTurnLimit,
            progress: llmContextTurnProgress,
            minLabel: minimumLlmContextTurnLabel,
            maxLabel: maximumLlmContextTurnLabel,
            onUpdate: (val: number) =>
                onUpdateLlmContextTurnLimit(
                    val >= LLM_CONTEXT_TURN_LIMIT_UNLIMITED_SLIDER_VALUE
                        ? null
                        : Math.min(
                              MAX_LLM_CONTEXT_TURN_LIMIT,
                              Math.max(MIN_LLM_CONTEXT_TURN_LIMIT, val),
                          ),
                ),
        },
        {
            id: "image-history",
            type: "slider",
            label: "Last Images Visible to LLM",
            detail: "Caps how many recent screenshots the model can inspect across the active conversation context. Move it to Unlimited to keep every image that still fits in the context window.",
            value: llmImageHistorySliderValue,
            min: MIN_LLM_IMAGE_HISTORY_LIMIT,
            max: LLM_IMAGE_HISTORY_UNLIMITED_SLIDER_VALUE,
            step: 1,
            displayValue: formattedLlmImageHistoryLimit,
            progress: llmImageHistoryProgress,
            minLabel: minimumLlmImageHistoryLabel,
            maxLabel: maximumLlmImageHistoryLabel,
            onUpdate: (val: number) =>
                onUpdateLlmImageHistoryLimit(
                    val >= LLM_IMAGE_HISTORY_UNLIMITED_SLIDER_VALUE
                        ? DEFAULT_LLM_IMAGE_HISTORY_LIMIT
                        : Math.min(
                              MAX_LLM_IMAGE_HISTORY_LIMIT,
                              Math.max(MIN_LLM_IMAGE_HISTORY_LIMIT, val),
                          ),
                ),
        },
        {
            id: "shortcut-region",
            type: "shortcut",
            label: "Look at Screen Region (During Call)",
            value: editedShortcut,
            onUpdate: (val: string) => onUpdateGlobalShortcut(val),
            onRemove: () => onUpdateGlobalShortcut(NO_GLOBAL_SHORTCUT),
            onDefault: () => onUpdateGlobalShortcut(DEFAULT_GLOBAL_SHORTCUT),
        },
        {
            id: "shortcut-entire",
            type: "shortcut",
            label: "Look at Entire Screen (During Call)",
            value: editedShortcutEntireScreen,
            onUpdate: (val: string) => onUpdateGlobalShortcutEntireScreen(val),
            onRemove: () =>
                onUpdateGlobalShortcutEntireScreen(NO_GLOBAL_SHORTCUT),
            onDefault: () =>
                onUpdateGlobalShortcutEntireScreen(
                    DEFAULT_GLOBAL_SHORTCUT_ENTIRE_SCREEN,
                ),
        },
        {
            id: "shortcut-mute",
            type: "shortcut",
            label: "Toggle Mute / Unmute (During Call)",
            value: editedShortcutToggleMute,
            onUpdate: (val: string) => onUpdateGlobalShortcutToggleMute(val),
            onRemove: () =>
                onUpdateGlobalShortcutToggleMute(NO_GLOBAL_SHORTCUT),
            onDefault: () =>
                onUpdateGlobalShortcutToggleMute(
                    DEFAULT_GLOBAL_SHORTCUT_TOGGLE_MUTE,
                ),
        },
        {
            id: "shortcut-interrupt",
            type: "shortcut",
            label: "Interrupt Speech (During Call)",
            value: editedShortcutInterrupt,
            onUpdate: (val: string) => onUpdateGlobalShortcutInterrupt(val),
            onRemove: () =>
                onUpdateGlobalShortcutInterrupt(NO_GLOBAL_SHORTCUT),
            onDefault: () =>
                onUpdateGlobalShortcutInterrupt(
                    DEFAULT_GLOBAL_SHORTCUT_INTERRUPT,
                ),
        },
    ]);

    const filteredSettings = $derived(
        settingsData.filter((s) => matchesSearch(s.label, s.detail)),
    );

    const hasMatches = $derived(filteredSettings.length > 0);
</script>

<button
    type="button"
    class="about-modal-backdrop"
    aria-label="Close Settings"
    onclick={closeAboutPopup}
></button>

<div
    id="about-popup"
    class="about-modal"
    role="dialog"
    aria-labelledby="about-modal-title"
    aria-modal="true"
>
    <div class="about-modal-header">
        <div class="about-modal-copy">
            <span class="about-modal-title" id="about-modal-title"
                >Settings</span
            >
            <span class="about-modal-subtitle"
                >App configuration and build metadata</span
            >
        </div>
        <button
            type="button"
            class="conversation-close-btn"
            onclick={closeAboutPopup}
            aria-label="Close Settings"
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
    <div class="about-modal-content">
        {#if buildInfo}
            <div class="about-hero">
                <div class="about-hero-copy">
                    <div class="about-app-name-row">
                        <span class="about-app-name">{buildInfo.app_name}</span>
                        {#if buildInfo.version_label}
                            <span class="about-version-label"
                                >{buildInfo.version_label}</span
                            >
                        {/if}
                    </div>
                    <span class="about-version-number"
                        >v{buildInfo.version}</span
                    >
                </div>
            </div>

            <div class="about-search-container">
                <div class="about-search-wrapper">
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        width="16"
                        height="16"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="2.5"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                        class="search-icon"
                        ><circle cx="11" cy="11" r="8" /><line
                            x1="21"
                            y1="21"
                            x2="16.65"
                            y2="16.65"
                        /></svg
                    >
                    <input
                        type="text"
                        class="about-search-input"
                        placeholder="Search settings..."
                        bind:value={searchQuery}
                    />
                    {#if searchQuery}
                        <button
                            type="button"
                            class="search-clear-btn"
                            onclick={() => (searchQuery = "")}
                            aria-label="Clear search"
                        >
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                width="14"
                                height="14"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="3"
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
                    {/if}
                </div>
            </div>

            <div class="about-metadata-card">
                {#each filteredSettings as setting (setting.id)}
                    {#if setting.type === "toggle"}
                        <div class="about-metadata-row">
                            <span class="about-metadata-label"
                                >{setting.label}</span
                            >
                            <button
                                type="button"
                                class="quantize-toggle"
                                class:active={setting.value}
                                onclick={() =>
                                    setting.onUpdate(!setting.value)}
                            >
                                <span class="quantize-dot"></span>
                                <span
                                    >{setting.value
                                        ? "Enabled"
                                        : "Disabled"}</span
                                >
                            </button>
                        </div>
                    {:else if setting.type === "slider"}
                        <div class="about-metadata-row slider-row">
                            <span class="about-metadata-label"
                                >{setting.label}</span
                            >
                            <div class="about-slider-control">
                                <div class="about-slider-header">
                                    {#if setting.detail}
                                        <span class="about-slider-detail"
                                            >{setting.detail}</span
                                        >
                                    {/if}
                                    <span class="about-slider-value"
                                        >{setting.displayValue}</span
                                    >
                                </div>
                                {#if setting.warning}
                                    <div class="about-slider-header">
                                        <span class="about-slider-detail"
                                            >{setting.warning}</span
                                        >
                                    </div>
                                {/if}
                                <div class="about-slider-surface">
                                    <input
                                        type="range"
                                        class="about-slider"
                                        min={setting.min}
                                        max={setting.max}
                                        step={setting.step}
                                        value={setting.value}
                                        style={`--slider-progress: ${setting.progress}%;`}
                                        aria-label={setting.label}
                                        disabled={setting.disabled}
                                        oninput={(event) =>
                                            setting.onUpdate(
                                                Number(
                                                    (
                                                        event.currentTarget as HTMLInputElement
                                                    ).value,
                                                ),
                                            )}
                                    />
                                    <div
                                        class="about-slider-scale"
                                        aria-hidden="true"
                                    >
                                        <span>{setting.minLabel}</span>
                                        <span>{setting.maxLabel}</span>
                                    </div>
                                </div>
                            </div>
                        </div>
                    {:else if setting.type === "shortcut"}
                        <div class="about-metadata-row shortcut-row">
                            <span class="about-metadata-label"
                                >{setting.label}</span
                            >
                            <div class="shortcut-input-wrapper">
                                <ShortcutCapture
                                    value={setting.value}
                                    onUpdate={(newValue) => {
                                        setting.onUpdate(newValue);
                                    }}
                                    onRemove={() => {
                                        setting.onRemove();
                                    }}
                                    onDefault={() => {
                                        setting.onDefault();
                                    }}
                                />
                            </div>
                        </div>
                    {/if}
                {/each}

                {#if !hasMatches}
                    <div class="search-empty-state">
                        <span class="about-empty-title">No settings found</span>
                        <span class="about-empty-detail"
                            >Try a different search term.</span
                        >
                    </div>
                {/if}
            </div>

            {#if !calling}
                <div class="about-update-card">
                    <div class="about-update-header">
                        <div class="about-update-copy">
                            <span class="about-update-title">Runtime Cache</span
                            >
                            <span class="about-update-detail"
                                >Clear local Python environment and bootstrap
                                caches.</span
                            >
                        </div>
                        <button
                            type="button"
                            class="utility-btn"
                            onclick={refreshRuntimeCaches}
                            disabled={isRefreshing}
                        >
                            <div class="refresh-btn-content">
                                {#if isRefreshing}
                                    <svg
                                        class="spinner"
                                        xmlns="http://www.w3.org/2000/svg"
                                        width="14"
                                        height="14"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="3"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                    >
                                        <path d="M21 12a9 9 0 1 1-6.219-8.56" />
                                    </svg>
                                {/if}
                                Refresh Caches
                            </div>
                        </button>
                    </div>

                    <div class="about-update-header">
                        <div class="about-update-copy">
                            <span class="about-update-title">Updates</span>
                            <span class="about-update-detail"
                                >{updateStatusDetail}</span
                            >
                        </div>
                        <button
                            type="button"
                            class="utility-btn about-update-check-btn"
                            onclick={checkForUpdates}
                            disabled={updateActionDisabled}
                        >
                            {checkButtonLabel}
                        </button>
                    </div>

                    {#if availableAppUpdate}
                        <div class="about-update-metadata">
                            <div class="about-update-row">
                                <span class="about-update-label"
                                    >Latest Version</span
                                >
                                <span
                                    class="about-update-value about-metadata-mono"
                                >
                                    {availableAppUpdate.version}
                                </span>
                            </div>
                            <div class="about-update-row">
                                <span class="about-update-label"
                                    >Current Version</span
                                >
                                <span
                                    class="about-update-value about-metadata-mono"
                                >
                                    {availableAppUpdate.currentVersion}
                                </span>
                            </div>
                            {#if formattedPublishedAt}
                                <div class="about-update-row">
                                    <span class="about-update-label"
                                        >Published</span
                                    >
                                    <span class="about-update-value"
                                        >{formattedPublishedAt}</span
                                    >
                                </div>
                            {/if}
                            <div class="about-update-row">
                                <span class="about-update-label">Target</span>
                                <span
                                    class="about-update-value about-metadata-mono"
                                >
                                    {availableAppUpdate.target}
                                </span>
                            </div>
                        </div>

                        {#if availableAppUpdate.notes}
                            <div class="about-update-notes">
                                {availableAppUpdate.notes}
                            </div>
                        {/if}

                        <div class="about-update-actions">
                            {#if appUpdateStatus === "available"}
                                <button
                                    type="button"
                                    class="utility-btn about-update-install-btn"
                                    onclick={handleDownloadFromGithub}
                                >
                                    Download from Github
                                </button>
                            {:else if appUpdateStatus === "installed"}
                                <button
                                    type="button"
                                    class="utility-btn about-update-install-btn"
                                    onclick={restartToApplyUpdate}
                                >
                                    Restart to Apply
                                </button>
                            {/if}
                        </div>
                    {:else if appUpdateError}
                        <div class="about-empty-state error">
                            <span class="about-empty-title"
                                >Update Check Failed</span
                            >
                            <span class="about-empty-detail"
                                >{appUpdateError}</span
                            >
                        </div>
                    {/if}
                </div>
            {/if}
        {:else if buildInfoError}
            <div class="about-empty-state error">
                <span class="about-empty-title">Unable to load build info</span>
                <span class="about-empty-detail">{buildInfoError}</span>
            </div>
        {:else}
            <div class="about-empty-state">
                <span class="about-empty-title">Loading build info...</span>
                <span class="about-empty-detail"
                    >Preparing version and commit metadata.</span
                >
            </div>
        {/if}
    </div>
</div>

<style>
    .about-metadata-label {
        font-size: 0.72rem;
        line-height: 1.35;
    }

    .about-search-wrapper {
        position: relative;
        display: flex;
        align-items: center;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 14px;
        padding: 0 12px;
        transition: all 0.2s ease;
    }

    .about-search-wrapper:focus-within {
        background: rgba(255, 255, 255, 0.06);
        border-color: rgba(255, 205, 64, 0.3);
        box-shadow: 0 0 0 4px rgba(255, 205, 64, 0.06);
    }

    .search-icon {
        color: rgba(255, 255, 255, 0.3);
        flex-shrink: 0;
    }

    .about-search-input {
        background: none;
        border: none;
        color: #fff;
        font-size: 0.82rem;
        height: 40px;
        width: 100%;
        padding: 0 10px;
        outline: none;
    }

    .about-search-input::placeholder {
        color: rgba(255, 255, 255, 0.25);
    }

    .search-clear-btn {
        background: rgba(255, 255, 255, 0.06);
        border: none;
        border-radius: 50%;
        color: rgba(255, 255, 255, 0.4);
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        height: 20px;
        width: 20px;
        padding: 0;
        flex-shrink: 0;
        transition: all 0.15s ease;
    }

    .search-clear-btn:hover {
        background: rgba(255, 255, 255, 0.1);
        color: #fff;
    }

    .search-empty-state {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        padding: 40px 20px;
        text-align: center;
        gap: 8px;
    }

    .about-slider-header {
        align-items: center;
        display: flex;
        gap: 12px;
        justify-content: space-between;
        margin-bottom: 10px;
    }

    .about-slider-control {
        min-width: 0;
    }

    .about-slider-surface {
        padding: 12px 14px 10px;
        border-radius: 16px;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.08);
        box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.04);
    }

    .about-slider {
        --slider-progress: 50%;
        --slider-fill-start: #ffdf63;
        --slider-fill-end: #ffcd40;
        --slider-track: rgba(255, 255, 255, 0.14);
        appearance: none;
        -webkit-appearance: none;
        width: 100%;
        height: 12px;
        border-radius: 999px;
        background: linear-gradient(
            90deg,
            var(--slider-fill-start) 0%,
            var(--slider-fill-end) var(--slider-progress),
            var(--slider-track) var(--slider-progress),
            var(--slider-track) 100%
        );
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
        cursor: pointer;
        transition:
            filter 0.18s ease,
            box-shadow 0.18s ease;
    }

    .about-slider:hover {
        filter: brightness(1.04);
    }

    .about-slider:focus-visible {
        outline: none;
        box-shadow:
            inset 0 0 0 1px rgba(255, 255, 255, 0.08),
            0 0 0 4px rgba(255, 205, 64, 0.14);
    }

    .about-slider::-webkit-slider-runnable-track {
        height: 12px;
        border-radius: 999px;
        background: transparent;
    }

    .about-slider::-webkit-slider-thumb {
        appearance: none;
        -webkit-appearance: none;
        width: 24px;
        height: 24px;
        margin-top: -6px;
        border-radius: 50%;
        border: 1px solid rgba(47, 37, 0, 0.36);
        background: linear-gradient(135deg, #fff2bf 0%, #ffcd40 100%);
        box-shadow:
            0 6px 16px rgba(255, 205, 64, 0.28),
            0 0 0 3px rgba(255, 205, 64, 0.1);
    }

    .about-slider::-moz-range-track {
        height: 12px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.14);
        box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.08);
    }

    .about-slider::-moz-range-progress {
        height: 12px;
        border-radius: 999px;
        background: linear-gradient(90deg, #ffdf63 0%, #ffcd40 100%);
    }

    .about-slider::-moz-range-thumb {
        width: 24px;
        height: 24px;
        border-radius: 50%;
        border: 1px solid rgba(47, 37, 0, 0.36);
        background: linear-gradient(135deg, #fff2bf 0%, #ffcd40 100%);
        box-shadow:
            0 6px 16px rgba(255, 205, 64, 0.28),
            0 0 0 3px rgba(255, 205, 64, 0.1);
        cursor: pointer;
    }

    .about-slider-scale {
        display: flex;
        justify-content: space-between;
        margin-top: 10px;
        color: rgba(255, 255, 255, 0.46);
        font-size: 0.7rem;
        font-weight: 700;
        letter-spacing: 0.04em;
        text-transform: uppercase;
    }

    .about-slider-value {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        padding: 5px 10px;
        border-radius: 999px;
        background: rgba(255, 205, 64, 0.1);
        border: 1px solid rgba(255, 205, 64, 0.22);
        color: #ffcd40;
        font-size: 0.78rem;
        font-variant-numeric: tabular-nums;
        font-weight: 800;
        letter-spacing: 0.04em;
        white-space: nowrap;
    }

    .about-slider-detail {
        color: rgba(255, 255, 255, 0.62);
        font-size: 0.76rem;
        line-height: 1.4;
        max-width: 44ch;
    }

    @media (max-width: 720px) {
        .about-slider-header {
            align-items: flex-start;
            flex-direction: column;
        }

        .about-slider-detail {
            max-width: none;
        }
    }
</style>
