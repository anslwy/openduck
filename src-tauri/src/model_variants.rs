// Backend enums for selectable models and voices, plus helpers for mapping keys to repos and cache requirements.
use crate::constants::*;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum GemmaVariant {
    E4b,
    E2b,
    Ollama,
}

impl GemmaVariant {
    pub(crate) fn from_key(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "e4b" => Ok(Self::E4b),
            "e2b" => Ok(Self::E2b),
            "ollama" => Ok(Self::Ollama),
            other => Err(format!("Unsupported LLM variant: {other}")),
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::E4b => "e4b",
            Self::E2b => "e2b",
            Self::Ollama => "ollama",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::E4b => "Gemma-4-E4B",
            Self::E2b => "Gemma-4-E2B",
            Self::Ollama => "Ollama",
        }
    }

    pub(crate) fn repo_id(self) -> &'static str {
        match self {
            Self::E4b => "mlx-community/gemma-4-E4B-it-8bit",
            Self::E2b => "mlx-community/gemma-4-E2B-it-4bit",
            Self::Ollama => "gemma2:2b",
        }
    }

    pub(crate) fn cache_dir(self) -> &'static str {
        match self {
            Self::E4b => "models--mlx-community--gemma-4-E4B-it-8bit",
            Self::E2b => "models--mlx-community--gemma-4-E2B-it-4bit",
            Self::Ollama => "",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum CsmVoice {
    Male,
    Female,
}

impl CsmVoice {
    pub(crate) fn from_key(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "male" => Ok(Self::Male),
            "female" => Ok(Self::Female),
            other => Err(format!("Unsupported CSM voice: {other}")),
        }
    }

    pub(crate) fn file_name(self) -> &'static str {
        match self {
            Self::Male => CSM_MALE_REFERENCE_AUDIO_FILE,
            Self::Female => CSM_FEMALE_REFERENCE_AUDIO_FILE,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CsmModelVariant {
    Expressiva1b,
    Kokoro82m,
    CosyVoice205b,
    CosyVoice305b8bit,
    CosyVoice305b4bit,
}

impl CsmModelVariant {
    pub(crate) fn from_key(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "expressiva_1b" | "expressiva-1b" | "csm" => Ok(Self::Expressiva1b),
            "kokoro_82m" | "kokoro-82m" | "kokoro" => Ok(Self::Kokoro82m),
            "cosyvoice2_0_5b" | "cosyvoice2-0-5b" | "cosyvoice2-0.5b" | "cosyvoice2"
            | "cosyvoice" => Ok(Self::CosyVoice205b),
            "cosyvoice3_0_5b_8bit"
            | "cosyvoice3-0-5b-8bit"
            | "cosyvoice3-0.5b-8bit"
            | "cosyvoice3" => Ok(Self::CosyVoice305b8bit),
            "cosyvoice3_0_5b_4bit" | "cosyvoice3-0-5b-4bit" | "cosyvoice3-0.5b-4bit" => {
                Ok(Self::CosyVoice305b4bit)
            }
            other => Err(format!("Unsupported speech model: {other}")),
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Expressiva1b => "expressiva_1b",
            Self::Kokoro82m => "kokoro_82m",
            Self::CosyVoice205b => "cosyvoice2_0_5b",
            Self::CosyVoice305b8bit => "cosyvoice3_0_5b_8bit",
            Self::CosyVoice305b4bit => "cosyvoice3_0_5b_4bit",
        }
    }

    pub(crate) fn worker_key(self) -> &'static str {
        match self {
            Self::Expressiva1b => "csm",
            Self::Kokoro82m => "kokoro",
            Self::CosyVoice205b => "cosyvoice2",
            Self::CosyVoice305b8bit => "cosyvoice3_8bit",
            Self::CosyVoice305b4bit => "cosyvoice3_4bit",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Expressiva1b => "CSM Expressiva 1B",
            Self::Kokoro82m => "Kokoro-82M",
            Self::CosyVoice205b => "CosyVoice2-0.5B",
            Self::CosyVoice305b8bit => "Fun-CosyVoice3-0.5B (8-bit)",
            Self::CosyVoice305b4bit => "Fun-CosyVoice3-0.5B (4-bit)",
        }
    }

    pub(crate) fn repo_id(self) -> &'static str {
        match self {
            Self::Expressiva1b => CSM_EXPRESSIVA_MODEL_REPO,
            Self::Kokoro82m => KOKORO_MODEL_REPO,
            Self::CosyVoice205b => COSYVOICE2_MODEL_REPO,
            Self::CosyVoice305b8bit => COSYVOICE3_8BIT_MODEL_REPO,
            Self::CosyVoice305b4bit => COSYVOICE3_4BIT_MODEL_REPO,
        }
    }

    pub(crate) fn cache_dir(self) -> &'static str {
        match self {
            Self::Expressiva1b => CSM_EXPRESSIVA_CACHE_DIR,
            Self::Kokoro82m => KOKORO_CACHE_DIR,
            Self::CosyVoice205b => COSYVOICE2_CACHE_DIR,
            Self::CosyVoice305b8bit => COSYVOICE3_8BIT_CACHE_DIR,
            Self::CosyVoice305b4bit => COSYVOICE3_4BIT_CACHE_DIR,
        }
    }

    pub(crate) fn required_files(self) -> &'static [&'static str] {
        match self {
            Self::Expressiva1b => &[CSM_EXPRESSIVA_MODEL_FILE],
            Self::Kokoro82m => &[
                KOKORO_CONFIG_FILE,
                KOKORO_MODEL_FILE,
                KOKORO_DEFAULT_VOICE_FILE,
            ],
            Self::CosyVoice205b => &[
                COSYVOICE2_CONFIG_FILE,
                COSYVOICE2_MODEL_FILE,
                COSYVOICE2_TOKENIZER_FILE,
                COSYVOICE2_TOKENIZER_CONFIG_FILE,
            ],
            Self::CosyVoice305b8bit | Self::CosyVoice305b4bit => &[
                COSYVOICE3_CONFIG_FILE,
                COSYVOICE3_MODEL_FILE,
                COSYVOICE3_TOKENIZER_FILE,
                COSYVOICE3_TOKENIZER_CONFIG_FILE,
            ],
        }
    }

    pub(crate) fn supports_quantization(self) -> bool {
        matches!(self, Self::Expressiva1b)
    }

    pub(crate) fn uses_reference_audio(self) -> bool {
        matches!(
            self,
            Self::CosyVoice205b | Self::CosyVoice305b8bit | Self::CosyVoice305b4bit
        )
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SttModelVariant {
    Gemma,
    WhisperLargeV3Turbo,
}

impl SttModelVariant {
    pub(crate) fn from_key(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "gemma" => Ok(Self::Gemma),
            "whisper_large_v3_turbo" | "whisper-large-v3-turbo" | "whisper" => {
                Ok(Self::WhisperLargeV3Turbo)
            }
            other => Err(format!("Unsupported STT model: {other}")),
        }
    }

    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Gemma => "gemma",
            Self::WhisperLargeV3Turbo => "whisper_large_v3_turbo",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Gemma => "Gemma",
            Self::WhisperLargeV3Turbo => "Whisper Large V3 Turbo",
        }
    }

    pub(crate) fn repo_id(self) -> Option<&'static str> {
        match self {
            Self::Gemma => None,
            Self::WhisperLargeV3Turbo => Some(STT_WHISPER_MODEL_REPO),
        }
    }

    pub(crate) fn cache_dir(self) -> Option<&'static str> {
        match self {
            Self::Gemma => None,
            Self::WhisperLargeV3Turbo => Some(STT_WHISPER_CACHE_DIR),
        }
    }

    pub(crate) fn required_files(self) -> &'static [&'static str] {
        match self {
            Self::Gemma => &[],
            Self::WhisperLargeV3Turbo => &[
                STT_WHISPER_ADDED_TOKENS_FILE,
                STT_WHISPER_CONFIG_FILE,
                STT_WHISPER_GENERATION_CONFIG_FILE,
                STT_WHISPER_MERGES_FILE,
                STT_WHISPER_MODEL_FILE,
                STT_WHISPER_NORMALIZER_FILE,
                STT_WHISPER_PREPROCESSOR_CONFIG_FILE,
                STT_WHISPER_SPECIAL_TOKENS_MAP_FILE,
                STT_WHISPER_TOKENIZER_FILE,
                STT_WHISPER_TOKENIZER_CONFIG_FILE,
                STT_WHISPER_VOCAB_FILE,
            ],
        }
    }

    pub(crate) fn uses_worker(self) -> bool {
        matches!(self, Self::WhisperLargeV3Turbo)
    }
}
