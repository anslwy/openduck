<!-- LLM model banner for downloading, loading, and switching the selected LLM variant. -->
<script lang="ts">
    import type { GemmaVariant, SelectOption } from "$lib/openduck/types";

    let {
        isDownloadingGemma,
        isGemmaDownloaded,
        isGemmaLoaded,
        selectedGemmaVariant,
        gemmaVariantOptions,
        gemmaVariantDisabled,
        gemmaVariantTooltip,
        gemmaDownloadError,
        gemmaDownloadMessage,
        gemmaDownloadProgress,
        gemmaDownloadIndeterminate,
        isCancellingGemmaDownload,
        isUnloadingGemma,
        isLoadingGemma,
        isClearingGemmaCache,
        formatDownloadPercent,
        handleGemmaVariantChange,
        handleCancelGemmaDownload,
        handleUnloadGemma,
        handleClearGemmaCache,
        handleDownloadGemma,
        handleLoadGemma,
        isExternalGemmaVariant,
        externalProviderLabel,
        externalProviderSupported,
        externalProviderGuideText,
        externalModels,
        selectedExternalModel,
        handleExternalModelChange,
        externalModelDisabled,
        openExternalConfig,
    } = $props<{
        isDownloadingGemma: boolean;
        isGemmaDownloaded: boolean;
        isGemmaLoaded: boolean;
        selectedGemmaVariant: GemmaVariant;
        gemmaVariantOptions: Array<SelectOption<GemmaVariant>>;
        gemmaVariantDisabled: boolean;
        gemmaVariantTooltip: string;
        gemmaDownloadError: string | null;
        gemmaDownloadMessage: string;
        gemmaDownloadProgress: number | null;
        gemmaDownloadIndeterminate: boolean;
        isCancellingGemmaDownload: boolean;
        isUnloadingGemma: boolean;
        isLoadingGemma: boolean;
        isClearingGemmaCache: boolean;
        formatDownloadPercent: (progress: number) => string;
        handleGemmaVariantChange: (event: Event) => Promise<void>;
        handleCancelGemmaDownload: () => Promise<void>;
        handleUnloadGemma: (options?: {
            suppressAlert?: boolean;
        }) => Promise<void>;
        handleClearGemmaCache: () => Promise<void>;
        handleDownloadGemma: () => Promise<void>;
        handleLoadGemma: () => Promise<void>;
        isExternalGemmaVariant: boolean;
        externalProviderLabel: string;
        externalProviderSupported: boolean;
        externalProviderGuideText: string | null;
        externalModels: string[];
        selectedExternalModel: string;
        handleExternalModelChange: (event: Event) => Promise<void>;
        externalModelDisabled: boolean;
        openExternalConfig: () => void;
    }>();
</script>

{#snippet externalModelSelect()}
    {#if isExternalGemmaVariant && externalModels.length > 0}
        <div class="tooltip-shell variant-select-shell external-model-shell">
            <select
                class="variant-select external-model-select"
                value={selectedExternalModel}
                aria-label={`${externalProviderLabel} model`}
                disabled={externalModelDisabled}
                onchange={handleExternalModelChange}
            >
                {#each externalModels as model}
                    <option value={model}>{model}</option>
                {/each}
            </select>
            <div class="tooltip-bubble variant-tooltip">
                Select the {externalProviderLabel} model to use.
            </div>
        </div>
    {/if}
{/snippet}

<div class="download-banner" class:ready={isGemmaDownloaded && isGemmaLoaded}>
    {#if isDownloadingGemma}
        <div class="download-content">
            <div class="banner-heading-row">
                <span class="banner-title">LLM</span>
                <div class="tooltip-shell variant-select-shell">
                    <select
                        class="variant-select"
                        value={selectedGemmaVariant}
                        aria-label="LLM variant"
                        disabled={gemmaVariantDisabled}
                        onchange={handleGemmaVariantChange}
                    >
                        {#each gemmaVariantOptions as option}
                            <option value={option.value} disabled={option.disabled}
                                >{option.label}</option
                            >
                        {/each}
                    </select>
                    <div class="tooltip-bubble variant-tooltip">
                        {gemmaVariantTooltip}
                    </div>
                </div>
                {@render externalModelSelect()}
            </div>
            <div class="download-row">
                <span
                    class="download-status-text"
                    class:failed={!!gemmaDownloadError}>{gemmaDownloadMessage}</span
                >
                {#if gemmaDownloadProgress !== null}
                    <span class="download-percent"
                        >{formatDownloadPercent(gemmaDownloadProgress)}</span
                    >
                {/if}
            </div>
            <div class="progress-row">
                <button
                    type="button"
                    class="progress-cancel-btn"
                    disabled={isCancellingGemmaDownload}
                    aria-label="Cancel LLM download"
                    title={isCancellingGemmaDownload
                        ? "Cancelling download..."
                        : "Cancel download"}
                    onclick={handleCancelGemmaDownload}
                >
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        width="14"
                        height="14"
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
                <div class="progress-track">
                    <div
                        class="progress-fill"
                        class:indeterminate={gemmaDownloadIndeterminate}
                        class:failed={!!gemmaDownloadError}
                        style:width={gemmaDownloadIndeterminate
                            ? "38%"
                            : `${gemmaDownloadProgress ?? 0}%`}
                    ></div>
                </div>
            </div>
        </div>
    {:else if isGemmaDownloaded}
        <div class="banner-row">
            {#if isGemmaLoaded}
                <div class="banner-status">
                    <div class="banner-copy">
                        <div class="banner-heading-row">
                            <span class="banner-title">LLM</span>
                            <div class="tooltip-shell variant-select-shell">
                                <select
                                    class="variant-select"
                                    value={selectedGemmaVariant}
                                    aria-label="LLM variant"
                                    disabled={gemmaVariantDisabled}
                                    onchange={handleGemmaVariantChange}
                                >
                                    {#each gemmaVariantOptions as option}
                                        <option value={option.value} disabled={option.disabled}
                                            >{option.label}</option
                                        >
                                    {/each}
                                </select>
                                <div class="tooltip-bubble variant-tooltip">
                                    {gemmaVariantTooltip}
                                </div>
                            </div>
                            {@render externalModelSelect()}
                        </div>
                        <span class="banner-subtitle">Loaded</span>
                    </div>
                    <div class="loaded-actions">
                        <button
                            class="utility-btn"
                            disabled={isUnloadingGemma}
                            onclick={() => handleUnloadGemma()}
                        >
                            {isUnloadingGemma
                                ? "Unloading..."
                                : isExternalGemmaVariant
                                  ? "Disconnect"
                                  : "Unload"}
                        </button>
                        <div class="status-icon">
                            <svg
                                xmlns="http://www.w3.org/2000/svg"
                                width="18"
                                height="18"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="#34c759"
                                stroke-width="3"
                                stroke-linecap="round"
                                stroke-linejoin="round"
                                ><polyline points="20 6 9 17 4 12" /></svg
                            >
                        </div>
                    </div>
                </div>
            {:else}
                <div class="banner-copy">
                    <div class="banner-heading-row">
                        <span class="banner-title">LLM</span>
                        <div class="tooltip-shell variant-select-shell">
                            <select
                                class="variant-select"
                                value={selectedGemmaVariant}
                                aria-label="LLM variant"
                                disabled={gemmaVariantDisabled}
                                onchange={handleGemmaVariantChange}
                            >
                                {#each gemmaVariantOptions as option}
                                    <option value={option.value} disabled={option.disabled}
                                        >{option.label}</option
                                    >
                                {/each}
                            </select>
                            <div class="tooltip-bubble variant-tooltip">
                                {gemmaVariantTooltip}
                            </div>
                        </div>
                        {@render externalModelSelect()}
                    </div>
                    <div class="banner-subtitle-row">
                        <span class="banner-subtitle"
                            >{isExternalGemmaVariant
                                ? "External"
                                : "Downloaded"}</span
                        >
                        {#if !isExternalGemmaVariant}
                            <button
                                type="button"
                                class="utility-btn subtitle-action-btn"
                                disabled={isLoadingGemma ||
                                    isUnloadingGemma ||
                                    isClearingGemmaCache}
                                onclick={handleClearGemmaCache}
                            >
                                {isClearingGemmaCache
                                    ? "Clearing..."
                                    : "Clear Cache"}
                            </button>
                        {/if}
                    </div>
                    {#if isExternalGemmaVariant && !externalProviderSupported && externalProviderGuideText}
                        <span class="banner-detail">{externalProviderGuideText}</span>
                    {/if}
                </div>
                <div class="banner-action-row">
                    <button
                        class="download-btn"
                        disabled={isLoadingGemma ||
                            isUnloadingGemma ||
                            isClearingGemmaCache}
                        onclick={handleLoadGemma}
                    >
                        {isLoadingGemma
                            ? "Loading..."
                            : isExternalGemmaVariant
                              ? "Connect"
                              : "Load Model"}
                    </button>
                    {#if isExternalGemmaVariant}
                        <button
                            type="button"
                            class="utility-btn config-btn"
                            onclick={openExternalConfig}
                            title={`Configure ${externalProviderLabel} connection`}
                        >
                            <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path><circle cx="12" cy="12" r="3"></circle></svg>
                        </button>
                    {/if}
                </div>
            {/if}
        </div>
    {:else}
        <div class="banner-row">
            <div class="banner-copy">
                <div class="banner-heading-row">
                    <span class="banner-title">LLM</span>
                    <div class="tooltip-shell variant-select-shell">
                        <select
                            class="variant-select"
                            value={selectedGemmaVariant}
                            aria-label="LLM variant"
                            disabled={gemmaVariantDisabled}
                            onchange={handleGemmaVariantChange}
                        >
                            {#each gemmaVariantOptions as option}
                                <option value={option.value} disabled={option.disabled}
                                    >{option.label}</option
                                >
                            {/each}
                        </select>
                        <div class="tooltip-bubble variant-tooltip">
                            {gemmaVariantTooltip}
                        </div>
                    </div>
                    {@render externalModelSelect()}
                </div>
                {#if gemmaDownloadError}
                    <span class="banner-subtitle error">Download failed</span>
                    <span class="banner-detail error">{gemmaDownloadError}</span>
                {:else}
                    <span class="banner-subtitle"
                        >{isExternalGemmaVariant
                            ? `${externalProviderLabel} service not detected`
                            : "Model not found in cache"}</span
                    >
                {/if}
            </div>
            <div class="banner-action-row">
                <button
                    class="download-btn"
                    disabled={isDownloadingGemma}
                    onclick={handleDownloadGemma}
                >
                    {isDownloadingGemma
                        ? "Downloading..."
                        : gemmaDownloadError
                          ? "Retry Download"
                          : "Download Model"}
                </button>
                {#if isExternalGemmaVariant}
                    <button
                        type="button"
                        class="utility-btn config-btn"
                        onclick={openExternalConfig}
                        title={`Configure ${externalProviderLabel} connection`}
                    >
                        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path><circle cx="12" cy="12" r="3"></circle></svg>
                    </button>
                {/if}
            </div>
        </div>
    {/if}
</div>
