// Shared static configuration for contacts, model defaults, and selectable presets on the home page.
import type {
  CsmModelVariant,
  GemmaVariant,
  ModelPreset,
  ModelSelection,
  SttModelVariant,
} from "./types";

export const DEFAULT_VOICE_SYSTEM_PROMPT =
  "You are a friendly voice AI assistant from OpenDuck. A local-first voice-call desktop application built for rubberducking. You are in a live voice call right now. Reply like a natural spoken conversation. Use plain sentences only. Never use markdown, bullets, headings, numbered lists, code fences, tables, emojis, or stage directions. Keep responses concise, direct, and easy to speak aloud. Try to ask follow-up questions more often.";
export const CONTACTS_STORAGE_KEY = "openduck.contacts.v1";
export const CONTACT_ICONS_DB_NAME = "openduck.contacts";
export const CONTACT_ICONS_STORE_NAME = "contact-icons";
export const DEFAULT_CONTACT_ID = "contact-openduck";
export const MODEL_PREFERENCES_STORAGE_KEY = "openduck.model-preferences.v1";
export const PONG_PLAYBACK_STORAGE_KEY = "openduck.pong-playback.v1";
export const GLOBAL_SHORTCUT_STORAGE_KEY = "openduck.global-shortcut.v1";
export const GLOBAL_SHORTCUT_ENTIRE_SCREEN_STORAGE_KEY = "openduck.global-shortcut-entire.v1";
export const DEFAULT_GEMMA_VARIANT: GemmaVariant = "e4b";
export const DEFAULT_CSM_MODEL: CsmModelVariant = "kokoro_82m";
export const DEFAULT_STT_MODEL: SttModelVariant = "whisper_large_v3_turbo";
export const DEFAULT_OLLAMA_MODEL = "gemma2:2b";
export const DEFAULT_GLOBAL_SHORTCUT = "Command+Shift+L";
export const DEFAULT_GLOBAL_SHORTCUT_ENTIRE_SCREEN = "Command+Shift+Option+L";

export const MODEL_PRESETS: Record<
  Exclude<ModelPreset, "custom">,
  {
    label: string;
    description: string;
    selection: ModelSelection;
  }
> = {
  lite: {
    label: "Lite (~4GB)",
    description: "Gemma 2B + STT Gemma + Kokoro",
    selection: {
      gemmaVariant: "e2b",
      csmModel: "kokoro_82m",
      sttModel: "gemma",
      ollamaModel: DEFAULT_OLLAMA_MODEL,
    },
  },
  normal: {
    label: "Normal (~10GB)",
    description: "Gemma 4B + Distil-Whisper + Kokoro",
    selection: {
      gemmaVariant: "e4b",
      csmModel: "kokoro_82m",
      sttModel: "distil_whisper_large_v3",
      ollamaModel: DEFAULT_OLLAMA_MODEL,
    },
  },
  realistic: {
    label: "Realistic (~13GB)",
    description: "Gemma 4B + Whisper + CosyVoice3 fp16",
    selection: {
      gemmaVariant: "e4b",
      csmModel: "cosyvoice3_0_5b_fp16",
      sttModel: "whisper_large_v3_turbo",
      ollamaModel: DEFAULT_OLLAMA_MODEL,
    },
  },
};
