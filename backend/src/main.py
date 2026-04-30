import sys
import json
import signal
import queue
import sounddevice as sd
import numpy as np
import wave
import os
import threading
import sys
from vad_engine import VADEngine

# Configuration
SAMPLE_RATE = 16000
CHUNK_DURATION_MS = 32  # 32ms is optimal for Silero (512 samples at 16kHz)
CHUNK_SIZE = int(SAMPLE_RATE * CHUNK_DURATION_MS / 1000)

audio_queue = queue.Queue(maxsize=50)

def audio_callback(indata, frames, time, status):
    if status:
        print(json.dumps({"type": "error", "message": str(status)}), flush=True)
    
    try:
        audio_queue.put_nowait(indata.copy())
    except queue.Full:
        # Drop oldest chunk to maintain low latency
        try:
            audio_queue.get_nowait()
            audio_queue.put_nowait(indata.copy())
            # Optional: log warning occasionally, but keep stdout clean for IPC
        except queue.Empty:
            pass

def handle_sigterm(signum, frame):
    print(json.dumps({"type": "shutdown", "message": "Received SIGTERM"}), flush=True)
    sys.exit(0)

def main():
    signal.signal(signal.SIGTERM, handle_sigterm)
    signal.signal(signal.SIGINT, handle_sigterm)

    import os
    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    model_path = os.path.join(base_dir, "models", "silero_vad.onnx")
    
    # Enforce line buffering for clean pipe communication
    try:
        sys.stdout.reconfigure(line_buffering=True)
    except AttributeError:
        # Fallback for environments where reconfigure is not available
        pass
    
    vad = VADEngine(model_path=model_path, sampling_rate=SAMPLE_RATE)
    
    utterance_buffer = []
    telemetry_counter = 0
    
    def async_save_wav(audio_data, path, sample_rate):
        try:
            # Convert float32 [-1.0, 1.0] to int16
            audio_int16 = (audio_data * 32767).astype(np.int16)
            with wave.open(path, 'wb') as wf:
                wf.setnchannels(1)
                wf.setsampwidth(2) # 16-bit
                wf.setframerate(sample_rate)
                wf.writeframes(audio_int16.tobytes())
        except Exception as e:
            print(json.dumps({"type": "error", "message": f"WAV Export Failed: {str(e)}"}), flush=True)

    print(json.dumps({"type": "system", "message": "VAD engine initialized. Listening..."}), flush=True)

    try:
        with sd.InputStream(samplerate=SAMPLE_RATE, channels=1, dtype='float32', blocksize=CHUNK_SIZE, callback=audio_callback):
            while True:
                chunk = audio_queue.get()
                # Process 1D array
                chunk_1d = chunk.flatten()
                
                event, amplitude, frequency = vad.process_chunk(chunk_1d)
                
                # Manage utterance buffer
                if event == "speech_start":
                    utterance_buffer = [chunk_1d]
                elif vad.is_speaking:
                    utterance_buffer.append(chunk_1d)
                
                # Emit telemetry (throttled to every 2 chunks ~64ms)
                telemetry_counter += 1
                if telemetry_counter >= 2:
                    print(json.dumps({
                        "type": "telemetry",
                        "amplitude": amplitude,
                        "frequency": frequency
                    }), flush=True)
                    telemetry_counter = 0
                
                # Emit VAD events
                if event:
                    print(json.dumps({
                        "type": event
                    }), flush=True)
                    
                    if event == "speech_end" and utterance_buffer:
                        # Save to temp wav file asynchronously
                        try:
                            audio_data = np.concatenate(utterance_buffer)
                            temp_path = os.path.join(base_dir, "temp_utterance.wav")
                            
                            # Move disk I/O to background thread
                            thread = threading.Thread(
                                target=async_save_wav, 
                                args=(audio_data, temp_path, SAMPLE_RATE)
                            )
                            thread.daemon = True
                            thread.start()
                            
                            utterance_buffer = []
                        except Exception as e:
                            print(json.dumps({"type": "error", "message": f"Async WAV Prep Failed: {str(e)}"}), flush=True)

    except Exception as e:
        print(json.dumps({"type": "error", "message": str(e)}), flush=True)
        sys.exit(1)

if __name__ == "__main__":
    main()
