#!/usr/bin/env python3
"""
Analyze TTS output WAV for gaps, clipping, and quality metrics.
Usage: python3 analyze_tts_wav.py <path/to/output_tts.wav>
"""
import struct
import math
import sys
import os


def read_wav(path: str) -> tuple:
    """Read WAV file and return (samples: list[float], sample_rate: int)."""
    with open(path, 'rb') as f:
        data = f.read()

    # Parse WAV header
    # Find 'data' chunk
    pos = 12  # skip RIFF header
    sample_rate = 0
    bits_per_sample = 0
    
    while pos + 8 < len(data):
        chunk_id = data[pos:pos+4]
        chunk_size = struct.unpack('<I', data[pos+4:pos+8])[0]
        
        if chunk_id == b'fmt ':
            fmt_data = data[pos+8:pos+8+chunk_size]
            audio_fmt = struct.unpack('<H', fmt_data[0:2])[0]
            num_channels = struct.unpack('<H', fmt_data[2:4])[0]
            sample_rate = struct.unpack('<I', fmt_data[4:8])[0]
            bits_per_sample = struct.unpack('<H', fmt_data[14:16])[0]
            
        elif chunk_id == b'data':
            sample_bytes = data[pos+8:pos+8+chunk_size]
            samples = []
            bytes_per_sample = bits_per_sample // 8
            
            for i in range(0, len(sample_bytes), bytes_per_sample):
                if i + bytes_per_sample <= len(sample_bytes):
                    if bits_per_sample == 16:
                        s = struct.unpack('<h', sample_bytes[i:i+2])[0]
                        samples.append(s / 32768.0)
                    elif bits_per_sample == 32:
                        s = struct.unpack('<i', sample_bytes[i:i+4])[0]
                        samples.append(s / 2147483648.0)
                    elif bits_per_sample == 8:
                        s = struct.unpack('B', sample_bytes[i:i+1])[0]
                        samples.append((s - 128) / 128.0)
            
        pos += 8 + chunk_size

    if not sample_rate:
        print("ERROR: Could not parse WAV header")
        sys.exit(1)

    return samples, sample_rate


def analyze(wav_path: str):
    print(f"Analyzing: {wav_path}")
    print("=" * 60)
    
    samples, sample_rate = read_wav(wav_path)
    duration = len(samples) / sample_rate
    
    print(f"Sample Rate: {sample_rate} Hz")
    print(f"Total Samples: {len(samples)}")
    print(f"Duration: {duration:.3f}s")
    
    # ── Silence Detection ──────────────────────────────────────────
    frame_ms = 10
    frame_size = int(sample_rate * frame_ms / 1000)
    silence_threshold = 0.02  # RMS below this = silence
    
    frames = []
    for i in range(0, len(samples), frame_size):
        frame = samples[i:i+frame_size]
        if len(frame) > 0:
            rms = math.sqrt(sum(s*s for s in frame) / len(frame))
            frames.append((i, i + len(frame), rms))
    
    # Find non-silent region (trim leading/trailing silence)
    non_silent_start = 0
    for start, end, rms in frames:
        if rms > silence_threshold:
            non_silent_start = start
            break
    
    non_silent_end = len(samples)
    for start, end, rms in reversed(frames):
        if rms > silence_threshold:
            non_silent_end = end
            break
    
    non_silent_duration = (non_silent_end - non_silent_start) / sample_rate
    
    print(f"Non-silent region: {non_silent_start/sample_rate:.3f}s - {non_silent_end/sample_rate:.3f}s ({non_silent_duration:.3f}s)")
    print(f"Leading silence: {non_silent_start/sample_rate:.3f}s")
    print(f"Trailing silence: {(len(samples) - non_silent_end)/sample_rate:.3f}s")
    
    # ── Gap Detection (internal silent regions >= 100ms) ──────────
    gap_threshold_ms = 100
    gap_threshold_samples = int(sample_rate * gap_threshold_ms / 1000)
    
    in_silence = False
    gap_start = 0
    gaps = []
    
    for start, end, rms in frames:
        if start < non_silent_start or start >= non_silent_end:
            continue
        if rms < silence_threshold:
            if not in_silence:
                gap_start = start
                in_silence = True
        else:
            if in_silence:
                gap_duration = start - gap_start
                if gap_duration >= gap_threshold_samples:
                    gaps.append((gap_start, start, gap_duration))
                in_silence = False
    
    # Also handle gap at the end of non-silent region
    if in_silence:
        gap_duration = non_silent_end - gap_start
        if gap_duration >= gap_threshold_samples:
            gaps.append((gap_start, non_silent_end, gap_duration))
    
    num_gaps = len(gaps)
    total_gap_duration = sum(dur for _, _, dur in gaps) / sample_rate
    speech_duration = non_silent_duration - total_gap_duration
    
    print(f"\n── Gaps Analysis ──")
    print(f"Gap threshold: {gap_threshold_ms}ms")
    print(f"Number of gaps: {num_gaps}")
    print(f"Total gap time: {total_gap_duration:.3f}s ({total_gap_duration/non_silent_duration*100:.1f}% of non-silent)")
    print(f"Estimated speech time: {speech_duration:.3f}s")
    
    if gaps:
        print(f"\nGap listing:")
        for i, (start, end, dur) in enumerate(gaps):
            print(f"  {i+1:2d}. {start/sample_rate:.3f}s - {end/sample_rate:.3f}s ({dur/sample_rate:.3f}s)")
    
    # ── Clipping Detection ──────────────────────────────────────────
    # Check last 50ms for clipping
    tail_ms = 50
    tail_samples_len = int(sample_rate * tail_ms / 1000)
    tail = samples[-tail_samples_len:] if len(samples) > tail_samples_len else samples
    
    if tail:
        tail_peak = max(abs(s) for s in tail) if tail else 0
        tail_rms = math.sqrt(sum(s*s for s in tail) / len(tail)) if tail else 0
        print(f"\n── Clipping Analysis ──")
        print(f"Tail (last {tail_ms}ms): peak={tail_peak:.4f}, rms={tail_rms:.4f}")
        if tail_peak < 0.01:
            print(f"⚠️  WARNING: Tail is near-silent (peak={tail_peak:.4f}) — possible clipped ending")
        else:
            print(f"✅ Tail has audio content")
    
    # Check overall waveform for hard clipping (samples at ±1.0)
    clipped = sum(1 for s in samples if abs(s) >= 0.999)
    if clipped > 0:
        print(f"⚠️  {clipped} samples are hard-clipped (abs >= 0.999)")
    else:
        print(f"✅ No hard clipping detected")
    
    # ── Quality Summary ─────────────────────────────────────────────
    print(f"\n── Summary ──")
    print(f"  Input: {os.path.basename(wav_path)}")
    print(f"  Duration: {duration:.2f}s")
    print(f"  Speech (est.): {speech_duration:.2f}s")
    print(f"  Silence (internal gaps): {total_gap_duration:.2f}s")
    print(f"  Silence ratio: {total_gap_duration/duration*100:.1f}%" if duration > 0 else "  N/A")
    print(f"  Gaps >= {gap_threshold_ms}ms: {num_gaps}")
    
    return {
        "duration_s": duration,
        "speech_s": speech_duration,
        "gap_total_s": total_gap_duration,
        "gap_count": num_gaps,
        "non_silent_start_s": non_silent_start / sample_rate,
        "tail_peak": tail_peak if tail else 0,
        "tail_rms": tail_rms if tail else 0,
    }


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <path/to/output_tts.wav>")
        sys.exit(1)
    
    wav_path = sys.argv[1]
    if not os.path.exists(wav_path):
        print(f"ERROR: File not found: {wav_path}")
        sys.exit(1)
    
    analyze(wav_path)
