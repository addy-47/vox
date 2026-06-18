"""
Audio Comparison Script: Python (ground truth) vs C++ (magpie-tts.cpp)
Generates side-by-side mel spectrograms, waveform plots, amplitude envelopes,
and summary statistics for matched clip pairs.
"""
import numpy as np
import matplotlib
matplotlib.use('Agg')  # Non-interactive backend
import matplotlib.pyplot as plt
import scipy.io.wavfile as wavfile
import scipy.signal
import os
import json
import glob

PYTHON_DIR = "/home/addy/projects/apps/vox/temp/python_tts_outputs"
CPP_DIR = "/home/addy/projects/apps/vox/temp/cpp_tts_outputs"
OUT_DIR = "/home/addy/projects/apps/vox/temp/audio_comparison"
os.makedirs(OUT_DIR, exist_ok=True)


def load_wav(path):
    """Load wav file, return (samples_float32, sample_rate)."""
    sr, data = wavfile.read(path)
    if data.dtype == np.int16:
        data = data.astype(np.float32) / 32768.0
    elif data.dtype == np.int32:
        data = data.astype(np.float32) / 2147483648.0
    elif data.dtype == np.float64:
        data = data.astype(np.float32)
    # Mono
    if len(data.shape) > 1:
        data = data.mean(axis=1)
    return data, sr


def compute_mel_spectrogram(audio, sr, n_fft=1024, hop_length=256, n_mels=80):
    """Compute log-mel spectrogram."""
    # STFT
    f, t, Zxx = scipy.signal.stft(audio, fs=sr, nperseg=n_fft, noverlap=n_fft - hop_length)
    mag = np.abs(Zxx)

    # Mel filterbank
    fmin, fmax = 0.0, sr / 2.0
    mel_min = 2595.0 * np.log10(1.0 + fmin / 700.0)
    mel_max = 2595.0 * np.log10(1.0 + fmax / 700.0)
    mel_points = np.linspace(mel_min, mel_max, n_mels + 2)
    hz_points = 700.0 * (10.0 ** (mel_points / 2595.0) - 1.0)
    bin_points = np.floor((n_fft + 1) * hz_points / sr).astype(int)

    fbank = np.zeros((n_mels, n_fft // 2 + 1))
    for m in range(1, n_mels + 1):
        f_m_minus = bin_points[m - 1]
        f_m = bin_points[m]
        f_m_plus = bin_points[m + 1]
        for k in range(f_m_minus, f_m):
            if f_m != f_m_minus:
                fbank[m - 1, k] = (k - f_m_minus) / (f_m - f_m_minus)
        for k in range(f_m, f_m_plus):
            if f_m_plus != f_m:
                fbank[m - 1, k] = (f_m_plus - k) / (f_m_plus - f_m)

    mel_spec = np.dot(fbank, mag)
    log_mel = np.log(np.maximum(mel_spec, 1e-10))
    return log_mel


def compute_rms_envelope(audio, frame_length=1024, hop_length=256):
    """Compute RMS energy envelope."""
    n_frames = 1 + (len(audio) - frame_length) // hop_length
    rms = np.zeros(n_frames)
    for i in range(n_frames):
        start = i * hop_length
        frame = audio[start:start + frame_length]
        rms[i] = np.sqrt(np.mean(frame ** 2))
    return rms


def audio_stats(audio, sr):
    """Compute summary statistics."""
    duration = len(audio) / sr
    rms = np.sqrt(np.mean(audio ** 2))
    peak = np.max(np.abs(audio))
    zcr = np.sum(np.diff(np.sign(audio)) != 0) / len(audio)
    return {
        "duration_sec": round(duration, 3),
        "rms_energy": round(float(rms), 5),
        "peak_amplitude": round(float(peak), 5),
        "zero_crossing_rate": round(float(zcr), 5),
        "sample_rate": sr,
    }


def plot_comparison(py_audio, py_sr, cpp_audio, cpp_sr, clip_id, text, out_path):
    """Generate a 3x2 comparison figure."""
    fig, axes = plt.subplots(3, 2, figsize=(16, 12))
    fig.suptitle(f"Clip: {clip_id}\n\"{text[:80]}{'...' if len(text) > 80 else ''}\"",
                 fontsize=13, fontweight='bold')

    # Row 1: Waveforms
    t_py = np.arange(len(py_audio)) / py_sr
    t_cpp = np.arange(len(cpp_audio)) / cpp_sr

    axes[0, 0].plot(t_py, py_audio, linewidth=0.3, color='#2196F3')
    axes[0, 0].set_title("Python (NeMo) — Waveform", fontsize=11)
    axes[0, 0].set_ylabel("Amplitude")
    axes[0, 0].set_ylim(-1, 1)

    axes[0, 1].plot(t_cpp, cpp_audio, linewidth=0.3, color='#FF5722')
    axes[0, 1].set_title("C++ (magpie-tts.cpp) — Waveform", fontsize=11)
    axes[0, 1].set_ylabel("Amplitude")
    axes[0, 1].set_ylim(-1, 1)

    # Row 2: Mel Spectrograms
    py_mel = compute_mel_spectrogram(py_audio, py_sr)
    cpp_mel = compute_mel_spectrogram(cpp_audio, cpp_sr)

    vmin = min(py_mel.min(), cpp_mel.min())
    vmax = max(py_mel.max(), cpp_mel.max())

    axes[1, 0].imshow(py_mel, aspect='auto', origin='lower', cmap='magma',
                      vmin=vmin, vmax=vmax)
    axes[1, 0].set_title("Python — Mel Spectrogram", fontsize=11)
    axes[1, 0].set_ylabel("Mel Bin")

    axes[1, 1].imshow(cpp_mel, aspect='auto', origin='lower', cmap='magma',
                      vmin=vmin, vmax=vmax)
    axes[1, 1].set_title("C++ — Mel Spectrogram", fontsize=11)
    axes[1, 1].set_ylabel("Mel Bin")

    # Row 3: RMS Envelope
    py_rms = compute_rms_envelope(py_audio)
    cpp_rms = compute_rms_envelope(cpp_audio)

    t_py_rms = np.arange(len(py_rms)) * 256 / py_sr
    t_cpp_rms = np.arange(len(cpp_rms)) * 256 / cpp_sr

    axes[2, 0].plot(t_py_rms, py_rms, color='#2196F3', linewidth=1.0)
    axes[2, 0].set_title("Python — RMS Energy Envelope", fontsize=11)
    axes[2, 0].set_xlabel("Time (s)")
    axes[2, 0].set_ylabel("RMS")

    axes[2, 1].plot(t_cpp_rms, cpp_rms, color='#FF5722', linewidth=1.0)
    axes[2, 1].set_title("C++ — RMS Energy Envelope", fontsize=11)
    axes[2, 1].set_xlabel("Time (s)")
    axes[2, 1].set_ylabel("RMS")

    plt.tight_layout(rect=[0, 0, 1, 0.94])
    plt.savefig(out_path, dpi=150)
    plt.close()
    print(f"  Saved comparison plot: {out_path}")


def main():
    # Find matching clips
    py_files = sorted(glob.glob(os.path.join(PYTHON_DIR, "clip_*.wav")))
    cpp_files = sorted(glob.glob(os.path.join(CPP_DIR, "clip_*.wav")))

    py_ids = {os.path.basename(f).replace(".wav", ""): f for f in py_files}
    cpp_ids = {os.path.basename(f).replace(".wav", ""): f for f in cpp_files}

    common = sorted(set(py_ids.keys()) & set(cpp_ids.keys()))

    if not common:
        print("No matching clips found between Python and C++ output directories!")
        print(f"  Python clips: {list(py_ids.keys())}")
        print(f"  C++ clips: {list(cpp_ids.keys())}")
        return

    print(f"Found {len(common)} matching clips: {common}\n")

    all_stats = []

    for clip_id in common:
        py_path = py_ids[clip_id]
        cpp_path = cpp_ids[clip_id]

        print(f"Processing {clip_id}...")
        py_audio, py_sr = load_wav(py_path)
        cpp_audio, cpp_sr = load_wav(cpp_path)

        py_s = audio_stats(py_audio, py_sr)
        cpp_s = audio_stats(cpp_audio, cpp_sr)

        print(f"  Python:  duration={py_s['duration_sec']}s  rms={py_s['rms_energy']}  peak={py_s['peak_amplitude']}")
        print(f"  C++:     duration={cpp_s['duration_sec']}s  rms={cpp_s['rms_energy']}  peak={cpp_s['peak_amplitude']}")

        # Determine text from benchmark results
        text = clip_id
        for results_file in [
            os.path.join(CPP_DIR, "cpp_benchmark_results.json"),
            os.path.join(PYTHON_DIR, "python_benchmark_results.json"),
        ]:
            if os.path.exists(results_file):
                with open(results_file) as f:
                    for entry in json.load(f):
                        if entry.get("clip_id") == clip_id:
                            text = entry.get("text", clip_id)
                            break

        out_path = os.path.join(OUT_DIR, f"{clip_id}_comparison.png")
        plot_comparison(py_audio, py_sr, cpp_audio, cpp_sr, clip_id, text, out_path)

        all_stats.append({
            "clip_id": clip_id,
            "text": text,
            "python": py_s,
            "cpp": cpp_s,
            "duration_ratio": round(cpp_s["duration_sec"] / py_s["duration_sec"], 3) if py_s["duration_sec"] > 0 else 0,
        })

    # Save summary
    summary_path = os.path.join(OUT_DIR, "comparison_summary.json")
    with open(summary_path, "w") as f:
        json.dump(all_stats, f, indent=2)
    print(f"\nSaved comparison summary: {summary_path}")


if __name__ == "__main__":
    main()
