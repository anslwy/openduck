<!-- Modal for configuring a dedicated OpenAI-compatible model for AI subtitle translation. -->
<script lang="ts">
    let {
        baseUrl,
        apiKey,
        modelId,
        onSave,
        onTestConnection,
        onClose,
    } = $props<{
        baseUrl: string;
        apiKey: string;
        modelId: string;
        onSave: (url: string, key: string, modelId: string) => Promise<void>;
        onTestConnection: (url: string, key: string) => Promise<string[]>;
        onClose: () => void;
    }>();

    let url = $state("");
    let key = $state("");
    let selectedModelId = $state("");
    let availableModels = $state<string[]>([]);
    let connectionState = $state<"idle" | "success" | "error">("idle");
    let connectionMessage = $state("");
    let isTesting = $state(false);
    let isSaving = $state(false);

    $effect(() => {
        url = baseUrl;
        key = apiKey;
        selectedModelId = modelId;
        availableModels = modelId ? [modelId] : [];
        connectionState = "idle";
        connectionMessage = "";
    });

    function validationMessage() {
        const hasUrl = url.trim() !== "";
        const hasModel = selectedModelId.trim() !== "";

        if (hasUrl !== hasModel) {
            return "Enter a Base URL and choose a model, or clear the Base URL and use the current call LLM.";
        }

        return null;
    }

    function normalizeErrorMessage(error: unknown) {
        if (typeof error === "string") {
            return error;
        }

        if (error instanceof Error && error.message) {
            return error.message;
        }

        return "Request failed.";
    }

    async function handleTestConnection() {
        const normalizedUrl = url.trim();
        const normalizedKey = key.trim();

        if (!normalizedUrl) {
            connectionState = "error";
            connectionMessage = "Enter a Base URL first.";
            return;
        }

        isTesting = true;
        connectionState = "idle";
        connectionMessage = "";

        try {
            const models = await onTestConnection(normalizedUrl, normalizedKey);
            availableModels = models;

            if (models.length === 0) {
                connectionState = "error";
                connectionMessage =
                    "Connected, but the server did not return any model IDs.";
                return;
            }

            if (
                !selectedModelId.trim() ||
                !models.includes(selectedModelId.trim())
            ) {
                selectedModelId = models[0];
            }

            connectionState = "success";
            connectionMessage =
                models.length === 1
                    ? "Connected. Found 1 model."
                    : `Connected. Found ${models.length} models.`;
        } catch (error) {
            connectionState = "error";
            connectionMessage = normalizeErrorMessage(error);
        } finally {
            isTesting = false;
        }
    }

    async function handleSave() {
        const validationError = validationMessage();
        if (validationError) {
            connectionState = "error";
            connectionMessage = validationError;
            return;
        }

        isSaving = true;
        try {
            await onSave(url.trim(), key.trim(), selectedModelId.trim());
            onClose();
        } catch (error) {
            connectionState = "error";
            connectionMessage = normalizeErrorMessage(error);
        } finally {
            isSaving = false;
        }
    }
</script>

<button
    type="button"
    class="about-modal-backdrop subtitle-translation-config-backdrop"
    aria-label="Close subtitle translation configuration"
    onclick={onClose}
></button>

<div
    class="about-modal subtitle-translation-config-modal"
    role="dialog"
    aria-labelledby="subtitle-translation-config-title"
    aria-modal="true"
>
    <div class="about-modal-header">
        <div class="about-modal-copy">
            <span
                class="about-modal-title"
                id="subtitle-translation-config-title"
                >AI Subtitle Translation LLM</span
            >
            <span class="about-modal-subtitle"
                >Use a separate OpenAI-compatible model for subtitle
                translation.</span
            >
        </div>
        <button
            type="button"
            class="conversation-close-btn"
            onclick={onClose}
            aria-label="Close subtitle translation configuration"
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

    <div class="config-form">
        <div class="form-group">
            <label for="subtitle-translation-url">Base URL</label>
            <input
                id="subtitle-translation-url"
                type="text"
                bind:value={url}
                placeholder="https://api.openai.com"
                class="config-input"
            />
            <p class="field-help">
                Clear the Base URL and choose "Use current call LLM" to keep
                using the current call LLM for translation.
            </p>
        </div>

        <div class="form-group">
            <label for="subtitle-translation-key">API Key (Optional)</label>
            <input
                id="subtitle-translation-key"
                type="password"
                bind:value={key}
                placeholder="Enter API key if required"
                class="config-input"
            />
            <p class="field-help">
                Required for providers like OpenAI. Optional for local servers or
                unauthenticated proxies.
            </p>
        </div>

        <div class="form-group">
            <div class="field-header">
                <label for="subtitle-translation-model">Model ID</label>
                <button
                    type="button"
                    class="utility-btn test-btn"
                    onclick={handleTestConnection}
                    disabled={isTesting}
                >
                    {isTesting ? "Testing..." : "Test Connection"}
                </button>
            </div>
            <select
                id="subtitle-translation-model"
                class="config-input config-select"
                value={selectedModelId}
                onchange={(event) =>
                    (selectedModelId = (
                        event.currentTarget as HTMLSelectElement
                    ).value)}
            >
                <option value=""
                    >{url.trim() === ""
                        ? "Use current call LLM"
                        : "Select a model"}</option
                >
                {#each availableModels as model}
                    <option value={model}>{model}</option>
                {/each}
            </select>
            <p class="field-help">
                Test Connection fetches `/v1/models` and lets you pick from the
                returned model IDs.
            </p>
            {#if connectionMessage}
                <p class="connection-status" data-state={connectionState}>
                    {connectionMessage}
                </p>
            {/if}
        </div>
    </div>

    <div class="about-modal-actions">
        <button type="button" class="utility-btn cancel-btn" onclick={onClose}>
            Cancel
        </button>
        <button
            type="button"
            class="utility-btn save-btn"
            onclick={handleSave}
            disabled={isSaving}
        >
            {isSaving ? "Saving..." : "Save Configuration"}
        </button>
    </div>
</div>

<style>
    .subtitle-translation-config-modal {
        max-width: 540px;
    }

    .config-form {
        padding: 20px 24px;
        display: flex;
        flex-direction: column;
        gap: 20px;
    }

    .form-group {
        display: flex;
        flex-direction: column;
        gap: 8px;
    }

    .field-header {
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
    }

    .form-group label {
        font-size: 0.85rem;
        font-weight: 600;
        color: rgba(255, 255, 255, 0.9);
    }

    .config-input {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        padding: 10px 12px;
        color: white;
        font-size: 0.9rem;
        outline: none;
        transition: border-color 0.2s;
    }

    .config-input:focus {
        border-color: rgba(255, 255, 255, 0.3);
    }

    .config-select {
        appearance: none;
        -webkit-appearance: none;
    }

    .field-help,
    .connection-status {
        font-size: 0.75rem;
        margin: 0;
    }

    .field-help {
        color: rgba(255, 255, 255, 0.5);
    }

    .connection-status[data-state="success"] {
        color: #9ae998;
    }

    .connection-status[data-state="error"] {
        color: #ff9a8b;
    }

    .test-btn {
        flex: none;
        min-width: 140px;
    }

    .save-btn,
    .cancel-btn {
        flex: 1;
    }

    .cancel-btn {
        border: 1px solid rgba(255, 255, 255, 0.1);
        margin-right: 5px;
    }
</style>
