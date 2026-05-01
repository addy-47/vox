import sys
import json
import signal
import queue
import sounddevice as sd
import numpy as np
import wave
import os
import threading
import time
from vad_engine import VADEngine
from stt_engine import STTEngine

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
        except queue.Empty:
            pass

class TranscriptionWorker(threading.Thread):
    def __init__(self, stt_engine):
        super().__init__(daemon=True)
        self.stt = stt_engine
        self.buffer = []
        self.lock = threading.Lock()
        self.is_speaking = False
        self.final_signal = threading.Event()
        self.running = True

    def run(self):
        while self.running:
            if self.is_speaking:
                # Periodic partial transcription
                audio_data = None
                with self.lock:
                    if len(self.buffer) > 0:
                        audio_data = np.concatenate(self.buffer)
                
                if audio_data is not None:
                    try:
                        text = self.stt.transcribe(audio_data)
                        if text:
                            print(json.dumps({"type": "transcript_partial", "text": text}), flush=True)
                    except Exception as e:
                        print(json.dumps({"type": "error", "message": f"Partial STT Error: {str(e)}"}), flush=True)
                
                # Wait 500ms or until final signal
                signaled = self.final_signal.wait(timeout=0.5)
                
                if signaled:
                    # Final transcription
                    audio_data = None
                    with self.lock:
                        if len(self.buffer) > 0:
                            audio_data = np.concatenate(self.buffer)
                        self.buffer = [] # Clear buffer for next turn
                    
                    if audio_data is not None:
                        try:
                            text = self.stt.transcribe(audio_data)
                            if text:
                                print(json.dumps({"type": "transcript_final", "text": text}), flush=True)
                        except Exception as e:
                            print(json.dumps({"type": "error", "message": f"Final STT Error: {str(e)}"}), flush=True)
                    
                    self.is_speaking = False
                    self.final_signal.clear()
            else:
                time.sleep(0.1)

    def start_utterance(self):
        with self.lock:
            self.buffer = []
        self.is_speaking = True
        self.final_signal.clear()

    def add_audio(self, chunk):
        if self.is_speaking:
            with self.lock:
                self.buffer.append(chunk)

    def end_utterance(self):
        self.final_signal.set()

def handle_sigterm(signum, frame):
    print(json.dumps({"type": "shutdown", "message": "Received SIGTERM"}), flush=True)
    sys.exit(0)

def main():
    signal.signal(signal.SIGTERM, handle_sigterm)
    signal.signal(signal.SIGINT, handle_sigterm)

    base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    vad_model_path = os.path.join(base_dir, "models", "silero_vad.onnx")
    stt_models_dir = os.path.join(base_dir, "models", "moonshine", "tiny")
    
    # Enforce line buffering for clean pipe communication
    try:
        sys.stdout.reconfigure(line_buffering=True)
    except AttributeError:
        pass
    
    # Initialize Engines
    try:
        vad = VADEngine(model_path=vad_model_path, sampling_rate=SAMPLE_RATE)
    except Exception as e:
        print(json.dumps({"type": "error", "message": f"VAD Init Failed: {str(e)}"}), flush=True)
        sys.exit(1)

    stt_worker = None
    try:
        stt_engine = STTEngine(models_dir=stt_models_dir)
        stt_worker = TranscriptionWorker(stt_engine)
        stt_worker.start()
    except Exception as e:
        # Clean error if models are missing or init fails, as per mandate
        print(json.dumps({"type": "error", "message": f"STT Engine unavailable: {str(e)}"}), flush=True)
        # We continue, but STT features will be disabled
    
    telemetry_counter = 0
    
    print(json.dumps({"type": "system", "message": "Vox backend initialized. Ready."}), flush=True)

    try:
        with sd.InputStream(samplerate=SAMPLE_RATE, channels=1, dtype='float32', blocksize=CHUNK_SIZE, callback=audio_callback):
            while True:
                chunk = audio_queue.get()
                chunk_1d = chunk.flatten()
                
                event, amplitude, frequency = vad.process_chunk(chunk_1d)
                
                # Manage STT Worker and Buffering
                if event == "speech_start":
                    if stt_worker:
                        stt_worker.start_utterance()
                        stt_worker.add_audio(chunk_1d)
                elif vad.is_speaking:
                    if stt_worker:
                        stt_worker.add_audio(chunk_1d)
                
                if event == "speech_end":
                    if stt_worker:
                        stt_worker.end_utterance()
                
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
                    print(json.dumps({"type": event}), flush=True)

    except Exception as e:
        print(json.dumps({"type": "error", "message": f"Runtime Error: {str(e)}"}), flush=True)
        sys.exit(1)

if __name__ == "__main__":
    main()
