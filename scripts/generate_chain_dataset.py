import os
import sys
import json
import urllib.request
import urllib.error
import time
import argparse

def load_gemini_key():
    env_path = "temp/.env"
    if not os.path.exists(env_path):
        for cand in ["../temp/.env", "../../temp/.env"]:
            if os.path.exists(cand):
                env_path = cand
                break
                
    gemini_key = ""
    if os.path.exists(env_path):
        with open(env_path, "r") as f:
            for line in f:
                line = line.strip()
                if line.startswith("GEMINI_API_KEY="):
                    gemini_key = line.split("=")[1].strip('"\'')
                    break
    return gemini_key

GEMINI_KEY = load_gemini_key()

def call_gemini(messages, model="gemini-2.5-flash-lite", retry_count=3):
    if not GEMINI_KEY:
        print("[ERROR] GEMINI_API_KEY is not available in temp/.env!", flush=True)
        return None
        
    url = "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {GEMINI_KEY}"
    }
    payload = {
        "model": model,
        "messages": messages,
        "max_tokens": 4096,
        "temperature": 0.25,
        "stream": False
    }
    
    for attempt in range(retry_count):
        req = urllib.request.Request(url, data=json.dumps(payload).encode("utf-8"), headers=headers)
        try:
            with urllib.request.urlopen(req, timeout=120) as response:
                body = response.read().decode("utf-8")
                res_json = json.loads(body)
                content = res_json["choices"][0]["message"]["content"]
                return content
        except urllib.error.HTTPError as e:
            err_msg = e.read().decode("utf-8")
            print(f"  [GEMINI ERROR] HTTP {e.code} on attempt {attempt+1}: {err_msg}", flush=True)
        except Exception as e:
            print(f"  [GEMINI ERROR] Exception on attempt {attempt+1}: {e}", flush=True)
            
        if attempt < retry_count - 1:
            wait_time = 2 ** attempt * 3
            print(f"  Retrying in {wait_time}s...", flush=True)
            time.sleep(wait_time)
            
    return None

def call_ollama(messages, model="gemma4:12b", url="http://100.86.62.14:11434", retry_count=3):
    # Using native Ollama /api/chat with full 256k context window and ample prediction space
    endpoint = f"{url}/api/chat"
    headers = {
        "Content-Type": "application/json"
    }
    timeout_secs = 300
    payload = {
        "model": model,
        "messages": messages,
        "stream": False,
        "options": {
            "num_ctx": 262144, # Full 256k context window
            "num_predict": 16384, # Massive generation headroom
            "temperature": 0.2
        }
    }
    
    for attempt in range(retry_count):
        req = urllib.request.Request(endpoint, data=json.dumps(payload).encode("utf-8"), headers=headers)
        try:
            print(f"    Sending API request to native Ollama /api/chat (256k context window, Timeout: {timeout_secs}s)...", flush=True)
            start_time = time.time()
            with urllib.request.urlopen(req, timeout=timeout_secs) as response:
                body = response.read().decode("utf-8")
                res_json = json.loads(body)
                content = res_json["message"]["content"]
                elapsed = time.time() - start_time
                print(f"    Ollama generated response in {elapsed:.1f}s (length: {len(content)} chars)", flush=True)
                return content
        except urllib.error.HTTPError as e:
            err_msg = e.read().decode("utf-8")
            print(f"  [OLLAMA ERROR] HTTP {e.code} on attempt {attempt+1}: {err_msg}", flush=True)
        except Exception as e:
            print(f"  [OLLAMA ERROR] Exception on attempt {attempt+1}: {e}", flush=True)
            
        if attempt < retry_count - 1:
            wait_time = 2 ** attempt * 3
            print(f"  Retrying in {wait_time}s...", flush=True)
            time.sleep(wait_time)
            
    return None

SYSTEM_PROMPT = """You are generating a multi-session conversational simulation dataset for testing Vox, a real-time voice assistant with cognitive memory.

Vox has three memory layers:
1. Working Memory (active conversation FIFO in RAM).
2. Time-Windowed Context Chaining (loads chronological text contexts from database).
3. Personal Memory Graph (stores facts categorized under 10 collections: Context, Constraints, Identity, Preferences, Relationships, Skills, Projects, Tasks, Goals, Experiences).

You are generating a dataset segment of exactly 20 consecutive turns. Each turn represents a simple, natural conversation turn between user and assistant:
- `turn`: integer representing the sequential turn index in the active session.
- `user`: a short, natural prompt (under 25 words) from the user.
- `assistant`: a natural response from the assistant.

CRITICAL INSTRUCTIONS FOR NATURAL FLOW:
- Do NOT overfit. The conversations must feel like an everyday personal companion interaction over days and weeks.
- Include everyday chatter (greetings, quick task confirmations, generic "thanks", "awesome") which will test the Hot-Path query classifier.
- Cover real-world topics: daily routines, culinary hobbies (e.g., trying sourdough, making lasagna), language learning (e.g., Spanish, Japanese), roommates, active coding projects (e.g., building Vox in Rust), travel, reading sci-fi, stargazing.
- Maintain subtle threads across sessions. For example, if the user mentions working on a Rust module or studying Spanish in an earlier session, they should naturally mention updates or make references to those activities in subsequent sessions.
- Build deep long-term connections. For example:
  - In Session 1, the user mentions living in Chicago.
  - In Session 4, the user mentions moving to Seattle. This is a fact update (contradiction) testing USER_SUPERSEDES / CONFLICTS.
  - In Session 6, the user probes: "Hey, do you remember which city I am based in now?"
  - In Session 2, the user mentions learning Spanish.
  - In Session 5, the user mentions starting Japanese.
  - In Session 8, the user mentions giving up Japanese but continuing Spanish.

CRITICAL THINKING / REASONING CONSTRAINT:
- KEEP YOUR INTERNAL THINKING/REASONING EXTREMELY CONCISE AND SHORT (UNDER 150 WORDS).
- Do NOT draft or write out the dialog turns in your thinking block. Just briefly outline the topics/facts in 2-3 sentences and then immediately proceed to output the JSON content.

CRITICAL OUTPUT FORMAT RULES:
- Output MUST be a valid JSON array of exactly 20 objects.
- Each object MUST have exactly these keys: "turn", "user", "assistant".
- Do NOT wrap the JSON inside markdown code blocks (such as ```json or ```).
- Do NOT output any intro, explanation, or conversational text before or after the JSON.
- Start your response immediately with `[` and end with `]`.

EXAMPLE VALID FORMAT TO DUPLICATE:
[
  {
    "turn": 1,
    "user": "Hey Vox, I just moved to Chicago today!",
    "assistant": "Welcome to Chicago! How is the new place coming along?"
  },
  {
    "turn": 2,
    "user": "Remember when I told you I lived in Seattle?",
    "assistant": "Yes, I remember you mentioning Seattle. But now you are in Chicago!"
  }
]
"""

def clean_json_string(s):
    cleaned = s.strip()
    if cleaned.startswith("```"):
        if "\n" in cleaned:
            cleaned = cleaned[cleaned.find("\n"):].strip()
    if cleaned.endswith("```"):
        cleaned = cleaned[:-3].strip()
    if cleaned.startswith("json"):
        cleaned = cleaned[4:].strip()
    return cleaned

def generate_session(session_num, previous_sessions_data, provider, model, ollama_url):
    """
    Generates a full session of 100 turns in 5 segmental steps of 20 turns each.
    """
    print(f"\n============================================================", flush=True)
    print(f"  GENERATING SESSION {session_num} via {provider.upper()}", flush=True)
    print(f"============================================================", flush=True)
    
    session_turns = []
    
    for segment in range(5):
        start_turn = segment * 20 + 1
        end_turn = (segment + 1) * 20
        print(f"  -> Batch {segment+1}/5: Turns {start_turn} to {end_turn} using {model}...", flush=True)
        
        # Build prompt messages
        messages = [
            {"role": "system", "content": SYSTEM_PROMPT}
        ]
        
        # Ingest past sessions context
        context_msg = "Here is the context of past generated sessions:\n"
        if previous_sessions_data:
            for i, prev_sess in enumerate(previous_sessions_data[-3:]):
                prev_num = len(previous_sessions_data) - len(previous_sessions_data[-3:]) + i + 1
                context_msg += f"\n--- Session {prev_num} ---\n"
                context_msg += json.dumps(prev_sess[:25], indent=2) + "\n... (remaining turns omitted for brevity) ...\n"
        else:
            context_msg += "This is the very first session (Session 1). Set up the user persona, active projects, daily routines, culinary efforts, and initial facts here."
            
        messages.append({"role": "user", "content": context_msg})
        
        # Ingest current session history so far (FULL cumulative history)
        curr_session_msg = f"We are currently generating Session {session_num}.\n"
        if session_turns:
            curr_session_msg += f"Here are all the turns generated in the current session so far:\n"
            curr_session_msg += json.dumps(session_turns, indent=2) + "\n"
        else:
            curr_session_msg += "This is the start of the current session.\n"
            
        curr_session_msg += f"\nNow, generate the next exactly 20 turns (Turns {start_turn} to {end_turn}). Ensure they flow perfectly from previous turns. KEEP YOUR THINKING VERY CONCISE (UNDER 150 WORDS). Return ONLY a valid JSON array of exactly 20 turn objects. Start immediately with `[` and end with `]`. Absolutely no other text."
        
        messages.append({"role": "user", "content": curr_session_msg})
        
        if provider == "ollama":
            raw_output = call_ollama(messages, model=model, url=ollama_url)
        else:
            raw_output = call_gemini(messages, model=model)
            
        if not raw_output:
            print(f"  [ERROR] Failed to generate batch {segment+1} for session {session_num}!", flush=True)
            sys.exit(1)
            
        cleaned = clean_json_string(raw_output)
        
        # Robust parsing fallback
        turns_data = None
        try:
            turns_data = json.loads(cleaned)
        except Exception as primary_err:
            # Fallback 1: check if it's missing surrounding array brackets [ and ]
            if cleaned.startswith("{") and cleaned.endswith("}"):
                print("  [WARN] Output started with '{' and ended with '}'. Attempting robust array-wrapping fallback...", flush=True)
                try:
                    wrapped = "[" + cleaned + "]"
                    turns_data = json.loads(wrapped)
                except Exception as wrap_err:
                    print(f"  [ERROR] Array-wrapping fallback also failed: {wrap_err}", flush=True)
            
            if turns_data is None:
                print(f"  [ERROR] JSON parsing failed: {primary_err}", flush=True)
                os.makedirs("temp", exist_ok=True)
                with open(f"temp/failed_session_{session_num}_batch_{segment+1}.txt", "w") as f:
                    f.write(raw_output)
                sys.exit(1)
                
        try:
            if not isinstance(turns_data, list):
                raise ValueError("Parsed output is not a JSON list")
            if len(turns_data) != 20:
                print(f"  [WARN] Expected exactly 20 turns, got {len(turns_data)}. Normalizing turns...", flush=True)
                
            # Assign correct sequential turn indices
            for idx, turn_item in enumerate(turns_data):
                turn_item["turn"] = start_turn + idx
                
            session_turns.extend(turns_data)
            print(f"  [SUCCESS] Batch {segment+1}/5 complete.", flush=True)
        except Exception as e:
            print(f"  [ERROR] Turn processing failed: {e}", flush=True)
            sys.exit(1)
            
    # Save the finalized 100 turns session
    out_dir = "tests"
    if not os.path.exists(out_dir):
        out_dir = "app/src-tauri/tests"
        
    os.makedirs(out_dir, exist_ok=True)
    out_path = os.path.join(out_dir, f"dataset_session{session_num}.json")
    with open(out_path, "w") as f:
        json.dump(session_turns, f, indent=2)
    print(f"======> Saved Session {session_num} (Total {len(session_turns)} turns) to {out_path} <======", flush=True)
    return session_turns

def main():
    parser = argparse.ArgumentParser(description="Segmental Chained Dataset Generator using Gemini or Ollama")
    parser.add_argument("--sessions", type=int, default=10, help="Number of sessions to generate (default: 10)")
    parser.add_argument("--provider", type=str, choices=["gemini", "ollama"], default="ollama", help="API Provider (default: ollama)")
    parser.add_argument("--model", type=str, default="gemma4:12b", help="Model name (default: gemma4:12b for Ollama)")
    parser.add_argument("--url", type=str, default="http://100.86.62.14:11434", help="Ollama Server URL (default: http://100.86.62.14:11434)")
    args = parser.parse_args()
    
    model_name = args.model
    if args.provider == "gemini" and args.model == "gemma4:12b":
        model_name = "gemini-2.5-flash-lite"
        
    print(f"[DatasetGen] Starting segmental chained generation using {args.provider.upper()}...", flush=True)
    print(f"Target sessions to generate: {args.sessions}", flush=True)
    print(f"Model: {model_name}", flush=True)
    if args.provider == "ollama":
        print(f"Ollama Server URL: {args.url}", flush=True)
    
    all_sessions_data = []
    
    out_dir = "tests"
    if not os.path.exists(out_dir):
        out_dir = "app/src-tauri/tests"
        
    for s in range(1, args.sessions + 1):
        target_path = os.path.join(out_dir, f"dataset_session{s}.json")
        
        # Premium feature: Skip generation if the target session file already exists!
        if os.path.exists(target_path):
            print(f"[DatasetGen] File already exists for Session {s}. Loading existing data and skipping generation...", flush=True)
            try:
                with open(target_path, "r") as f:
                    session_turns = json.load(f)
                    all_sessions_data.append(session_turns)
                    continue
            except Exception as e:
                print(f"[DatasetGen] Failed to load existing Session {s} ({e}). Re-generating...", flush=True)
                
        session_turns = generate_session(s, all_sessions_data, args.provider, model_name, args.url)
        all_sessions_data.append(session_turns)
        
    print(f"\n[DatasetGen] Successfully verified/generated {args.sessions} sessions under {out_dir}/ directory!", flush=True)

if __name__ == "__main__":
    main()
