<script lang="ts">
    import { onMount } from "svelte";

    let { value, onUpdate } = $props<{
        value: string;
        onUpdate: (newValue: string) => void;
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
    {#if isCapturing}
        <span class="capture-prompt">Press keys...</span>
    {:else}
        <span class="shortcut-display">
            {#each getSortedParts(value) as part}
                <span class="shortcut-key" class:modifier={modifierOrder.includes(part)}>
                    {modifierSymbols[part] || keySymbols[part] || part}
                </span>
            {/each}
        </span>
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

    .capture-prompt {
        color: #ffcd40;
        font-size: 0.9rem;
        font-weight: 600;
        padding-left: 4px;
    }
</style>
