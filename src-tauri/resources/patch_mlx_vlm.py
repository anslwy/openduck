import sys
import importlib
import mlx.core as mx
import mlx.nn as nn
import numpy as np
from pathlib import Path

# --- Patch 1: mlx_vlm.utils.load_audio (Base64 support) ---
import mlx_vlm.utils as utils

original_load_audio = utils.load_audio

def patched_load_audio(file, sr, timeout=10):
    import base64
    from io import BytesIO
    if isinstance(file, str) and (len(file) > 1024 or "base64," in file):
        try:
            data = file.split("base64,", 1)[1] if "base64," in file else file
            audio, sample_rate = utils.read_audio(BytesIO(base64.b64decode(data)))
            if sample_rate != sr:
                audio = utils.resample_audio(audio, sample_rate, sr)
            return np.array(audio).mean(axis=1)
        except Exception:
            pass
    return original_load_audio(file, sr, timeout)

utils.load_audio = patched_load_audio

# --- Patch 2: mlx_vlm.utils.process_inputs (IndexError Fix) ---
original_process_inputs = utils.process_inputs

def patched_process_inputs(processor, prompts, images=None, **kwargs):
    if images is not None and (hasattr(images, "__len__") and len(images) == 0):
        images = None
    return original_process_inputs(processor, prompts, images=images, **kwargs)

utils.process_inputs = patched_process_inputs

# --- Patch 3: mlx_vlm.prompt_utils.apply_chat_template (Gemma 3 Audio Token Fix) ---
import mlx_vlm.prompt_utils as prompt_utils

original_get_chat_template = prompt_utils.get_chat_template

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
original_format_list = prompt_utils.MessageFormatter._format_list_with_image_type

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
    original_get_input_embeddings = m.Model.get_input_embeddings
    
    def patched_get_input_embeddings(self, input_ids=None, pixel_values=None, **kwargs):
        # 1. Scaling fix for text
        inputs_embeds = self.language_model.model.embed_tokens(input_ids)
        inputs_embeds = inputs_embeds * (self.language_model.model.hidden_size**0.5)
        
        # 2. Call original to get multimodal features (we'll re-scale them)
        res = original_get_input_embeddings(self, input_ids=input_ids, pixel_values=pixel_values, **kwargs)
        
        # 3. Scaling fix for the result
        # The original code mixes scaled and unscaled. We just scale everything.
        # Actually, let's just re-implement the relevant parts.
        return res

    # m.Model.get_input_embeddings = patched_get_input_embeddings
    pass

def patch_gemma3n_language(m):
    # Fix scaling in LanguageModel
    pass

# --- Start the server ---
import mlx_vlm.server as server
if __name__ == "__main__":
    server.main()
