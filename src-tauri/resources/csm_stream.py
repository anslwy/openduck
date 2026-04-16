import argparse
import base64
import contextlib
import gc
import importlib.util
import inspect
import io
import json
import os
import sys
import time
import traceback
import wave
from pathlib import Path

try:
    import numpy as np
except ImportError:
    np = None


# Lazy imports for heavy dependencies.
def import_numpy():
    if np is None:
        import numpy as np_import

        return np_import
    return np


def import_huggingface():
    from huggingface_hub import hf_hub_download

    return hf_hub_download


def import_tqdm():
    from tqdm.auto import tqdm

    return tqdm


sys.dont_write_bytecode = True

CSM_MODEL_REPO = "senstella/csm-expressiva-1b"
CSM_MODEL_FILE = "mlx-ckpt.safetensors"
CSM_MODEL_SPEAKER = 4
KOKORO_MODEL_REPO = "mlx-community/Kokoro-82M-bf16"
KOKORO_MODEL_FILE = "kokoro-v1_0.safetensors"
KOKORO_CONFIG_FILE = "config.json"
KOKORO_LANG_CODE = "a"
KOKORO_DEFAULT_VOICE = "af_heart"
KOKORO_DEFAULT_VOICE_FILE = f"voices/{KOKORO_DEFAULT_VOICE}.pt"
COSYVOICE2_MODEL_REPO = "mlx-community/CosyVoice2-0.5B-fp16"
COSYVOICE2_MODEL_FILE = "model.safetensors"
COSYVOICE2_CONFIG_FILE = "config.json"
COSYVOICE2_TOKENIZER_FILE = "tokenizer.json"
COSYVOICE2_TOKENIZER_CONFIG_FILE = "tokenizer_config.json"
COSYVOICE2_S3_TOKENIZER_REPO = "mlx-community/S3TokenizerV2"
COSYVOICE2_S3_TOKENIZER_CONFIG_FILE = "config.json"
COSYVOICE2_S3_TOKENIZER_MODEL_FILE = "model.safetensors"
COSYVOICE3_8BIT_MODEL_REPO = "mlx-community/Fun-CosyVoice3-0.5B-2512-8bit"
COSYVOICE3_4BIT_MODEL_REPO = "mlx-community/Fun-CosyVoice3-0.5B-2512-4bit"
COSYVOICE3_FP16_MODEL_REPO = "mlx-community/Fun-CosyVoice3-0.5B-2512-fp16"
COSYVOICE3_MODEL_FILE = "model.safetensors"
COSYVOICE3_CONFIG_FILE = "config.json"
COSYVOICE3_TOKENIZER_FILE = "tokenizer.json"
COSYVOICE3_TOKENIZER_CONFIG_FILE = "tokenizer_config.json"
SAMPLE_RATE = 24_000
CSM_WARMUP_TEXT = "Okay."
CSM_WARMUP_MAX_AUDIO_LENGTH_MS = 320
# Accumulate a few decoder frames per emitted chunk so playback can start
# early without overwhelming the JSON/stdout bridge with tiny packets.
CSM_STREAM_ACCUMULATION_SIZE = 4
DEVNULL = open(os.devnull, "w")

# Keep PyTorch MPS fallback enabled for any speech dependency that may import torch.
os.environ.setdefault("PYTORCH_ENABLE_MPS_FALLBACK", "1")


def emit(payload: dict) -> None:
    output = getattr(sys, "__stdout__", None) or sys.stdout
    output.write(json.dumps(payload) + "\n")
    output.flush()


def emit_status(message: str) -> None:
    emit({"type": "status", "message": message})


@contextlib.contextmanager
def redirect_library_stdout():
    # Third-party TTS libraries sometimes print warnings/progress to stdout.
    # The Rust side treats stdout as a JSON event stream, so route that noise
    # to stderr to keep the protocol intact.
    with contextlib.redirect_stdout(sys.stderr):
        yield


class CheckpointProgressTqdm:
    progress_label = "checkpoint"

    def __new__(cls, *args, **kwargs):
        tqdm = import_tqdm()

        class CheckpointProgressTqdmImpl(tqdm):
            def __init__(self, *args, **kwargs):
                kwargs.setdefault("file", DEVNULL)
                kwargs.setdefault("leave", False)
                kwargs.setdefault("mininterval", 0.25)
                super().__init__(*args, **kwargs)
                self._emit_progress()

            def update(self, n=1):
                result = super().update(n)
                self._emit_progress()
                return result

            def close(self):
                self._emit_progress(force=True)
                return super().close()

            def _emit_progress(self, force: bool = False) -> None:
                if self.total:
                    progress = max(
                        0.0, min(100.0, float(self.n) / float(self.total) * 100.0)
                    )
                    emit_status(
                        f"Downloading {CheckpointProgressTqdm.progress_label}... {progress:.0f}%"
                    )
                elif force:
                    emit_status(
                        f"Download complete. Initializing {CheckpointProgressTqdm.progress_label}..."
                    )
                else:
                    emit_status(
                        f"Downloading {CheckpointProgressTqdm.progress_label}..."
                    )

        return CheckpointProgressTqdmImpl(*args, **kwargs)


def download_hf_file(repo_id: str, filename: str, label: str) -> str:
    hf_hub_download = import_huggingface()
    emit_status(f"Resolving {label}...")
    try:
        return hf_hub_download(
            repo_id=repo_id,
            filename=filename,
            local_files_only=True,
        )
    except Exception:
        emit_status(f"Downloading {label}... This can take a while on first load.")

    supports_hf_tqdm = "tqdm_class" in inspect.signature(hf_hub_download).parameters
    if supports_hf_tqdm:
        CheckpointProgressTqdm.progress_label = label
        return hf_hub_download(
            repo_id=repo_id,
            filename=filename,
            tqdm_class=CheckpointProgressTqdm,
        )

    return hf_hub_download(repo_id=repo_id, filename=filename)


def download_csm_weights() -> str:
    return download_hf_file(CSM_MODEL_REPO, CSM_MODEL_FILE, "CSM checkpoint")


def download_kokoro_assets() -> list[str]:
    return [
        download_hf_file(KOKORO_MODEL_REPO, KOKORO_CONFIG_FILE, "Kokoro config"),
        download_hf_file(KOKORO_MODEL_REPO, KOKORO_MODEL_FILE, "Kokoro checkpoint"),
        download_hf_file(
            KOKORO_MODEL_REPO,
            KOKORO_DEFAULT_VOICE_FILE,
            f"Kokoro voice {KOKORO_DEFAULT_VOICE}",
        ),
    ]


def download_cosyvoice2_assets() -> list[str]:
    return [
        download_hf_file(
            COSYVOICE2_MODEL_REPO, COSYVOICE2_CONFIG_FILE, "CosyVoice2 config"
        ),
        download_hf_file(
            COSYVOICE2_MODEL_REPO, COSYVOICE2_MODEL_FILE, "CosyVoice2 checkpoint"
        ),
        download_hf_file(
            COSYVOICE2_MODEL_REPO, COSYVOICE2_TOKENIZER_FILE, "CosyVoice2 tokenizer"
        ),
        download_hf_file(
            COSYVOICE2_MODEL_REPO,
            COSYVOICE2_TOKENIZER_CONFIG_FILE,
            "CosyVoice2 tokenizer config",
        ),
        download_hf_file(
            COSYVOICE2_S3_TOKENIZER_REPO,
            COSYVOICE2_S3_TOKENIZER_CONFIG_FILE,
            "CosyVoice2 speech tokenizer config",
        ),
        download_hf_file(
            COSYVOICE2_S3_TOKENIZER_REPO,
            COSYVOICE2_S3_TOKENIZER_MODEL_FILE,
            "CosyVoice2 speech tokenizer",
        ),
    ]


def download_cosyvoice3_assets(repo_id: str) -> list[str]:
    return [
        download_hf_file(repo_id, COSYVOICE3_CONFIG_FILE, "CosyVoice3 config"),
        download_hf_file(repo_id, COSYVOICE3_MODEL_FILE, "CosyVoice3 checkpoint"),
        download_hf_file(repo_id, COSYVOICE3_TOKENIZER_FILE, "CosyVoice3 tokenizer"),
        download_hf_file(
            repo_id,
            COSYVOICE3_TOKENIZER_CONFIG_FILE,
            "CosyVoice3 tokenizer config",
        ),
        # Assuming it uses the same S3Tokenizer repo as CosyVoice2 if not specified otherwise
        download_hf_file(
            COSYVOICE2_S3_TOKENIZER_REPO,
            COSYVOICE2_S3_TOKENIZER_CONFIG_FILE,
            "CosyVoice3 speech tokenizer config",
        ),
        download_hf_file(
            COSYVOICE2_S3_TOKENIZER_REPO,
            COSYVOICE2_S3_TOKENIZER_MODEL_FILE,
            "CosyVoice3 speech tokenizer",
        ),
    ]


def encode_wav_base64(audio, sample_rate: int) -> str:
    np = import_numpy()
    audio = np.asarray(audio, dtype=np.float32).reshape(-1)
    if audio.size == 0:
        return ""

    clipped_audio = np.clip(audio, -1.0, 1.0)
    pcm16 = (clipped_audio * np.iinfo(np.int16).max).astype("<i2")
    buffer = io.BytesIO()

    with wave.open(buffer, "wb") as wav_file:
        wav_file.setnchannels(1)
        wav_file.setsampwidth(2)
        wav_file.setframerate(sample_rate)
        wav_file.writeframes(pcm16.tobytes())

    return base64.b64encode(buffer.getvalue()).decode("ascii")


def normalize_audio_array(audio):
    np = import_numpy()
    if hasattr(audio, "detach"):
        audio = audio.detach()
    if hasattr(audio, "cpu"):
        audio = audio.cpu()
    if hasattr(audio, "numpy"):
        audio = audio.numpy()

    return np.asarray(audio, dtype=np.float32).reshape(-1)


def build_csm_model(quantize: bool):
    emit_status("Importing MLX and CSM runtime...")
    from mlx import nn
    from csm_mlx import CSM, csm_1b

    weights_path = download_csm_weights()
    emit_status("Initializing CSM model graph...")
    model = CSM(csm_1b())
    emit_status("Loading CSM weights into memory...")
    model.load_weights(weights_path, strict=True)

    if quantize:
        emit_status("Quantizing CSM model...")
        nn.quantize(model)

    return model


def build_kokoro_model():
    emit_status("Importing MLX-Audio runtime...")
    from mlx_audio.tts.utils import load_model

    if importlib.util.find_spec("en_core_web_sm") is None:
        raise RuntimeError(
            "Kokoro via mlx-audio requires the spaCy English model 'en_core_web_sm'. "
            "Re-run scripts/setup_python_env.sh to install the Kokoro dependencies."
        )

    emit_status("Resolving Kokoro MLX assets...")
    download_kokoro_assets()
    emit_status("Initializing Kokoro model...")
    with redirect_library_stdout():
        return load_model(KOKORO_MODEL_REPO)


def build_cosyvoice2_model():
    emit_status("Importing MLX-Audio-Plus runtime...")
    from mlx_audio.tts.utils import load_model

    emit_status("Resolving CosyVoice2 MLX assets...")
    download_cosyvoice2_assets()
    emit_status("Initializing CosyVoice2 model...")
    with redirect_library_stdout():
        model = load_model(COSYVOICE2_MODEL_REPO)

    patch_cosyvoice_tokenizer(model, "CosyVoice2")
    return model


def build_cosyvoice3_model(repo_id: str):
    emit_status("Importing MLX-Audio-Plus runtime...")
    from mlx_audio.tts.utils import load_model

    emit_status("Resolving CosyVoice3 MLX assets...")
    download_cosyvoice3_assets(repo_id)
    emit_status("Initializing CosyVoice3 model...")
    with redirect_library_stdout():
        model = load_model(repo_id)

    patch_cosyvoice_tokenizer(model, "CosyVoice3")
    return model


class CosyVoiceTokenizerAdapter:
    def __init__(self, tokenizer):
        self._tokenizer = tokenizer

    def encode(self, *args, **kwargs):
        encoded = self._tokenizer.encode(*args, **kwargs)
        if hasattr(encoded, "ids"):
            return list(encoded.ids)
        return encoded

    def __getattr__(self, name):
        return getattr(self._tokenizer, name)


def patch_cosyvoice_tokenizer(model, label: str) -> None:
    if getattr(model, "_openduck_cosyvoice_tokenizer_patched", False):
        return

    ensure_tokenizers_loaded = getattr(model, "_ensure_tokenizers_loaded", None)
    if not callable(ensure_tokenizers_loaded):
        return

    emit_status(f"Preparing {label} tokenizers...")
    with redirect_library_stdout():
        # Temporarily patch transformers tokenizer loading if needed
        import transformers

        original_from_pretrained = transformers.AutoTokenizer.from_pretrained

        def patched_from_pretrained(*args, **kwargs):
            return original_from_pretrained(*args, **kwargs, fix_mistral_regex=True)

        try:
            transformers.AutoTokenizer.from_pretrained = patched_from_pretrained
            ensure_tokenizers_loaded()
        except TypeError:
            # Fallback if fix_mistral_regex isn't supported by this version of transformers
            transformers.AutoTokenizer.from_pretrained = original_from_pretrained
            ensure_tokenizers_loaded()
        finally:
            transformers.AutoTokenizer.from_pretrained = original_from_pretrained

    tokenizer = getattr(model, "_tokenizer", None)
    if tokenizer is None:
        return

    probe = tokenizer.encode("hello", add_special_tokens=False)
    if hasattr(probe, "ids"):
        model._tokenizer = CosyVoiceTokenizerAdapter(tokenizer)

    model._openduck_cosyvoice_tokenizer_patched = True


def resolve_model_sample_rate(model) -> int:
    try:
        return int(getattr(model, "sample_rate", SAMPLE_RATE))
    except (TypeError, ValueError):
        return SAMPLE_RATE


def supported_generate_kwargs(model, **candidate_kwargs):
    filtered_kwargs = {
        key: value for key, value in candidate_kwargs.items() if value is not None
    }
    try:
        parameters = inspect.signature(model.generate).parameters
    except (TypeError, ValueError):
        return filtered_kwargs

    if any(
        parameter.kind == inspect.Parameter.VAR_KEYWORD
        for parameter in parameters.values()
    ):
        return filtered_kwargs

    return {key: value for key, value in filtered_kwargs.items() if key in parameters}


def load_reference_audio(context_audio: Path | None, read_audio):
    if context_audio is None:
        return None

    emit_status(f"Loading voice reference from {context_audio.name}...")
    return read_audio(context_audio, SAMPLE_RATE)


def append_context_segment(
    context: list,
    speaker: int,
    audio,
    text: str,
    Segment,
) -> None:
    if audio is None:
        return

    context.append(
        Segment(
            speaker=speaker,
            text=text,
            audio=audio,
        )
    )


def run_csm_server(
    quantize: bool, context_audio: Path | None = None, context_text: str = ""
) -> int:
    try:
        emit_status("Importing generation helpers...")
        import mlx.core as mx
        from mlx_lm.sample_utils import make_sampler
        from csm_mlx import Segment, generate, stream_generate
        from csm_mlx.utils import read_audio
    except Exception as exc:
        emit({"type": "error", "message": f"Failed to import csm-mlx: {exc}"})
        traceback.print_exc(file=sys.stderr)
        return 1

    try:
        emit_status("Building CSM worker...")
        model = build_csm_model(quantize)
    except Exception as exc:
        emit({"type": "error", "message": f"Failed to load CSM model: {exc}"})
        traceback.print_exc(file=sys.stderr)
        return 1

    try:
        emit_status("Warming up CSM runtime...")
        warmup_sampler = make_sampler(temp=0.0, top_k=1)
        warmup_audio = generate(
            model,
            text=CSM_WARMUP_TEXT,
            speaker=CSM_MODEL_SPEAKER,
            context=[],
            max_audio_length_ms=CSM_WARMUP_MAX_AUDIO_LENGTH_MS,
            sampler=warmup_sampler,
        )
        # Materialize the generated audio once so MLX allocates the decoder
        # runtime during model load instead of on the first live reply.
        normalize_audio_array(warmup_audio)
    except Exception as exc:
        emit_status(f"CSM warmup skipped: {exc}")
        traceback.print_exc(file=sys.stderr)
    finally:
        with contextlib.suppress(Exception):
            del warmup_audio
        with contextlib.suppress(Exception):
            del warmup_sampler
        gc.collect()

    emit_status("CSM worker ready.")
    emit({"type": "ready", "sample_rate": SAMPLE_RATE})
    reference_audio = None
    # Keep a small rolling carry-over from the last synthesized segment so
    # adjacent requests inherit the same delivery without losing the base voice.
    last_response_audio = None
    last_response_text = ""
    in_progress_request_id = None
    in_progress_audio = None
    in_progress_text = ""

    def reset_dynamic_context() -> None:
        nonlocal last_response_audio, last_response_text
        nonlocal in_progress_request_id, in_progress_audio, in_progress_text
        last_response_audio = None
        last_response_text = ""
        in_progress_request_id = None
        in_progress_audio = None
        in_progress_text = ""

    if context_audio is not None:
        try:
            reference_audio = load_reference_audio(context_audio, read_audio)
        except Exception as exc:
            emit({"type": "error", "message": f"Failed to load context audio: {exc}"})
            traceback.print_exc(file=sys.stderr)
            return 1

    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError as exc:
            emit({"type": "error", "message": f"Invalid JSON request: {exc}"})
            continue

        request_type = request.get("type")
        if request_type == "shutdown":
            break
        if request_type == "set_context":
            context_audio_path = request.get("context_audio_path")
            try:
                reference_audio = (
                    load_reference_audio(Path(str(context_audio_path)), read_audio)
                    if context_audio_path
                    else None
                )
                reset_dynamic_context()
            except Exception as exc:
                emit(
                    {"type": "error", "message": f"Failed to load context audio: {exc}"}
                )
                traceback.print_exc(file=sys.stderr)
            continue
        if request_type == "reset_context":
            reset_dynamic_context()
            continue
        if request_type == "finalize_response":
            request_id = request.get("request_id")
            if request_id == in_progress_request_id and in_progress_audio is not None:
                last_response_audio = in_progress_audio
                last_response_text = in_progress_text
            in_progress_request_id = None
            in_progress_audio = None
            in_progress_text = ""
            continue
        if request_type != "synthesize":
            emit(
                {
                    "type": "error",
                    "message": f"Unsupported request type: {request_type}",
                }
            )
            continue

        request_id = int(request.get("request_id"))
        speaker = int(request.get("speaker", CSM_MODEL_SPEAKER))
        text = str(request.get("text", "")).strip()
        if not text:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": "Cannot synthesize an empty response.",
                }
            )
            continue

        try:
            synthesis_started_at = time.perf_counter()
            sampler = make_sampler(
                temp=float(request.get("temperature", 0.8)),
                top_k=int(request.get("top_k", 50)),
            )
            context = []
            append_context_segment(
                context,
                speaker,
                reference_audio,
                context_text,
                Segment,
            )
            if request_id == in_progress_request_id:
                append_context_segment(
                    context,
                    speaker,
                    in_progress_audio,
                    in_progress_text,
                    Segment,
                )
            else:
                append_context_segment(
                    context,
                    speaker,
                    last_response_audio,
                    last_response_text,
                    Segment,
                )
            emitted_audio = False
            streamed_audio_chunks = []
            for audio_chunk in stream_generate(
                model,
                text=text,
                speaker=speaker,
                context=context,
                max_audio_length_ms=int(request.get("max_audio_length_ms", 10_000)),
                accumulation_size=CSM_STREAM_ACCUMULATION_SIZE,
                sampler=sampler,
            ):
                normalized_chunk = normalize_audio_array(audio_chunk)
                if normalized_chunk.size == 0:
                    continue

                streamed_audio_chunks.append(normalized_chunk)
                audio_wav_base64 = encode_wav_base64(normalized_chunk, SAMPLE_RATE)
                if not audio_wav_base64:
                    continue

                emit(
                    {
                        "type": "chunk",
                        "request_id": request_id,
                        "audio_wav_base64": audio_wav_base64,
                    }
                )
                emitted_audio = True

            if not emitted_audio:
                raise RuntimeError("CSM generated an empty audio response.")

            if len(streamed_audio_chunks) == 1:
                audio = mx.array(streamed_audio_chunks[0])
            else:
                np = import_numpy()
                audio = mx.array(np.concatenate(streamed_audio_chunks))

            in_progress_request_id = request_id
            in_progress_audio = audio
            in_progress_text = text
            emit(
                {
                    "type": "timing",
                    "request_id": request_id,
                    "text": text,
                    "elapsed_ms": round(
                        (time.perf_counter() - synthesis_started_at) * 1000.0, 2
                    ),
                }
            )
            emit({"type": "done", "request_id": request_id})
        except Exception as exc:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": f"CSM synthesis failed: {exc}",
                }
            )
            traceback.print_exc(file=sys.stderr)
        finally:
            gc.collect()

    return 0


def run_kokoro_server(_quantize: bool) -> int:
    try:
        emit_status("Building Kokoro worker...")
        model = build_kokoro_model()
    except Exception as exc:
        emit({"type": "error", "message": f"Failed to load Kokoro: {exc}"})
        traceback.print_exc(file=sys.stderr)
        return 1

    sample_rate = resolve_model_sample_rate(model)
    try:
        emit_status("Warming up Kokoro runtime...")
        with redirect_library_stdout():
            generator = model.generate(
                text="Okay.",
                voice=KOKORO_DEFAULT_VOICE,
                speed=1.0,
                lang_code=KOKORO_LANG_CODE,
            )
            for result in generator:
                normalize_audio_array(result.audio)
                break
    except Exception as exc:
        emit_status(f"Kokoro warmup skipped: {exc}")

    emit_status("Kokoro worker ready.")
    emit({"type": "ready", "sample_rate": sample_rate})

    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError as exc:
            emit({"type": "error", "message": f"Invalid JSON request: {exc}"})
            continue

        request_type = request.get("type")
        if request_type == "shutdown":
            break
        if request_type in {"set_context", "reset_context", "finalize_response"}:
            continue
        if request_type != "synthesize":
            emit(
                {
                    "type": "error",
                    "message": f"Unsupported request type: {request_type}",
                }
            )
            continue

        request_id = int(request.get("request_id"))
        text = str(request.get("text", "")).strip()
        if not text:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": "Cannot synthesize an empty response.",
                }
            )
            continue

        try:
            synthesis_started_at = time.perf_counter()
            emitted_audio = False
            with redirect_library_stdout():
                generator = model.generate(
                    text=text,
                    voice=str(request.get("voice", KOKORO_DEFAULT_VOICE)),
                    speed=float(request.get("speed", 1.0)),
                    lang_code=KOKORO_LANG_CODE,
                )
                for result in generator:
                    audio_wav_base64 = encode_wav_base64(
                        normalize_audio_array(result.audio),
                        sample_rate,
                    )
                    if not audio_wav_base64:
                        continue

                    emit(
                        {
                            "type": "chunk",
                            "request_id": request_id,
                            "audio_wav_base64": audio_wav_base64,
                        }
                    )
                    emitted_audio = True

            if not emitted_audio:
                raise RuntimeError("Kokoro generated an empty audio response.")

            emit(
                {
                    "type": "timing",
                    "request_id": request_id,
                    "text": text,
                    "elapsed_ms": round(
                        (time.perf_counter() - synthesis_started_at) * 1000.0, 2
                    ),
                }
            )
            emit({"type": "done", "request_id": request_id})
        except Exception as exc:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": f"Kokoro synthesis failed: {exc}",
                }
            )
            traceback.print_exc(file=sys.stderr)
        finally:
            gc.collect()

    return 0


def run_cosyvoice2_server(_quantize: bool, context_audio: Path | None = None) -> int:
    try:
        emit_status("Importing CosyVoice2 helpers...")
        import mlx.core as mx
        from mlx_audio.tts.generate import load_audio
    except Exception as exc:
        emit(
            {
                "type": "error",
                "message": f"Failed to import CosyVoice2 runtime helpers: {exc}",
            }
        )
        traceback.print_exc(file=sys.stderr)
        return 1

    try:
        emit_status("Building CosyVoice2 worker...")
        model = build_cosyvoice2_model()
    except Exception as exc:
        emit({"type": "error", "message": f"Failed to load CosyVoice2: {exc}"})
        traceback.print_exc(file=sys.stderr)
        return 1

    sample_rate = resolve_model_sample_rate(model)
    reference_audio = None

    def load_reference_audio():
        nonlocal reference_audio
        if context_audio is None:
            reference_audio = None
            return None

        emit_status(f"Loading CosyVoice2 reference audio from {context_audio.name}...")
        with redirect_library_stdout():
            reference_audio = load_audio(str(context_audio), sample_rate=sample_rate)
        return reference_audio

    if context_audio is not None:
        try:
            load_reference_audio()
        except Exception as exc:
            emit(
                {
                    "type": "error",
                    "message": f"Failed to load CosyVoice2 reference audio: {exc}",
                }
            )
            traceback.print_exc(file=sys.stderr)
            return 1

    if reference_audio is not None:
        try:
            emit_status("Warming up CosyVoice2 runtime...")
            warmup_kwargs = supported_generate_kwargs(
                model,
                text="Okay.",
                ref_audio=reference_audio,
                ref_text="",
                stream=True,
            )
            with redirect_library_stdout():
                generator = model.generate(**warmup_kwargs)
                for result in generator:
                    normalize_audio_array(result.audio)
                    break
        except Exception as exc:
            emit_status(f"CosyVoice2 warmup skipped: {exc}")

    emit_status("CosyVoice2 worker ready.")
    emit({"type": "ready", "sample_rate": sample_rate})

    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError as exc:
            emit({"type": "error", "message": f"Invalid JSON request: {exc}"})
            continue

        request_type = request.get("type")
        if request_type == "shutdown":
            break
        if request_type == "set_context":
            context_audio_path = request.get("context_audio_path")
            try:
                context_audio = (
                    Path(str(context_audio_path)) if context_audio_path else None
                )
                load_reference_audio()
            except Exception as exc:
                emit(
                    {
                        "type": "error",
                        "message": f"Failed to load CosyVoice2 reference audio: {exc}",
                    }
                )
                traceback.print_exc(file=sys.stderr)
            continue
        if request_type in {"reset_context", "finalize_response"}:
            continue
        if request_type != "synthesize":
            emit(
                {
                    "type": "error",
                    "message": f"Unsupported request type: {request_type}",
                }
            )
            continue

        request_id = int(request.get("request_id"))
        text = str(request.get("text", "")).strip()
        if not text:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": "Cannot synthesize an empty response.",
                }
            )
            continue

        if reference_audio is None:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": "CosyVoice2 requires reference audio before synthesis.",
                }
            )
            continue

        try:
            synthesis_started_at = time.perf_counter()
            emitted_audio = False
            generate_kwargs = supported_generate_kwargs(
                model,
                text=text,
                ref_audio=reference_audio,
                ref_text=request.get("ref_text"),
                speed=float(request.get("speed", 1.0)),
                temperature=float(request.get("temperature", 0.7)),
                top_k=int(request.get("top_k", 50)),
                stream=True,
            )
            with redirect_library_stdout():
                generator = model.generate(**generate_kwargs)
                for result in generator:
                    audio_wav_base64 = encode_wav_base64(
                        normalize_audio_array(result.audio),
                        sample_rate,
                    )
                    if not audio_wav_base64:
                        continue

                    emit(
                        {
                            "type": "chunk",
                            "request_id": request_id,
                            "audio_wav_base64": audio_wav_base64,
                        }
                    )
                    emitted_audio = True

            if not emitted_audio:
                raise RuntimeError("CosyVoice2 generated an empty audio response.")

            emit(
                {
                    "type": "timing",
                    "request_id": request_id,
                    "text": text,
                    "elapsed_ms": round(
                        (time.perf_counter() - synthesis_started_at) * 1000.0, 2
                    ),
                }
            )
            emit({"type": "done", "request_id": request_id})
        except Exception as exc:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": f"CosyVoice2 synthesis failed: {exc}",
                }
            )
            traceback.print_exc(file=sys.stderr)
        finally:
            mx.clear_cache()
            gc.collect()

    return 0


def run_cosyvoice3_server(
    _quantize: bool, repo_id: str, context_audio: Path | None = None
) -> int:
    try:
        emit_status("Importing CosyVoice3 helpers...")
        import mlx.core as mx
        from mlx_audio.tts.generate import load_audio
    except Exception as exc:
        emit(
            {
                "type": "error",
                "message": f"Failed to import CosyVoice3 runtime helpers: {exc}",
            }
        )
        traceback.print_exc(file=sys.stderr)
        return 1

    try:
        emit_status("Building CosyVoice3 worker...")
        model = build_cosyvoice3_model(repo_id)
    except Exception as exc:
        emit({"type": "error", "message": f"Failed to load CosyVoice3: {exc}"})
        traceback.print_exc(file=sys.stderr)
        return 1

    sample_rate = resolve_model_sample_rate(model)
    reference_audio = None

    def load_reference_audio():
        nonlocal reference_audio
        if context_audio is None:
            reference_audio = None
            return None

        emit_status(f"Loading CosyVoice3 reference audio from {context_audio.name}...")
        with redirect_library_stdout():
            reference_audio = load_audio(str(context_audio), sample_rate=sample_rate)
        return reference_audio

    if context_audio is not None:
        try:
            load_reference_audio()
        except Exception as exc:
            emit(
                {
                    "type": "error",
                    "message": f"Failed to load CosyVoice3 reference audio: {exc}",
                }
            )
            traceback.print_exc(file=sys.stderr)
            return 1

    if reference_audio is not None:
        try:
            emit_status("Warming up CosyVoice3 runtime...")
            warmup_kwargs = supported_generate_kwargs(
                model,
                text="Okay.",
                ref_audio=reference_audio,
                ref_text="",
                stream=True,
            )
            with redirect_library_stdout():
                generator = model.generate(**warmup_kwargs)
                for result in generator:
                    normalize_audio_array(result.audio)
                    break
        except Exception as exc:
            emit_status(f"CosyVoice3 warmup skipped: {exc}")

    emit_status("CosyVoice3 worker ready.")
    emit({"type": "ready", "sample_rate": sample_rate})

    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError as exc:
            emit({"type": "error", "message": f"Invalid JSON request: {exc}"})
            continue

        request_type = request.get("type")
        if request_type == "shutdown":
            break
        if request_type == "set_context":
            context_audio_path = request.get("context_audio_path")
            try:
                context_audio = (
                    Path(str(context_audio_path)) if context_audio_path else None
                )
                load_reference_audio()
            except Exception as exc:
                emit(
                    {
                        "type": "error",
                        "message": f"Failed to load CosyVoice3 reference audio: {exc}",
                    }
                )
                traceback.print_exc(file=sys.stderr)
            continue
        if request_type in {"reset_context", "finalize_response"}:
            continue
        if request_type != "synthesize":
            emit(
                {
                    "type": "error",
                    "message": f"Unsupported request type: {request_type}",
                }
            )
            continue

        request_id = int(request.get("request_id"))
        text = str(request.get("text", "")).strip()
        if not text:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": "Cannot synthesize an empty response.",
                }
            )
            continue

        if reference_audio is None:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": "CosyVoice3 requires reference audio before synthesis.",
                }
            )
            continue

        try:
            synthesis_started_at = time.perf_counter()
            emitted_audio = False
            generate_kwargs = supported_generate_kwargs(
                model,
                text=text,
                ref_audio=reference_audio,
                ref_text=request.get("ref_text"),
                speed=float(request.get("speed", 1.0)),
                temperature=float(request.get("temperature", 0.7)),
                top_k=int(request.get("top_k", 50)),
                stream=True,
            )
            with redirect_library_stdout():
                generator = model.generate(**generate_kwargs)
                for result in generator:
                    audio_wav_base64 = encode_wav_base64(
                        normalize_audio_array(result.audio),
                        sample_rate,
                    )
                    if not audio_wav_base64:
                        continue

                    emit(
                        {
                            "type": "chunk",
                            "request_id": request_id,
                            "audio_wav_base64": audio_wav_base64,
                        }
                    )
                    emitted_audio = True

            if not emitted_audio:
                raise RuntimeError("CosyVoice3 generated an empty audio response.")

            emit(
                {
                    "type": "timing",
                    "request_id": request_id,
                    "text": text,
                    "elapsed_ms": round(
                        (time.perf_counter() - synthesis_started_at) * 1000.0, 2
                    ),
                }
            )
            emit({"type": "done", "request_id": request_id})
        except Exception as exc:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": f"CosyVoice3 synthesis failed: {exc}",
                }
            )
            traceback.print_exc(file=sys.stderr)
        finally:
            mx.clear_cache()
            gc.collect()

    return 0


def run_server(
    model_name: str,
    quantize: bool,
    context_audio: Path | None = None,
    context_text: str = "",
) -> int:
    if model_name == "csm":
        return run_csm_server(quantize, context_audio, context_text)
    if model_name == "kokoro":
        return run_kokoro_server(quantize)
    if model_name == "cosyvoice2":
        return run_cosyvoice2_server(quantize, context_audio)
    if model_name == "cosyvoice3_8bit":
        return run_cosyvoice3_server(
            quantize, COSYVOICE3_8BIT_MODEL_REPO, context_audio
        )
    if model_name == "cosyvoice3_4bit":
        return run_cosyvoice3_server(
            quantize, COSYVOICE3_4BIT_MODEL_REPO, context_audio
        )
    if model_name == "cosyvoice3_fp16":
        return run_cosyvoice3_server(
            quantize, COSYVOICE3_FP16_MODEL_REPO, context_audio
        )

    emit({"type": "error", "message": f"Unsupported speech model: {model_name}"})
    return 2


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--download", action="store_true")
    parser.add_argument("--server", action="store_true")
    parser.add_argument("--model", default="csm")
    parser.add_argument("--quantize", action="store_true")
    parser.add_argument("--context-audio", type=Path)
    parser.add_argument("--context-text", default="")
    args = parser.parse_args()

    if args.download:
        try:
            if args.model == "kokoro":
                paths = download_kokoro_assets()
                emit({"type": "downloaded", "paths": paths})
            elif args.model == "cosyvoice2":
                paths = download_cosyvoice2_assets()
                emit({"type": "downloaded", "paths": paths})
            elif args.model == "cosyvoice3_8bit":
                paths = download_cosyvoice3_assets(COSYVOICE3_8BIT_MODEL_REPO)
                emit({"type": "downloaded", "paths": paths})
            elif args.model == "cosyvoice3_4bit":
                paths = download_cosyvoice3_assets(COSYVOICE3_4BIT_MODEL_REPO)
                emit({"type": "downloaded", "paths": paths})
            elif args.model == "cosyvoice3_fp16":
                paths = download_cosyvoice3_assets(COSYVOICE3_FP16_MODEL_REPO)
                emit({"type": "downloaded", "paths": paths})
            else:
                path = download_csm_weights()
                emit({"type": "downloaded", "path": path})
            return 0
        except Exception as exc:
            emit(
                {
                    "type": "error",
                    "message": f"Failed to download {args.model} assets: {exc}",
                }
            )
            traceback.print_exc(file=sys.stderr)
            return 1

    if args.server:
        return run_server(
            args.model,
            args.quantize,
            args.context_audio,
            args.context_text,
        )

    parser.error("Expected either --download or --server")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
