<!-- Speech model banner for managing the active TTS backend and its optional quantization toggle. -->
<script lang="ts">
    import type { CsmModelVariant, SelectOption } from "$lib/openduck/types";

    let {
        isDownloadingCsm,
        isCsmDownloaded,
        isCsmLoaded,
        selectedCsmModel,
        csmModelOptions,
        csmVariantDisabled,
        csmModelTooltip,
        csmQuantizeAvailable,
        isCsmQuantized,
        isUpdatingCsmQuantize,
        selectedCsmModelLabel,
        csmDownloadError,
        csmDownloadMessage,
        csmDownloadProgress,
        csmDownloadIndeterminate,
        csmLoadMessage,
        csmNotificationMessage,
        isCancellingCsmDownload,
        isUnloadingCsm,
        isLoadingCsm,
        isClearingCsmCache,
        formatDownloadPercent,
        handleCsmModelChange,
        handleCancelCsmDownload,
        handleUnloadCsm,
        handleClearCsmCache,
        handleDownloadCsm,
        handleLoadCsm,
        handleCsmQuantizeToggle,
    } = $props<{
        isDownloadingCsm: boolean;
        isCsmDownloaded: boolean;
        isCsmLoaded: boolean;
        selectedCsmModel: CsmModelVariant;
        csmModelOptions: Array<SelectOption<CsmModelVariant>>;
        csmVariantDisabled: boolean;
        csmModelTooltip: string;
        csmQuantizeAvailable: boolean;
        isCsmQuantized: boolean;
        isUpdatingCsmQuantize: boolean;
        selectedCsmModelLabel: string;
        csmDownloadError: string | null;
        csmDownloadMessage: string;
        csmDownloadProgress: number | null;
        csmDownloadIndeterminate: boolean;
        csmLoadMessage: string;
        csmNotificationMessage: string | null;
        isCancellingCsmDownload: boolean;
        isUnloadingCsm: boolean;
        isLoadingCsm: boolean;
        isClearingCsmCache: boolean;
        formatDownloadPercent: (progress: number) => string;
        handleCsmModelChange: (event: Event) => Promise<void>;
        handleCancelCsmDownload: () => Promise<void>;
        handleUnloadCsm: () => Promise<void>;
        handleClearCsmCache: () => Promise<void>;
        handleDownloadCsm: () => Promise<void>;
        handleLoadCsm: () => Promise<void>;
        handleCsmQuantizeToggle: () => Promise<void>;
    }>();
</script>

<div
    class="download-banner voice-config-banner"
    class:ready={isCsmDownloaded && isCsmLoaded}
>
    {#if isDownloadingCsm}
        <div class="download-content">
            <div class="banner-heading-row">
                <span class="banner-title">TTS</span>
                <div class="tooltip-shell variant-select-shell">
                    <select
                        class="variant-select"
                        value={selectedCsmModel}
                        aria-label="TTS model"
                        disabled={csmVariantDisabled}
                        onchange={handleCsmModelChange}
                    >
                        {#each csmModelOptions as option}
                            <option value={option.value}>{option.label}</option>
                        {/each}
                    </select>
                    <div class="tooltip-bubble variant-tooltip">
                        {csmModelTooltip}
                    </div>
                </div>
            </div>
            <div class="download-row">
                <span
                    class="download-status-text"
                    class:failed={!!csmDownloadError}
                    >{selectedCsmModelLabel}: {csmDownloadMessage}</span
                >
                {#if csmDownloadProgress !== null}
                    <span class="download-percent"
                        >{formatDownloadPercent(csmDownloadProgress)}</span
                    >
                {/if}
            </div>
            <div class="progress-row">
                <button
                    type="button"
                    class="progress-cancel-btn"
                    disabled={isCancellingCsmDownload}
                    aria-label="Cancel TTS model download"
                    title={isCancellingCsmDownload
                        ? "Cancelling download..."
                        : "Cancel download"}
                    onclick={handleCancelCsmDownload}
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
                        class:indeterminate={csmDownloadIndeterminate}
                        class:failed={!!csmDownloadError}
                        style:width={csmDownloadIndeterminate
                            ? "38%"
                            : `${csmDownloadProgress ?? 0}%`}
                    ></div>
                </div>
            </div>
        </div>
    {:else if isCsmDownloaded}
        <div class="banner-row">
            {#if isCsmLoaded}
                <div class="banner-status">
                    <div class="banner-copy">
                        <div class="banner-heading-row">
                            <span class="banner-title">TTS</span>
                            <div class="tooltip-shell variant-select-shell">
                                <select
                                    class="variant-select"
                                    value={selectedCsmModel}
                                    aria-label="TTS model"
                                    disabled={csmVariantDisabled}
                                    onchange={handleCsmModelChange}
                                >
                                    {#each csmModelOptions as option}
                                        <option value={option.value}
                                            >{option.label}</option
                                        >
                                    {/each}
                                </select>
                                <div class="tooltip-bubble variant-tooltip">
                                    {csmModelTooltip}
                                </div>
                            </div>
                        </div>
                        <span class="banner-subtitle">Loaded</span>
                        {#if csmNotificationMessage}
                            <div class="banner-notification">
                                {csmNotificationMessage}
                            </div>
                        {/if}
                    </div>
                    <div class="loaded-actions">
                        <button
                            class="utility-btn"
                            disabled={isUnloadingCsm}
                            onclick={handleUnloadCsm}
                        >
                            {isUnloadingCsm ? "Unloading..." : "Unload"}
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
                        <span class="banner-title">TTS</span>
                        <div class="tooltip-shell variant-select-shell">
                            <select
                                class="variant-select"
                                value={selectedCsmModel}
                                aria-label="TTS model"
                                disabled={csmVariantDisabled}
                                onchange={handleCsmModelChange}
                            >
                                {#each csmModelOptions as option}
                                    <option value={option.value}
                                        >{option.label}</option
                                    >
                                {/each}
                            </select>
                            <div class="tooltip-bubble variant-tooltip">
                                {csmModelTooltip}
                            </div>
                        </div>
                        {#if csmQuantizeAvailable}
                            <div class="tooltip-shell">
                                <button
                                    type="button"
                                    class="quantize-toggle"
                                    class:active={isCsmQuantized}
                                    disabled={isLoadingCsm ||
                                        isDownloadingCsm ||
                                        isClearingCsmCache ||
                                        isUpdatingCsmQuantize}
                                    onclick={handleCsmQuantizeToggle}
                                >
                                    <span class="quantize-dot"></span>
                                    <span>Quantize</span>
                                </button>
                                <div class="tooltip-bubble">
                                    This can speed up the audio generation but
                                    lose the quality.
                                </div>
                            </div>
                        {/if}
                    </div>
                    {#if isLoadingCsm}
                        <span class="banner-subtitle">{csmLoadMessage}</span>
                    {:else}
                        <div class="banner-subtitle-row">
                            <span class="banner-subtitle">Downloaded</span>
                            <button
                                type="button"
                                class="utility-btn subtitle-action-btn"
                                disabled={isLoadingCsm ||
                                    isUnloadingCsm ||
                                    isClearingCsmCache}
                                onclick={handleClearCsmCache}
                            >
                                {isClearingCsmCache ? "Clearing..." : "Clear Cache"}
                            </button>
                        </div>
                    {/if}
                </div>
                <button
                    class="download-btn"
                    disabled={isLoadingCsm ||
                        isUnloadingCsm ||
                        isClearingCsmCache}
                    onclick={handleLoadCsm}
                >
                    {isLoadingCsm ? "Loading..." : "Load Model"}
                </button>
            {/if}
        </div>
    {:else}
        <div class="banner-row">
            <div class="banner-copy">
                <div class="banner-heading-row">
                    <span class="banner-title">TTS</span>
                    <div class="tooltip-shell variant-select-shell">
                        <select
                            class="variant-select"
                            value={selectedCsmModel}
                            aria-label="TTS model"
                            disabled={csmVariantDisabled}
                            onchange={handleCsmModelChange}
                        >
                            {#each csmModelOptions as option}
                                <option value={option.value}>{option.label}</option>
                            {/each}
                        </select>
                        <div class="tooltip-bubble variant-tooltip">
                            {csmModelTooltip}
                        </div>
                    </div>
                    {#if csmQuantizeAvailable}
                        <div class="tooltip-shell">
                            <button
                                type="button"
                                class="quantize-toggle"
                                class:active={isCsmQuantized}
                                disabled={isDownloadingCsm ||
                                    isClearingCsmCache ||
                                    isUpdatingCsmQuantize}
                                onclick={handleCsmQuantizeToggle}
                            >
                                <span class="quantize-dot"></span>
                                <span>Quantize</span>
                            </button>
                            <div class="tooltip-bubble">
                                This can speed up the audio generation but lose
                                the quality.
                            </div>
                        </div>
                    {/if}
                </div>
                {#if csmDownloadError}
                    <span class="banner-subtitle error">Download failed</span>
                    <span class="banner-detail error">{csmDownloadError}</span>
                {:else}
                    <span class="banner-subtitle">Model not found in cache</span>
                {/if}
            </div>
            <button
                class="download-btn"
                disabled={isDownloadingCsm}
                onclick={handleDownloadCsm}
            >
                {isDownloadingCsm
                    ? "Downloading..."
                    : csmDownloadError
                      ? "Retry Download"
                      : "Download Model"}
            </button>
        </div>
    {/if}
</div>

<style>
    .banner-notification {
        font-size: 11px;
        color: #ff9500;
        margin-top: 2px;
        line-height: 1.2;
        max-width: 320px;
    }
</style>
