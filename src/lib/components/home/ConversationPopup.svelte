<!-- Floating live transcript panel shown during active calls. -->
<script lang="ts">
    import type { ConversationLogEntry } from "$lib/openduck/types";

    let {
        conversationLogEntries,
        closeConversationPopup,
        isSavingConversationLogEntryEdit,
        saveConversationLogEntryEdit,
        setConversationLogViewport,
    } = $props<{
        conversationLogEntries: ConversationLogEntry[];
        closeConversationPopup: () => void;
        isSavingConversationLogEntryEdit: boolean;
        saveConversationLogEntryEdit: (
            entryId: number,
            nextText: string,
            clearImage: boolean,
        ) => Promise<boolean>;
        setConversationLogViewport: (element: HTMLDivElement | null) => void;
    }>();

    let viewport: HTMLDivElement | null = null;
    let editingEntryId = $state<number | null>(null);
    let editingText = $state("");
    let editingImageRemoved = $state(false);

    $effect(() => {
        setConversationLogViewport(viewport);

        return () => {
            setConversationLogViewport(null);
        };
    });

    $effect(() => {
        if (editingEntryId == null) {
            return;
        }

        const activeEntry = conversationLogEntries.find(
            (entry) => entry.id === editingEntryId,
        );
        if (!activeEntry || activeEntry.contextEntryId == null) {
            cancelEditingConversationEntry();
        }
    });

    function startEditingConversationEntry(entry: ConversationLogEntry) {
        editingEntryId = entry.id;
        editingText = entry.text;
        editingImageRemoved = false;
    }

    function cancelEditingConversationEntry() {
        editingEntryId = null;
        editingText = "";
        editingImageRemoved = false;
    }

    async function handleSaveConversationEntryEdit() {
        if (editingEntryId == null) {
            return;
        }

        const didSave = await saveConversationLogEntryEdit(
            editingEntryId,
            editingText,
            editingImageRemoved,
        );
        if (didSave) {
            cancelEditingConversationEntry();
        }
    }
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
                    <div class="conversation-entry-content" data-role={entry.role}>
                        <div class="conversation-bubble" data-role={entry.role}>
                            {#if editingEntryId === entry.id}
                                {#if entry.imageUrl && !editingImageRemoved}
                                    <div class="conversation-attachment-shell">
                                        <img
                                            class="conversation-attachment-image"
                                            src={entry.imageUrl}
                                            alt="Attached screen capture"
                                            loading="lazy"
                                        />
                                        <button
                                            type="button"
                                            class="conversation-attachment-remove-btn"
                                            onclick={() => {
                                                editingImageRemoved = true;
                                            }}
                                            disabled={isSavingConversationLogEntryEdit}
                                        >
                                            Remove image
                                        </button>
                                    </div>
                                {/if}
                                <textarea
                                    class="conversation-editor-textarea"
                                    data-role={entry.role}
                                    bind:value={editingText}
                                    rows="4"
                                    placeholder="Edit this message"
                                    disabled={isSavingConversationLogEntryEdit}
                                ></textarea>
                            {:else}
                                {#if entry.imageUrl}
                                    <img
                                        class="conversation-attachment-image"
                                        src={entry.imageUrl}
                                        alt="Attached screen capture"
                                        loading="lazy"
                                    />
                                {/if}
                                {#if entry.text}
                                    <div class="conversation-entry-text">
                                        {entry.text}
                                    </div>
                                {/if}
                            {/if}
                        </div>

                        {#if editingEntryId === entry.id}
                            <div
                                class="conversation-entry-actions"
                                data-role={entry.role}
                            >
                                <button
                                    type="button"
                                    class="conversation-entry-action-btn secondary"
                                    onclick={cancelEditingConversationEntry}
                                    disabled={isSavingConversationLogEntryEdit}
                                >
                                    Cancel
                                </button>
                                <button
                                    type="button"
                                    class="conversation-entry-action-btn primary"
                                    onclick={handleSaveConversationEntryEdit}
                                    disabled={isSavingConversationLogEntryEdit}
                                >
                                    {isSavingConversationLogEntryEdit
                                        ? "Saving..."
                                        : "Save"}
                                </button>
                            </div>
                        {:else if entry.contextEntryId !== null}
                            <div
                                class="conversation-entry-actions"
                                data-role={entry.role}
                            >
                                <button
                                    type="button"
                                    class="conversation-entry-action-btn secondary"
                                    onclick={() => {
                                        startEditingConversationEntry(entry);
                                    }}
                                    disabled={isSavingConversationLogEntryEdit}
                                >
                                    Edit
                                </button>
                            </div>
                        {/if}
                    </div>
                </div>
            {/each}
        {/if}
    </div>
</div>
