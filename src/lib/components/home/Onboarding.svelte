<script lang="ts">
    import { fade, fly } from "svelte/transition";
    import {
        ONBOARDING_SEEN_STORAGE_KEY,
        CALL_MODE_STORAGE_KEY,
        MODEL_PREFERENCES_STORAGE_KEY,
    } from "$lib/openduck/config";
    import { loadModelPreferencesFromStorage } from "$lib/openduck/model-preferences";
    import type {
        CallMode,
        KokoroLanguage,
        SttLanguage,
        StoredModelPreferences,
        GemmaVariant,
    } from "$lib/openduck/types";

    let { onComplete } = $props<{ onComplete: () => void }>();

    let currentPage = $state(0);
    let direction = $state(1);
    let selectedMode = $state<CallMode>("natural");
    let selectedLanguage = $state<KokoroLanguage>("american_english");
    let selectedGemmaVariant = $state<GemmaVariant>("e4b");

    const languages: { label: string; value: KokoroLanguage; flag: string }[] =
        [
            { label: "English", value: "american_english", flag: "🇺🇸" },
            { label: "Japanese", value: "japanese", flag: "🇯🇵" },
            { label: "French", value: "french", flag: "🇫🇷" },
            { label: "Chinese", value: "mandarin_chinese", flag: "🇨🇳" },
            { label: "Spanish", value: "spanish", flag: "🇪🇸" },
            { label: "Hindi", value: "hindi", flag: "🇮🇳" },
            { label: "Italian", value: "italian", flag: "🇮🇹" },
            {
                label: "Brazilian Portuguese",
                value: "brazilian_portuguese",
                flag: "🇧🇷",
            },
        ];

    function nextPage() {
        if (currentPage < 3) {
            direction = 1;
            currentPage++;
        }
    }

    function prevPage() {
        if (currentPage > 0) {
            direction = -1;
            currentPage--;
        }
    }

    function setPage(page: number) {
        if (page < currentPage) {
            direction = -1;
            currentPage = page;
        } else if (page > currentPage) {
            direction = 1;
            currentPage = page;
        }
    }

    function finishOnboarding() {
        // Save mode
        localStorage.setItem(
            CALL_MODE_STORAGE_KEY,
            JSON.stringify({ version: 1, mode: selectedMode }),
        );

        // Load existing preferences to preserve external model settings
        const existingPrefs = loadModelPreferencesFromStorage();

        // Save model preferences (Whisper Turbo + Kokoro language)
        const prefs: StoredModelPreferences = {
            ...existingPrefs,
            version: 1,
            gemmaVariant: selectedGemmaVariant,
            csmModel: "kokoro_82m",
            kokoroLanguage: selectedLanguage,
            sttModel: "whisper_large_v3_turbo",
            sttLanguage: selectedLanguage as SttLanguage,
        };
        localStorage.setItem(
            MODEL_PREFERENCES_STORAGE_KEY,
            JSON.stringify(prefs),
        );

        // Mark as seen
        localStorage.setItem(ONBOARDING_SEEN_STORAGE_KEY, "true");
        onComplete();
    }

    function selectLanguage(lang: KokoroLanguage) {
        selectedLanguage = lang;
        nextPage();
    }

    function selectGemmaVariant(variant: GemmaVariant) {
        selectedGemmaVariant = variant;
        finishOnboarding();
    }
</script>

<div class="onboarding-overlay" transition:fade={{ duration: 300 }}>
    <div class="onboarding-card">
        <div class="page-content">
            {#if currentPage === 0}
                <div
                    class="page welcome-page"
                    in:fly={{ x: direction * 20, duration: 300 }}
                    out:fly={{ x: -direction * 20, duration: 300 }}
                >
                    <div class="duck-icon">
                        <img src="/icon.png" alt="OpenDuck Icon" />
                    </div>
                    <p class="description">
                        OpenDuck is an app that allows you to speak with
                        different AI characters in 8 different languages.
                    </p>
                    <p class="description highlight">
                        Now we will go through some initial settings to optimize
                        the best experience for you.
                    </p>
                    <button class="primary-btn" onclick={nextPage}
                        >Get Started</button
                    >
                </div>
            {:else if currentPage === 1}
                <div
                    class="page mode-page"
                    in:fly={{ x: direction * 20, duration: 300 }}
                    out:fly={{ x: -direction * 20, duration: 300 }}
                >
                    <h2>Choose your interaction mode</h2>
                    <div class="modes-container">
                        <button
                            class="mode-card"
                            class:selected={selectedMode === "push_to_talk"}
                            onclick={() => {
                                selectedMode = "push_to_talk";
                                nextPage();
                            }}
                        >
                            <h3>Push-to-Talk</h3>
                            <p class="recommend">Recommend for beginner</p>
                            <div class="explanation">
                                <p>
                                    You control exactly when the AI listens to
                                    you by holding a button.
                                </p>
                            </div>
                        </button>
                        <button
                            class="mode-card"
                            class:selected={selectedMode === "natural"}
                            onclick={() => {
                                selectedMode = "natural";
                                nextPage();
                            }}
                        >
                            <h3>Natural</h3>
                            <p class="recommend">
                                Recommend for advanced learner
                            </p>
                            <div class="explanation">
                                <p>
                                    The AI listens and responds naturally just
                                    like a real conversation.
                                </p>
                            </div>
                        </button>
                    </div>
                </div>
            {:else if currentPage === 2}
                <div
                    class="page language-page"
                    in:fly={{ x: direction * 20, duration: 300 }}
                    out:fly={{ x: -direction * 20, duration: 300 }}
                >
                    <h2>What language do you wish to speak?</h2>
                    <div class="languages-list">
                        {#each languages as lang}
                            <button
                                class="language-item"
                                onclick={() => selectLanguage(lang.value)}
                            >
                                <span class="flag">{lang.flag}</span>
                                <span class="label">{lang.label}</span>
                            </button>
                        {/each}
                    </div>
                </div>
            {:else if currentPage === 3}
                <div
                    class="page model-page"
                    in:fly={{ x: direction * 20, duration: 300 }}
                    out:fly={{ x: -direction * 20, duration: 300 }}
                >
                    <h2>Choose AI model size</h2>
                    <div class="modes-container">
                        <button
                            class="mode-card"
                            class:selected={selectedGemmaVariant === "e2b"}
                            onclick={() => selectGemmaVariant("e2b")}
                        >
                            <h3>Gemma-4-E2B</h3>
                            <p class="recommend">Faster & Lighter</p>
                            <div class="explanation">
                                <p>
                                    Uses less RAM and responds quicker.
                                    Recommended for basic performance.
                                </p>
                            </div>
                        </button>
                        <button
                            class="mode-card"
                            class:selected={selectedGemmaVariant === "e4b"}
                            onclick={() => selectGemmaVariant("e4b")}
                        >
                            <h3>Gemma-4-E4B</h3>
                            <p class="recommend">Smarter & More Capable</p>
                            <div class="explanation">
                                <p>
                                    Better reasoning but takes more RAM and time
                                    to respond. Recommended for 16GB+ RAM.
                                </p>
                            </div>
                        </button>
                    </div>
                </div>
            {/if}
        </div>

        <div class="pagination">
            {#each [0, 1, 2, 3] as i}
                <button
                    class="dot"
                    class:active={currentPage === i}
                    onclick={() => setPage(i)}
                    disabled={i > currentPage}
                ></button>
            {/each}
        </div>
    </div>
</div>

<style>
    .onboarding-overlay {
        position: fixed;
        inset: 0;
        background: rgba(0, 0, 0, 0.85);
        backdrop-filter: blur(20px);
        display: flex;
        align-items: center;
        justify-content: center;
        z-index: 1000;
        padding: 20px;
    }

    .onboarding-card {
        background: linear-gradient(
            180deg,
            rgba(38, 34, 20, 0.96) 0%,
            rgba(17, 17, 19, 0.97) 100%
        );
        border: 1px solid rgba(255, 220, 102, 0.2);
        border-radius: 32px;
        width: min(700px, 100%);
        min-height: 500px;
        padding: 40px;
        display: flex;
        flex-direction: column;
        position: relative;
        box-shadow: 0 40px 100px rgba(0, 0, 0, 0.6);
        overflow: hidden;
    }

    .page-content {
        flex: 1;
        display: grid;
        grid-template-columns: 1fr;
        grid-template-rows: 1fr;
        min-height: 0;
    }

    .page {
        grid-row: 1;
        grid-column: 1;
        display: flex;
        flex-direction: column;
        align-items: center;
        text-align: center;
    }

    h2 {
        font-size: 1.6rem;
        color: #ffdf63;
        margin-bottom: 32px;
        font-weight: 700;
    }

    .duck-icon {
        width: 120px;
        height: 120px;
        margin-bottom: 32px;
        background: white;
        padding: 10px;
        border-radius: 30px;
        box-shadow: 0 10px 30px rgba(255, 223, 99, 0.2);
    }

    .duck-icon img {
        width: 100%;
        height: 100%;
        object-fit: contain;
    }

    .description {
        color: rgba(255, 255, 255, 0.85);
        font-size: 1.1rem;
        line-height: 1.6;
        margin-bottom: 20px;
        max-width: 440px;
    }

    .highlight {
        color: #7fe37c;
        font-weight: 600;
    }

    .primary-btn {
        margin-top: auto;
        background: #ffcd40;
        color: #2f2500;
        border: none;
        padding: 16px 40px;
        border-radius: 20px;
        font-size: 1.1rem;
        font-weight: 750;
        cursor: pointer;
        transition: transform 0.2s;
    }

    .primary-btn:hover {
        transform: scale(1.05);
    }

    .modes-container {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 20px;
        width: 100%;
    }

    .mode-card {
        background: rgba(255, 255, 255, 0.05);
        border: 2px solid rgba(255, 255, 255, 0.1);
        border-radius: 24px;
        padding: 24px;
        cursor: pointer;
        transition: all 0.2s;
        text-align: left;
        color: white;
        display: flex;
        flex-direction: column;
    }

    .mode-card:hover {
        background: rgba(255, 255, 255, 0.08);
        border-color: rgba(255, 223, 99, 0.4);
        transform: translateY(-4px);
    }

    .mode-card.selected {
        background: rgba(255, 223, 99, 0.1);
        border-color: #ffcd40;
    }

    .mode-card h3 {
        margin: 0 0 8px 0;
        font-size: 1.3rem;
    }

    .recommend {
        font-size: 0.75rem;
        color: #7fe37c;
        font-weight: 700;
        margin-bottom: 16px;
        text-transform: uppercase;
        letter-spacing: 0.05em;
    }

    .explanation {
        font-size: 0.95rem;
        color: rgba(255, 255, 255, 0.6);
        line-height: 1.4;
    }

    .languages-list {
        display: grid;
        grid-template-columns: 1fr 1fr;
        gap: 12px;
        width: 100%;
        overflow-y: auto;
        max-height: 380px;
        padding-right: 8px;
    }

    .language-item {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 16px;
        padding: 12px 20px;
        display: flex;
        align-items: center;
        gap: 12px;
        cursor: pointer;
        color: white;
        transition: all 0.2s;
        text-align: left;
    }

    .language-item:hover {
        background: rgba(255, 255, 255, 0.1);
        border-color: #ffcd40;
        transform: translateX(4px);
    }

    .flag {
        font-size: 1.5rem;
    }

    .label {
        font-weight: 600;
        font-size: 1rem;
    }

    .pagination {
        display: flex;
        justify-content: center;
        gap: 12px;
        margin-top: 40px;
    }

    .dot {
        width: 12px;
        height: 12px;
        border-radius: 50%;
        background: rgba(255, 255, 255, 0.2);
        border: none;
        cursor: pointer;
        padding: 0;
        transition: all 0.2s;
    }

    .dot.active {
        background: #ffcd40;
        transform: scale(1.2);
    }

    .dot:disabled {
        cursor: default;
    }

    /* Scrollbar for languages list */
    .languages-list::-webkit-scrollbar {
        width: 6px;
    }
    .languages-list::-webkit-scrollbar-track {
        background: rgba(255, 255, 255, 0.02);
        border-radius: 3px;
    }
    .languages-list::-webkit-scrollbar-thumb {
        background: rgba(255, 255, 255, 0.1);
        border-radius: 3px;
    }
</style>
