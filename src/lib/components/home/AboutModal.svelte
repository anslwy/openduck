<!-- Custom About modal that shows build metadata such as version labels and commit hashes. -->
<script lang="ts">
    import { onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";

    import type {
        AppUpdateInfo,
        AppUpdateStatus,
        BuildInfo,
    } from "$lib/openduck/types";

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
    }>();

    let copyState = $state<"idle" | "copied" | "failed">("idle");
    let copyResetTimeout: ReturnType<typeof window.setTimeout> | null = null;

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

    async function handleCopyBuildInfo() {
        if (!buildInfo?.copy_text) {
            return;
        }

        try {
            await navigator.clipboard.writeText(buildInfo.copy_text);
            copyState = "copied";
        } catch (error) {
            console.error("Failed to copy build info:", error);
            copyState = "failed";
        }

        queueCopyFeedbackReset();
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
    aria-label="Close About OpenDuck"
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
            <span class="about-modal-title" id="about-modal-title">About</span>
            <span class="about-modal-subtitle">Build and release metadata</span>
        </div>
        <button
            type="button"
            class="conversation-close-btn"
            onclick={closeAboutPopup}
            aria-label="Close About OpenDuck"
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
            {#if buildInfo.git_sha}
                <div class="about-metadata-row">
                    <span class="about-metadata-label">Commit</span>
                    <span class="about-metadata-value about-metadata-mono">
                        {buildInfo.git_sha}
                    </span>
                </div>
            {/if}
            <div class="about-metadata-row">
                <span class="about-metadata-label">Build ID</span>
                <span class="about-metadata-value about-metadata-mono">
                    {buildInfo.build_id ?? buildInfo.version}
                </span>
            </div>
            {#if buildInfo.build_channel}
                <div class="about-metadata-row">
                    <span class="about-metadata-label">Channel</span>
                    <span class="about-metadata-value"
                        >{buildInfo.build_channel}</span
                    >
                </div>
            {/if}
            {#if buildInfo.build_number}
                <div class="about-metadata-row">
                    <span class="about-metadata-label">Build Number</span>
                    <span class="about-metadata-value"
                        >{buildInfo.build_number}</span
                    >
                </div>
            {/if}
            {#if buildInfo.is_dirty}
                <div class="about-metadata-row">
                    <span class="about-metadata-label">Working Tree</span>
                    <div class="about-metadata-value-stack">
                        <span class="about-metadata-value about-metadata-warning"
                            >Local Changes</span
                        >
                        {#if buildInfo.dirty_files && buildInfo.dirty_files.length > 0}
                            <div class="about-metadata-files">
                                {#each buildInfo.dirty_files as file}
                                    <div class="about-metadata-file">{file}</div>
                                {/each}
                            </div>
                        {/if}
                    </div>
                </div>
            {/if}
        </div>

        <div class="about-update-card">
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

        <div class="about-modal-actions">
            <button
                type="button"
                class="utility-btn about-copy-btn"
                onclick={handleCopyBuildInfo}
            >
                {copyButtonLabel}
            </button>
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
