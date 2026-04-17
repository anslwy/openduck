<!-- STT model banner for choosing between Gemma-based transcription and a separate Whisper worker. -->
<script lang="ts">
    import type { SelectOption, SttModelVariant } from "$lib/openduck/types";

    let {
        selectedGemmaVariant,
        sttUsesGemma,
        isGemmaLoaded,
        isDownloadingStt,
        isSttDownloaded,
        isSttLoaded,
        selectedSttModel,
        sttModelOptions,
        sttVariantDisabled,
        sttModelTooltip,
        selectedSttModelLabel,
        sttDownloadError,
        sttDownloadMessage,
        sttDownloadProgress,
        sttDownloadIndeterminate,
        sttLoadMessage,
        isCancellingSttDownload,
        isUnloadingStt,
        isLoadingStt,
        isClearingSttCache,
        formatDownloadPercent,
        handleSttModelChange,
        handleCancelSttDownload,
        handleUnloadStt,
        handleClearSttCache,
        handleDownloadStt,
        handleLoadStt,
        isExternalGemmaVariant,
        externalProviderLabel,
    } = $props<{
        sttUsesGemma: boolean;
        isGemmaLoaded: boolean;
        isDownloadingStt: boolean;
        isSttDownloaded: boolean;
        isSttLoaded: boolean;
        selectedSttModel: SttModelVariant;
        sttModelOptions: Array<SelectOption<SttModelVariant>>;
        sttVariantDisabled: boolean;
        sttModelTooltip: string;
        selectedSttModelLabel: string;
        sttDownloadError: string | null;
        sttDownloadMessage: string;
        sttDownloadProgress: number | null;
        sttDownloadIndeterminate: boolean;
        sttLoadMessage: string;
        selectedGemmaVariant: string;
        isCancellingSttDownload: boolean;
        isUnloadingStt: boolean;
        isLoadingStt: boolean;
        isClearingSttCache: boolean;
        formatDownloadPercent: (progress: number) => string;
        handleSttModelChange: (event: Event) => Promise<void>;
        handleCancelSttDownload: () => Promise<void>;
        handleUnloadStt: () => Promise<void>;
        handleClearSttCache: () => Promise<void>;
        handleDownloadStt: () => Promise<void>;
        handleLoadStt: () => Promise<void>;
        isExternalGemmaVariant: boolean;
        externalProviderLabel: string;
    }>();
</script>

<div
    class="download-banner"
    class:ready={sttUsesGemma ? isGemmaLoaded : isSttDownloaded && isSttLoaded}
>
    {#if sttUsesGemma}
        <div class="banner-row">
            <div class="banner-copy">
                <div class="banner-heading-row">
                    <span class="banner-title">STT</span>
                    <div class="tooltip-shell variant-select-shell">
                        <select
                            class="variant-select"
                            value={selectedSttModel}
                            aria-label="STT model"
                            disabled={sttVariantDisabled}
                            onchange={handleSttModelChange}
                        >
                            {#each sttModelOptions as option}
                                <option value={option.value} disabled={option.disabled}
                                    >{option.label}</option
                                >
                            {/each}
                        </select>
                        <div class="tooltip-bubble variant-tooltip">
                            {sttModelTooltip}
                        </div>
                    </div>
                </div>
                <span class="banner-subtitle"
                    >{isExternalGemmaVariant
                        ? `Not supported with ${externalProviderLabel}`
                        : isGemmaLoaded
                          ? "Using the loaded Gemma model"
                          : "Loads with the Gemma model above"}</span
                >
            </div>
        </div>
    {:else if isDownloadingStt}
        <div class="download-content">
            <div class="banner-heading-row">
                <span class="banner-title">STT</span>
                <div class="tooltip-shell variant-select-shell">
                    <select
                        class="variant-select"
                        value={selectedSttModel}
                        aria-label="STT model"
                        disabled={sttVariantDisabled}
                        onchange={handleSttModelChange}
                    >
                        {#each sttModelOptions as option}
                            <option value={option.value}>{option.label}</option>
                        {/each}
                    </select>
                    <div class="tooltip-bubble variant-tooltip">
                        {sttModelTooltip}
                    </div>
                </div>
            </div>
            <div class="download-row">
                <span
                    class="download-status-text"
                    class:failed={!!sttDownloadError}
                    >{selectedSttModelLabel}: {sttDownloadMessage}</span
                >
                {#if sttDownloadProgress !== null}
                    <span class="download-percent"
                        >{formatDownloadPercent(sttDownloadProgress)}</span
                    >
                {/if}
            </div>
            <div class="progress-row">
                <button
                    type="button"
                    class="progress-cancel-btn"
                    disabled={isCancellingSttDownload}
                    aria-label="Cancel STT download"
                    title={isCancellingSttDownload
                        ? "Cancelling download..."
                        : "Cancel download"}
                    onclick={handleCancelSttDownload}
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
                        class:indeterminate={sttDownloadIndeterminate}
                        class:failed={!!sttDownloadError}
                        style:width={sttDownloadIndeterminate
                            ? "38%"
                            : `${sttDownloadProgress ?? 0}%`}
                    ></div>
                </div>
            </div>
        </div>
    {:else if isSttDownloaded}
        <div class="banner-row">
            {#if isSttLoaded}
                <div class="banner-status">
                    <div class="banner-copy">
                        <div class="banner-heading-row">
                            <span class="banner-title">STT</span>
                            <div class="tooltip-shell variant-select-shell">
                                <select
                                    class="variant-select"
                                    value={selectedSttModel}
                                    aria-label="STT model"
                                    disabled={sttVariantDisabled}
                                    onchange={handleSttModelChange}
                                >
                                    {#each sttModelOptions as option}
                                        <option value={option.value}
                                            >{option.label}</option
                                        >
                                    {/each}
                                </select>
                                <div class="tooltip-bubble variant-tooltip">
                                    {sttModelTooltip}
                                </div>
                            </div>
                        </div>
                        <span class="banner-subtitle">Loaded</span>
                    </div>
                    <div class="loaded-actions">
                        <button
                            class="utility-btn"
                            disabled={isUnloadingStt}
                            onclick={handleUnloadStt}
                        >
                            {isUnloadingStt ? "Unloading..." : "Unload"}
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
                        <span class="banner-title">STT</span>
                        <div class="tooltip-shell variant-select-shell">
                            <select
                                class="variant-select"
                                value={selectedSttModel}
                                aria-label="STT model"
                                disabled={sttVariantDisabled}
                                onchange={handleSttModelChange}
                            >
                                {#each sttModelOptions as option}
                                    <option value={option.value}
                                        >{option.label}</option
                                    >
                                {/each}
                            </select>
                            <div class="tooltip-bubble variant-tooltip">
                                {sttModelTooltip}
                            </div>
                        </div>
                    </div>
                    {#if isLoadingStt}
                        <span class="banner-subtitle">{sttLoadMessage}</span>
                    {:else}
                        <div class="banner-subtitle-row">
                            <span class="banner-subtitle">Downloaded</span>
                            <button
                                type="button"
                                class="utility-btn subtitle-action-btn"
                                disabled={isLoadingStt ||
                                    isUnloadingStt ||
                                    isClearingSttCache}
                                onclick={handleClearSttCache}
                            >
                                {isClearingSttCache ? "Clearing..." : "Clear Cache"}
                            </button>
                        </div>
                    {/if}
                </div>
                <button
                    class="download-btn"
                    disabled={isLoadingStt ||
                        isUnloadingStt ||
                        isClearingSttCache}
                    onclick={handleLoadStt}
                >
                    {isLoadingStt ? "Loading..." : "Load Model"}
                </button>
            {/if}
        </div>
    {:else}
        <div class="banner-row">
            <div class="banner-copy">
                <div class="banner-heading-row">
                    <span class="banner-title">STT</span>
                    <div class="tooltip-shell variant-select-shell">
                        <select
                            class="variant-select"
                            value={selectedSttModel}
                            aria-label="STT model"
                            disabled={sttVariantDisabled}
                            onchange={handleSttModelChange}
                        >
                            {#each sttModelOptions as option}
                                <option value={option.value} disabled={option.disabled}
                                    >{option.label}</option
                                >
                            {/each}
                        </select>
                        <div class="tooltip-bubble variant-tooltip">
                            {sttModelTooltip}
                        </div>
                    </div>
                </div>
                {#if sttDownloadError}
                    <span class="banner-subtitle error">Download failed</span>
                    <span class="banner-detail error">{sttDownloadError}</span>
                {:else}
                    <span class="banner-subtitle">Model not found in cache</span>
                {/if}
            </div>
            <button
                class="download-btn"
                disabled={isDownloadingStt}
                onclick={handleDownloadStt}
            >
                {isDownloadingStt
                    ? "Downloading..."
                    : sttDownloadError
                      ? "Retry Download"
                      : "Download Model"}
            </button>
        </div>
    {/if}
</div>
