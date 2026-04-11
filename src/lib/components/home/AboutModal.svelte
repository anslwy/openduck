<!-- Custom About modal that shows build metadata such as version labels and commit hashes. -->
<script lang="ts">
    import { onDestroy } from "svelte";

    import type { BuildInfo } from "$lib/openduck/types";

    let {
        buildInfo,
        buildInfoError,
        closeAboutPopup,
    } = $props<{
        buildInfo: BuildInfo | null;
        buildInfoError: string | null;
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

    const copyButtonLabel = $derived(
        copyState === "copied"
            ? "Copied"
            : copyState === "failed"
              ? "Copy Failed"
              : "Copy",
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
            <span class="about-modal-subtitle"
                >Build and release metadata</span
            >
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
            <div class="about-metadata-row">
                <span class="about-metadata-label">Commit</span>
                <span class="about-metadata-value about-metadata-mono">
                    {buildInfo.git_sha ?? "Unavailable"}
                </span>
            </div>
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
                    <span class="about-metadata-value about-metadata-warning"
                        >Dirty</span
                    >
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
