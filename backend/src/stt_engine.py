import os
import numpy as np
import moonshine_onnx

class STTEngine:
    def __init__(self, models_dir=None):
        """
        Initializes the Moonshine STT Engine.
        models_dir: Path to the directory containing Moonshine ONNX models.
                    Expected structure:
                    models_dir/
                      preprocess.onnx
                      encode.onnx
                      decode.onnx
                      ...
        """
        if models_dir is None:
            # Default path relative to project root
            base_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
            models_dir = os.path.join(base_dir, "models", "moonshine", "tiny")

        self.models_dir = models_dir
        
        # Verify model files exist
        required_files = ["preprocess.onnx", "encode.onnx", "decode.onnx", "uncased.txt"]
        if not os.path.exists(models_dir):
            raise FileNotFoundError(f"Moonshine model directory not found: {models_dir}")
        
        missing_files = [f for f in required_files if not os.path.exists(os.path.join(models_dir, f))]
        if missing_files:
            raise FileNotFoundError(f"Missing Moonshine model files in {models_dir}: {', '.join(missing_files)}")

        print(f"Loading Moonshine STT from {models_dir}...")
        try:
            # Initialize the model
            self.model = moonshine_onnx.MoonshineOnnxModel(models_dir=models_dir)
        except Exception as e:
            raise RuntimeError(f"Failed to initialize Moonshine STT: {str(e)}")

    def transcribe(self, audio_data: np.ndarray) -> str:
        """
        Transcribes audio data (1D float32 numpy array, 16kHz).
        Returns the transcription text.
        """
        if len(audio_data) == 0:
            return ""
        
        # Moonshine expects [1, num_samples]
        audio_input = audio_data.reshape(1, -1).astype(np.float32)
        
        try:
            # The 'generate' method returns a list of strings (one for each batch item)
            results = self.model.generate(audio_input)
            if results and len(results) > 0:
                return results[0].strip()
        except Exception as e:
            print(f"Transcription error: {str(e)}")
            return ""
            
        return ""
