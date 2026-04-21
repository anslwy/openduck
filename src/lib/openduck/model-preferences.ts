// Helpers for validating, persisting, and restoring frontend model selections.
import {
    DEFAULT_CSM_MODEL,
    DEFAULT_GEMMA_VARIANT,
    DEFAULT_LMSTUDIO_MODEL,
    DEFAULT_OPENAI_COMPATIBLE_MODEL,
    DEFAULT_STT_MODEL,
    DEFAULT_OLLAMA_MODEL,
    MODEL_PREFERENCES_STORAGE_KEY,
    MODEL_PRESETS,
} from "./config";
import type {
    CsmModelVariant,
    GemmaVariant,
    ModelPreset,
    ModelSelection,
    StoredModelPreferences,
    SttModelVariant,
} from "./types";

export function createDefaultModelPreferences(): StoredModelPreferences {
    return {
        version: 1,
        gemmaVariant: DEFAULT_GEMMA_VARIANT,
        csmModel: DEFAULT_CSM_MODEL,
        sttModel: DEFAULT_STT_MODEL,
        ollamaModel: DEFAULT_OLLAMA_MODEL,
        lmstudioModel: DEFAULT_LMSTUDIO_MODEL,
        openaiCompatibleModel: DEFAULT_OPENAI_COMPATIBLE_MODEL,
    };
}

export function isGemmaVariant(value: unknown): value is GemmaVariant {
    return (
        value === "e4b" ||
        value === "e2b" ||
        value === "ollama" ||
        value === "lmstudio" ||
        value === "openai_compatible"
    );
}

export function isCsmModelVariant(value: unknown): value is CsmModelVariant {
    return (
        value === "expressiva_1b" ||
        value === "kokoro_82m" ||
        value === "cosyvoice2_0_5b" ||
        value === "cosyvoice3_0_5b_8bit" ||
        value === "cosyvoice3_0_5b_4bit" ||
        value === "cosyvoice3_0_5b_fp16" ||
        value === "chatterbox_turbo_8bit"
    );
}

export function isSttModelVariant(value: unknown): value is SttModelVariant {
    return (
        value === "gemma" ||
        value === "distil_whisper_large_v3" ||
        value === "whisper_large_v3_turbo"
    );
}

export function selectionsMatch(
    left: ModelSelection,
    right: ModelSelection,
): boolean {
    const baseMatch =
        left.gemmaVariant === right.gemmaVariant &&
        left.csmModel === right.csmModel &&
        left.sttModel === right.sttModel;

    if (!baseMatch) {
        return false;
    }

    if (left.gemmaVariant === "ollama") {
        return left.ollamaModel === right.ollamaModel;
    }

    if (left.gemmaVariant === "lmstudio") {
        return left.lmstudioModel === right.lmstudioModel;
    }

    if (left.gemmaVariant === "openai_compatible") {
        return left.openaiCompatibleModel === right.openaiCompatibleModel;
    }

    return true;
}

export function resolveModelPreset(selection: ModelSelection): ModelPreset {
    if (selectionsMatch(selection, MODEL_PRESETS.lite.selection)) {
        return "lite";
    }

    if (selectionsMatch(selection, MODEL_PRESETS.normal.selection)) {
        return "normal";
    }

    if (selectionsMatch(selection, MODEL_PRESETS.realistic.selection)) {
        return "realistic";
    }

    return "custom";
}

export function getModelPresetDescription(preset: ModelPreset) {
    if (preset === "custom") {
        return "Custom uses the current manual model combination.";
    }

    return MODEL_PRESETS[preset].description;
}

export function loadModelPreferencesFromStorage(): StoredModelPreferences {
    const fallback = createDefaultModelPreferences();

    if (typeof window === "undefined") {
        return fallback;
    }

    const rawPayload = window.localStorage.getItem(
        MODEL_PREFERENCES_STORAGE_KEY,
    );
    if (!rawPayload) {
        return fallback;
    }

    try {
        const parsed = JSON.parse(rawPayload) as {
            version?: unknown;
            gemmaVariant?: unknown;
            csmModel?: unknown;
            sttModel?: unknown;
            ollamaModel?: unknown;
            lmstudioModel?: unknown;
            openaiCompatibleModel?: unknown;
        };

        if (parsed.version !== 1) {
            return fallback;
        }

        return {
            version: 1,
            gemmaVariant: isGemmaVariant(parsed.gemmaVariant)
                ? parsed.gemmaVariant
                : DEFAULT_GEMMA_VARIANT,
            csmModel: isCsmModelVariant(parsed.csmModel)
                ? parsed.csmModel
                : DEFAULT_CSM_MODEL,
            sttModel: isSttModelVariant(parsed.sttModel)
                ? parsed.sttModel
                : DEFAULT_STT_MODEL,
            ollamaModel:
                typeof parsed.ollamaModel === "string"
                    ? parsed.ollamaModel
                    : DEFAULT_OLLAMA_MODEL,
            lmstudioModel:
                typeof parsed.lmstudioModel === "string"
                    ? parsed.lmstudioModel
                    : DEFAULT_LMSTUDIO_MODEL,
            openaiCompatibleModel:
                typeof parsed.openaiCompatibleModel === "string"
                    ? parsed.openaiCompatibleModel
                    : DEFAULT_OPENAI_COMPATIBLE_MODEL,
        };
    } catch (err) {
        console.error("Failed to restore model preferences:", err);
        return fallback;
    }
}
