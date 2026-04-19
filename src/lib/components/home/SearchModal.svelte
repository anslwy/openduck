<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import type { SearchResult } from "$lib/openduck/types";

    let { onClose, onSelect, onNewChat } = $props<{
        onClose: () => void;
        onSelect: (sessionId: string) => void;
        onNewChat: () => void;
    }>();

    let searchTerm = $state("");
    let results = $state<SearchResult[]>([]);
    let isSearching = $state(false);
    let selectedIndex = $state(0);

    async function handleSearch() {
        console.log("handleSearch called with:", searchTerm);
        if (!searchTerm.trim()) {
            results = [];
            selectedIndex = 0;
            return;
        }

        isSearching = true;
        try {
            results = await invoke<SearchResult[]>("search_sessions", {
                query: searchTerm,
            });
            selectedIndex = 0;
        } catch (err) {
            console.error("Search failed:", err);
        } finally {
            isSearching = false;
        }
    }

    $effect(() => {
        searchTerm; // Explicitly track searchTerm
        const timeout = setTimeout(handleSearch, 150);
        return () => clearTimeout(timeout);
    });

    function handleKeydown(event: KeyboardEvent) {
        if (event.key === "Escape") {
            onClose();
        } else if (event.key === "ArrowDown") {
            selectedIndex = (selectedIndex + 1) % results.length;
            event.preventDefault();
        } else if (event.key === "ArrowUp") {
            selectedIndex =
                (selectedIndex - 1 + results.length) % results.length;
            event.preventDefault();
        } else if (event.key === "Enter") {
            if (results[selectedIndex]) {
                onSelect(results[selectedIndex].session_id);
            }
        }
    }

    function highlightMatch(text: string, query: string) {
        if (!query || !text) return text;
        const escapedQuery = query.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
        const parts = text.split(new RegExp(`(${escapedQuery})`, "gi"));
        return parts
            .map((part) =>
                part.toLowerCase() === query.toLowerCase()
                    ? `<mark class="search-highlight">${part}</mark>`
                    : part,
            )
            .join("");
    }

    function autofocus(node: HTMLInputElement) {
        node.focus();
    }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="search-modal-backdrop" onclick={onClose}></div>

<div
    class="search-modal"
    onkeydown={handleKeydown}
    role="dialog"
    aria-label="Search"
>
    <div class="search-header">
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="20"
            height="20"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            class="search-icon"
            ><circle cx="11" cy="11" r="8" /><path d="m21 21-4.3-4.3" /></svg
        >
        <input
            type="text"
            class="search-input"
            placeholder="Search sessions..."
            bind:value={searchTerm}
            use:autofocus
        />
        {#if searchTerm}
            <button
                class="search-clear-btn"
                onclick={() => (searchTerm = "")}
                title="Clear search"
            >
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="18"
                    height="18"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2.5"
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
        {/if}
    </div>

    <div class="search-results">
        {#each results as result, i}
            <button
                class="search-result-item"
                class:selected={i === selectedIndex}
                onclick={() => onSelect(result.session_id)}
            >
                <div class="result-icon">
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
                        ><path
                            d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"
                        /></svg
                    >
                </div>
                <div class="result-content">
                    <div class="result-title">
                        {@html highlightMatch(result.session_title, searchTerm)}
                    </div>
                    {#if result.matched_text}
                        <div class="result-snippet">
                            {@html highlightMatch(
                                result.matched_text,
                                searchTerm,
                            )}
                        </div>
                    {/if}
                </div>
            </button>
        {/each}

        {#if results.length === 0 && searchTerm.trim() && !isSearching}
            <div class="search-empty-state">No sessions found</div>
        {/if}

        {#if isSearching}
            <div class="search-empty-state">Searching...</div>
        {/if}
    </div>
</div>

<style>
    .search-modal-backdrop {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.4);
        backdrop-filter: blur(12px);
        z-index: 1000;
    }

    .search-modal {
        position: fixed;
        top: 20%;
        left: 50%;
        transform: translateX(-50%);
        width: min(600px, calc(100vw - 40px));
        background: rgba(30, 30, 32, 0.95);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 20px;
        box-shadow: 0 24px 60px rgba(0, 0, 0, 0.4);
        display: flex;
        flex-direction: column;
        overflow: hidden;
        z-index: 1001;
        backdrop-filter: blur(20px);
        animation: modalSlideIn 0.2s ease-out;
    }

    @keyframes modalSlideIn {
        from {
            opacity: 0;
            transform: translate(-50%, -20px);
        }
        to {
            opacity: 1;
            transform: translate(-50%, 0);
        }
    }

    .search-header {
        display: flex;
        align-items: center;
        padding: 16px 20px;
        border-bottom: 1px solid rgba(255, 255, 255, 0.06);
        gap: 12px;
    }

    .search-icon {
        color: rgba(255, 255, 255, 0.4);
        flex-shrink: 0;
    }

    .search-input {
        flex: 1;
        border: none;
        background: transparent;
        font-size: 1.15rem;
        font-weight: 500;
        color: rgba(255, 255, 255, 0.9);
        padding: 4px 0;
        outline: none;
    }

    .search-input::placeholder {
        color: rgba(255, 255, 255, 0.3);
    }

    .search-clear-btn {
        background: transparent;
        border: none;
        color: rgba(255, 255, 255, 0.3);
        cursor: pointer;
        padding: 4px;
        display: flex;
        align-items: center;
        justify-content: center;
        border-radius: 50%;
        transition: background-color 0.2s;
    }

    .search-clear-btn:hover {
        background: rgba(255, 255, 255, 0.05);
        color: rgba(255, 255, 255, 0.6);
    }

    .search-results {
        max-height: 480px;
        overflow-y: auto;
        padding: 8px;
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .search-result-item {
        display: flex;
        align-items: flex-start;
        gap: 14px;
        padding: 12px 14px;
        border: none;
        background: transparent;
        border-radius: 12px;
        text-align: left;
        cursor: pointer;
        transition: background-color 0.15s;
        width: 100%;
    }

    .search-result-item:hover {
        background: rgba(255, 255, 255, 0.035);
    }

    .search-result-item.selected {
        background: rgba(255, 255, 255, 0.08);
    }

    .result-icon {
        width: 32px;
        height: 32px;
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.04);
        display: flex;
        align-items: center;
        justify-content: center;
        color: rgba(255, 255, 255, 0.5);
        flex-shrink: 0;
        margin-top: 2px;
    }

    .result-content {
        flex: 1;
        min-width: 0;
        display: flex;
        flex-direction: column;
        gap: 2px;
    }

    .result-title {
        font-size: 1rem;
        font-weight: 600;
        color: rgba(255, 255, 255, 0.95);
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
    }

    .result-snippet {
        font-size: 0.88rem;
        color: rgba(255, 255, 255, 0.5);
        line-height: 1.4;
        display: -webkit-box;
        -webkit-line-clamp: 2;
        -webkit-box-orient: vertical;
        overflow: hidden;
        word-break: break-word;
    }

    :global(.search-highlight) {
        background: rgba(255, 205, 64, 0.25);
        color: #ffcd40;
        padding: 0 2px;
        border-radius: 3px;
    }

    .search-empty-state {
        padding: 32px;
        text-align: center;
        color: rgba(255, 255, 255, 0.4);
        font-size: 0.95rem;
    }
</style>
