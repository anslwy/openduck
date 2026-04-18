<!-- Modal for configuring external LLM connection details like URL and API key. -->
<script lang="ts">
    let { providerName, baseUrl, apiKey, urlPlaceholder, onSave, onClose } = $props<{
        providerName: string;
        baseUrl: string;
        apiKey: string;
        urlPlaceholder: string;
        onSave: (url: string, key: string) => Promise<void>;
        onClose: () => void;
    }>();

    let url = $state("");
    let key = $state("");
    let isSaving = $state(false);

    $effect(() => {
        url = baseUrl;
        key = apiKey;
    });

    async function handleSave() {
        isSaving = true;
        try {
            await onSave(url, key);
            onClose();
        } catch (error) {
            console.error(`Failed to save ${providerName} config:`, error);
            alert("Failed to save configuration.");
        } finally {
            isSaving = false;
        }
    }
</script>

<button
    type="button"
    class="about-modal-backdrop"
    aria-label="Close configuration"
    onclick={onClose}
></button>

<div
    class="about-modal external-llm-config-modal"
    role="dialog"
    aria-labelledby="external-llm-config-title"
    aria-modal="true"
>
    <div class="about-modal-header">
        <div class="about-modal-copy">
            <span class="about-modal-title" id="external-llm-config-title"
                >{providerName} Configuration</span
            >
            <span class="about-modal-subtitle"
                >Configure connection to your {providerName} service</span
            >
        </div>
        <button
            type="button"
            class="conversation-close-btn"
            onclick={onClose}
            aria-label="Close configuration"
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
            <label for="external-llm-url">Base URL</label>
            <input
                id="external-llm-url"
                type="text"
                bind:value={url}
                placeholder={urlPlaceholder}
                class="config-input"
            />
            <p class="field-help">The root URL of your {providerName} service.</p>
        </div>

        <div class="form-group">
            <label for="external-llm-key">API Key (Optional)</label>
            <input
                id="external-llm-key"
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
    </div>

    <div class="about-modal-actions">
        <button type="button" class="utility-btn cancel-btn" onclick={onClose}>
            Cancel
        </button>
        <button
            type="button"
            class="utility-btn save-btn"
            onclick={handleSave}
            disabled={isSaving || !url}
        >
            {isSaving ? "Saving..." : "Save Configuration"}
        </button>
    </div>
</div>

<style>
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

    .field-help {
        font-size: 0.75rem;
        color: rgba(255, 255, 255, 0.5);
        margin: 0;
    }

    .save-btn,
    .cancel-btn {
        flex: 1;
    }

    .cancel-btn {
        border: 1px solid rgba(255, 255, 255, 0.1);
        margin-right: 5px;
    }

    .cancel-btn {
        border: 1px solid rgba(255, 255, 255, 0.1);
    }
</style>
