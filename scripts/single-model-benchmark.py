#!/usr/bin/env python3
import os
import sys
import json
import subprocess
import glob
import time
import argparse
from datetime import datetime

# Absolute paths
VOX_ROOT = "/home/addy/projects/apps/vox"
TAURI_DIR = os.path.join(VOX_ROOT, "app/src-tauri")
AUDIO_DIR = os.path.join(VOX_ROOT, "data/Corpus_extracted/Corpus/adult/audio/test_split")
BENCHMARKS_DIR = os.path.join(VOX_ROOT, "benchmarks")

WAV_FILES = [
    "AD09001.wav",
    "AD09004.wav",
    "AD09021.wav",
    "AD09039.wav",
    "AD09051.wav",
]

def get_latest_run_metrics():
    # Find newest run folder inside TAURI_DIR/outputs/run_* by sorting chronologically
    outputs_pattern = os.path.join(TAURI_DIR, "outputs", "run_*")
    run_folders = glob.glob(outputs_pattern)
    if not run_folders:
        return None
    latest_folder = sorted(run_folders)[-1]
    metrics_path = os.path.join(latest_folder, "metrics.json")
    
    # Wait up to 5 seconds for metrics.json to be written if there's any file buffering delay
    for _ in range(10):
        if os.path.exists(metrics_path):
            break
        time.sleep(0.5)
        
    if not os.path.exists(metrics_path):
        return None
    
    # Read transcripts
    stt_txt = ""
    llm_txt = ""
    stt_path = os.path.join(latest_folder, "stt_transcript.txt")
    llm_path = os.path.join(latest_folder, "llm_response.txt")
    if os.path.exists(stt_path):
        with open(stt_path, "r", encoding="utf-8") as f:
            stt_txt = f.read().strip()
    if os.path.exists(llm_path):
        with open(llm_path, "r", encoding="utf-8") as f:
            llm_txt = f.read().strip()
            
    with open(metrics_path, "r", encoding="utf-8") as f:
        data = json.load(f)
        
    return data, stt_txt, llm_txt

def main():
    parser = argparse.ArgumentParser(description="Vox Single Model Benchmark")
    parser.add_argument("--llm", type=str, required=True, help="LLF GGUF filename (e.g. llama/Llama-3.2-1B-Instruct-Q4_K_M.gguf)")
    args = parser.parse_args()
    
    llm_filename = args.llm
    llm_basename = os.path.basename(llm_filename)
    
    print("=" * 60)
    print("VOX 0.7.0 DYNAMIC MODEL BENCHMARK RUNNER")
    print(f"Model: {llm_filename}")
    print(f"Targeting: 5 WAV files sequentially from: {AUDIO_DIR}")
    print("=" * 60)
    
    os.makedirs(BENCHMARKS_DIR, exist_ok=True)
    
    runs = []
    
    for idx, fname in enumerate(WAV_FILES, 1):
        wav_path = os.path.join(AUDIO_DIR, fname)
        if not os.path.exists(wav_path):
            print(f"[Error] File not found: {wav_path}")
            continue
            
        file_size_kb = os.path.getsize(wav_path) / 1024.0
        
        print(f"\n[{idx}/5] Running benchmark on {fname} ({file_size_kb:.1f} KB)...")
        
        # Run cargo release command passing the LLM file dynamically
        cmd = ["cargo", "run", "--release", "--bin", "vox-bench", "--", "--input", wav_path, "--llm", llm_filename]
        
        start_time = time.time()
        process = subprocess.Popen(
            cmd,
            cwd=TAURI_DIR,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True
        )
        
        try:
            stdout, stderr = process.communicate(timeout=120)
            elapsed = time.time() - start_time
        except subprocess.TimeoutExpired:
            process.kill()
            stdout, stderr = process.communicate()
            print(f" -> Timeout (120s) reached for {fname}!")
            runs.append({
                "filename": fname,
                "file_size_kb": file_size_kb,
                "input_duration_sec": os.path.getsize(wav_path) / 32000.0,
                "stt_transcript": "TIMEOUT",
                "llm_response": "TIMEOUT",
                "stt_rtf": 0.0,
                "llm_tps": 0.0,
                "ttfa_sec": 0.0,
                "total_time_sec": 0.0,
                "peak_rss_mb": 0.0,
                "stt_ram_mb": 0.0,
                "llm_ram_mb": 0.0,
                "tts_ram_mb": 0.0
            })
            continue

        if process.returncode != 0:
            print(f"[Error] Command failed for {fname}")
            print(stderr)
            continue
            
        # Extract metrics
        res = get_latest_run_metrics()
        if not res:
            print(f"[Error] Failed to resolve metrics for {fname}")
            continue
            
        metrics, stt_trans, llm_resp = res
        
        # Correctly map JSON fields matching our actual metrics structure
        latency = metrics.get("latency", {})
        memory = metrics.get("memory_mb", {})
        throughput = metrics.get("throughput", {})
        
        stt_rtf = throughput.get("stt_rtf", 0.0)
        llm_tps = throughput.get("llm_tps", 0.0)
        ttfa = latency.get("ttfa_sec", 0.0)
        
        # Total active turn processing time
        tot_time = latency.get("stt_proc_sec", 0.0) + latency.get("llm_proc_sec", 0.0) + latency.get("tts_proc_sec", 0.0)
        
        peak_rss = memory.get("peak_process_rss_mb", 0.0)
        stt_ram = memory.get("stt", 0.0)
        llm_ram = memory.get("llm", 0.0)
        tts_ram = memory.get("tts", 0.0)
        
        # Standard Hindi dataset uses 16kHz mono audio -> 32KB/sec -> size in bytes / 32000
        input_dur = os.path.getsize(wav_path) / 32000.0
        
        runs.append({
            "filename": fname,
            "file_size_kb": file_size_kb,
            "input_duration_sec": input_dur,
            "stt_transcript": stt_trans,
            "llm_response": llm_resp,
            "stt_rtf": stt_rtf,
            "llm_tps": llm_tps,
            "ttfa_sec": ttfa,
            "total_time_sec": tot_time,
            "peak_rss_mb": peak_rss,
            "stt_ram_mb": stt_ram,
            "llm_ram_mb": llm_ram,
            "tts_ram_mb": tts_ram
        })
        
        print(f" -> Done in {elapsed:.1f}s | TTFA: {ttfa:.2f}s | STT RTF: {stt_rtf:.2f}x | LLM TPS: {llm_tps:.2f} | Peak RSS: {peak_rss:.0f}MB")
        
    if not runs:
        print("[Error] No runs successfully completed.")
        sys.exit(1)
        
    # Calculate Averages
    avg_stt_rtf = sum(r["stt_rtf"] for r in runs) / len(runs)
    avg_llm_tps = sum(r["llm_tps"] for r in runs) / len(runs)
    avg_ttfa = sum(r["ttfa_sec"] for r in runs) / len(runs)
    avg_total_time = sum(r["total_time_sec"] for r in runs) / len(runs)
    avg_peak_rss = sum(r["peak_rss_mb"] for r in runs) / len(runs)
    
    stt_ram_fixed = runs[0]["stt_ram_mb"]
    llm_ram_fixed = runs[0]["llm_ram_mb"]
    tts_ram_fixed = runs[0]["tts_ram_mb"]
    
    # Output JSON string block of averages and runs so parent process can parse it easily
    summary_data = {
        "model": llm_filename,
        "avg_stt_rtf": avg_stt_rtf,
        "avg_llm_tps": avg_llm_tps,
        "avg_ttfa": avg_ttfa,
        "avg_total_time": avg_total_time,
        "avg_peak_rss": avg_peak_rss,
        "stt_ram": stt_ram_fixed,
        "llm_ram": llm_ram_fixed,
        "tts_ram": tts_ram_fixed,
        "runs": runs
    }
    
    # Save a temporary JSON report for this specific model
    report_json_path = os.path.join(BENCHMARKS_DIR, f"temp_{llm_basename}.json")
    with open(report_json_path, "w", encoding="utf-8") as f:
        json.dump(summary_data, f, indent=2)
        
    print("\n" + "=" * 60)
    print(f"SUCCESS: BENCHMARK FOR {llm_basename} COMPLETED SUCCESSFULLY!")
    print(f"Metrics JSON written to: {report_json_path}")
    print("=" * 60)

if __name__ == "__main__":
    main()
