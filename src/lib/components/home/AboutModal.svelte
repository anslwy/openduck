<!-- Custom About modal that shows build metadata such as version labels and commit hashes. -->
<script lang="ts">
    import { onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { ask } from "@tauri-apps/plugin-dialog";

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
        onUpdateGlobalShortcut,
        onUpdateGlobalShortcutEntireScreen,
        onUpdatePongPlayback,
        onUpdateSelectLastSession,
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
        onUpdateGlobalShortcut: (shortcut: string) => void;
        onUpdateGlobalShortcutEntireScreen: (shortcut: string) => void;
        onUpdatePongPlayback: (enabled: boolean) => void;
        onUpdateSelectLastSession: (enabled: boolean) => void;
    }>();

    let copyState = $state<"idle" | "copied" | "failed">("idle");
    let copyResetTimeout: ReturnType<typeof window.setTimeout> | null = null;
    let isRefreshing = $state(false);
    let editedShortcut = $state(globalShortcut);
    let editedShortcutEntireScreen = $state(globalShortcutEntireScreen);

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
            <span class="about-modal-title" id="about-modal-title">Settings</span>
            <span class="about-modal-subtitle">App configuration and build metadata</span>
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
                <span class="about-metadata-label">Enable Pop Sound (Screenshots / Processing Audio / Finished Response)</span>
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
                <span class="about-metadata-label">Select Last Session When Startup</span>
                <button
                    type="button"
                    class="quantize-toggle"
                    class:active={selectLastSessionEnabled}
                    onclick={() => onUpdateSelectLastSession(!selectLastSessionEnabled)}
                >
                    <span class="quantize-dot"></span>
                    <span>{selectLastSessionEnabled ? "Enabled" : "Disabled"}</span>
                </button>
            </div>
            <div class="about-metadata-row shortcut-row">
                <span class="about-metadata-label">Look at Screen Region (During Call)</span>
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
                <span class="about-metadata-label">Look at Entire Screen (During Call)</span>
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
        font-size: 10px;
    }
</style>
