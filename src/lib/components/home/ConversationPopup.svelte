<!-- Floating live transcript panel shown during active calls. -->
<script lang="ts">
    import type { ConversationLogEntry } from "$lib/openduck/types";

    let {
        conversationLogEntries,
        closeConversationPopup,
        setConversationLogViewport,
    } = $props<{
        conversationLogEntries: ConversationLogEntry[];
        closeConversationPopup: () => void;
        setConversationLogViewport: (element: HTMLDivElement | null) => void;
    }>();

    let viewport: HTMLDivElement | null = null;

    $effect(() => {
        setConversationLogViewport(viewport);

        return () => {
            setConversationLogViewport(null);
        };
    });
</script>

<div
    id="conversation-log-popup"
    class="conversation-popup"
    role="dialog"
    aria-label="Conversation log"
    aria-modal="false"
>
    <div class="conversation-popup-header">
        <div class="conversation-popup-copy">
            <span class="conversation-popup-title">Conversation</span>
            <span class="conversation-popup-subtitle">Live call transcript</span>
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

    <div class="conversation-log" bind:this={viewport}>
        {#if conversationLogEntries.length === 0}
            <div class="conversation-empty">
                Start talking and the transcript will appear here.
            </div>
        {:else}
            {#each conversationLogEntries as entry (entry.id)}
                <div class="conversation-entry" data-role={entry.role}>
                    <div class="conversation-bubble" data-role={entry.role}>
                        {#if entry.imageUrl}
                            <img
                                class="conversation-attachment-image"
                                src={entry.imageUrl}
                                alt="Attached screen capture"
                                loading="lazy"
                            />
                        {/if}
                        {#if entry.text}
                            <div class="conversation-entry-text">{entry.text}</div>
                        {/if}
                    </div>
                </div>
            {/each}
        {/if}
    </div>
</div>
