import argparse
import base64
import gc
import io
import inspect
import json
import os
import sys
import traceback
import wave
from pathlib import Path

import numpy as np
from huggingface_hub import hf_hub_download
from tqdm.auto import tqdm

MODEL_REPO = "senstella/csm-expressiva-1b"
MODEL_FILE = "mlx-ckpt.safetensors"
MODEL_SPEAKER = 4
SAMPLE_RATE = 24_000
DEVNULL = open(os.devnull, "w")


def emit(payload: dict) -> None:
    sys.stdout.write(json.dumps(payload) + "\n")
    sys.stdout.flush()


def emit_status(message: str) -> None:
    emit({"type": "status", "message": message})


class CheckpointProgressTqdm(tqdm):
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
            progress = max(0.0, min(100.0, float(self.n) / float(self.total) * 100.0))
            emit_status(f"Downloading CSM checkpoint... {progress:.0f}%")
        elif force:
            emit_status("Download complete. Initializing CSM model...")
        else:
            emit_status("Downloading CSM checkpoint...")


def download_weights() -> str:
    emit_status("Resolving CSM checkpoint...")
    try:
        return hf_hub_download(
            repo_id=MODEL_REPO,
            filename=MODEL_FILE,
            local_files_only=True,
        )
    except Exception:
        emit_status("Downloading CSM checkpoint... This can take a while on first load.")

    supports_hf_tqdm = "tqdm_class" in inspect.signature(hf_hub_download).parameters
    if supports_hf_tqdm:
        return hf_hub_download(
            repo_id=MODEL_REPO,
            filename=MODEL_FILE,
            tqdm_class=CheckpointProgressTqdm,
        )

    return hf_hub_download(repo_id=MODEL_REPO, filename=MODEL_FILE)


def encode_wav_base64(audio: np.ndarray, sample_rate: int) -> str:
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


def build_model(quantize: bool):
    emit_status("Importing MLX and CSM runtime...")
    from mlx import nn
    from csm_mlx import CSM, csm_1b

    weights_path = download_weights()
    emit_status("Initializing CSM model graph...")
    model = CSM(csm_1b())
    emit_status("Loading CSM weights into memory...")
    model.load_weights(weights_path, strict=True)

    if quantize:
        emit_status("Quantizing CSM model...")
        nn.quantize(model)

    return model


def load_reference_audio(context_audio: Path | None, read_audio) -> np.ndarray | None:
    if context_audio is None:
        return None

    emit_status(f"Loading voice reference from {context_audio.name}...")
    return read_audio(context_audio, SAMPLE_RATE)


def run_server(
    quantize: bool, context_audio: Path | None = None, context_text: str = ""
) -> int:
    try:
        emit_status("Importing generation helpers...")
        import mlx.core as mx
        from mlx_lm.sample_utils import make_sampler
        from csm_mlx import Segment, generate
        from csm_mlx.utils import read_audio
    except Exception as exc:
        emit({"type": "error", "message": f"Failed to import csm-mlx: {exc}"})
        traceback.print_exc(file=sys.stderr)
        return 1

    try:
        emit_status("Building CSM worker...")
        model = build_model(quantize)
    except Exception as exc:
        emit({"type": "error", "message": f"Failed to load CSM model: {exc}"})
        traceback.print_exc(file=sys.stderr)
        return 1

    emit_status("CSM worker ready.")
    emit({"type": "ready", "sample_rate": SAMPLE_RATE})
    reference_audio = None
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
            except Exception as exc:
                emit({"type": "error", "message": f"Failed to load context audio: {exc}"})
                traceback.print_exc(file=sys.stderr)
            continue
        if request_type == "reset_context":
            continue
        if request_type == "finalize_response":
            continue
        if request_type != "synthesize":
            emit({"type": "error", "message": f"Unsupported request type: {request_type}"})
            continue

        request_id = int(request.get("request_id"))
        speaker = MODEL_SPEAKER
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
            sampler = make_sampler(
                temp=float(request.get("temperature", 0.8)),
                top_k=int(request.get("top_k", 50)),
            )
            context = []
            if reference_audio is not None:
                context.append(
                    Segment(
                        speaker=speaker,
                        text=context_text,
                        audio=reference_audio,
                    )
                )
            audio = generate(
                model,
                text=text,
                speaker=speaker,
                context=context,
                max_audio_length_ms=int(request.get("max_audio_length_ms", 10_000)),
                sampler=sampler,
            )
            audio_wav_base64 = encode_wav_base64(audio, SAMPLE_RATE)
            if audio_wav_base64:
                emit(
                    {
                        "type": "chunk",
                        "request_id": request_id,
                        "audio_wav_base64": audio_wav_base64,
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
            mx.clear_cache()
            gc.collect()

    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--download", action="store_true")
    parser.add_argument("--server", action="store_true")
    parser.add_argument("--quantize", action="store_true")
    parser.add_argument("--context-audio", type=Path)
    parser.add_argument("--context-text", default="")
    args = parser.parse_args()

    if args.download:
        try:
            path = download_weights()
            emit({"type": "downloaded", "path": path})
            return 0
        except Exception as exc:
            emit({"type": "error", "message": f"Failed to download CSM weights: {exc}"})
            traceback.print_exc(file=sys.stderr)
            return 1

    if args.server:
        return run_server(args.quantize, args.context_audio, args.context_text)

    parser.error("Expected either --download or --server")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
