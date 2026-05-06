import argparse
import base64
import json
import os
import sys
import traceback
from io import BytesIO
from pathlib import Path

try:
    import numpy as np
    import onnxruntime as ort
    from transformers import WhisperFeatureExtractor
except ImportError as e:
    print(json.dumps({
        "type": "error",
        "message": f"Required Python library missing: {e}. Please run ./scripts/setup_python_env.sh"
    }))
    sys.exit(1)

sys.dont_write_bytecode = True

def emit(payload: dict) -> None:
    output = getattr(sys, "__stdout__", None) or sys.stdout
    output.write(json.dumps(payload) + "\n")
    output.flush()

def emit_status(message: str) -> None:
    emit({"type": "status", "message": message})

def truncate_audio_to_last_n_seconds(audio_array, n_seconds=8, sampling_rate=16000):
    max_samples = n_seconds * sampling_rate
    if len(audio_array) > max_samples:
        return audio_array[-max_samples:]
    elif len(audio_array) < max_samples:
        padding = np.zeros(max_samples - len(audio_array), dtype=np.float32)
        return np.concatenate((padding, audio_array))
    return audio_array

def load_model(model_path: str):
    emit_status(f"Loading Smart Turn model from {model_path}...")
    so = ort.SessionOptions()
    so.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
    so.inter_op_num_threads = 1
    so.graph_optimization_level = ort.GraphOptimizationLevel.ORT_ENABLE_ALL
    session = ort.InferenceSession(model_path, sess_options=so)

    feature_extractor = WhisperFeatureExtractor(chunk_length=8)
    return session, feature_extractor

def run_server(model_path: str) -> int:
    try:
        session, feature_extractor = load_model(model_path)
    except Exception as exc:
        emit({"type": "error", "message": f"Failed to load Smart Turn model: {exc}"})
        traceback.print_exc(file=sys.stderr)
        return 1

    emit_status("Smart Turn worker ready.")
    emit({"type": "ready"})

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

        if request_type == "predict":
            request_id = request.get("request_id")
            audio_base64 = request.get("audio_base64")
            threshold = request.get("threshold", 0.8)

            emit_status(f"Received prediction request {request_id} (threshold={threshold})")

            if not audio_base64:
                emit({"type": "error", "request_id": request_id, "message": "Missing audio_base64"})
                continue

            try:
                # Decode base64 audio (expected to be raw float32 PCM at 16kHz)
                audio_data = base64.b64decode(audio_base64)
                audio_array = np.frombuffer(audio_data, dtype=np.float32)

                # Basic signal check
                peak = np.max(np.abs(audio_array))
                if peak < 1e-4:
                    emit({
                        "type": "prediction",
                        "request_id": request_id,
                        "probability": 0.0,
                        "completed": False,
                        "note": "Silent input"
                    })
                    continue

                # Preprocess
                audio_array = truncate_audio_to_last_n_seconds(audio_array, n_seconds=8)

                inputs = feature_extractor(
                    audio_array,
                    sampling_rate=16000,
                    return_tensors="np",
                    padding="max_length",
                    max_length=8 * 16000,
                    truncation=True,
                    do_normalize=True,
                )

                input_features = inputs.input_features.squeeze(0).astype(np.float32)
                input_features = np.expand_dims(input_features, axis=0)

                # Inference
                outputs = session.run(None, {"input_features": input_features})
                raw_val = float(outputs[0][0].item())

                # The model outputs sigmoid probabilities, but let's be conservative.
                # Threshold moved to 0.7 to avoid false positives on mid-sentence pauses.
                probability = raw_val
                completed = probability > threshold

                emit({
                    "type": "prediction",
                    "request_id": request_id,
                    "probability": probability,
                    "completed": completed
                })

            except Exception as exc:
                emit({"type": "error", "request_id": request_id, "message": f"Prediction failed: {exc}"})
                traceback.print_exc(file=sys.stderr)

    return 0

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--model", required=True)
    args = parser.parse_args()
    sys.exit(run_server(args.model))
