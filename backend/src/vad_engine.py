import numpy as np
import onnxruntime as ort
import os
import time

class VADEngine:
    def __init__(self, model_path="backend/models/silero_vad.onnx", threshold=0.5, sampling_rate=16000, min_silence_duration_ms=500):
        self.sampling_rate = sampling_rate
        self.threshold = threshold
        self.min_silence_duration_ms = min_silence_duration_ms
        self.min_silence_samples = (min_silence_duration_ms * sampling_rate) // 1000
        
        # Load ONNX model
        opts = ort.SessionOptions()
        opts.inter_op_num_threads = 1
        opts.intra_op_num_threads = 1
        self.session = ort.InferenceSession(model_path, providers=['CPUExecutionProvider'], sess_options=opts)
        
        self.reset_states()
        
        self.is_speaking = False
        self.silence_counter = 0
        self.last_speech_time = time.time()

    def reset_states(self):
        self._h = np.zeros((2, 1, 64)).astype('float32')
        self._c = np.zeros((2, 1, 64)).astype('float32')

    def maybe_reset(self):
        """Reset LSTM states only after 10 seconds of continuous silence"""
        if not self.is_speaking and (time.time() - self.last_speech_time) > 10:
            self.reset_states()

    def process_chunk(self, chunk: np.ndarray):
        """
        Processes a chunk of audio (1D numpy array, float32, 16kHz).
        Returns:
            event (str): "speech_start", "speech_end", or None
            amplitude (float): RMS amplitude
            frequency (float): Dominant frequency
        """
        # Calculate RMS amplitude
        rms = np.sqrt(np.mean(chunk**2))
        amplitude = float(rms)
        
        # Calculate dominant frequency using FFT
        if amplitude > 0.001:  # Only compute frequency if there's significant audio
            fft_data = np.fft.rfft(chunk)
            fft_freqs = np.fft.rfftfreq(len(chunk), d=1.0/self.sampling_rate)
            magnitudes = np.abs(fft_data)
            dominant_idx = np.argmax(magnitudes)
            frequency = float(fft_freqs[dominant_idx])
        else:
            frequency = 0.0

        # Run VAD
        ort_inputs = {
            'input': chunk.reshape(1, -1).astype('float32'),
            'sr': np.array([self.sampling_rate], dtype='int64'),
            'h': self._h,
            'c': self._c
        }
        
        ort_outs = self.session.run(None, ort_inputs)
        prob = ort_outs[0][0][0]
        self._h, self._c = ort_outs[1], ort_outs[2]
        
        event = None
        
        if prob >= self.threshold:
            self.silence_counter = 0
            self.last_speech_time = time.time()
            if not self.is_speaking:
                self.is_speaking = True
                event = "speech_start"
        else:
            if self.is_speaking:
                self.silence_counter += len(chunk)
                if self.silence_counter >= self.min_silence_samples:
                    self.is_speaking = False
                    event = "speech_end"
                    self.silence_counter = 0
            else:
                self.maybe_reset()

        return event, amplitude, frequency
