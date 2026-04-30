<!-- Modal for configuring external LLM connection details like URL and API key. -->
<script lang="ts">
    let {
        providerName,
        baseUrl,
        hasApiKey,
        urlPlaceholder,
        onSave,
        onClose,
    } = $props<{
        providerName: string;
        baseUrl: string;
        hasApiKey: boolean;
        urlPlaceholder: string;
        onSave: (url: string, key: string, clearKey: boolean) => Promise<void>;
        onClose: () => void;
    }>();

    let url = $state("");
    let key = $state("");
    let clearSavedKey = $state(false);
    let isSaving = $state(false);

    const providers = [
        {
            id: "openai",
            name: "OpenAI",
            baseUrl: "https://api.openai.com",
            helpText: "Get your API key from the",
            linkText: "OpenAI Dashboard",
            linkUrl: "https://platform.openai.com/api-keys",
        },
        {
            id: "gemini",
            name: "Gemini",
            baseUrl: "https://generativelanguage.googleapis.com/v1beta/",
            helpText: "Get your API key from the",
            linkText: "Google AI Studio",
            linkUrl: "https://aistudio.google.com/app/apikey",
        },
        {
            id: "deepseek",
            name: "DeepSeek",
            baseUrl: "https://api.deepseek.com",
            helpText: "Get your API key from the",
            linkText: "DeepSeek Platform",
            linkUrl: "https://platform.deepseek.com/api_keys",
        },
        {
            id: "groq",
            name: "Groq",
            baseUrl: "https://api.groq.com/openai",
            helpText: "Get your API key from the",
            linkText: "Groq Console",
            linkUrl: "https://console.groq.com/keys",
        },
        {
            id: "openrouter",
            name: "OpenRouter",
            baseUrl: "https://openrouter.ai/api",
            helpText: "Get your API key from the",
            linkText: "OpenRouter Settings",
            linkUrl: "https://openrouter.ai/keys",
        },
        {
            id: "mistral",
            name: "Mistral AI",
            baseUrl: "https://api.mistral.ai/v1",
            helpText: "Get your API key from the",
            linkText: "Mistral Console",
            linkUrl: "https://console.mistral.ai/api-keys/",
        },
        { id: "custom", name: "Custom" },
    ];

    let selectedProviderId = $state(providers[0].id);
    const isOpenAiCompatible = $derived(
        providerName === "OpenAI-compatible API",
    );

    $effect(() => {
        url = baseUrl;
        key = "";
        clearSavedKey = false;

        if (isOpenAiCompatible) {
            const matchingProvider = providers.find((p) => p.baseUrl === baseUrl);
            if (matchingProvider) {
                selectedProviderId = matchingProvider.id;
            } else {
                selectedProviderId = "custom";
            }
        }
    });

    $effect(() => {
        if (isOpenAiCompatible && selectedProviderId !== "custom") {
            const provider = providers.find((p) => p.id === selectedProviderId);
            if (provider && provider.baseUrl) {
                url = provider.baseUrl;
            }
        }
    });

    async function handleSave() {
        const normalizedKey = key.trim();
        const shouldClearKey = normalizedKey === "" && clearSavedKey;

        isSaving = true;
        try {
            await onSave(url, normalizedKey, shouldClearKey);
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
        {#if isOpenAiCompatible}
            <div class="form-group">
                <label for="provider-select">Provider</label>
                <select
                    id="provider-select"
                    bind:value={selectedProviderId}
                    class="config-input config-select"
                >
                    {#each providers as provider}
                        <option value={provider.id}>{provider.name}</option>
                    {/each}
                </select>
                {#if selectedProviderId !== "custom"}
                    {@const provider = providers.find(
                        (p) => p.id === selectedProviderId,
                    )}
                    {#if provider}
                        <p class="field-help">
                            {provider.helpText}
                            <a
                                href={provider.linkUrl}
                                target="_blank"
                                rel="noopener noreferrer"
                                class="help-link">{provider.linkText}</a
                            >.
                        </p>
                    {/if}
                {/if}
            </div>
        {/if}

        {#if !isOpenAiCompatible || selectedProviderId === "custom"}
            <div class="form-group">
                <label for="external-llm-url">Base URL</label>
                <input
                    id="external-llm-url"
                    type="text"
                    bind:value={url}
                    placeholder={urlPlaceholder}
                    class="config-input"
                />
                <p class="field-help">
                    The root URL of your {providerName} service.
                </p>
            </div>
        {/if}

        <div class="form-group">
            <label for="external-llm-key">API Key (Optional)</label>
            <input
                id="external-llm-key"
                type="password"
                bind:value={key}
                oninput={() => {
                    if (key.trim() !== "") {
                        clearSavedKey = false;
                    }
                }}
                placeholder="Enter API key if required"
                class="config-input"
            />
            <p class="field-help">
                {#if hasApiKey}
                    A key is already saved in your system credential store. Leave
                    this blank to keep it, or enter a new key to replace it.
                {:else}
                    Required for providers like OpenAI. Optional for local
                    servers or unauthenticated proxies.
                {/if}
            </p>
            {#if hasApiKey}
                <label class="checkbox-row">
                    <input
                        type="checkbox"
                        bind:checked={clearSavedKey}
                        disabled={key.trim() !== ""}
                    />
                    <span>Clear saved API key</span>
                </label>
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

    .config-select {
        appearance: none;
        -webkit-appearance: none;
        background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='white' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
        background-repeat: no-repeat;
        background-position: right 12px center;
        padding-right: 36px;
    }

    .field-help {
        font-size: 0.75rem;
        color: rgba(255, 255, 255, 0.5);
        margin: 0;
    }

    .help-link {
        color: #9ae998;
        text-decoration: none;
    }

    .help-link:hover {
        text-decoration: underline;
    }

    .checkbox-row {
        display: flex;
        align-items: center;
        gap: 10px;
        font-size: 0.78rem;
        color: rgba(255, 255, 255, 0.72);
    }

    .checkbox-row input {
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
