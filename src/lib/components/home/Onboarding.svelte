<script lang="ts">
    import { fade, fly } from "svelte/transition";
    import {
        ONBOARDING_SEEN_STORAGE_KEY,
        CALL_MODE_STORAGE_KEY,
        MODEL_PREFERENCES_STORAGE_KEY,
        SHOW_AI_SUBTITLE_STORAGE_KEY,
        AI_SUBTITLE_TARGET_LANGUAGE_STORAGE_KEY,
    } from "$lib/openduck/config";
    import { loadModelPreferencesFromStorage } from "$lib/openduck/model-preferences";
    import type {
        CallMode,
        KokoroLanguage,
        SttLanguage,
        StoredModelPreferences,
        GemmaVariant,
        AiSubtitleTargetLanguage,
        AiSubtitleTranslationProvider,
        AppleTranslationLanguagePackStatus,
    } from "$lib/openduck/types";

    let {
        onComplete,
        aiSubtitleTargetLanguage,
        subtitleTranslationProvider,
        subtitleTranslationLlmConfigured,
        subtitleTranslationLlmTested,
        appleTranslationLanguagePackStatus,
        appleTranslationLanguagePackMessage,
        isCheckingAppleTranslationLanguagePack,
        isInstallingAppleTranslationLanguagePack,
        onOpenSubtitleTranslationLlmConfig,
        onUpdateSubtitleTranslationProvider,
        onCheckAppleTranslationLanguagePack,
        onInstallAppleTranslationLanguagePack,
        onUpdateAiSubtitleTargetLanguage,
        onUpdateShowAiSubtitle,
    } = $props<{
        onComplete: () => void;
        aiSubtitleTargetLanguage: AiSubtitleTargetLanguage;
        subtitleTranslationProvider: AiSubtitleTranslationProvider;
        subtitleTranslationLlmConfigured: boolean;
        subtitleTranslationLlmTested: boolean;
        appleTranslationLanguagePackStatus: AppleTranslationLanguagePackStatus | null;
        appleTranslationLanguagePackMessage: string;
        isCheckingAppleTranslationLanguagePack: boolean;
        isInstallingAppleTranslationLanguagePack: boolean;
        onOpenSubtitleTranslationLlmConfig: () => void;
        onUpdateSubtitleTranslationProvider: (
            provider: AiSubtitleTranslationProvider,
        ) => Promise<void>;
        onCheckAppleTranslationLanguagePack: (
            targetLang: AiSubtitleTargetLanguage,
            sourceLanguage: KokoroLanguage,
        ) => Promise<AppleTranslationLanguagePackStatus | null>;
        onInstallAppleTranslationLanguagePack: (
            targetLang: AiSubtitleTargetLanguage,
            sourceLanguage: KokoroLanguage,
        ) => Promise<AppleTranslationLanguagePackStatus | null>;
        onUpdateAiSubtitleTargetLanguage: (
            lang: AiSubtitleTargetLanguage,
        ) => void;
        onUpdateShowAiSubtitle: (enabled: boolean) => void;
    }>();

    let currentPage = $state(0);
    let direction = $state(1);
    let selectedMode = $state<CallMode>("natural");
    let selectedLanguage = $state<KokoroLanguage>("american_english");
    let selectedGemmaVariant = $state<GemmaVariant>("e4b");
    let selectedTranslationLanguage = $state<AiSubtitleTargetLanguage>("none");

    $effect(() => {
        selectedTranslationLanguage = aiSubtitleTargetLanguage;
    });

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

    const translationLanguages: { label: string; value: AiSubtitleTargetLanguage }[] = [
        { value: "none", label: "Do Not Translate" },
        { value: "ar", label: "Arabic" },
        { value: "bn", label: "Bengali" },
        { value: "zh", label: "Chinese (Simplified)" },
        { value: "tw", label: "Chinese (Traditional)" },
        { value: "en", label: "English" },
        { value: "fr", label: "French" },
        { value: "de", label: "German" },
        { value: "gu", label: "Gujarati" },
        { value: "hi", label: "Hindi" },
        { value: "id", label: "Indonesian" },
        { value: "it", label: "Italian" },
        { value: "jp", label: "Japanese" },
        { value: "ko", label: "Korean" },
        { value: "mr", label: "Marathi" },
        { value: "fa", label: "Persian" },
        { value: "pt", label: "Portuguese" },
        { value: "pa", label: "Punjabi" },
        { value: "ru", label: "Russian" },
        { value: "es", label: "Spanish" },
        { value: "ta", label: "Tamil" },
        { value: "te", label: "Telugu" },
        { value: "th", label: "Thai" },
        { value: "tr", label: "Turkish" },
        { value: "ur", label: "Urdu" },
        { value: "vi", label: "Vietnamese" },
    ];

    const demoTranslations: Record<string, string> = {
        none: "Or is it simply an identification number?",
        ar: "أم أنه مجرد رقم تعريف؟",
        bn: "নাকি এটি কেবল একটি সনাক্তকরণ নম্বর?",
        zh: "或者它仅仅是一个识别号码？",
        tw: "或者它僅僅是一個識別號碼？",
        en: "Or is it simply an identification number?",
        fr: "Ou s'agit-il simplement d'un numéro d'identification ?",
        de: "Oder ist es einfach eine Identifikationsnummer?",
        hi: "या यह केवल एक पहचान संख्या है?",
        it: "O è semplicemente un numero di identificazione?",
        jp: "それとも単に識別番号ですか？",
        ko: "아니면 단순히 식별 번호입니까?",
        pt: "Ou é simplesmente um número de identificação?",
        ru: "Или это просто идентификационный номер?",
        es: "¿O es simplemente un número de identificación?",
        th: "หรือมันเป็นเพียงแค่หมายเลขระบุตัวตน?",
        tr: "Yoksa sadece bir kimlik numarası mı?",
        vi: "Hay nó chỉ đơn giản là một số nhận dạng?",
    };

    function getDemoText(lang: string) {
        const mapping: Record<string, string> = {
            american_english: "en",
            british_english: "en",
            japanese: "jp",
            mandarin_chinese: "zh",
            french: "fr",
            spanish: "es",
            hindi: "hi",
            italian: "it",
            brazilian_portuguese: "pt",
        };
        const key = mapping[lang] || lang;
        return demoTranslations[key] || demoTranslations["en"];
    }

    function appleTranslationStatusText() {
        if (selectedTranslationLanguage === "none") {
            return "Choose a translation language first.";
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

    async function updateTranslationProvider(event: Event) {
        const provider = (event.currentTarget as HTMLSelectElement)
            .value as AiSubtitleTranslationProvider;
        await onUpdateSubtitleTranslationProvider(provider);
    }

    let lastAppleTranslationStatusKey = "";

    $effect(() => {
        const statusKey = `${subtitleTranslationProvider}:${selectedTranslationLanguage}:${selectedLanguage}`;

        if (
            subtitleTranslationProvider !== "apple" ||
            selectedTranslationLanguage === "none"
        ) {
            if (lastAppleTranslationStatusKey !== statusKey) {
                lastAppleTranslationStatusKey = statusKey;
            }
            return;
        }

        if (lastAppleTranslationStatusKey === statusKey) {
            return;
        }

        lastAppleTranslationStatusKey = statusKey;
        void onCheckAppleTranslationLanguagePack(
            selectedTranslationLanguage,
            selectedLanguage,
        );
    });

    function nextPage() {
        if (currentPage < 4) {
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

    function skipTranslation() {
        selectedTranslationLanguage = "none";
        nextPage();
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

        // Save AI Subtitle settings
        onUpdateShowAiSubtitle(selectedTranslationLanguage !== "none");
        onUpdateAiSubtitleTargetLanguage(selectedTranslationLanguage);

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
                    class="page translation-page"
                    in:fly={{ x: direction * 20, duration: 300 }}
                    out:fly={{ x: -direction * 20, duration: 300 }}
                >
                    <h2>Learn with AI Subtitle Translation</h2>
                    <div class="translation-demo">
                        <div class="subtitle-mockup">
                            <div class="subtitle-content">
                                {#if selectedTranslationLanguage !== "none"}
                                    <span class="subtitle-translation">
                                        {getDemoText(selectedTranslationLanguage)}
                                    </span>
                                    <span class="subtitle-original">
                                        {getDemoText(selectedLanguage)}
                                    </span>
                                {:else}
                                    <span class="subtitle-translation">
                                        {getDemoText(selectedLanguage)}
                                    </span>
                                {/if}
                            </div>
                        </div>
                    </div>
                    <p class="description">
                        See real-time translations alongside the original text to help you learn faster.
                    </p>
                    
                    <div class="translation-controls">
                        <div class="control-group">
                            <label for="onboarding-translation-provider">Translation Type</label>
                            <select
                                id="onboarding-translation-provider"
                                class="onboarding-select"
                                value={subtitleTranslationProvider}
                                onchange={updateTranslationProvider}
                            >
                                <option value="apple">Apple</option>
                                <option value="openai_compatible">OpenAI-compatible</option>
                            </select>
                        </div>

                        <div class="control-group">
                            <label for="onboarding-translation-lang">AI Subtitle Translation</label>
                            <select 
                                id="onboarding-translation-lang"
                                class="onboarding-select"
                                value={selectedTranslationLanguage}
                                onchange={(e) => {
                                    selectedTranslationLanguage = (e.currentTarget as HTMLSelectElement).value as AiSubtitleTargetLanguage;
                                }}
                            >
                                {#each translationLanguages as lang}
                                    <option value={lang.value}>{lang.label}</option>
                                {/each}
                            </select>
                        </div>

                        {#if subtitleTranslationProvider === "apple"}
                            <div class="apple-pack-panel">
                                <div
                                    class="config-status apple-status"
                                    class:configured={appleTranslationLanguagePackStatus?.status === "installed"}
                                    class:error={appleTranslationLanguagePackStatus?.status === "unsupported" || !!appleTranslationLanguagePackMessage}
                                >
                                    <span class="status-dot"></span>
                                    {appleTranslationStatusText()}
                                </div>
                                {#if selectedTranslationLanguage !== "none" && appleTranslationLanguagePackStatus?.status !== "installed" && appleTranslationLanguagePackStatus?.status !== "unsupported"}
                                    <button
                                        type="button"
                                        class="config-llm-btn"
                                        onclick={() =>
                                            onInstallAppleTranslationLanguagePack(
                                                selectedTranslationLanguage,
                                                selectedLanguage,
                                            )}
                                        disabled={isInstallingAppleTranslationLanguagePack}
                                    >
                                        {isInstallingAppleTranslationLanguagePack ? 'Installing...' : 'Download & Install'}
                                    </button>
                                {/if}
                            </div>
                        {:else}
                            <div class="config-row">
                                <button
                                    class="config-llm-btn"
                                    class:configured={subtitleTranslationLlmConfigured}
                                    onclick={onOpenSubtitleTranslationLlmConfig}
                                >
                                    <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"></path>
                                        <circle cx="12" cy="12" r="3"></circle>
                                    </svg>
                                    {subtitleTranslationLlmConfigured ? 'Edit Translation LLM' : 'Configure Translation LLM'}
                                </button>
                                <div class="config-status"
                                    class:configured={subtitleTranslationLlmConfigured && subtitleTranslationLlmTested}
                                    class:error={subtitleTranslationLlmConfigured && !subtitleTranslationLlmTested}
                                >
                                    <span class="status-dot"></span>
                                    {subtitleTranslationLlmConfigured && subtitleTranslationLlmTested ? 'Ready' : !subtitleTranslationLlmConfigured ? 'Not set' : 'Invalid key'}
                                </div>
                            </div>
                        {/if}
                    </div>

                    <div class="onboarding-actions">
                        <button class="secondary-btn" onclick={skipTranslation}>Skip for now</button>
                        <button class="primary-btn" onclick={nextPage}>Continue</button>
                    </div>
                </div>
            {:else if currentPage === 4}
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
            {#each [0, 1, 2, 3, 4] as i}
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
        max-height: 95vh;
        padding: 40px;
        display: flex;
        flex-direction: column;
        position: relative;
        box-shadow: 0 40px 100px rgba(0, 0, 0, 0.6);
        overflow-y: auto;
    }

    @media (max-height: 800px) or (max-width: 600px) {
        .onboarding-overlay {
            align-items: stretch;
        }

        .onboarding-card {
            padding: 24px;
            min-height: auto;
            max-height: calc(100vh - 40px);
            border-radius: 24px;
        }

        h2 {
            font-size: 1.3rem;
            margin-bottom: 20px;
        }

        .description {
            font-size: 0.95rem;
            line-height: 1.4;
            margin-bottom: 16px;
        }

        .translation-demo {
            max-width: 440px;
            min-height: 150px;
            padding: 14px;
            margin-bottom: 16px;
        }

        .translation-controls {
            gap: 10px;
            margin-bottom: 16px;
        }

        .primary-btn, .secondary-btn {
            padding: 12px 24px;
            font-size: 0.95rem;
            border-radius: 16px;
        }

        .onboarding-actions {
            gap: 12px;
            margin-top: 12px;
        }
        
        .pagination {
            margin-top: 24px;
        }

        .duck-icon {
            width: 80px;
            height: 80px;
            margin-bottom: 20px;
        }
    }


    @media (max-height: 700px) or (max-width: 600px) {
        .onboarding-card {
            padding: 24px;
            min-height: auto;
        }

        h2 {
            font-size: 1.4rem;
            margin-bottom: 20px;
        }

        .description {
            font-size: 1rem;
            margin-bottom: 16px;
        }

        .translation-demo {
            min-height: 120px;
            padding: 12px;
            margin-bottom: 16px;
        }

        .translation-controls {
            gap: 8px;
            margin-bottom: 12px;
        }

        .translation-page .description {
            margin-bottom: 12px;
        }

        .apple-pack-panel {
            padding: 10px;
            gap: 8px;
        }

        .apple-pack-panel .config-llm-btn {
            min-height: 42px;
            padding: 10px 14px;
        }
    }

    .page-content {
        flex: 1 0 auto;
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
        background: #ffcd40;
        color: #2f2500;
        border: none;
        padding: 14px 32px;
        border-radius: 20px;
        font-size: 1rem;
        font-weight: 750;
        cursor: pointer;
        transition: transform 0.2s;
    }

    .secondary-btn {
        background: rgba(255, 255, 255, 0.05);
        color: rgba(255, 255, 255, 0.8);
        border: 1px solid rgba(255, 255, 255, 0.15);
        padding: 14px 32px;
        border-radius: 20px;
        font-size: 1rem;
        font-weight: 600;
        cursor: pointer;
        transition: all 0.2s;
    }

    .secondary-btn:hover {
        background: rgba(255, 255, 255, 0.1);
        color: white;
        border-color: rgba(255, 255, 255, 0.3);
    }

    .secondary-btn:hover {
        background: rgba(255, 255, 255, 0.05);
        color: white;
        border-color: rgba(255, 255, 255, 0.4);
    }

    .onboarding-actions {
        margin-top: auto;
        display: flex;
        gap: 16px;
        width: 100%;
        justify-content: center;
        flex-shrink: 0;
    }

    .translation-demo {
        width: 100%;
        max-width: 520px;
        margin-bottom: 20px;
        border-radius: 24px;
        overflow: hidden;
        border: 1px solid rgba(255, 223, 99, 0.2);
        box-shadow: 0 15px 30px rgba(0, 0, 0, 0.4);
        background: linear-gradient(135deg, #111, #222);
        display: flex;
        align-items: center;
        justify-content: center;
        min-height: 200px;
        padding: 20px;
    }

    .subtitle-mockup {
        padding: 16px 24px;
        border-radius: 20px;
        background: rgba(14, 15, 11, 0.85);
        border: 1px solid rgba(255, 223, 99, 0.2);
        backdrop-filter: blur(10px);
        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
        max-width: 90%;
    }

    .subtitle-content {
        display: flex;
        flex-direction: column;
        gap: 8px;
        text-align: center;
    }

    .subtitle-translation {
        color: #fff;
        font-size: 1.2rem;
        font-weight: 750;
        line-height: 1.2;
        text-shadow: 0 2px 8px rgba(0, 0, 0, 0.5);
    }

    .subtitle-original {
        color: rgba(255, 255, 255, 0.6);
        font-size: 0.95rem;
        font-weight: 600;
        line-height: 1.3;
    }

    .translation-controls {
        width: 100%;
        max-width: 440px;
        display: flex;
        flex-direction: column;
        gap: 12px;
        margin-bottom: 24px;
    }

    .control-group {
        display: flex;
        flex-direction: column;
        gap: 8px;
        text-align: left;
    }

    .control-group label {
        font-size: 0.9rem;
        font-weight: 600;
        color: #ffdf63;
        margin-left: 4px;
    }

    .onboarding-select {
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 16px;
        padding: 12px 16px;
        color: white;
        font-size: 1rem;
        outline: none;
        cursor: pointer;
        appearance: none;
        background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='white' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
        background-repeat: no-repeat;
        background-position: right 16px center;
        padding-right: 44px;
    }

    .onboarding-select:hover {
        background-color: rgba(255, 255, 255, 0.08);
        border-color: rgba(255, 223, 99, 0.4);
    }

    .config-row {
        display: flex;
        align-items: center;
        gap: 16px;
        width: 100%;
    }

    .config-llm-btn {
        flex: 1;
        background: rgba(255, 255, 255, 0.05);
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 16px;
        padding: 12px 16px;
        color: white;
        font-size: 0.95rem;
        font-weight: 600;
        cursor: pointer;
        display: flex;
        align-items: center;
        justify-content: center;
        gap: 10px;
        transition: all 0.2s;
    }

    .config-llm-btn:disabled {
        opacity: 0.55;
        cursor: wait;
    }

    .apple-pack-panel {
        display: flex;
        flex-direction: column;
        gap: 10px;
        text-align: left;
        padding: 12px;
        border: 1px solid rgba(255, 255, 255, 0.1);
        border-radius: 16px;
        background: rgba(255, 255, 255, 0.04);
        min-width: 0;
    }

    .config-status {
        display: flex;
        align-items: center;
        gap: 8px;
        font-size: 0.85rem;
        font-weight: 700;
        color: rgba(255, 255, 255, 0.4);
        white-space: nowrap;
        min-width: 100px;
    }

    .config-status.apple-status {
        align-items: flex-start;
        min-width: 0;
        max-width: 100%;
        white-space: normal;
        line-height: 1.35;
        overflow-wrap: anywhere;
    }

    .config-status.apple-status .status-dot {
        margin-top: 0.35em;
    }

    .status-dot {
        width: 8px;
        height: 8px;
        border-radius: 50%;
        background: rgba(255, 255, 255, 0.2);
        flex-shrink: 0;
    }

    .config-status.configured {
        color: #7fe37c;
    }

    .config-status.error {
        color: #ff9a8b;
    }

    .config-status.error .status-dot {
        background: #ff9a8b;
        box-shadow: 0 0 8px rgba(255, 154, 139, 0.5);
    }

    .config-status.configured .status-dot {
        background: #7fe37c;
        box-shadow: 0 0 8px rgba(127, 227, 124, 0.5);
    }

    .config-llm-btn:hover {
        background: rgba(255, 255, 255, 0.08);
        border-color: #ffcd40;
    }

    .config-llm-btn.configured {
        background: rgba(127, 227, 124, 0.1);
        border-color: rgba(127, 227, 124, 0.4);
        color: #7fe37c;
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
        flex-shrink: 0;
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
