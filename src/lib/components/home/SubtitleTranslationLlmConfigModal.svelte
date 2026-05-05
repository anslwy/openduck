<!-- Modal for configuring Apple or OpenAI-compatible AI subtitle translation. -->
<script lang="ts">
    import type {
        AiSubtitleTargetLanguage,
        AiSubtitleTranslationProvider,
        AppleTranslationLanguagePackStatus,
        KokoroLanguage,
    } from "$lib/openduck/types";

    let {
        provider,
        baseUrl,
        hasApiKey,
        modelId,
        targetLanguage,
        sourceLanguage,
        appleTranslationLanguagePackStatus,
        appleTranslationLanguagePackMessage,
        isCheckingAppleTranslationLanguagePack,
        isInstallingAppleTranslationLanguagePack,
        onSave,
        onTestConnection,
        onCheckAppleTranslationLanguagePack,
        onInstallAppleTranslationLanguagePack,
        onClose,
    } = $props<{
        provider: AiSubtitleTranslationProvider;
        baseUrl: string;
        hasApiKey: boolean;
        modelId: string;
        targetLanguage: AiSubtitleTargetLanguage;
        sourceLanguage: KokoroLanguage;
        appleTranslationLanguagePackStatus: AppleTranslationLanguagePackStatus | null;
        appleTranslationLanguagePackMessage: string;
        isCheckingAppleTranslationLanguagePack: boolean;
        isInstallingAppleTranslationLanguagePack: boolean;
        onSave: (
            provider: AiSubtitleTranslationProvider,
            url: string,
            key: string,
            clearKey: boolean,
            modelId: string,
        ) => Promise<void>;
        onTestConnection: (
            url: string,
            key: string,
            useSavedKey: boolean,
        ) => Promise<string[]>;
        onCheckAppleTranslationLanguagePack: (
            targetLang: AiSubtitleTargetLanguage,
            sourceLanguage: KokoroLanguage,
        ) => Promise<AppleTranslationLanguagePackStatus | null>;
        onInstallAppleTranslationLanguagePack: (
            targetLang: AiSubtitleTargetLanguage,
            sourceLanguage: KokoroLanguage,
        ) => Promise<AppleTranslationLanguagePackStatus | null>;
        onClose: () => void;
    }>();

    let selectedTranslationProvider =
        $state<AiSubtitleTranslationProvider>("openai_compatible");
    let url = $state("");
    let key = $state("");
    let clearSavedKey = $state(false);
    let selectedModelId = $state("");
    let availableModels = $state<string[]>([]);
    let connectionState = $state<"idle" | "success" | "error">("idle");
    let connectionMessage = $state("");
    let isTesting = $state(false);
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
        {
            id: "xai",
            name: "xAI",
            baseUrl: "https://api.x.ai",
            helpText: "Get your API key from the",
            linkText: "xAI Console",
            linkUrl: "https://console.x.ai/",
        },
        { id: "custom", name: "Custom" },
    ];

    let selectedProviderId = $state(providers[0].id);

    $effect(() => {
        selectedTranslationProvider = provider;
        url = baseUrl;
        key = "";
        clearSavedKey = false;
        selectedModelId = modelId;
        availableModels = modelId ? [modelId] : [];
        connectionState = "idle";
        connectionMessage = "";

        const matchingProvider = providers.find(
            (p) => p.id !== "custom" && p.baseUrl === baseUrl,
        );
        if (matchingProvider) {
            selectedProviderId = matchingProvider.id;
        } else {
            selectedProviderId = "custom";
        }
    });

    let lastAppleTranslationStatusKey = "";

    $effect(() => {
        const statusKey = `${selectedTranslationProvider}:${targetLanguage}:${sourceLanguage}`;

        if (
            selectedTranslationProvider !== "apple" ||
            targetLanguage === "none"
        ) {
            lastAppleTranslationStatusKey = statusKey;
            return;
        }

        if (lastAppleTranslationStatusKey === statusKey) {
            return;
        }

        lastAppleTranslationStatusKey = statusKey;
        void onCheckAppleTranslationLanguagePack(
            targetLanguage,
            sourceLanguage,
        );
    });

    $effect(() => {
        if (selectedProviderId !== "custom") {
            const provider = providers.find((p) => p.id === selectedProviderId);
            if (provider) {
                url = provider.baseUrl;
            }
        }
    });

    function validationMessage() {
        if (selectedTranslationProvider === "apple") {
            if (targetLanguage === "none") {
                return "Choose a subtitle translation language before using Apple translation.";
            }

            return null;
        }

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

    function appleTranslationStatusText() {
        if (targetLanguage === "none") {
            return "Choose a subtitle translation language first.";
        }

        if (isCheckingAppleTranslationLanguagePack) {
            return "Checking Apple language pack...";
        }

        if (appleTranslationLanguagePackMessage) {
            return appleTranslationLanguagePackMessage;
        }

        if (!appleTranslationLanguagePackStatus) {
            return "Apple language pack status has not been checked yet.";
        }

        if (appleTranslationLanguagePackStatus.status === "installed") {
            return `${appleTranslationLanguagePackStatus.targetLanguage} is installed for Apple translation.`;
        }

        if (appleTranslationLanguagePackStatus.status === "supported") {
            return `${appleTranslationLanguagePackStatus.targetLanguage} needs an Apple language pack download.`;
        }

        if (appleTranslationLanguagePackStatus.status === "unsupported") {
            return `Apple translation does not support ${appleTranslationLanguagePackStatus.sourceLanguage} to ${appleTranslationLanguagePackStatus.targetLanguage}.`;
        }

        return "Apple language pack status is unknown.";
    }

    async function handleInstallAppleLanguagePack() {
        connectionState = "idle";
        connectionMessage = "";
        await onInstallAppleTranslationLanguagePack(
            targetLanguage,
            sourceLanguage,
        );
    }

    async function handleTestConnection() {
        const normalizedUrl = url.trim();
        const normalizedKey = key.trim();
        const useSavedKey = normalizedKey === "" && hasApiKey && !clearSavedKey;

        if (!normalizedUrl) {
            connectionState = "error";
            connectionMessage = "Enter a Base URL first.";
            return;
        }

        isTesting = true;
        connectionState = "idle";
        connectionMessage = "";

        try {
            const models = await onTestConnection(
                normalizedUrl,
                normalizedKey,
                useSavedKey,
            );
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

        const normalizedKey = key.trim();
        const shouldClearKey =
            normalizedKey === "" &&
            (clearSavedKey ||
                selectedTranslationProvider === "apple" ||
                (url.trim() === "" && selectedModelId.trim() === ""));

        isSaving = true;
        try {
            await onSave(
                selectedTranslationProvider,
                selectedTranslationProvider === "apple" ? "" : url.trim(),
                selectedTranslationProvider === "apple" ? "" : normalizedKey,
                shouldClearKey,
                selectedTranslationProvider === "apple"
                    ? ""
                    : selectedModelId.trim(),
            );
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
                >Use Apple translation or a separate OpenAI-compatible model for
                subtitle translation.</span
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
            <label for="translation-type-select">Translation Type</label>
            <select
                id="translation-type-select"
                bind:value={selectedTranslationProvider}
                class="config-input config-select"
            >
                <option value="apple">Apple</option>
                <option value="openai_compatible">OpenAI-compatible</option>
            </select>
        </div>

        {#if selectedTranslationProvider === "apple"}
            <div class="apple-pack-panel">
                <div class="field-header">
                    <span class="apple-pack-title">Apple Language Pack</span>
                    <button
                        type="button"
                        class="utility-btn test-btn"
                        onclick={() =>
                            onCheckAppleTranslationLanguagePack(
                                targetLanguage,
                                sourceLanguage,
                            )}
                        disabled={isCheckingAppleTranslationLanguagePack ||
                            targetLanguage === "none"}
                    >
                        {isCheckingAppleTranslationLanguagePack
                            ? "Checking..."
                            : "Check"}
                    </button>
                </div>
                <p
                    class="connection-status apple-pack-status"
                    data-state={appleTranslationLanguagePackStatus?.status ===
                    "installed"
                        ? "success"
                        : appleTranslationLanguagePackStatus?.status ===
                                "unsupported" ||
                            appleTranslationLanguagePackMessage
                          ? "error"
                          : "idle"}
                >
                    {appleTranslationStatusText()}
                </p>
                {#if connectionMessage}
                    <p class="connection-status" data-state={connectionState}>
                        {connectionMessage}
                    </p>
                {/if}
                {#if targetLanguage !== "none" && appleTranslationLanguagePackStatus?.status !== "installed" && appleTranslationLanguagePackStatus?.status !== "unsupported"}
                    <button
                        type="button"
                        class="utility-btn save-btn apple-install-btn"
                        onclick={handleInstallAppleLanguagePack}
                        disabled={isInstallingAppleTranslationLanguagePack}
                    >
                        {isInstallingAppleTranslationLanguagePack
                            ? "Installing..."
                            : "Download & Install"}
                    </button>
                {/if}
            </div>
        {:else}
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
                            {#if provider.linkUrl}
                                <a
                                    href={provider.linkUrl}
                                    target="_blank"
                                    rel="noopener noreferrer"
                                    class="help-link">{provider.linkText}</a
                                >.
                            {/if}
                        </p>
                    {/if}
                {/if}
            </div>

            {#if selectedProviderId === "custom"}
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
                        Clear the Base URL and choose "Use current call LLM" to
                        keep using the current call LLM for translation.
                    </p>
                </div>
            {/if}

            <div class="form-group">
                <label for="subtitle-translation-key">API Key (Optional)</label>
                <input
                    id="subtitle-translation-key"
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
                        A key is already saved in your system credential store.
                        Leave this blank to keep it, or enter a new key to
                        replace it.
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
                    Test Connection fetches `/v1/models` and lets you pick from
                    the returned model IDs.
                </p>
                {#if connectionMessage}
                    <p class="connection-status" data-state={connectionState}>
                        {connectionMessage}
                    </p>
                {/if}
            </div>
        {/if}
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
        z-index: 1101;
    }

    :global(.subtitle-translation-config-backdrop) {
        z-index: 1100 !important;
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
        background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='white' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
        background-repeat: no-repeat;
        background-position: right 12px center;
        padding-right: 36px;
    }

    .help-link {
        color: #9ae998;
        text-decoration: none;
    }

    .help-link:hover {
        text-decoration: underline;
    }

    .field-help,
    .connection-status {
        font-size: 0.75rem;
        margin: 0;
    }

    .field-help {
        color: rgba(255, 255, 255, 0.5);
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

    .apple-pack-panel {
        display: flex;
        flex-direction: column;
        gap: 12px;
        padding: 14px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 8px;
        background: rgba(255, 255, 255, 0.04);
    }

    .apple-pack-title {
        font-size: 0.85rem;
        font-weight: 650;
        color: rgba(255, 255, 255, 0.9);
    }

    .apple-pack-status {
        line-height: 1.4;
    }

    .apple-install-btn {
        width: 100%;
    }

    .connection-status {
        color: white;
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
