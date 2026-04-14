<script lang="ts">
    let { currentSessionTitle, showSessionsPopup, calling, onToggleSessions } =
        $props<{
            currentSessionTitle: string | null;
            showSessionsPopup: boolean;
            calling: boolean;
            onToggleSessions: () => void;
        }>();
</script>

<header class="app-header">
    <div class="header-left">
        {#if !calling}
            <button
                class="sessions-dropdown-btn"
                class:active={showSessionsPopup}
                class:in-call={calling}
                onclick={() => !calling && onToggleSessions()}
                disabled={calling}
                aria-label="Toggle session history"
            >
                <span class="sessions-dropdown-label">
                    {currentSessionTitle || "Past Sessions"}
                </span>
                <svg
                    xmlns="http://www.w3.org/2000/svg"
                    width="16"
                    height="16"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    stroke-width="2.5"
                    stroke-linecap="round"
                    stroke-linejoin="round"
                    class="dropdown-arrow"
                    class:open={showSessionsPopup}
                >
                    <path d="m6 9 6 6 6-6" />
                </svg>
            </button>
        {/if}
    </div>
</header>
