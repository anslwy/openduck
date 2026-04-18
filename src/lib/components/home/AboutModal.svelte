<!-- Custom About modal that shows build metadata such as version labels and commit hashes. -->
<script lang="ts">
    import { onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { ask } from "@tauri-apps/plugin-dialog";

    import {
        DEFAULT_LLM_IMAGE_HISTORY_LIMIT,
        END_OF_UTTERANCE_SILENCE_STEP_MS,
        LLM_IMAGE_HISTORY_UNLIMITED_SLIDER_VALUE,
        MAX_END_OF_UTTERANCE_SILENCE_MS,
        MAX_LLM_IMAGE_HISTORY_LIMIT,
        MIN_END_OF_UTTERANCE_SILENCE_MS,
        MIN_LLM_IMAGE_HISTORY_LIMIT,
    } from "$lib/openduck/config";
    import type {
        AppUpdateInfo,
        AppUpdateStatus,
        BuildInfo,
    } from "$lib/openduck/types";
    import ShortcutCapture from "./ShortcutCapture.svelte";

    let {
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
        pongPlaybackEnabled,
        selectLastSessionEnabled,
        showStatEnabled,
        endOfUtteranceSilenceMs,
        llmImageHistoryLimit,
        onUpdateGlobalShortcut,
        onUpdateGlobalShortcutEntireScreen,
        onUpdatePongPlayback,
        onUpdateSelectLastSession,
        onUpdateShowStat,
        onUpdateEndOfUtteranceSilenceMs,
        onUpdateLlmImageHistoryLimit,
    } = $props<{
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
        pongPlaybackEnabled: boolean;
        selectLastSessionEnabled: boolean;
        showStatEnabled: boolean;
        endOfUtteranceSilenceMs: number;
        llmImageHistoryLimit: number | null;
        onUpdateGlobalShortcut: (shortcut: string) => void;
        onUpdateGlobalShortcutEntireScreen: (shortcut: string) => void;
        onUpdatePongPlayback: (enabled: boolean) => void;
        onUpdateSelectLastSession: (enabled: boolean) => void;
        onUpdateShowStat: (enabled: boolean) => void;
        onUpdateEndOfUtteranceSilenceMs: (milliseconds: number) => void;
        onUpdateLlmImageHistoryLimit: (limit: number | null) => void;
    }>();

    let copyState = $state<"idle" | "copied" | "failed">("idle");
    let copyResetTimeout: ReturnType<typeof window.setTimeout> | null = null;
    let isRefreshing = $state(false);
    let editedShortcut = $state("");
    let editedShortcutEntireScreen = $state("");

    $effect(() => {
        editedShortcut = globalShortcut;
    });

    $effect(() => {
        editedShortcutEntireScreen = globalShortcutEntireScreen;
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

    {#if buildInfo}
        <div class="about-hero">
            <img class="about-app-icon" src="/icon.png" alt="" />
            <div class="about-hero-copy">
                <div class="about-app-name-row">
                    <span class="about-app-name">{buildInfo.app_name}</span>
                    {#if buildInfo.version_label}
                        <span class="about-version-label"
                            >{buildInfo.version_label}</span
                        >
                    {/if}
                </div>
                <span class="about-version-number">{buildInfo.version}</span>
            </div>
        </div>

        <div class="about-metadata-card">
            <div class="about-metadata-row">
                <span class="about-metadata-label"
                    >Enable Pop Sound (Screenshots / Processing Audio / Finished
                    Response)</span
                >
                <button
                    type="button"
                    class="quantize-toggle"
                    class:active={pongPlaybackEnabled}
                    onclick={() => onUpdatePongPlayback(!pongPlaybackEnabled)}
                >
                    <span class="quantize-dot"></span>
                    <span>{pongPlaybackEnabled ? "Enabled" : "Disabled"}</span>
                </button>
            </div>
            <div class="about-metadata-row">
                <span class="about-metadata-label"
                    >Select Last Session When Startup</span
                >
                <button
                    type="button"
                    class="quantize-toggle"
                    class:active={selectLastSessionEnabled}
                    onclick={() =>
                        onUpdateSelectLastSession(!selectLastSessionEnabled)}
                >
                    <span class="quantize-dot"></span>
                    <span
                        >{selectLastSessionEnabled
                            ? "Enabled"
                            : "Disabled"}</span
                    >
                </button>
            </div>
            <div class="about-metadata-row">
                <span class="about-metadata-label"
                    >Show Stats (Latency, Memory Usage) [Experimental]</span
                >
                <button
                    type="button"
                    class="quantize-toggle"
                    class:active={showStatEnabled}
                    onclick={() => onUpdateShowStat(!showStatEnabled)}
                >
                    <span class="quantize-dot"></span>
                    <span>{showStatEnabled ? "Enabled" : "Disabled"}</span>
                </button>
            </div>
            <div class="about-metadata-row slider-row">
                <span class="about-metadata-label"
                    >Silence Before Sending Audio to STT</span
                >
                <div class="about-slider-control">
                    <div class="about-slider-header">
                        <span class="about-slider-detail"
                            >Longer waits capture more pause-heavy speech before
                            transcription starts. The minimum stays conservative
                            to avoid mid-sentence cutoffs.</span
                        >
                        <span class="about-slider-value"
                            >{formattedEndOfUtteranceSilence}</span
                        >
                    </div>
                    <div class="about-slider-surface">
                        <input
                            type="range"
                            class="about-slider"
                            min={MIN_END_OF_UTTERANCE_SILENCE_MS}
                            max={MAX_END_OF_UTTERANCE_SILENCE_MS}
                            step={END_OF_UTTERANCE_SILENCE_STEP_MS}
                            value={endOfUtteranceSilenceMs}
                            style={`--slider-progress: ${endOfUtteranceSilenceProgress}%;`}
                            aria-label="Silence before sending audio to STT"
                            oninput={(event) =>
                                onUpdateEndOfUtteranceSilenceMs(
                                    Number(
                                        (
                                            event.currentTarget as HTMLInputElement
                                        ).value,
                                    ),
                                )}
                        />
                        <div class="about-slider-scale" aria-hidden="true">
                            <span>{minimumEndOfUtteranceSilenceLabel}</span>
                            <span>{maximumEndOfUtteranceSilenceLabel}</span>
                        </div>
                    </div>
                </div>
            </div>
            <div class="about-metadata-row slider-row">
                <span class="about-metadata-label"
                    >Last Images Visible to LLM</span
                >
                <div class="about-slider-control">
                    <div class="about-slider-header">
                        <span class="about-slider-detail"
                            >Caps how many recent screenshots the model can
                            inspect across the active conversation context. Move
                            it to Unlimited to keep every image that still fits
                            in the context window.</span
                        >
                        <span class="about-slider-value"
                            >{formattedLlmImageHistoryLimit}</span
                        >
                    </div>
                    <div class="about-slider-surface">
                        <input
                            type="range"
                            class="about-slider"
                            min={MIN_LLM_IMAGE_HISTORY_LIMIT}
                            max={LLM_IMAGE_HISTORY_UNLIMITED_SLIDER_VALUE}
                            step="1"
                            value={llmImageHistorySliderValue}
                            style={`--slider-progress: ${llmImageHistoryProgress}%;`}
                            aria-label="Last images visible to LLM"
                            oninput={(event) => {
                                const value = Number(
                                    (
                                        event.currentTarget as HTMLInputElement
                                    ).value,
                                );

                                onUpdateLlmImageHistoryLimit(
                                    value >=
                                        LLM_IMAGE_HISTORY_UNLIMITED_SLIDER_VALUE
                                        ? DEFAULT_LLM_IMAGE_HISTORY_LIMIT
                                        : Math.min(
                                              MAX_LLM_IMAGE_HISTORY_LIMIT,
                                              Math.max(
                                                  MIN_LLM_IMAGE_HISTORY_LIMIT,
                                                  value,
                                              ),
                                          ),
                                );
                            }}
                        />
                        <div class="about-slider-scale" aria-hidden="true">
                            <span>{minimumLlmImageHistoryLabel}</span>
                            <span>{maximumLlmImageHistoryLabel}</span>
                        </div>
                    </div>
                </div>
            </div>
            <div class="about-metadata-row shortcut-row">
                <span class="about-metadata-label"
                    >Look at Screen Region (During Call)</span
                >
                <div class="shortcut-input-wrapper">
                    <ShortcutCapture
                        value={editedShortcut}
                        onUpdate={(newValue) => {
                            editedShortcut = newValue;
                            onUpdateGlobalShortcut(newValue);
                        }}
                    />
                </div>
            </div>
            <div class="about-metadata-row shortcut-row">
                <span class="about-metadata-label"
                    >Look at Entire Screen (During Call)</span
                >
                <div class="shortcut-input-wrapper">
                    <ShortcutCapture
                        value={editedShortcutEntireScreen}
                        onUpdate={(newValue) => {
                            editedShortcutEntireScreen = newValue;
                            onUpdateGlobalShortcutEntireScreen(newValue);
                        }}
                    />
                </div>
            </div>
        </div>

        <div class="about-update-card">
            <div class="about-update-header">
                <div class="about-update-copy">
                    <span class="about-update-title">Runtime Cache</span>
                    <span class="about-update-detail"
                        >Clear local Python environment and bootstrap caches.</span
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
                    <span class="about-update-detail">{updateStatusDetail}</span
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
                        <span class="about-update-label">Latest Version</span>
                        <span class="about-update-value about-metadata-mono">
                            {availableAppUpdate.version}
                        </span>
                    </div>
                    <div class="about-update-row">
                        <span class="about-update-label">Current Version</span>
                        <span class="about-update-value about-metadata-mono">
                            {availableAppUpdate.currentVersion}
                        </span>
                    </div>
                    {#if formattedPublishedAt}
                        <div class="about-update-row">
                            <span class="about-update-label">Published</span>
                            <span class="about-update-value"
                                >{formattedPublishedAt}</span
                            >
                        </div>
                    {/if}
                    <div class="about-update-row">
                        <span class="about-update-label">Target</span>
                        <span class="about-update-value about-metadata-mono">
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
                    <span class="about-empty-title">Update Check Failed</span>
                    <span class="about-empty-detail">{appUpdateError}</span>
                </div>
            {/if}
        </div>
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

<style>
    .about-metadata-label {
        font-size: 0.72rem;
        line-height: 1.35;
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
