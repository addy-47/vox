#!/usr/bin/env python3
import os
import sys
import subprocess
import time
import json
import glob
from pathlib import Path

# Paths
WORKSPACE_DIR = Path("/home/addy/projects/apps/vox")
TAURI_DIR = WORKSPACE_DIR / "app/src-tauri"
BENCHMARK_DIR = WORKSPACE_DIR / "benchmarks"
LOGS_DIR = BENCHMARK_DIR / "logs"
SUMMARY_FILE = BENCHMARK_DIR / "benchmarks_summary.md"

MODELS = [
    "gemma4/Gemma-4-E2B-Uncensored-HauhauCS-Aggressive-Q2_K_P.gguf",
    "gemma4/google_gemma-4-E2B-it-Q4_K_M.gguf",
    "llama/Llama-3.2-1B-Instruct-Q4_K_M.gguf",
    "llama/Llama-3.2-1B-Instruct-Q6_K.gguf",
    "llama/Llama-3.2-3B-Instruct-Q4_K_M.gguf",
    "llama/Llama-3.2-3B-Instruct-Q6_K_L.gguf",
    "NVIDIA-Nemotron3-Nano-4B-Q4_K_M.gguf",
    "qwen/qwen2.5-1.5b-instruct-q4_k_m.gguf",
    "qwen/qwen2.5-3b-instruct-q4_k_m.gguf",
    "qwen/Qwen3-4B-OBLITERATED.Q4_K_M.gguf"
]

CLIPS = [
    WORKSPACE_DIR / "data/Corpus_extracted/Corpus/adult/audio/test_split/AD09001.wav"
]

# Ensure directories exist
LOGS_DIR.mkdir(parents=True, exist_ok=True)

def build_binary():
    print(">>> Compiling vox-bench in release mode to isolate compilation time...")
    res = subprocess.run(
        ["cargo", "build", "--release", "--bin", "vox-bench"],
        cwd=TAURI_DIR,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True
    )
    if res.returncode != 0:
        print("ERROR: Compilation failed!")
        print(res.stderr)
        sys.exit(1)
    print(">>> Compilation complete! Proceeding with runs...")

def get_latest_run_dir():
    run_dirs = glob.glob(str(TAURI_DIR / "outputs/run_*"))
    if not run_dirs:
        return None
    return Path(max(run_dirs, key=os.path.getmtime))

def run_benchmark():
    build_binary()
    
    results = []
    
    # Initialize markdown summary file
    with open(SUMMARY_FILE, "w") as f:
        f.write("# Vox LLM Benchmarks Comparative Analysis\n\n")
        f.write(f"Generated on: {time.strftime('%Y-%m-%d %H:%M:%S')}\n\n")
        f.write(f"## ⚡ Benchmark Matrix ({len(MODELS)} Models x 1 Audio Clip)\n\n")
        f.write("| Run | Model | Clip | Status | Peak RSS (MB) | LLM TPS | TTFA (s) | Total (s) | STT RTF | Response Snippet |\n")
        f.write("| :--- | :--- | :--- | :--- | :---: | :---: | :---: | :---: | :---: | :--- |\n")
    
    run_idx = 1
    total_runs = len(MODELS) * len(CLIPS)
    
    for model in MODELS:
        model_name = Path(model).name
        for clip in CLIPS:
            clip_name = clip.name
            log_file = LOGS_DIR / f"{model_name}_{clip_name}.log"
            
            print(f"\n[{run_idx}/{total_runs}] Running: Model={model_name} | Clip={clip_name}")
            
            # Start timer
            start_time = time.time()
            
            # Run the pre-compiled binary directly to isolate compilation time
            cmd = [
                "./target/release/vox-bench",
                "--input", str(clip),
                "--llm", model
            ]
            
            status = "Success"
            peak_rss = "N/A"
            tps = "N/A"
            ttfa = "N/A"
            total_time = "N/A"
            stt_rtf = "N/A"
            response_snippet = "N/A"
            
            try:
                # 2 minutes timeout constraint as requested by the user
                res = subprocess.run(
                    cmd,
                    cwd=TAURI_DIR,
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                    text=True,
                    timeout=120
                )
                
                # Save execution logs
                with open(log_file, "w") as lf:
                    lf.write("=== STDOUT ===\n")
                    lf.write(res.stdout)
                    lf.write("\n=== STDERR ===\n")
                    lf.write(res.stderr)
                
                if res.returncode != 0:
                    status = f"Failed (Code {res.returncode})"
                    print(f"❌ Run failed! Check logs: {log_file}")
                else:
                    # Parse results from the outputs folder
                    latest_run = get_latest_run_dir()
                    if latest_run:
                        metrics_path = latest_run / "metrics.json"
                        response_path = latest_run / "llm_response.txt"
                        
                        if metrics_path.exists():
                            with open(metrics_path, "r") as mf:
                                metrics = json.load(mf)
                                
                            # Extract granular details
                            latency = metrics.get("latency", {})
                            throughput = metrics.get("throughput", {})
                            mem = metrics.get("memory_mb", {})
                            
                            peak_rss = mem.get("total", "N/A")
                            tps = f"{throughput.get('llm_tps', 0.0):.2f}"
                            ttfa = f"{latency.get('ttfa_sec', 0.0) or 0.0:.2f}"
                            total_time = f"{latency.get('llm_proc_sec', 0.0) or 0.0:.2f}"
                            
                            stt_rtf = f"{throughput.get('stt_rtf', 0.0):.2f}x"
                        
                        if response_path.exists():
                            with open(response_path, "r") as rf:
                                full_resp = rf.read().strip()
                                response_snippet = full_resp[:120].replace("\n", " ").replace("|", "\\|") + ("..." if len(full_resp) > 120 else "")
                        
                        # Clean up previous run directories to save disk space
                        import shutil
                        run_dirs = sorted(glob.glob(str(TAURI_DIR / "outputs/run_*")), key=os.path.getmtime)
                        for d in run_dirs[:-1]:
                            try:
                                shutil.rmtree(d)
                            except Exception:
                                pass
                    
                    print(f"✅ Success! RSS: {peak_rss}MB | TPS: {tps} | TTFA: {ttfa}s")
            
            except subprocess.TimeoutExpired as e:
                status = "Timeout (120s)"
                print(f"⚠️ Timeout (120s) reached for model {model_name}!")
                
                # Write partial outputs to logs
                with open(log_file, "w") as lf:
                    lf.write("=== TIMEOUT EXPIRED (120 SECONDS) ===\n")
                    lf.write(f"Stdout captured before timeout:\n{e.stdout or ''}\n")
                    lf.write(f"Stderr captured before timeout:\n{e.stderr or ''}\n")
            
            except Exception as ex:
                status = f"Error: {str(ex)}"
                print(f"❌ Error encountered: {ex}")
            
            # Append line to summary report
            with open(SUMMARY_FILE, "a") as f:
                f.write(f"| #{run_idx} | `{model_name}` | `{clip_name}` | **{status}** | {peak_rss} | {tps} | {ttfa} | {total_time} | {stt_rtf} | {response_snippet} |\n")
            
            run_idx += 1
            
            # Simple cooling sleep to let memory reclaim completely between runs
            time.sleep(2)

if __name__ == "__main__":
    run_benchmark()
