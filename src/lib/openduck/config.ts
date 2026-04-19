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
export const SELECT_LAST_SESSION_STORAGE_KEY = "openduck.select-last-session.v1";
export const SHOW_STAT_STORAGE_KEY = "openduck.show-stat.v1";
export const SHOW_SUBTITLE_STORAGE_KEY = "openduck.show-subtitle.v1";
export const AUTO_UNMUTE_ON_PASTED_SCREENSHOT_STORAGE_KEY =
  "openduck.auto-unmute-on-pasted-screenshot.v1";
export const END_OF_UTTERANCE_SILENCE_STORAGE_KEY =
  "openduck.end-of-utterance-silence-ms.v1";
export const AUTO_CONTINUE_SILENCE_STORAGE_KEY =
  "openduck.auto-continue-silence-ms.v1";
export const AUTO_CONTINUE_MAX_COUNT_STORAGE_KEY =
  "openduck.auto-continue-max-count.v1";
export const LLM_CONTEXT_TURN_LIMIT_STORAGE_KEY =
  "openduck.llm-context-turn-limit.v1";
export const LLM_IMAGE_HISTORY_LIMIT_STORAGE_KEY =
  "openduck.llm-image-history-limit.v1";
export const GLOBAL_SHORTCUT_STORAGE_KEY = "openduck.global-shortcut.v1";
export const GLOBAL_SHORTCUT_ENTIRE_SCREEN_STORAGE_KEY =
  "openduck.global-shortcut-entire.v1";
export const GLOBAL_SHORTCUT_TOGGLE_MUTE_STORAGE_KEY =
  "openduck.global-shortcut-toggle-mute.v1";
export const DEFAULT_GEMMA_VARIANT: GemmaVariant = "e4b";
export const DEFAULT_CSM_MODEL: CsmModelVariant = "kokoro_82m";
export const DEFAULT_STT_MODEL: SttModelVariant = "whisper_large_v3_turbo";
export const DEFAULT_OLLAMA_MODEL = "gemma2:2b";
export const DEFAULT_LMSTUDIO_MODEL = "";
export const DEFAULT_OPENAI_COMPATIBLE_MODEL = "";
export const DEFAULT_END_OF_UTTERANCE_SILENCE_MS = 2000;
export const MIN_END_OF_UTTERANCE_SILENCE_MS = 500;
export const MAX_END_OF_UTTERANCE_SILENCE_MS = 5000;
export const END_OF_UTTERANCE_SILENCE_STEP_MS = 100;
export const DEFAULT_AUTO_CONTINUE_SILENCE_MS: number | null = null;
export const MIN_AUTO_CONTINUE_SILENCE_MS = 3000;
export const MAX_AUTO_CONTINUE_SILENCE_MS = 7000;
export const AUTO_CONTINUE_SILENCE_STEP_MS = 1000;
export const AUTO_CONTINUE_NEVER_SLIDER_VALUE =
  MAX_AUTO_CONTINUE_SILENCE_MS + AUTO_CONTINUE_SILENCE_STEP_MS;
export const DEFAULT_AUTO_CONTINUE_MAX_COUNT: number | null = 3;
export const MIN_AUTO_CONTINUE_MAX_COUNT = 1;
export const MAX_AUTO_CONTINUE_MAX_COUNT = 10;
export const AUTO_CONTINUE_MAX_COUNT_CONTINUOUS_SLIDER_VALUE =
  MAX_AUTO_CONTINUE_MAX_COUNT + 1;
export const DEFAULT_LLM_CONTEXT_TURN_LIMIT: number | null = 7;
export const MIN_LLM_CONTEXT_TURN_LIMIT = 2;
export const MAX_LLM_CONTEXT_TURN_LIMIT = 50;
export const LLM_CONTEXT_TURN_LIMIT_UNLIMITED_SLIDER_VALUE =
  MAX_LLM_CONTEXT_TURN_LIMIT + 1;
export const DEFAULT_LLM_IMAGE_HISTORY_LIMIT: number | null = null;
export const MIN_LLM_IMAGE_HISTORY_LIMIT = 1;
export const MAX_LLM_IMAGE_HISTORY_LIMIT = 9;
export const LLM_IMAGE_HISTORY_UNLIMITED_SLIDER_VALUE =
  MAX_LLM_IMAGE_HISTORY_LIMIT + 1;
export const DEFAULT_GLOBAL_SHORTCUT = "Command+Shift+L";
export const DEFAULT_GLOBAL_SHORTCUT_ENTIRE_SCREEN = "Command+Shift+Option+L";
export const DEFAULT_GLOBAL_SHORTCUT_TOGGLE_MUTE =
  "Command+Shift+Option+U";
export const DEFAULT_AUTO_UNMUTE_ON_PASTED_SCREENSHOT = true;

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
      lmstudioModel: DEFAULT_LMSTUDIO_MODEL,
      openaiCompatibleModel: DEFAULT_OPENAI_COMPATIBLE_MODEL,
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
      lmstudioModel: DEFAULT_LMSTUDIO_MODEL,
      openaiCompatibleModel: DEFAULT_OPENAI_COMPATIBLE_MODEL,
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
      lmstudioModel: DEFAULT_LMSTUDIO_MODEL,
      openaiCompatibleModel: DEFAULT_OPENAI_COMPATIBLE_MODEL,
    },
  },
};
