import argparse
import contextlib
import json
import os
import sys
import traceback
from pathlib import Path

from huggingface_hub import snapshot_download

sys.dont_write_bytecode = True


def emit(payload: dict) -> None:
    output = getattr(sys, "__stdout__", None) or sys.stdout
    output.write(json.dumps(payload) + "\n")
    output.flush()


def emit_status(message: str) -> None:
    emit({"type": "status", "message": message})


@contextlib.contextmanager
def redirect_library_stdout():
    with contextlib.redirect_stdout(sys.stderr):
        yield


def resolve_local_model_path(repo_id: str) -> Path:
    emit_status("Resolving Whisper model files...")
    try:
        with redirect_library_stdout():
            snapshot_path = snapshot_download(repo_id=repo_id, local_files_only=True)
    except Exception as exc:
        raise RuntimeError(
            f"{repo_id} is not downloaded. Download it in the app before loading the STT worker."
        ) from exc

    return Path(snapshot_path)


def load_stt_model(repo_id: str):
    emit_status("Importing MLX-Audio runtime...")
    from mlx_audio.stt.utils import load_model

    model_path = resolve_local_model_path(repo_id)
    emit_status("Loading Whisper model into memory...")
    with redirect_library_stdout():
        model = load_model(str(model_path))

    return model


def extract_transcription_text(result) -> str:
    if hasattr(result, "text"):
        return str(result.text).strip()

    if isinstance(result, dict):
        return str(result.get("text", "")).strip()

    return str(result).strip()


def run_server(repo_id: str) -> int:
    try:
        model = load_stt_model(repo_id)
    except Exception as exc:
        emit({"type": "error", "message": f"Failed to load Whisper STT model: {exc}"})
        traceback.print_exc(file=sys.stderr)
        return 1

    emit_status("Whisper STT worker ready.")
    emit({"type": "ready"})

    for raw_line in sys.stdin:
        line = raw_line.strip()
        if not line:
            continue

        try:
            request = json.loads(line)
        except json.JSONDecodeError as exc:
            emit({"type": "error", "message": f"Invalid STT worker request JSON: {exc}"})
            continue

        request_type = request.get("type")
        if request_type == "shutdown":
            emit_status("Shutting down Whisper STT worker.")
            return 0

        request_id = request.get("request_id")
        if request_type != "transcribe":
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": f"Unsupported STT worker request type: {request_type}",
                }
            )
            continue

        audio_path = request.get("audio_path")
        if not audio_path:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": "Missing audio_path for STT request.",
                }
            )
            continue

        emit_status(f"Transcribing {Path(str(audio_path)).name}...")

        try:
            with redirect_library_stdout():
                result = model.generate(
                    audio=str(audio_path),
                    verbose=False,
                    condition_on_previous_text=False,
                    temperature=0.0,
                )
            emit(
                {
                    "type": "transcription",
                    "request_id": request_id,
                    "text": extract_transcription_text(result),
                }
            )
        except Exception as exc:
            emit(
                {
                    "type": "error",
                    "request_id": request_id,
                    "message": f"Failed to transcribe audio: {exc}",
                }
            )
            traceback.print_exc(file=sys.stderr)

    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--server", action="store_true")
    parser.add_argument("--model", required=True)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    if not args.server:
        raise SystemExit("Only --server mode is supported.")

    return run_server(str(args.model))


if __name__ == "__main__":
    raise SystemExit(main())
