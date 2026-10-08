#!/bin/bash
set -eo pipefail

if [ "$#" -ne 2 ]; then
    echo "[Error] Usage: $0 <remote_path> <server_port>"
    exit 1
fi

REMOTE_SANDBOX_DIR="$1"
SERVER_PORT="$2"

REMOTE_WORK_DIR="${REMOTE_SANDBOX_DIR}/chatterbox-rs"
REMOTE_MODEL_DIR="${REMOTE_SANDBOX_DIR}/models/tts/chatterbox"
REMOTE_LOG_DIR="${REMOTE_SANDBOX_DIR}/logs"
REMOTE_LOG_FILE="${REMOTE_LOG_DIR}/server.log"

echo "=== Phase 1: Setup Workspace directories ==="
# Remove any symlink to models and create directories
[ -L "${REMOTE_SANDBOX_DIR}/models" ] && rm -f "${REMOTE_SANDBOX_DIR}/models"
mkdir -p "${REMOTE_SANDBOX_DIR}" "${REMOTE_MODEL_DIR}" "${REMOTE_LOG_DIR}"
echo "[+] Sandbox workspace directories set up successfully."

echo "=== Phase 2: Sync Code via Git on Remote ==="
if [ ! -d "${REMOTE_WORK_DIR}/.git" ]; then
    echo "[*] Cloning repository from https://github.com/addy-47/chatterbox-rs.git into ${REMOTE_WORK_DIR}..."
    rm -rf "${REMOTE_WORK_DIR}"
    git clone --recursive https://github.com/addy-47/chatterbox-rs.git "${REMOTE_WORK_DIR}"
else
    echo "[*] Pulling latest changes from git..."
    cd "${REMOTE_WORK_DIR}"
    git fetch origin
    git reset --hard origin/main
    git submodule update --init --recursive
fi
echo "[+] Code synchronized successfully via Git."

echo "=== Phase 3: Sync Models to Remote ==="
models=("chatterbox-t3-mtl-q4_0.gguf" "chatterbox-s3gen-mtl-f16.gguf")
urls=(
    "https://huggingface.co/addyo07/vox-models/resolve/main/tts/chatterbox/chatterbox-t3-mtl-q4_0.gguf"
    "https://huggingface.co/addyo07/vox-models/resolve/main/tts/chatterbox/chatterbox-s3gen-mtl-f16.gguf"
)

for i in "${!models[@]}"; do
    model_name="${models[$i]}"
    url="${urls[$i]}"
    target_file="${REMOTE_MODEL_DIR}/${model_name}"
    
    echo "[*] Checking model ${model_name}..."
    # Perform download if missing or incomplete
    if [ ! -f "${target_file}" ]; then
        echo "[*] Downloading ${model_name} directly to remote GPU server..."
        wget -c -O "${target_file}" "${url}"
        echo "[+] Model ${model_name} downloaded successfully."
    else
        echo "[+] Model ${model_name} already exists. Skipping download."
    fi
done

echo "=== Phase 4: Build Server on Remote (CUDA Auto-detect) ==="
# Detect Cargo/CUDA bin path or auto-install Rust
export PATH="$HOME/.cargo/bin:/usr/local/cuda/bin:$PATH"
export LD_LIBRARY_PATH="/usr/local/cuda/lib64:$LD_LIBRARY_PATH"

if [ -f "$HOME/.cargo/env" ]; then
    source "$HOME/.cargo/env"
fi

if ! command -v cargo &> /dev/null; then
    echo "[*] Cargo/Rust not found on remote server. Attempting automatic installation via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
    fi
    export PATH="$HOME/.cargo/bin:$PATH"
    if ! command -v cargo &> /dev/null; then
        echo "[-] Failed to auto-install Rust. Please install Rust manually (https://rustup.rs/)."
        exit 1
    fi
    echo "[+] Rust/Cargo installed successfully."
fi

# Detect CUDA capability
HAS_CUDA=false
if command -v nvidia-smi &> /dev/null || command -v nvcc &> /dev/null || [ -d "/usr/local/cuda" ]; then
    HAS_CUDA=true
    echo "[+] CUDA capability detected."
else
    echo "[*] CUDA not detected. Falling back to CPU-only execution."
fi

cd "${REMOTE_WORK_DIR}"
if [ "$HAS_CUDA" = true ]; then
    echo "[*] Compiling server with CUDA feature enabled..."
    cargo build --release --features cuda,server --example tts_server
else
    echo "[*] Compiling server in CPU mode..."
    cargo build --release --features server --example tts_server
fi
echo "[+] Server built successfully."

echo "=== Phase 5: Launch Server ==="
echo "[*] Killing existing tts_server instances..."
pkill -f tts_server || true
sleep 1

# Launch in background, saving PID, adapting gpu-layers to capability
if [ "$HAS_CUDA" = true ]; then
    echo "[*] Launching server with GPU layers enabled (gpu-layers = 99)..."
    nohup "${REMOTE_WORK_DIR}/target/release/examples/tts_server" \
        --t3-gguf "${REMOTE_MODEL_DIR}/chatterbox-t3-mtl-q4_0.gguf" \
        --s3gen-gguf "${REMOTE_MODEL_DIR}/chatterbox-s3gen-mtl-f16.gguf" \
        --gpu-layers 99 \
        --port "${SERVER_PORT}" > "${REMOTE_LOG_FILE}" 2>&1 &
else
    echo "[*] Launching server in CPU-only mode (gpu-layers = 0)..."
    nohup "${REMOTE_WORK_DIR}/target/release/examples/tts_server" \
        --t3-gguf "${REMOTE_MODEL_DIR}/chatterbox-t3-mtl-q4_0.gguf" \
        --s3gen-gguf "${REMOTE_MODEL_DIR}/chatterbox-s3gen-mtl-f16.gguf" \
        --gpu-layers 0 \
        --port "${SERVER_PORT}" > "${REMOTE_LOG_FILE}" 2>&1 &
fi
sleep 2
echo "[+] Launch command issued."

echo "=== Phase 6: Health Check ==="
url="http://127.0.0.1:${SERVER_PORT}/health"
echo "[*] Polling local server endpoint ${url}..."

success=false
for attempt in {1..30}; do
    if curl -s -f "${url}" | grep -q '"status":"ok"'; then
        echo "[+] Health check passed! Server is active."
        success=true
        break
    fi
    sleep 2
done

if [ "$success" = false ]; then
    echo "[-] Health check timed out. Showing last 30 lines of remote log:"
    tail -n 30 "${REMOTE_LOG_FILE}"
    exit 1
fi

echo "=== Phase 7: Smoke Test ==="
tts_url="http://127.0.0.1:${SERVER_PORT}/tts"
payload='{"text": "Hello from Chatterbox remote GPU server. Setup complete.", "language": "en"}'

echo "[*] Sending test synthesis request to local port..."
status_code=$(curl -s -o /dev/null -w "%{http_code}" -X POST -H "Content-Type: application/json" -d "${payload}" "${tts_url}")

if [ "${status_code}" -eq 200 ]; then
    echo "[+] Smoke test passed! Output received successfully (HTTP 200)."
else
    echo "[-] Smoke test failed with HTTP status: ${status_code}"
    exit 1
fi
