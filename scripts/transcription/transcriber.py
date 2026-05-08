# type: ignore
import os
os.environ['TORCH_USE_CUDA_DSA'] = '0'

"""
Robust Whisper Transcription Script
Handles long audio files (2+ hours) with detailed progress tracking
"""

import os
import sys
import json
import time
import logging
import traceback
from pathlib import Path
from datetime import datetime, timedelta
from typing import List, Dict, Optional

import torch
from faster_whisper import WhisperModel
from faster_whisper import BatchedInferencePipeline
from tqdm import tqdm
import ffmpeg


# Force unbuffered output for real-time logging
sys.stdout.reconfigure(line_buffering=True)
sys.stderr.reconfigure(line_buffering=True)

# ==================== CONFIGURATION ====================
INPUT_DIR = "data/audiobooks-hi"
OUTPUT_DIR = "transcripts"
MODEL_SIZE = "large-v3-turbo"
DEVICE = "cuda"  # or "cpu" if GPU issues
COMPUTE_TYPE = "float16"
BEAM_SIZE = 1
VAD_FILTER = True
CHUNK_LENGTH = 30
BATCH_SIZE = 24

INITIAL_PROMPT = """
Casual Hindi-English conversational speech from an audiobook.
Preserve slang, Hinglish, and code-switching exactly as spoken.
Do not add annotations like [laughter] or [music].
Maintain high fidelity for audiobook dataset generation.
"""

# File processing limits
MAX_FILES = None  # Set to number to limit, e.g., 1 for testing
SKIP_EXISTING = True  # Skip already transcribed files

# ==================== LOGGING SETUP ====================
def setup_logging():
    """Configure comprehensive logging"""
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    
    log_file = os.path.join(OUTPUT_DIR, f"transcription_{datetime.now().strftime('%Y%m%d_%H%M%S')}.log")
    
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(levelname)s - %(message)s',
        handlers=[
            logging.FileHandler(log_file),
            logging.StreamHandler(sys.stdout)
        ]
    )
    return logging.getLogger(__name__)

logger = setup_logging()

# ==================== SYSTEM CHECK ====================
def check_system():
    """Verify CUDA, GPU memory, and dependencies"""
    logger.info("=" * 60)
    logger.info("SYSTEM CHECK")
    logger.info("=" * 60)
    
    # Check CUDA availability
    cuda_available = torch.cuda.is_available()
    logger.info(f"PyTorch CUDA available: {cuda_available}")
    
    if cuda_available:
        logger.info(f"CUDA version: {torch.version.cuda}")
        logger.info(f"GPU count: {torch.cuda.device_count()}")
        
        for i in range(torch.cuda.device_count()):
            logger.info(f"GPU {i}: {torch.cuda.get_device_name(i)}")
            props = torch.cuda.get_device_properties(i)
            memory_gb = props.total_memory / 1024**3
            logger.info(f"  Memory: {memory_gb:.1f} GB")
            
            # Warn if low memory
            if memory_gb < 8 and MODEL_SIZE in ["large-v3", "large-v2"]:
                logger.warning(f"GPU {i} has only {memory_gb:.1f} GB. Large model may not fit!")
                logger.warning("Consider using 'medium' model or CPU fallback")
    
    # Check CPU cores
    cpu_count = os.cpu_count()
    logger.info(f"CPU cores: {cpu_count}")
    
    # Test model loading
    logger.info("Testing model loading (tiny model for verification)...")
    try:
        test_model = WhisperModel("tiny", device="cpu", compute_type="int8")
        logger.info("✓ Model loading test passed")
        del test_model
    except Exception as e:
        logger.error(f"✗ Model loading test failed: {e}")
        logger.error("Check your faster-whisper installation")
        return False
    
    logger.info("=" * 60)
    return True

# ==================== MODEL LOADING ====================
def load_model():
    """Load Whisper model and wrap in BatchedInferencePipeline"""
    logger.info(f"Loading Whisper model: {MODEL_SIZE}")
    logger.info(f"Device: {DEVICE}, Compute type: {COMPUTE_TYPE}")
    
    try:
        start_time = time.time()
        
        model = WhisperModel(
            MODEL_SIZE,
            device=DEVICE,
            compute_type=COMPUTE_TYPE,
            cpu_threads=os.cpu_count(),
            num_workers=min(4, os.cpu_count() // 2)
        )
        
        pipeline = BatchedInferencePipeline(
            model=model,
            use_vad_model=True
        )
        
        load_time = time.time() - start_time
        logger.info(f"✓ Pipeline initialized successfully in {load_time:.2f} seconds")
        return pipeline
        
    except Exception as e:
        logger.error(f"✗ Failed to load model: {e}")
        logger.error(traceback.format_exc())
        
        # Fallback to CPU if GPU fails
        if DEVICE == "cuda":
            logger.warning("Attempting fallback to CPU...")
            try:
                fallback_model = WhisperModel(
                    MODEL_SIZE,
                    device="cpu",
                    compute_type="int8"
                )
                fallback_pipeline = BatchedInferencePipeline(
                    model=fallback_model,
                    use_vad_model=True
                )
                logger.warning("Running on CPU (will be much slower)")
                return fallback_pipeline
            except Exception as fallback_error:
                logger.error(f"CPU fallback also failed: {fallback_error}")
                raise
        
        raise

# ==================== AUDIO VALIDATION ====================
def get_audio_info(filepath: str) -> Dict:
    """Extract audio file information"""
    try:
        probe = ffmpeg.probe(filepath)
        audio_stream = next(
            (stream for stream in probe['streams'] if stream['codec_type'] == 'audio'),
            None
        )
        
        if not audio_stream:
            return {'error': 'No audio stream found'}
        
        duration = float(audio_stream.get('duration', 0))
        bitrate = audio_stream.get('bit_rate', 'unknown')
        sample_rate = audio_stream.get('sample_rate', 'unknown')
        
        return {
            'duration': duration,
            'duration_formatted': str(timedelta(seconds=int(duration))),
            'bitrate': bitrate,
            'sample_rate': sample_rate,
            'size_gb': os.path.getsize(filepath) / (1024**3)
        }
    except Exception as e:
        logger.warning(f"Could not get audio info: {e}")
        return {'error': str(e), 'duration': 0}

# ==================== TRANSCRIPTION WITH PROGRESS ====================
class TranscriptionProgress:
    """Track transcription progress for long files"""
    def __init__(self, total_duration: float):
        self.total_duration = total_duration
        self.processed_duration = 0
        self.segment_count = 0
        self.start_time = time.time()
        self.last_update = self.start_time
        
    def update(self, segment_end: float):
        """Update progress based on segment end time"""
        self.processed_duration = max(self.processed_duration, segment_end)
        self.segment_count += 1
        
        # Update every 5 seconds to avoid spam
        now = time.time()
        if now - self.last_update >= 5:
            progress_pct = (self.processed_duration / self.total_duration) * 100
            elapsed = now - self.start_time
            
            # Estimate remaining time
            if progress_pct > 0:
                eta_seconds = (elapsed / progress_pct) * (100 - progress_pct)
                eta = str(timedelta(seconds=int(eta_seconds)))
            else:
                eta = "calculating..."
            
            logger.info(f"  Progress: {progress_pct:.1f}% | "
                      f"Time: {str(timedelta(seconds=int(self.processed_duration)))} / "
                      f"{str(timedelta(seconds=int(self.total_duration)))} | "
                      f"Segments: {self.segment_count} | ETA: {eta}")
            
            self.last_update = now

def transcribe_file(pipeline: BatchedInferencePipeline, filepath: str, audio_info: Dict) -> Dict:
    """Transcribe audio file using BatchedInferencePipeline"""
    logger.info(f"Starting batched transcription: {os.path.basename(filepath)}")
    logger.info(f"  Duration: {audio_info['duration_formatted']}")
    logger.info(f"  File size: {audio_info.get('size_gb', 0):.2f} GB")
    
    try:
        start_time = time.time()
        
        # Run batched transcription
        segments_gen, info = pipeline.transcribe(
            filepath,
            batch_size=BATCH_SIZE,
            beam_size=BEAM_SIZE,
            vad_filter=VAD_FILTER,
            chunk_length=CHUNK_LENGTH,
            word_timestamps=True,
            condition_on_previous_text=False,
            initial_prompt=INITIAL_PROMPT
        )
        
        logger.info(f"  Detected language: {info.language} "
                   f"(probability: {info.language_probability:.2f})")
        
        all_segments = []
        all_words = []
        full_text = []
        
        for segment in segments_gen:
            seg_dict = {
                "start": round(segment.start, 2),
                "end": round(segment.end, 2),
                "text": segment.text.strip(),
                "avg_logprob": getattr(segment, 'avg_logprob', None)
            }
            all_segments.append(seg_dict)
            full_text.append(segment.text.strip())
            
            if hasattr(segment, 'words') and segment.words:
                for word in segment.words:
                    all_words.append({
                        "word": word.word.strip(),
                        "start": round(word.start, 2),
                        "end": round(word.end, 2),
                        "probability": round(word.probability, 4)
                    })
        
        transcribe_time = time.time() - start_time
        realtime_factor = transcribe_time / audio_info['duration'] if audio_info['duration'] > 0 else 0
        
        logger.info(f"✓ Transcription completed in {transcribe_time:.1f} seconds")
        logger.info(f"  Real-time factor: {realtime_factor:.2f}x")
        
        return {
            "text_raw": " ".join(full_text),
            "words": all_words,
            "segments": all_segments
        }
        
    except Exception as e:
        logger.error(f"✗ Transcription failed: {e}")
        logger.error(traceback.format_exc())
        raise

# ==================== MAIN PROCESSING LOOP ====================
def main():
    """Main processing function"""
    logger.info("=" * 60)
    logger.info("WHISPER TRANSCRIPTION SYSTEM")
    logger.info(f"Started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    logger.info("=" * 60)
    
    # Check system
    if not check_system():
        logger.error("System check failed. Exiting.")
        sys.exit(1)
    
    # Create directories
    os.makedirs(INPUT_DIR, exist_ok=True)
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    
    # Find audio files
    audio_extensions = ['.mp3', '.wav', '.m4a', '.flac', '.ogg']
    files = []
    for ext in audio_extensions:
        files.extend(Path(INPUT_DIR).glob(f"*{ext}"))
    
    files = sorted(files)
    logger.info(f"Found {len(files)} audio files in {INPUT_DIR}")
    
    if not files:
        logger.error(f"No audio files found in {INPUT_DIR}")
        logger.info(f"Supported formats: {', '.join(audio_extensions)}")
        sys.exit(1)
    
    # Apply file limit if specified
    if MAX_FILES:
        files = files[:MAX_FILES]
        logger.info(f"Limiting to first {MAX_FILES} files")
    
    # Load model once
    model = load_model()
    
    # Process each file
    successful = 0
    skipped = 0
    failed = 0
    
    for idx, filepath in enumerate(tqdm(files, desc="Overall Progress", unit="file"), 1):
        logger.info(f"\n{'='*60}")
        logger.info(f"Processing file {idx}/{len(files)}: {filepath.name}")
        logger.info(f"{'='*60}")
        
        output_path = Path(OUTPUT_DIR) / f"{filepath.stem}.json"
        
        # Skip if output exists
        if SKIP_EXISTING and output_path.exists():
            logger.info(f"Output already exists: {output_path}")
            logger.info("Skipping (use SKIP_EXISTING=False to override)")
            skipped += 1
            continue
        
        # Get audio info
        audio_info = get_audio_info(str(filepath))
        if 'error' in audio_info:
            logger.error(f"Cannot read audio file: {audio_info['error']}")
            failed += 1
            continue
        
        try:
            # Transcribe
            result_data = transcribe_file(model, str(filepath), audio_info)
            
            # Combine with metadata
            output_data = {
                "metadata": {
                    "file": filepath.name,
                    "model": MODEL_SIZE,
                    "device": DEVICE,
                    "compute_type": COMPUTE_TYPE,
                    "transcription_date": datetime.now().isoformat(),
                    "audio_info": audio_info,
                    "beam_size": BEAM_SIZE,
                    "vad_filter": VAD_FILTER,
                    "chunk_length": CHUNK_LENGTH,
                    "batch_size": BATCH_SIZE
                },
                **result_data
            }
            
            with open(output_path, "w", encoding="utf-8") as f:
                json.dump(output_data, f, indent=2, ensure_ascii=False)
            
            logger.info(f"✓ Saved transcription to: {output_path}")
            logger.info(f"  Total text length: {len(output_data['text_raw'])} characters")
            
            successful += 1
            
        except Exception as e:
            logger.error(f"✗ Failed to process {filepath.name}: {e}")
            logger.error(traceback.format_exc())
            failed += 1
            
            # Save error info
            error_path = Path(OUTPUT_DIR) / f"{filepath.stem}_ERROR.json"
            with open(error_path, "w") as f:
                json.dump({
                    "file": filepath.name,
                    "error": str(e),
                    "timestamp": datetime.now().isoformat()
                }, f, indent=2)
    
    # Final summary
    logger.info("\n" + "="*60)
    logger.info("PROCESSING COMPLETE")
    logger.info("="*60)
    logger.info(f"Total files: {len(files)}")
    logger.info(f"✓ Successful: {successful}")
    logger.info(f"⏭ Skipped: {skipped}")
    logger.info(f"✗ Failed: {failed}")
    logger.info(f"Output directory: {OUTPUT_DIR}")
    logger.info(f"Finished at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    logger.info("="*60)

if __name__ == "__main__":
    try:
        main()
    except KeyboardInterrupt:
        logger.warning("\n\nUser interrupted. Exiting gracefully...")
        sys.exit(0)
    except Exception as e:
        logger.error(f"Fatal error: {e}")
        logger.error(traceback.format_exc())
        sys.exit(1)