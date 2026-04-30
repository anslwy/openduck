<script lang="ts">
    let {
        currentSessionTitle,
        showSessionsPopup,
        calling,
        onboarding,
        isMemoryProcessing,
        onToggleSessions,
    } = $props<{
        currentSessionTitle: string | null;
        showSessionsPopup: boolean;
        calling: boolean;
        onboarding?: boolean;
        isMemoryProcessing?: boolean;
        onToggleSessions: () => void;
    }>();
</script>

<header class="app-header">
    <div class="header-left">
        {#if !calling && !onboarding}
            <button
                class="sessions-dropdown-btn"
                class:active={showSessionsPopup}
                class:in-call={calling}
                onclick={onToggleSessions}
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

    <div class="header-right">
        {#if isMemoryProcessing}
            <div
                class="memory-processing-indicator"
                title="Processing memories..."
            >
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
                >
                    <path
                        d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"
                    />
                </svg>
            </div>
        {/if}
    </div>
</header>
