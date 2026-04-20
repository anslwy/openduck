<!-- Floating live transcript panel shown during active calls. -->
<script lang="ts">
    import type { ConversationLogEntry } from "$lib/openduck/types";
    import { onMount } from "svelte";
    import ConfirmDialog from "../ui/ConfirmDialog.svelte";

    let {
        conversationLogEntries,
        sessionTitle,
        popupActionsBusy,
        calling,
        onClearHistory,
        onClose,
        onFork,
        onPreviewImage,
        saveConversationLogEntryEdit,
        deleteConversationLogEntry,
        setConversationLogViewport,
    } = $props<{
        conversationLogEntries: ConversationLogEntry[];
        sessionTitle: string | null;
        popupActionsBusy: boolean;
        calling: boolean;
        onClearHistory: () => void;
        onClose: () => void;
        onFork: (entry: ConversationLogEntry) => void;
        onPreviewImage: (url: string) => void;
        saveConversationLogEntryEdit: (
            entryId: number,
            text: string,
            clearImage: boolean,
        ) => Promise<boolean>;
        deleteConversationLogEntry: (entryId: number) => Promise<boolean>;
        setConversationLogViewport: (el: HTMLElement | null) => void;
    }>();

    let viewport = $state<HTMLElement | null>(null);
    let editingEntryId = $state<number | null>(null);
    let editingText = $state("");
    let editingImageRemoved = $state(false);
    let messageToDelete = $state<ConversationLogEntry | null>(null);
    let editingTitle = $state(false);
    let titleValue = $state("");

    // Use internal state for busy to avoid naming conflict with prop
    let isSavingConversationLogEntryEdit = $state(false);
    let isClearingConversationLogImages = $state(false);

    const isBusy = $derived(
        popupActionsBusy ||
            isSavingConversationLogEntryEdit ||
            isClearingConversationLogImages,
    );
    const hasClearableConversationImages = $derived(
        conversationLogEntries.some((entry) => entry.imageUrls.length > 0),
    );

    $effect(() => {
        setConversationLogViewport(viewport);

        return () => {
            setConversationLogViewport(null);
        };
    });

    function startEditingTitle() {
        titleValue = sessionTitle || "Conversation";
        editingTitle = true;
    }

    async function saveTitle() {
        editingTitle = false;
        const event = new CustomEvent("rename-session", { detail: titleValue });
        window.dispatchEvent(event);
    }

    $effect(() => {
        if (editingEntryId == null) {
            return;
        }

        const activeEntry = conversationLogEntries.find(
            (entry) => entry.id === editingEntryId,
        );
        if (!activeEntry) {
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

        isSavingConversationLogEntryEdit = true;
        try {
            const didSave = await saveConversationLogEntryEdit(
                editingEntryId,
                editingText,
                editingImageRemoved,
            );
            if (didSave) {
                cancelEditingConversationEntry();
            }
        } finally {
            isSavingConversationLogEntryEdit = false;
        }
    }

    async function handleClearConversationImages() {
        cancelEditingConversationEntry();
        isClearingConversationLogImages = true;
        try {
            await onClearHistory();
        } finally {
            isClearingConversationLogImages = false;
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
            <div class="conversation-popup-title-row">
                {#if editingTitle}
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                        type="text"
                        class="conversation-popup-title-input"
                        bind:value={titleValue}
                        onblur={saveTitle}
                        onkeydown={(e) => e.key === "Enter" && saveTitle()}
                        autofocus
                    />
                {:else}
                    <span class="conversation-popup-title"
                        >{sessionTitle || "Conversation"}</span
                    >
                    <button
                        type="button"
                        class="title-edit-btn"
                        onclick={startEditingTitle}
                        aria-label="Edit title"
                    >
                        <svg
                            xmlns="http://www.w3.org/2000/svg"
                            width="14"
                            height="14"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                            ><path
                                d="M17 3a2.85 2.83 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5Z"
                            /><path d="m15 5 4 4" /></svg
                        >
                    </button>
                {/if}
            </div>
            <span class="conversation-popup-subtitle">Live call transcript</span
            >
        </div>
        <div class="conversation-popup-actions">
            <button
                type="button"
                class="conversation-header-btn"
                onclick={handleClearConversationImages}
                disabled={!hasClearableConversationImages || isBusy}
            >
                {isClearingConversationLogImages
                    ? "Clearing..."
                    : "Clear Image History"}
            </button>
            <button
                type="button"
                class="conversation-close-btn"
                onclick={onClose}
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
    </div>

    <div class="conversation-log" bind:this={viewport}>
        {#if conversationLogEntries.length === 0}
            <div class="conversation-empty">
                Start talking and the transcript will appear here.
            </div>
        {:else}
            {#each conversationLogEntries as entry (entry.id)}
                <div class="conversation-entry" data-role={entry.role}>
                    <div
                        class="conversation-entry-content"
                        data-role={entry.role}
                    >
                        <div class="conversation-bubble" data-role={entry.role}>
                            {#if editingEntryId === entry.id}
                                {#if entry.imageUrls.length > 0 && !editingImageRemoved}
                                    <div class="conversation-attachment-shell">
                                        <div
                                            class="conversation-attachment-images"
                                        >
                                            {#each entry.imageUrls as imageUrl}
                                                <!-- svelte-ignore a11y_click_events_have_key_events -->
                                                <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                                                <img
                                                    class="conversation-attachment-image zoomable"
                                                    src={imageUrl}
                                                    alt="Attached screen capture"
                                                    loading="lazy"
                                                    onclick={() =>
                                                        onPreviewImage(
                                                            imageUrl,
                                                        )}
                                                />
                                            {/each}
                                        </div>
                                        <button
                                            type="button"
                                            class="conversation-attachment-remove-btn"
                                            onclick={() => {
                                                editingImageRemoved = true;
                                            }}
                                            disabled={isBusy}
                                        >
                                            Remove images
                                        </button>
                                    </div>
                                {/if}
                                <textarea
                                    class="conversation-editor-textarea"
                                    data-role={entry.role}
                                    bind:value={editingText}
                                    rows="4"
                                    placeholder="Edit this message"
                                    disabled={isBusy}
                                ></textarea>
                            {:else}
                                {#if entry.imageUrls.length > 0}
                                    <div class="conversation-attachment-images">
                                        {#each entry.imageUrls as imageUrl}
                                            <!-- svelte-ignore a11y_click_events_have_key_events -->
                                            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
                                            <img
                                                class="conversation-attachment-image zoomable"
                                                src={imageUrl}
                                                alt="Attached screen capture"
                                                loading="lazy"
                                                onclick={() =>
                                                    onPreviewImage(imageUrl)}
                                            />
                                        {/each}
                                    </div>
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
                                    disabled={isBusy}
                                >
                                    Cancel
                                </button>
                                <button
                                    type="button"
                                    class="conversation-entry-action-btn primary"
                                    onclick={handleSaveConversationEntryEdit}
                                    disabled={isBusy}
                                >
                                    {isSavingConversationLogEntryEdit
                                        ? "Saving..."
                                        : "Save"}
                                </button>
                            </div>
                        {:else}
                            <div
                                class="conversation-entry-actions"
                                data-role={entry.role}
                            >
                                <button
                                    type="button"
                                    class="conversation-entry-action-btn secondary icon-only"
                                    onclick={() => {
                                        startEditingConversationEntry(entry);
                                    }}
                                    disabled={isBusy}
                                    aria-label="Edit message"
                                >
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        width="15"
                                        height="15"
                                        viewBox="0 0 24 24"
                                        fill="none"
                                        stroke="currentColor"
                                        stroke-width="2.5"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        ><path
                                            d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"
                                        /><path
                                            d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"
                                        /></svg
                                    >
                                </button>
                                {#if entry.role === "assistant" && !calling}
                                    <div class="tooltip-shell">
                                        <button
                                            type="button"
                                            class="conversation-entry-action-btn secondary icon-only"
                                            onclick={() => onFork(entry)}
                                            disabled={isBusy}
                                            aria-label="Fork conversation"
                                        >
                                            <svg
                                                xmlns="http://www.w3.org/2000/svg"
                                                width="17"
                                                height="17"
                                                viewBox="0 0 24 24"
                                                fill="none"
                                                stroke="currentColor"
                                                stroke-width="2"
                                                stroke-linecap="round"
                                                stroke-linejoin="round"
                                            >
                                                <circle cx="12" cy="18" r="2" />
                                                <circle cx="7" cy="6" r="2" />
                                                <circle cx="17" cy="6" r="2" />
                                                <path
                                                    d="M7 8v2a2 2 0 0 0 2 2h6a2 2 0 0 0 2 -2v-2"
                                                />
                                                <path d="M12 12v4" />
                                            </svg>
                                        </button>
                                        <div
                                            class="tooltip-bubble control-tooltip left-aligned"
                                        >
                                            Copy messages up to this point into
                                            a new session. (Similar to Git Fork)
                                        </div>
                                    </div>
                                {/if}
                                {#if entry.role !== "assistant"}
                                    <button
                                        type="button"
                                        class="conversation-entry-action-btn secondary icon-only conversation-action-delete-btn"
                                        onclick={() => {
                                            messageToDelete = entry;
                                        }}
                                        disabled={isBusy}
                                        aria-label="Delete message"
                                    >
                                        <svg
                                            xmlns="http://www.w3.org/2000/svg"
                                            width="15"
                                            height="15"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="currentColor"
                                            stroke-width="2"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            ><path d="M3 6h18" /><path
                                                d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"
                                            /><path
                                                d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"
                                            /><line
                                                x1="10"
                                                y1="11"
                                                x2="10"
                                                y2="17"
                                            /><line
                                                x1="14"
                                                y1="11"
                                                x2="14"
                                                y2="17"
                                            /></svg
                                        >
                                    </button>
                                {/if}
                            </div>
                        {/if}
                    </div>
                </div>
            {/each}
        {/if}
    </div>

    {#if messageToDelete}
        <ConfirmDialog
            title="Delete message?"
            message="This action cannot be undone and the AI response will be deleted as well. Do you wish to continue?"
            onConfirm={async () => {
                if (messageToDelete) {
                    await deleteConversationLogEntry(messageToDelete.id);
                }
                messageToDelete = null;
            }}
            onCancel={() => (messageToDelete = null)}
        />
    {/if}
</div>
