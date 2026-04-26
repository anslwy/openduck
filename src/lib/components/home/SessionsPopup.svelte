<script lang="ts">
    import type { SessionMetadata, ContactProfile } from "$lib/openduck/types";
    import { onMount } from "svelte";
    import ConfirmDialog from "../ui/ConfirmDialog.svelte";

    let {
        sessions,
        activeSessionId,
        contacts,
        selectedContactId,
        onSelect,
        onDelete,
        onRename,
        onNewChat,
        onOpenSearch,
        onClose,
    } = $props<{
        sessions: SessionMetadata[];
        activeSessionId: string | null;
        contacts: ContactProfile[];
        selectedContactId: string;
        onSelect: (session: SessionMetadata) => void;
        onDelete: (session: SessionMetadata) => void;
        onRename: (session: SessionMetadata, newTitle: string) => void;
        onNewChat: () => void;
        onOpenSearch: () => void;
        onClose: () => void;
    }>();

    let selectedIndex = $state(0);
    let sessionToDelete = $state<SessionMetadata | null>(null);
    let editingSessionId = $state<string | null>(null);
    let editingTitle = $state("");

    onMount(() => {
        if (activeSessionId) {
            const index = sessions.findIndex((s) => s.id === activeSessionId);
            if (index !== -1) {
                selectedIndex = index;
            }
        }
    });

    const sessionsByDate = $derived(
        sessions.reduce(
            (acc, session) => {
                const date = new Date(session.updated_at * 1000);
                const dateStr = date.toLocaleDateString(undefined, {
                    weekday: "short",
                    year: "numeric",
                    month: "short",
                    day: "numeric",
                });
                if (!acc[dateStr]) {
                    acc[dateStr] = [];
                }
                acc[dateStr].push(session);
                return acc;
            },
            {} as Record<string, SessionMetadata[]>,
        ),
    );

    const dateHeaders = $derived(Object.keys(sessionsByDate));

    function handleKeydown(event: KeyboardEvent) {
        if (editingSessionId) return;
        if (event.key === "Escape") {
            onClose();
        } else if (event.key === "ArrowDown") {
            selectedIndex = (selectedIndex + 1) % sessions.length;
            event.preventDefault();
        } else if (event.key === "ArrowUp") {
            selectedIndex =
                (selectedIndex - 1 + sessions.length) % sessions.length;
            event.preventDefault();
        } else if (event.key === "Enter") {
            if (sessions[selectedIndex]) {
                onSelect(sessions[selectedIndex]);
            }
        } else if (event.ctrlKey && event.key === "d") {
            if (sessions[selectedIndex]) {
                onDelete(sessions[selectedIndex]);
            }
            event.preventDefault();
        } else if (event.ctrlKey && event.key === "r") {
            if (sessions[selectedIndex]) {
                startEditing(sessions[selectedIndex]);
            }
            event.preventDefault();
        }
    }

    function startEditing(session: SessionMetadata) {
        editingSessionId = session.id;
        editingTitle = session.title;
    }

    function saveEdit(session: SessionMetadata) {
        if (editingTitle.trim() && editingTitle !== session.title) {
            onRename(session, editingTitle);
        }
        editingSessionId = null;
    }

    function cancelEdit() {
        editingSessionId = null;
    }

    function autofocus(node: HTMLInputElement) {
        node.focus();
        node.select();
    }

    function formatTime(timestamp: number) {
        return new Date(timestamp * 1000).toLocaleTimeString(undefined, {
            hour: "numeric",
            minute: "2-digit",
        });
    }
</script>

<div
    class="sessions-popup"
    role="dialog"
    aria-label="Sessions"
    onkeydown={handleKeydown}
    tabindex="-1"
>
    <div class="sessions-list">
        <div class="sessions-search">
            <button
                type="button"
                class="session-item-wrapper sessions-search-trigger"
                onclick={onOpenSearch}
            >
                <div class="session-item">
                    <div class="new-session-content">
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
                            ><circle cx="11" cy="11" r="8" /><path
                                d="m21 21-4.3-4.3"
                            /></svg
                        >
                        <span class="session-item-title">Search</span>
                    </div>
                </div>
            </button>
        </div>
        <button
            type="button"
            class="session-item-wrapper new-session-item"
            onclick={onNewChat}
        >
            <div class="session-item">
                <div class="new-session-content">
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
                        ><path d="M12 5v14M5 12h14" /></svg
                    >
                    <span class="session-item-title">New Chat</span>
                </div>
            </div>
        </button>

        {#each dateHeaders as date}
            <div class="sessions-date-header">{date}</div>
            {#each sessionsByDate[date] as session}
                {@const globalIndex = sessions.indexOf(session)}
                {@const characterId = session.character_id || "contact-openduck"}
                {@const sessionContact =
                    contacts.find((c) => c.id === characterId) || contacts[0]}
                {@const isCurrentCharacter = characterId === selectedContactId}
                <div
                    class="session-item-wrapper"
                    class:selected={globalIndex === selectedIndex}
                    class:active={session.id === activeSessionId}
                    class:editing={editingSessionId === session.id}
                    class:non-current-character={!isCurrentCharacter}
                    onmouseenter={() => (selectedIndex = globalIndex)}
                >
                    {#if editingSessionId === session.id}
                        <div class="session-edit-container">
                            <input
                                type="text"
                                class="session-edit-input"
                                bind:value={editingTitle}
                                onkeydown={(e) => {
                                    if (e.key === "Enter") saveEdit(session);
                                    if (e.key === "Escape") cancelEdit();
                                    e.stopPropagation();
                                }}
                                use:autofocus
                            />
                            <button
                                type="button"
                                class="session-save-btn"
                                onclick={() => saveEdit(session)}
                                title="Save"
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
                                    ><polyline points="20 6 9 17 4 12" /></svg
                                >
                            </button>
                            <button
                                type="button"
                                class="session-cancel-btn"
                                onclick={cancelEdit}
                                title="Cancel"
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
                        </div>
                    {:else}
                        <button
                            type="button"
                            class="session-item"
                            onclick={() => onSelect(session)}
                        >
                            <div class="session-item-left">
                                <div
                                    class="session-avatar"
                                    style="background-image: url('{sessionContact?.iconDataUrl || "/icon.png"}')"
                                ></div>
                                <span class="session-item-title"
                                    >{session.title}</span
                                >
                            </div>
                            <span class="session-item-time"
                                >{formatTime(session.updated_at)}</span
                                >
                        </button>
                        <button
                            type="button"
                            class="session-edit-btn"
                            onclick={(e) => {
                                e.stopPropagation();
                                startEditing(session);
                            }}
                            title="Rename session"
                        >
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                width="14"
                                height="14"
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
                        <button
                            type="button"
                            class="session-delete-btn"
                            onclick={(e) => {
                                e.stopPropagation();
                                sessionToDelete = session;
                            }}
                            title="Delete session"
                        >
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                width="14"
                                height="14"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="2.5"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                ><path d="M3 6h18" /><path
                                    d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"
                                /><path
                                    d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"
                                /><line x1="10" y1="11" x2="10" y2="17" /><line
                                    x1="14"
                                    y1="11"
                                    x2="14"
                                    y2="17"
                                /></svg
                            >
                        </button>
                    {/if}
                </div>
            {/each}
        {/each}
        {#if sessions.length === 0}
            <div class="conversation-empty">No sessions found</div>
        {/if}
    </div>

    {#if sessionToDelete}
        <ConfirmDialog
            title="Delete session?"
            message="This action cannot be undone. Do you wish to continue?"
            onConfirm={() => {
                if (sessionToDelete) onDelete(sessionToDelete);
                sessionToDelete = null;
            }}
            onCancel={() => (sessionToDelete = null)}
        />
    {/if}
</div>

<style>
    .sessions-search-trigger {
        background: rgba(255, 205, 64, 0.05);
        border: 1px dashed rgba(255, 205, 64, 0.3);
        margin-bottom: 8px;
    }

    .sessions-search-trigger:hover {
        background: rgba(255, 205, 64, 0.1);
        border-color: rgba(255, 205, 64, 0.5);
    }

    .sessions-search-trigger .session-item-title {
        color: #ffdf63;
    }

    .sessions-search-trigger svg {
        color: #ffdf63;
    }

    .new-session-item {
        border: 1px dashed rgba(255, 205, 64, 0.3);
        background: rgba(255, 205, 64, 0.05);
        margin-bottom: 12px;
    }

    .new-session-item:hover {
        background: rgba(255, 205, 64, 0.1);
        border-color: rgba(255, 205, 64, 0.5);
    }

    .new-session-content {
        display: flex;
        align-items: center;
        gap: 10px;
        color: #ffdf63;
    }

    .session-item-left {
        display: flex;
        align-items: center;
        gap: 10px;
        flex: 1;
        min-width: 0;
    }

    .session-avatar {
        width: 20px;
        height: 20px;
        border-radius: 4px;
        background-size: cover;
        background-position: center;
        flex-shrink: 0;
        border: 1px solid rgba(255, 255, 255, 0.1);
    }

    .non-current-character {
        opacity: 0.5;
    }

    .non-current-character:hover {
        opacity: 0.8;
    }
</style>
