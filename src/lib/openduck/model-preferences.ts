// Helpers for validating, persisting, and restoring frontend model selections.
import {
    DEFAULT_CSM_MODEL,
    DEFAULT_GEMMA_VARIANT,
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
    };
}

export function isGemmaVariant(value: unknown): value is GemmaVariant {
    return value === "e4b" || value === "e2b" || value === "ollama";
}

export function isCsmModelVariant(value: unknown): value is CsmModelVariant {
    return (
        value === "expressiva_1b" ||
        value === "kokoro_82m" ||
        value === "cosyvoice2_0_5b"
    );
}

export function isSttModelVariant(value: unknown): value is SttModelVariant {
    return value === "gemma" || value === "whisper_large_v3_turbo";
}

export function selectionsMatch(
    left: ModelSelection,
    right: ModelSelection,
): boolean {
    return (
        left.gemmaVariant === right.gemmaVariant &&
        left.csmModel === right.csmModel &&
        left.sttModel === right.sttModel &&
        left.ollamaModel === right.ollamaModel
    );
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
        };
    } catch (err) {
        console.error("Failed to restore model preferences:", err);
        return fallback;
    }
}
