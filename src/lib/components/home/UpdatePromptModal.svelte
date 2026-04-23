<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";

    import type {
        AppUpdateInfo,
        AppUpdateStatus,
    } from "$lib/openduck/types";

    const DEFAULT_RELEASE_NOTES_URL =
        "https://github.com/anslwy/openduck/releases/latest";

    let {
        availableAppUpdate,
        appUpdateStatus,
        appUpdateError,
        onInstall,
        onSkipVersion,
        onRemindLater,
    } = $props<{
        availableAppUpdate: AppUpdateInfo;
        appUpdateStatus: AppUpdateStatus;
        appUpdateError: string | null;
        onInstall: () => void;
        onSkipVersion: () => void;
        onRemindLater: () => void;
    }>();

    const actionDisabled = $derived(appUpdateStatus === "installing");
    const showSkipButton = $derived(appUpdateStatus !== "installed");
    const primaryActionLabel = $derived(
        appUpdateStatus === "installing"
            ? "Installing..."
            : appUpdateStatus === "installed"
              ? "Restart to Apply"
              : appUpdateStatus === "error"
                ? "Retry Install"
                : "Install Update",
    );
    const title = $derived(
        appUpdateStatus === "installed" ? "Update Installed" : "Update Available",
    );
    const subtitle = $derived(
        appUpdateStatus === "installed"
            ? `Version ${availableAppUpdate.version} is installed. Restart OpenDuck to finish applying it.`
            : `Version ${availableAppUpdate.version} is available for OpenDuck.`,
    );
    const remindLaterLabel = $derived(
        appUpdateStatus === "installed" ? "Later" : "Remind Later",
    );
    const effectiveReleaseNotesUrl = $derived(
        availableAppUpdate.releaseNotesUrl?.trim() || DEFAULT_RELEASE_NOTES_URL,
    );
    const formattedPublishedAt = $derived.by(() => {
        if (!availableAppUpdate.publishedAt) {
            return null;
        }

        const parsedDate = new Date(availableAppUpdate.publishedAt);
        if (Number.isNaN(parsedDate.valueOf())) {
            return availableAppUpdate.publishedAt;
        }

        return parsedDate.toLocaleString(undefined, {
            dateStyle: "medium",
            timeStyle: "short",
        });
    });

    async function openReleaseNotes() {
        try {
            await invoke("plugin:shell|open", {
                path: effectiveReleaseNotesUrl,
            });
        } catch (error) {
            console.error("Failed to open release notes:", error);
        }
    }

    function handleBackdropClick() {
        if (actionDisabled) {
            return;
        }

        onRemindLater();
    }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
    class="update-prompt-backdrop"
    role="presentation"
    onclick={handleBackdropClick}
>
    <div
        class="update-prompt-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="update-prompt-title"
        tabindex="-1"
        onclick={(event) => event.stopPropagation()}
    >
        <div class="update-prompt-header">
            <div class="update-prompt-copy">
                <span class="update-prompt-kicker">Software Update</span>
                <h2 class="update-prompt-title" id="update-prompt-title">
                    {title}
                </h2>
                <p class="update-prompt-subtitle">{subtitle}</p>
            </div>
            <span class="update-prompt-badge">v{availableAppUpdate.version}</span>
        </div>

        <div class="update-prompt-metadata">
            <div class="update-prompt-row">
                <span class="update-prompt-label">Current</span>
                <span class="update-prompt-value update-prompt-mono">
                    {availableAppUpdate.currentVersion || "Unknown"}
                </span>
            </div>
            <div class="update-prompt-row">
                <span class="update-prompt-label">Latest</span>
                <span class="update-prompt-value update-prompt-mono">
                    {availableAppUpdate.version}
                </span>
            </div>
            {#if formattedPublishedAt}
                <div class="update-prompt-row">
                    <span class="update-prompt-label">Published</span>
                    <span class="update-prompt-value">{formattedPublishedAt}</span>
                </div>
            {/if}
        </div>

        {#if availableAppUpdate.notes}
            <div class="update-prompt-notes">{availableAppUpdate.notes}</div>
        {/if}

        <button
            type="button"
            class="update-prompt-link"
            onclick={openReleaseNotes}
        >
            See the full release notes
        </button>

        {#if appUpdateError}
            <div class="update-prompt-error">{appUpdateError}</div>
        {/if}

        <div class="update-prompt-actions">
            {#if showSkipButton}
                <button
                    type="button"
                    class="utility-btn update-prompt-secondary"
                    onclick={onSkipVersion}
                    disabled={actionDisabled}
                >
                    Skip for this version
                </button>
            {/if}
            <button
                type="button"
                class="utility-btn update-prompt-secondary"
                onclick={onRemindLater}
                disabled={actionDisabled}
            >
                {remindLaterLabel}
            </button>
            <button
                type="button"
                class="utility-btn update-prompt-primary"
                onclick={onInstall}
                disabled={actionDisabled}
            >
                {primaryActionLabel}
            </button>
        </div>
    </div>
</div>

<style>
    .update-prompt-backdrop {
        position: fixed;
        inset: 0;
        z-index: 1200;
        display: flex;
        align-items: center;
        justify-content: center;
        padding: 20px;
        background: rgba(7, 7, 9, 0.56);
        backdrop-filter: blur(18px);
    }

    .update-prompt-modal {
        width: min(100%, 560px);
        max-height: min(calc(100vh - 40px), 760px);
        overflow-y: auto;
        display: flex;
        flex-direction: column;
        gap: 16px;
        padding: 24px;
        border-radius: 28px;
        background: linear-gradient(
            180deg,
            rgba(42, 38, 22, 0.98) 0%,
            rgba(16, 16, 18, 0.98) 100%
        );
        border: 1px solid rgba(255, 220, 102, 0.16);
        box-shadow:
            0 34px 80px rgba(0, 0, 0, 0.46),
            0 0 0 1px rgba(255, 255, 255, 0.03) inset;
    }

    .update-prompt-header {
        display: flex;
        align-items: flex-start;
        justify-content: space-between;
        gap: 16px;
    }

    .update-prompt-copy {
        display: flex;
        flex-direction: column;
        gap: 6px;
        min-width: 0;
    }

    .update-prompt-kicker {
        color: rgba(255, 217, 120, 0.9);
        font-size: 0.76rem;
        font-weight: 800;
        letter-spacing: 0.12em;
        text-transform: uppercase;
    }

    .update-prompt-title {
        margin: 0;
        color: rgba(255, 247, 221, 0.98);
        font-size: 1.45rem;
        font-weight: 800;
        letter-spacing: -0.03em;
    }

    .update-prompt-subtitle {
        margin: 0;
        color: rgba(255, 255, 255, 0.7);
        font-size: 0.94rem;
        line-height: 1.5;
    }

    .update-prompt-badge {
        flex-shrink: 0;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        padding: 8px 12px;
        border-radius: 999px;
        background: rgba(255, 205, 64, 0.14);
        border: 1px solid rgba(255, 205, 64, 0.26);
        color: #ffe8a3;
        font-size: 0.82rem;
        font-weight: 800;
        letter-spacing: 0.04em;
    }

    .update-prompt-metadata {
        display: grid;
        gap: 10px;
        padding: 16px;
        border-radius: 20px;
        background: rgba(255, 255, 255, 0.03);
        border: 1px solid rgba(255, 255, 255, 0.06);
    }

    .update-prompt-row {
        display: grid;
        grid-template-columns: minmax(88px, 112px) minmax(0, 1fr);
        gap: 12px;
        align-items: center;
    }

    .update-prompt-label {
        color: rgba(255, 255, 255, 0.52);
        font-size: 0.76rem;
        font-weight: 700;
        letter-spacing: 0.08em;
        text-transform: uppercase;
    }

    .update-prompt-value {
        color: rgba(255, 255, 255, 0.9);
        font-size: 0.92rem;
        line-height: 1.4;
        word-break: break-word;
    }

    .update-prompt-mono {
        font-family:
            ui-monospace,
            SFMono-Regular,
            SF Mono,
            Menlo,
            Monaco,
            Consolas,
            Liberation Mono,
            monospace;
    }

    .update-prompt-notes {
        padding: 14px 16px;
        border-radius: 20px;
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.05);
        color: rgba(255, 255, 255, 0.82);
        font-size: 0.9rem;
        line-height: 1.55;
        white-space: pre-wrap;
    }

    .update-prompt-link {
        align-self: flex-start;
        border: none;
        background: none;
        padding: 0;
        color: #ffd974;
        font-size: 0.9rem;
        font-weight: 700;
        letter-spacing: -0.01em;
        cursor: pointer;
    }

    .update-prompt-link:hover {
        color: #ffe6a2;
    }

    .update-prompt-error {
        padding: 12px 14px;
        border-radius: 18px;
        background: rgba(255, 112, 98, 0.12);
        border: 1px solid rgba(255, 112, 98, 0.24);
        color: #ffbbb4;
        font-size: 0.88rem;
        line-height: 1.45;
        white-space: pre-wrap;
    }

    .update-prompt-actions {
        display: flex;
        justify-content: flex-end;
        gap: 10px;
        flex-wrap: wrap;
    }

    .update-prompt-secondary {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.08);
    }

    .update-prompt-primary {
        background: linear-gradient(180deg, #ffd86d 0%, #f0b93c 100%);
        color: #1e1602;
        border: 1px solid rgba(255, 216, 109, 0.5);
        box-shadow: 0 16px 30px rgba(240, 185, 60, 0.18);
    }

    .update-prompt-primary:hover:not(:disabled) {
        background: linear-gradient(180deg, #ffe08a 0%, #f3c85d 100%);
    }

    @media (max-width: 640px) {
        .update-prompt-backdrop {
            padding: 12px;
        }

        .update-prompt-modal {
            padding: 20px;
            border-radius: 24px;
        }

        .update-prompt-header {
            flex-direction: column;
        }

        .update-prompt-badge {
            align-self: flex-start;
        }

        .update-prompt-row {
            grid-template-columns: minmax(0, 1fr);
            gap: 4px;
        }

        .update-prompt-actions {
            flex-direction: column-reverse;
        }

        .update-prompt-actions :global(.utility-btn) {
            width: 100%;
        }
    }
</style>
