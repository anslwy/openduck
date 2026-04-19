<script lang="ts">
    import { onDestroy, onMount } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { listen, type UnlistenFn } from "@tauri-apps/api/event";
    import {
        currentMonitor,
        cursorPosition,
        getCurrentWindow,
        monitorFromPoint,
        PhysicalPosition,
        PhysicalSize,
    } from "@tauri-apps/api/window";
    import {
        DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY,
        SHOW_SUBTITLE_STORAGE_KEY,
        SHOW_HIDDEN_WINDOW_OVERLAY_STORAGE_KEY,
    } from "$lib/openduck/config";
    import type {
        CallStageEvent,
        OverlayNotificationEvent,
        TranscriptEvent,
        TranscriptPartialEvent,
    } from "$lib/openduck/types";

    type OverlayToast = {
        id: number;
        message: string;
        timeoutId: ReturnType<typeof window.setTimeout>;
    };

    type StoredShowSubtitlePreference = {
        version: 1;
        enabled: boolean;
    };

    const OVERLAY_WINDOW_WIDTH_MIN = 420;
    const OVERLAY_WINDOW_WIDTH_MAX = 980;
    const OVERLAY_WINDOW_HEIGHT = 260;
    const OVERLAY_WINDOW_BOTTOM_MARGIN = 32;
    const OVERLAY_WINDOW_HORIZONTAL_MARGIN = 24;
    const OVERLAY_TOAST_DURATION_MS = 2200;
    const OVERLAY_SUBTITLE_DURATION_MS = 5_000;
    const OVERLAY_TOAST_LIMIT = 3;

    const overlayWindow = getCurrentWindow();
    let eventUnlisteners: UnlistenFn[] = [];
    let mainWindowVisibilityPoll: ReturnType<typeof window.setInterval> | null =
        null;
    let overlaySubtitleTimeout: ReturnType<typeof window.setTimeout> | null =
        null;
    let nextToastId = 1;
    let mainWindowVisible = $state(true);
    let showSubtitleEnabled = $state(true);
    let showHiddenWindowOverlayEnabled = $state(
        DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY,
    );
    let callStagePhase = $state<CallStageEvent["phase"]>("idle");
    let liveTranscriptSubtitle = $state("");
    let overlayToasts = $state<OverlayToast[]>([]);

    const showOverlaySubtitle = $derived(
        showHiddenWindowOverlayEnabled &&
            !mainWindowVisible &&
            showSubtitleEnabled &&
            liveTranscriptSubtitle.trim().length > 0,
    );
    const showOverlay = $derived(
        showHiddenWindowOverlayEnabled &&
            !mainWindowVisible &&
            (overlayToasts.length > 0 || showOverlaySubtitle),
    );

    function loadShowSubtitlePreferenceFromStorage() {
        const rawPayload = window.localStorage.getItem(SHOW_SUBTITLE_STORAGE_KEY);
        if (!rawPayload) {
            return true;
        }

        try {
            const parsed = JSON.parse(
                rawPayload,
            ) as Partial<StoredShowSubtitlePreference>;
            return parsed.version === 1 && typeof parsed.enabled === "boolean"
                ? parsed.enabled
                : true;
        } catch (err) {
            console.error(
                "Failed to restore overlay show subtitle preference:",
                err,
            );
            return true;
        }
    }

    function loadShowHiddenWindowOverlayPreferenceFromStorage() {
        const rawPayload = window.localStorage.getItem(
            SHOW_HIDDEN_WINDOW_OVERLAY_STORAGE_KEY,
        );
        if (!rawPayload) {
            return DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY;
        }

        try {
            const parsed = JSON.parse(
                rawPayload,
            ) as Partial<StoredShowSubtitlePreference>;
            return parsed.version === 1 && typeof parsed.enabled === "boolean"
                ? parsed.enabled
                : DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY;
        } catch (err) {
            console.error(
                "Failed to restore hidden-window overlay preference:",
                err,
            );
            return DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY;
        }
    }

    function clearToastTimeout(toast: OverlayToast) {
        window.clearTimeout(toast.timeoutId);
    }

    function removeOverlayToast(id: number) {
        const toast = overlayToasts.find((entry) => entry.id === id);
        if (!toast) {
            return;
        }

        clearToastTimeout(toast);
        overlayToasts = overlayToasts.filter((entry) => entry.id !== id);
    }

    function clearOverlayToasts() {
        for (const toast of overlayToasts) {
            clearToastTimeout(toast);
        }
        overlayToasts = [];
    }

    function clearOverlaySubtitleTimeout() {
        if (overlaySubtitleTimeout) {
            clearTimeout(overlaySubtitleTimeout);
            overlaySubtitleTimeout = null;
        }
    }

    function clearOverlaySubtitle() {
        clearOverlaySubtitleTimeout();
        liveTranscriptSubtitle = "";
    }

    async function syncOverlayWindowBounds() {
        const cursor = await cursorPosition();
        const monitor =
            (await monitorFromPoint(cursor.x, cursor.y)) ??
            (await currentMonitor());

        if (!monitor) {
            return;
        }

        const width = Math.max(
            OVERLAY_WINDOW_WIDTH_MIN,
            Math.min(
                OVERLAY_WINDOW_WIDTH_MAX,
                monitor.workArea.size.width - OVERLAY_WINDOW_HORIZONTAL_MARGIN * 2,
            ),
        );
        const x =
            monitor.workArea.position.x +
            Math.round((monitor.workArea.size.width - width) / 2);
        const y =
            monitor.workArea.position.y +
            monitor.workArea.size.height -
            OVERLAY_WINDOW_HEIGHT -
            OVERLAY_WINDOW_BOTTOM_MARGIN;

        await Promise.all([
            overlayWindow.setSize(
                new PhysicalSize(width, OVERLAY_WINDOW_HEIGHT),
            ),
            overlayWindow.setPosition(new PhysicalPosition(x, y)),
        ]);
    }

    async function refreshMainWindowVisibility() {
        try {
            const nextVisible = await invoke<boolean>(
                "is_main_window_visible_to_user",
            );
            if (nextVisible === mainWindowVisible) {
                if (!nextVisible) {
                    await syncOverlayWindowBounds();
                }
                return;
            }

            mainWindowVisible = nextVisible;
            if (nextVisible) {
                clearOverlayToasts();
                return;
            }

            await syncOverlayWindowBounds();
        } catch (err) {
            console.error("Failed to refresh overlay window visibility:", err);
        }
    }

    async function enqueueOverlayToast(message: string) {
        const trimmedMessage = message.trim();
        if (!trimmedMessage) {
            return;
        }

        await refreshMainWindowVisibility();
        if (mainWindowVisible || !showHiddenWindowOverlayEnabled) {
            return;
        }

        await syncOverlayWindowBounds();

        const nextToast: OverlayToast = {
            id: nextToastId++,
            message: trimmedMessage,
            timeoutId: window.setTimeout(() => {
                removeOverlayToast(nextToast.id);
            }, OVERLAY_TOAST_DURATION_MS),
        };

        const nextToasts = [...overlayToasts, nextToast];
        while (nextToasts.length > OVERLAY_TOAST_LIMIT) {
            const removedToast = nextToasts.shift();
            if (removedToast) {
                clearToastTimeout(removedToast);
            }
        }

        overlayToasts = nextToasts;
    }

    async function applyTranscriptSubtitle(text: string) {
        const nextText = text.trim();
        if (!nextText || !showHiddenWindowOverlayEnabled) {
            return;
        }

        liveTranscriptSubtitle = nextText;
        clearOverlaySubtitleTimeout();
        overlaySubtitleTimeout = window.setTimeout(() => {
            clearOverlaySubtitle();
        }, OVERLAY_SUBTITLE_DURATION_MS);
        if (!mainWindowVisible) {
            await syncOverlayWindowBounds();
        }
    }

    function applyCallStage(payload: CallStageEvent) {
        callStagePhase = payload.phase;

        if (payload.phase === "speaking" || payload.phase === "idle") {
            clearOverlaySubtitle();
        }
    }

    onMount(() => {
        const handleStorage = (event: StorageEvent) => {
            if (event.key === SHOW_SUBTITLE_STORAGE_KEY) {
                showSubtitleEnabled = loadShowSubtitlePreferenceFromStorage();
                return;
            }

            if (event.key === SHOW_HIDDEN_WINDOW_OVERLAY_STORAGE_KEY) {
                showHiddenWindowOverlayEnabled =
                    loadShowHiddenWindowOverlayPreferenceFromStorage();
                if (!showHiddenWindowOverlayEnabled) {
                    clearOverlayToasts();
                    clearOverlaySubtitle();
                }
            }
        };

        showSubtitleEnabled = loadShowSubtitlePreferenceFromStorage();
        showHiddenWindowOverlayEnabled =
            loadShowHiddenWindowOverlayPreferenceFromStorage();
        window.addEventListener("storage", handleStorage);

        void (async () => {
            try {
                await Promise.all([
                    overlayWindow.setAlwaysOnTop(true),
                    overlayWindow.setSkipTaskbar(true),
                    overlayWindow.setFocusable(false),
                    overlayWindow.setIgnoreCursorEvents(true),
                    overlayWindow.setVisibleOnAllWorkspaces(true),
                    overlayWindow.setShadow(false),
                ]);
            } catch (err) {
                console.error("Failed to configure overlay window:", err);
            }

            await refreshMainWindowVisibility();
            mainWindowVisibilityPoll = window.setInterval(() => {
                void refreshMainWindowVisibility();
            }, 500);

            try {
                eventUnlisteners = await Promise.all([
                    listen<OverlayNotificationEvent>(
                        "overlay-notification",
                        ({ payload }) => {
                            void enqueueOverlayToast(payload.message);
                        },
                    ),
                    listen<CallStageEvent>("call-stage", ({ payload }) => {
                        applyCallStage(payload);
                    }),
                    listen<TranscriptPartialEvent>(
                        "transcript-partial",
                        ({ payload }) => {
                            void applyTranscriptSubtitle(payload.text);
                        },
                    ),
                    listen<TranscriptEvent>("transcript-ready", ({ payload }) => {
                        void applyTranscriptSubtitle(payload.text);
                    }),
                ]);
            } catch (err) {
                console.error(
                    "Failed to register overlay Tauri event listeners:",
                    err,
                );
            }
        })();

        return () => {
            window.removeEventListener("storage", handleStorage);
        };
    });

    onDestroy(() => {
        if (mainWindowVisibilityPoll) {
            clearInterval(mainWindowVisibilityPoll);
            mainWindowVisibilityPoll = null;
        }

        clearOverlayToasts();
        clearOverlaySubtitleTimeout();

        for (const unlisten of eventUnlisteners) {
            unlisten();
        }
        eventUnlisteners = [];
    });
</script>

{#if showOverlay}
    <div class="overlay-root" aria-live="polite" aria-atomic="true">
        <div class="overlay-stack">
            {#if overlayToasts.length > 0}
                <div class="overlay-toast-list">
                    {#each overlayToasts as toast (toast.id)}
                        <div class="overlay-toast">
                            <span class="overlay-toast-text"
                                >{toast.message}</span
                            >
                        </div>
                    {/each}
                </div>
            {/if}

            {#if showOverlaySubtitle}
                <div class="overlay-subtitle">
                    <span class="overlay-subtitle-text"
                        >{liveTranscriptSubtitle}</span
                    >
                </div>
            {/if}
        </div>
    </div>
{/if}

<style>
    :global(html),
    :global(body) {
        margin: 0;
        width: 100%;
        height: 100%;
        background: transparent !important;
        overflow: hidden;
    }

    :global(body) {
        user-select: none;
        cursor: default;
        -webkit-user-select: none;
        -webkit-tap-highlight-color: transparent;
        font-family:
            -apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue",
            Arial, sans-serif;
    }

    .overlay-root {
        position: fixed;
        inset: 0;
        display: flex;
        align-items: flex-end;
        justify-content: center;
        padding: 24px;
        box-sizing: border-box;
        pointer-events: none;
    }

    .overlay-stack {
        width: min(100%, 980px);
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 12px;
    }

    .overlay-toast-list {
        width: 100%;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 10px;
    }

    .overlay-toast,
    .overlay-subtitle {
        width: max-content;
        max-width: min(100%, 960px);
        box-sizing: border-box;
        padding: 12px 16px;
        border-radius: 18px;
        background: rgba(14, 15, 11, 0.82);
        border: 1px solid rgba(127, 227, 124, 0.18);
        box-shadow:
            0 12px 30px rgba(0, 0, 0, 0.3),
            0 0 0 1px rgba(255, 255, 255, 0.03) inset;
        backdrop-filter: blur(14px);
        text-align: center;
        animation: overlay-rise 0.18s ease-out;
    }

    .overlay-toast {
        background: rgba(16, 17, 14, 0.88);
    }

    .overlay-toast-text,
    .overlay-subtitle-text {
        display: block;
        white-space: pre-wrap;
        overflow-wrap: anywhere;
        color: rgba(255, 255, 255, 0.96);
        text-shadow: 0 2px 12px rgba(0, 0, 0, 0.55);
    }

    .overlay-toast-text {
        font-size: 0.98rem;
        font-weight: 760;
        letter-spacing: -0.012em;
        line-height: 1.24;
    }

    .overlay-subtitle-text {
        font-size: 1.06rem;
        font-weight: 750;
        letter-spacing: -0.015em;
        line-height: 1.25;
    }

    @keyframes overlay-rise {
        from {
            opacity: 0;
            transform: translateY(8px);
        }
        to {
            opacity: 1;
            transform: translateY(0);
        }
    }
</style>
