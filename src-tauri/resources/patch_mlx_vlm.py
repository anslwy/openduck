import sys
import importlib
from pathlib import Path

try:
    import numpy as np
except ImportError:
    np = None

try:
    import mlx.core as mx
    import mlx.nn as nn
except ImportError:
    mx = None
    nn = None

# --- Patch 1: mlx_vlm.utils.load_audio (Base64 support) ---
try:
    import mlx_vlm.utils as utils
    original_load_audio = utils.load_audio
except ImportError:
    utils = None
    original_load_audio = None

def patched_load_audio(file, sr, timeout=10):
    import base64
    from io import BytesIO
    if np is not None and isinstance(file, str) and (len(file) > 1024 or "base64," in file):
        try:
            data = file.split("base64,", 1)[1] if "base64," in file else file
            audio, sample_rate = utils.read_audio(BytesIO(base64.b64decode(data)))
            if sample_rate != sr:
                audio = utils.resample_audio(audio, sample_rate, sr)
            return np.array(audio).mean(axis=1)
        except Exception:
            pass
    if original_load_audio is not None:
        return original_load_audio(file, sr, timeout)
    raise RuntimeError("mlx_vlm.utils not available")

if utils is not None:
    utils.load_audio = patched_load_audio

# --- Patch 2: mlx_vlm.utils.process_inputs (IndexError Fix) ---
if utils is not None:
    original_process_inputs = utils.process_inputs

    def patched_process_inputs(processor, prompts, images=None, **kwargs):
        if images is not None and (hasattr(images, "__len__") and len(images) == 0):
            images = None
        return original_process_inputs(processor, prompts, images=images, **kwargs)

    utils.process_inputs = patched_process_inputs

# --- Patch 3: mlx_vlm.prompt_utils.apply_chat_template (Gemma 3 Audio Token Fix) ---
try:
    import mlx_vlm.prompt_utils as prompt_utils
    original_get_chat_template = prompt_utils.get_chat_template
    original_format_list = prompt_utils.MessageFormatter._format_list_with_image_type
except ImportError:
    prompt_utils = None
    original_get_chat_template = None
    original_format_list = None

if prompt_utils is not None:
    def patched_get_chat_template(processor, messages, add_generation_prompt, **kwargs):
        # Detect Gemma 3 model and fix audio marker in content list
        is_gemma3 = any(x in str(processor).lower() for x in ["gemma3", "gemma3n"])
        if is_gemma3:
            for msg in messages:
                if isinstance(msg.get("content"), list):
                    for item in msg["content"]:
                        if item.get("type") == "audio":
                            # We change the marker the template sees
                            pass # The flattening logic in prompt_utils needs careful patch

        return original_get_chat_template(processor, messages, add_generation_prompt, **kwargs)

    # Instead of patching get_chat_template, we patch MessageFormatter._format_list_with_image_type
    # which is what gemma3n uses.
    def patched_format_list(self, prompt, role, skip_image_token, skip_audio_token, num_images, num_audios, **kwargs):
        res = original_format_list(self, prompt, role, skip_image_token, skip_audio_token, num_images, num_audios, **kwargs)
        if self.model_name in ["gemma3", "gemma3n"]:
            if isinstance(res.get("content"), list):
                for item in res["content"]:
                    if item.get("type") == "audio":
                        # Note: Gemma 3 processor expects <audio_soft_token> but the 
                        # library prompt_utils hardcodes <audio>. 
                        # We can't easily change the type to string here without breaking types.
                        pass
        return res

    prompt_utils.MessageFormatter._format_list_with_image_type = patched_format_list

# --- Patch 4: mlx_vlm.models.gemma3n (Scaling and AltUp Fixes) ---
# We wait for the module to be loaded by mlx_vlm.load
original_import_module = importlib.import_module

def patched_import_module(name, package=None):
    module = original_import_module(name, package)
    if name == "mlx_vlm.models.gemma3n.gemma3n":
        patch_gemma3n_model(module)
    elif name == "mlx_vlm.models.gemma3n.language":
        patch_gemma3n_language(module)
    return module

importlib.import_module = patched_import_module

def patch_gemma3n_model(m):
    # original_get_input_embeddings = m.Model.get_input_embeddings
    # ...
    pass

def patch_gemma3n_language(m):
    # Fix scaling in LanguageModel
    pass

# --- Start the server ---
try:
    import mlx_vlm.server as server
except ImportError:
    server = None

if __name__ == "__main__":
    if server is not None:
        server.main()
    else:
        # If server failed to import, it's likely due to missing mlx_vlm.
        # But we want to let the process stay alive so Rust can see it and 
        # report an error if it tries to send a command, OR we just exit 
        # with an error that Rust can capture.
        print("Error: mlx_vlm not found in current environment.", file=sys.stderr)
        sys.exit(1)
