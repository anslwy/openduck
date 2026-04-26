// Shared static configuration for contacts, model defaults, and selectable presets on the home page.
import type {
  CsmModelVariant,
  GemmaVariant,
  KokoroLanguage,
  ModelPreset,
  ModelSelection,
  SttLanguage,
  SttModelVariant,
} from "./types";

export const DEFAULT_CONTACT_PROMPT =
  "You are a friendly voice AI assistant from OpenDuck. A local-first voice-call desktop application built for rubberducking.";
export const BUILT_IN_VOICE_CALL_PROMPT =
  "You are in a live voice call right now. Reply like a natural spoken conversation. Use plain short sentences only. Never use markdown, bullets, headings, numbered lists, code fences, tables, emojis, or stage directions. Keep responses concise, direct, and easy to speak aloud. Try to ask follow-up questions more often. Make sure your first sentence in every reply is short. Never display your thinking process. Answer directly.";
export const CONTACTS_STORAGE_KEY = "openduck.contacts.v1";
export const CONTACT_ICONS_DB_NAME = "openduck.contacts";
export const CONTACT_ASSETS_DB_VERSION = 2;
export const CONTACT_ICONS_STORE_NAME = "contact-icons";
export const CONTACT_CUBISM_ZIPS_STORE_NAME = "contact-cubism-model-zips";
export const DEFAULT_CONTACT_ID = "contact-openduck";
export const MODEL_PREFERENCES_STORAGE_KEY = "openduck.model-preferences.v1";
export const APP_UPDATE_PREFERENCES_STORAGE_KEY =
  "openduck.app-update-preferences.v1";
export const PONG_PLAYBACK_STORAGE_KEY = "openduck.pong-playback.v1";
export const SELECT_LAST_SESSION_STORAGE_KEY = "openduck.select-last-session.v1";
export const AUTO_LOAD_MODELS_ON_STARTUP_STORAGE_KEY = "openduck.auto-load-models-on-startup.v1";
export const SHOW_STAT_STORAGE_KEY = "openduck.show-stat.v1";
export const SHOW_SUBTITLE_STORAGE_KEY = "openduck.show-subtitle.v1";
export const SHOW_AI_SUBTITLE_STORAGE_KEY = "openduck.show-ai-subtitle.v1";
export const AI_SUBTITLE_TARGET_LANGUAGE_STORAGE_KEY = "openduck.ai-subtitle-target-language.v1";
export const SHOW_CALL_TIMER_STORAGE_KEY = "openduck.show-call-timer.v1";
export const SHOW_HIDDEN_WINDOW_OVERLAY_STORAGE_KEY =
  "openduck.show-hidden-window-overlay.v1";
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
export const GLOBAL_SHORTCUT_TOGGLE_MUTE_STORAGE_KEY = "openduck.global-shortcut-toggle-mute.v1";
export const GLOBAL_SHORTCUT_INTERRUPT_STORAGE_KEY = "openduck.global-shortcut-interrupt.v1";
export const DEFAULT_GEMMA_VARIANT: GemmaVariant = "e4b";

export const DEFAULT_CSM_MODEL: CsmModelVariant = "kokoro_82m";
export const DEFAULT_KOKORO_LANGUAGE: KokoroLanguage = "american_english";
export const DEFAULT_STT_MODEL: SttModelVariant = "whisper_large_v3_turbo";
export const DEFAULT_STT_LANGUAGE: SttLanguage = "auto";
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
export const DEFAULT_GLOBAL_SHORTCUT_TOGGLE_MUTE = "Command+Shift+Option+U";
export const DEFAULT_GLOBAL_SHORTCUT_INTERRUPT = "Command+Shift+Option+I";
export const NO_GLOBAL_SHORTCUT = "None";
export const DEFAULT_AUTO_UNMUTE_ON_PASTED_SCREENSHOT = true;

export const DEFAULT_SHOW_AI_SUBTITLE = true;
export const DEFAULT_AI_SUBTITLE_TARGET_LANGUAGE = "none";
export const DEFAULT_SHOW_CALL_TIMER = true;
export const DEFAULT_SHOW_HIDDEN_WINDOW_OVERLAY = true;
export const DEFAULT_AUTO_LOAD_MODELS_ON_STARTUP = true;
export const DEFAULT_AUTO_CHECK_APP_UPDATES = true;

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
