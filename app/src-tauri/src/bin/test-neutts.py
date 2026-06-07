import os
import numpy as np
import soundfile as sf
from neutts import NeuTTS

def main():
    print("Initializing python NeuTTS...")
    
    # Paths
    home = os.path.expanduser("~")
    backbone_path = os.path.join(home, ".vox/models/tts/neutts-nano/neutts-nano-q8.gguf")
    decoder_path = os.path.join(home, ".vox/models/tts/neucodec-decoder/neucodec_decoder.safetensors")
    
    ref_codes_path = os.path.join(home, ".vox/models/tts/neutts-nano/voices/jo.npy")
    ref_txt_path = os.path.join(home, ".vox/models/tts/neutts-nano/voices/jo.txt")
    
    # Load reference transcript
    with open(ref_txt_path, "r", encoding="utf-8") as f:
        ref_text = f.read().strip()
        
    print(f"Loaded ref transcript: {ref_text}")
    
    # Load reference codes
    ref_codes = np.load(ref_codes_path)
    print(f"Loaded ref codes shape: {ref_codes.shape}")
    
    # Load NeuTTS model
    import random
    random.randint = lambda a, b: 42
    tts = NeuTTS(backbone_repo=backbone_path, codec_repo="neuphonic/neucodec", language="en-us")
    print("Model loaded successfully!")
    
    # Run inference
    target_text = "Hello, how are you doing today? The weather seems lovely."
    print(f"Synthesizing: '{target_text}'")
    
    tokens_str = tts._infer_ggml(ref_codes, ref_text, target_text)
    print("PYTHON GENERATED TOKENS STRING:", tokens_str)
    
    audio = tts.infer(target_text, ref_codes, ref_text)
    
    output_path = "/home/addy/projects/apps/vox/docs/benchmarks/audio_outputs/output_py_jo.wav"
    os.makedirs(os.path.dirname(output_path), exist_ok=True)
    sf.write(output_path, audio, 24000)
    print(f"Saved generated audio to {output_path}")

if __name__ == "__main__":
    main()
