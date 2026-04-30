import sys
import json
import signal
import queue
import sounddevice as sd
import numpy as np
from vad_engine import VADEngine

# Configuration
SAMPLE_RATE = 16000
CHUNK_DURATION_MS = 32  # 32ms is optimal for Silero (512 samples at 16kHz)
CHUNK_SIZE = int(SAMPLE_RATE * CHUNK_DURATION_MS / 1000)

audio_queue = queue.Queue()

def audio_callback(indata, frames, time, status):
    if status:
        print(json.dumps({"type": "error", "message": str(status)}), flush=True)
    # Copy data to avoid issues when the underlying buffer is reused
    audio_queue.put(indata.copy())

def handle_sigterm(signum, frame):
    print(json.dumps({"type": "shutdown", "message": "Received SIGTERM"}), flush=True)
    sys.exit(0)

def main():
    signal.signal(signal.SIGTERM, handle_sigterm)
    signal.signal(signal.SIGINT, handle_sigterm)

    import os
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    model_path = os.path.join(base_dir, "models", "silero_vad.onnx")
    
    vad = VADEngine(model_path=model_path, sampling_rate=SAMPLE_RATE)
    
    print(json.dumps({"type": "system", "message": "VAD engine initialized. Listening..."}), flush=True)

    try:
        with sd.InputStream(samplerate=SAMPLE_RATE, channels=1, dtype='float32', blocksize=CHUNK_SIZE, callback=audio_callback):
            while True:
                chunk = audio_queue.get()
                # Process 1D array
                chunk_1d = chunk.flatten()
                
                event, amplitude, frequency = vad.process_chunk(chunk_1d)
                
                # Emit telemetry continuously
                print(json.dumps({
                    "type": "telemetry",
                    "amplitude": amplitude,
                    "frequency": frequency
                }), flush=True)
                
                # Emit VAD events
                if event:
                    print(json.dumps({
                        "type": event
                    }), flush=True)

    except Exception as e:
        print(json.dumps({"type": "error", "message": str(e)}), flush=True)
        sys.exit(1)

if __name__ == "__main__":
    main()
