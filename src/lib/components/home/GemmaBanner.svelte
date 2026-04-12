<!-- Gemma model banner for downloading, loading, and switching the selected LLM variant. -->
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
        handleUnloadGemma: () => Promise<void>;
        handleClearGemmaCache: () => Promise<void>;
        handleDownloadGemma: () => Promise<void>;
        handleLoadGemma: () => Promise<void>;
    }>();
</script>

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
                            <option value={option.value}>{option.label}</option>
                        {/each}
                    </select>
                    <div class="tooltip-bubble variant-tooltip">
                        {gemmaVariantTooltip}
                    </div>
                </div>
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
                    aria-label="Cancel Gemma download"
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
                                        <option value={option.value}
                                            >{option.label}</option
                                        >
                                    {/each}
                                </select>
                                <div class="tooltip-bubble variant-tooltip">
                                    {gemmaVariantTooltip}
                                </div>
                            </div>
                        </div>
                        <span class="banner-subtitle">Loaded</span>
                    </div>
                    <div class="loaded-actions">
                        <button
                            class="utility-btn"
                            disabled={isUnloadingGemma}
                            onclick={handleUnloadGemma}
                        >
                            {isUnloadingGemma ? "Unloading..." : "Unload"}
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
                                    <option value={option.value}
                                        >{option.label}</option
                                    >
                                {/each}
                            </select>
                            <div class="tooltip-bubble variant-tooltip">
                                {gemmaVariantTooltip}
                            </div>
                        </div>
                    </div>
                    <div class="banner-subtitle-row">
                        <span class="banner-subtitle">Downloaded</span>
                        <button
                            type="button"
                            class="utility-btn subtitle-action-btn"
                            disabled={isLoadingGemma ||
                                isUnloadingGemma ||
                                isClearingGemmaCache}
                            onclick={handleClearGemmaCache}
                        >
                            {isClearingGemmaCache ? "Clearing..." : "Clear Cache"}
                        </button>
                    </div>
                </div>
                <button
                    class="download-btn"
                    disabled={isLoadingGemma ||
                        isUnloadingGemma ||
                        isClearingGemmaCache}
                    onclick={handleLoadGemma}
                >
                    {isLoadingGemma ? "Loading..." : "Load Model"}
                </button>
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
                                <option value={option.value}>{option.label}</option>
                            {/each}
                        </select>
                        <div class="tooltip-bubble variant-tooltip">
                            {gemmaVariantTooltip}
                        </div>
                    </div>
                </div>
                {#if gemmaDownloadError}
                    <span class="banner-subtitle error">Download failed</span>
                    <span class="banner-detail error">{gemmaDownloadError}</span>
                {:else}
                    <span class="banner-subtitle">Model not found in cache</span>
                {/if}
            </div>
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
        </div>
    {/if}
</div>
