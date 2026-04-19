<script lang="ts">
    import { onMount } from "svelte";

    let { value, onUpdate, onRemove, onDefault } = $props<{
        value: string;
        onUpdate: (newValue: string) => void;
        onRemove?: () => void;
        onDefault?: () => void;
    }>();

    let isCapturing = $state(false);
    let capturedValue = $state(value);
    let inputEl = $state<HTMLElement | null>(null);

    const modifierSymbols: Record<string, string> = {
        Command: "⌘",
        Shift: "⇧",
        Option: "⌥",
        Control: "⌃",
    };

    const keySymbols: Record<string, string> = {
        ArrowUp: "↑",
        ArrowDown: "↓",
        ArrowLeft: "←",
        ArrowRight: "→",
        Enter: "↵",
        Escape: "⎋",
        Backspace: "⌫",
        Tab: "⇥",
        " ": "Space",
    };

    function formatForDisplay(val: string) {
        if (!val) return "None";
        const parts = val.split("+");
        return parts
            .map((part) => modifierSymbols[part] || keySymbols[part] || part)
            .join(" ");
    }

    function handleKeyDown(e: KeyboardEvent) {
        if (!isCapturing) return;

        if (e.key === "Escape") {
            isCapturing = false;
            e.preventDefault();
            e.stopPropagation();
            inputEl?.blur();
            return;
        }

        // Don't finish if only a modifier is pressed
        if (["Meta", "Control", "Alt", "Shift"].includes(e.key)) {
            e.preventDefault();
            e.stopPropagation();
            return;
        }

        e.preventDefault();
        e.stopPropagation();

        const modifiers: string[] = [];
        // Use a consistent order for the backend value: Command, Control, Option, Shift
        if (e.metaKey) modifiers.push("Command");
        if (e.ctrlKey) modifiers.push("Control");
        if (e.altKey) modifiers.push("Option");
        if (e.shiftKey) modifiers.push("Shift");

        let key = "";
        // Use e.code for letters and digits to avoid character transformation (e.g., Option+L -> ò)
        if (e.code.startsWith("Key")) {
            key = e.code.slice(3); // "KeyL" -> "L"
        } else if (e.code.startsWith("Digit")) {
            key = e.code.slice(5); // "Digit1" -> "1"
        } else {
            key = e.key;
            if (key === " ") key = "Space";
            // Capitalize single characters for consistency
            if (key.length === 1) {
                key = key.toUpperCase();
            }
        }

        if (key) {
            const newValue = [...modifiers, key].filter(Boolean).join("+");
            capturedValue = newValue;
            isCapturing = false;
            onUpdate(newValue);
            inputEl?.blur();
        }
    }

    function startCapture() {
        isCapturing = true;
    }

    function cancelCapture() {
        isCapturing = false;
    }

    const modifierOrder = ["Control", "Option", "Shift", "Command"];

    function getSortedParts(val: string) {
        if (!val || val === "None") return [];
        const parts = val.split("+");
        const modifiers = parts.filter(p => modifierOrder.includes(p));
        const nonModifiers = parts.filter(p => !modifierOrder.includes(p));

        modifiers.sort((a, b) => modifierOrder.indexOf(a) - modifierOrder.indexOf(b));

        return [...modifiers, ...nonModifiers];
    }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
    bind:this={inputEl}
    class="shortcut-capture"
    class:capturing={isCapturing}
    role="button"
    tabindex="0"
    onclick={startCapture}
    onkeydown={handleKeyDown}
    onblur={cancelCapture}
>
    <div class="shortcut-capture-inner">
        {#if isCapturing}
            <span class="capture-prompt">Press keys...</span>
        {:else}
            <span class="shortcut-display">
                {#if getSortedParts(value).length === 0}
                    <span class="none-label">None</span>
                {:else}
                    {#each getSortedParts(value) as part}
                        <span class="shortcut-key" class:modifier={modifierOrder.includes(part)}>
                            {modifierSymbols[part] || keySymbols[part] || part}
                        </span>
                    {/each}
                {/if}
            </span>
        {/if}
    </div>

    {#if !isCapturing}
        <div class="shortcut-actions">
            {#if onRemove}
                <div class="tooltip-shell">
                    <button
                        type="button"
                        class="shortcut-action-icon-btn"
                        onclick={(e) => {
                            e.stopPropagation();
                            onRemove();
                        }}
                        aria-label="Remove Shortcut"
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"></path><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"></path><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"></path></svg>
                    </button>
                    <div class="tooltip-bubble control-tooltip">Remove Shortcut</div>
                </div>
            {/if}
            {#if onDefault}
                <div class="tooltip-shell">
                    <button
                        type="button"
                        class="shortcut-action-icon-btn"
                        onclick={(e) => {
                            e.stopPropagation();
                            onDefault();
                        }}
                        aria-label="Restore Default"
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8"></path><path d="M3 3v5h5"></path></svg>
                    </button>
                    <div class="tooltip-bubble control-tooltip">Restore Default</div>
                </div>
            {/if}
        </div>
    {/if}
</div>

<style>
    .shortcut-capture {
        background: rgba(255, 255, 255, 0.04);
        border: 1px solid rgba(255, 255, 255, 0.08);
        border-radius: 12px;
        padding: 6px 10px;
        min-height: 40px;
        display: flex;
        align-items: center;
        cursor: pointer;
        transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
        outline: none;
        box-sizing: border-box;
        width: 100%;
    }

    .shortcut-capture:hover {
        background: rgba(255, 255, 255, 0.07);
        border-color: rgba(255, 255, 255, 0.15);
    }

    .shortcut-capture:focus-within,
    .shortcut-capture.capturing {
        border-color: rgba(255, 205, 64, 0.5);
        background: rgba(255, 205, 64, 0.08);
        box-shadow: 0 0 0 4px rgba(255, 205, 64, 0.12);
    }

    .shortcut-capture-inner {
        flex: 1;
        display: flex;
        align-items: center;
        min-width: 0;
    }

    .shortcut-actions {
        display: flex;
        align-items: center;
        gap: 4px;
        margin-left: 8px;
        opacity: 0;
        transition: opacity 0.2s ease;
    }

    .shortcut-capture:hover .shortcut-actions {
        opacity: 1;
    }

    .shortcut-action-icon-btn {
        display: flex;
        align-items: center;
        justify-content: center;
        width: 28px;
        height: 28px;
        border-radius: 6px;
        border: none;
        background: transparent;
        color: rgba(255, 255, 255, 0.3);
        cursor: pointer;
        transition: all 0.2s ease;
    }

    .shortcut-action-icon-btn:hover {
        background: rgba(255, 255, 255, 0.08);
        color: rgba(255, 255, 255, 0.9);
    }

    .shortcut-display {
        display: flex;
        gap: 4px;
        align-items: center;
    }

    .shortcut-key {
        display: inline-flex;
        align-items: center;
        justify-content: center;
        min-width: 26px;
        height: 26px;
        padding: 0 8px;
        background: rgba(255, 205, 64, 0.1);
        border: 1px solid rgba(255, 205, 64, 0.2);
        border-radius: 6px;
        color: #ffcd40;
        font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
        font-size: 0.9rem;
        font-weight: 700;
    }

    .shortcut-key.modifier {
        background: transparent;
        border: none;
        color: rgba(255, 255, 255, 0.4);
        padding: 0 2px;
        min-width: auto;
        font-size: 1.1rem;
    }

    .none-label {
        color: rgba(255, 255, 255, 0.2);
        font-size: 0.9rem;
        font-style: italic;
        padding-left: 4px;
    }

    .capture-prompt {
        color: #ffcd40;
        font-size: 0.9rem;
        font-weight: 600;
        padding-left: 4px;
    }
</style>
