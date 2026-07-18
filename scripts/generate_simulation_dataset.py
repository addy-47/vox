import json
import os
import re
import urllib.request
import urllib.parse
import sys
import time

def load_env_key():
    candidates = [
        "temp/.env",
        "../temp/.env",
        "../../temp/.env",
        "app/src-tauri/temp/.env"
    ]
    for p in candidates:
        if os.path.exists(p):
            with open(p, "r") as f:
                for line in f:
                    if line.startswith("NVIDIA_API_KEY="):
                        return line.strip().split("=")[1].strip('"\'')
    return os.environ.get("NVIDIA_API_KEY", "")

API_KEY = load_env_key()
if not API_KEY:
    print("Error: NVIDIA_API_KEY not found in temp/.env or environment.")
    sys.exit(1)

API_URL = "https://integrate.api.nvidia.com/v1/chat/completions"

# We will use meta/llama-3.1-70b-instruct directly as our primary model for generation
PRIMARY_MODEL = "meta/llama-3.1-70b-instruct"

def call_nvidia(messages, model=PRIMARY_MODEL, max_tokens=4096):
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {API_KEY}"
    }
    data = {
        "model": model,
        "messages": messages,
        "max_tokens": max_tokens,
        "temperature": 0.2,
        "stream": True
    }
    req = urllib.request.Request(API_URL, data=json.dumps(data).encode("utf-8"), headers=headers)
    
    full_content = []
    start_time = time.time()
    try:
        # Stream the response line-by-line to prevent timeouts
        with urllib.request.urlopen(req, timeout=90) as response:
            for line in response:
                line = line.decode("utf-8").strip()
                if not line:
                    continue
                if line.startswith("data:"):
                    data_str = line[5:].strip()
                    if data_str == "[DONE]":
                        break
                    try:
                        chunk = json.loads(data_str)
                        delta = chunk["choices"][0]["delta"]
                        if "content" in delta:
                            content = delta["content"]
                            print(content, end="", flush=True)
                            full_content.append(content)
                    except Exception:
                        pass
        print()
        elapsed = time.time() - start_time
        print(f"  [API] Response streamed successfully in {elapsed:.1f}s")
        return "".join(full_content)
    except Exception as e:
        print(f"\n[Error] Failed calling Nvidia API ({model}): {e}")
        return None

SYSTEM_CONTEXT = """You are generating a multi-session conversational simulation dataset for testing Vox, a real-time voice assistant with cognitive memory.
Vox has three memory layers:
1. Working Memory (active conversation).
2. Episodic Memory (past conversation search).
3. Personal Memory (immutable graph profile).
Personal Memory does:
- Fact extraction (compaction on idle).
- 3-Pass Edge Resolution (USER_SUPERSEDES for pointer swaps; SUPPORTS for pulling in related facts; CONFLICTS for shadowing contradictions, newer wins).
- Token budgeting and round-robin collection interleaving.

Your task is to generate realistic, natural, and diverse multi-session conversations. Each turn must have a short user prompt and an expected assistant response.
Do NOT overfit. The conversation must cover diverse real-world topics (e.g. daily routine, technical projects, food, travel, work, hobbies) and feel like a user communicating with their personal desktop voice assistant over several days/weeks.

Across the sessions:
1. The user must state facts (e.g., identity details, coding/text preferences, active projects, hobbies).
2. The user must update facts in later sessions (e.g., Session 4: "I moved from Chicago to Seattle", Session 7: "I gave up Neovim and now use VS Code") to trigger CONFLICTS and USER_SUPERSEDES relations in the graph.
3. The user must ask questions in later sessions (probes) to test if the assistant recalls details from earlier sessions (e.g. "Do you remember where I live now?" or "What project was I building?").
4. The user must mention supporting details (e.g. "I am on a tight $500 travel budget" or "I am building a high-performance voice engine") and the assistant should leverage supporting facts (e.g. "I need cheap vacation ideas").

Format each session as a JSON array of turn objects:
[
  {
    "turn": 1,
    "user": "User prompt text",
    "assistant": "Expected ground truth assistant response containing the facts",
    "is_probe": true/false, // true if this turn explicitly tests memory recall of a previously stated fact
    "expected_facts": ["List of ground truth facts that must be recalled"] // only present/needed if is_probe is true
  }
]
Keep user prompt text short and natural (under 25 words). Keep assistant responses conversational but containing the factual answers.
"""

def clean_json_string(s):
    # Strip markdown block if present
    cleaned = s.strip()
    if cleaned.startswith("```"):
        if "\n" in cleaned:
            cleaned = cleaned[cleaned.find("\n"):].strip()
    if cleaned.endswith("```"):
        cleaned = cleaned[:-3].strip()
    return cleaned

def main():
    print("[DatasetGen] Beginning Phase 1: Generating 10-Session/1000-Turn Dataset...")
    
    # We will accumulate all generated sessions here
    all_sessions = []
    
    # Generate in 5 steps (2 sessions per step, total 10 sessions)
    for step in range(5):
        session_a = step * 2 + 1
        session_b = step * 2 + 2
        print(f"\n--- Generating Step {step+1}/5: Sessions {session_a} and {session_b} ---")
        
        # Build history context
        messages = [
            {"role": "system", "content": SYSTEM_CONTEXT}
        ]
        
        if all_sessions:
            history_str = json.dumps(all_sessions[-4:], indent=2) # Pass the last 4 sessions as context to keep history cohesive
            messages.append({
                "role": "user",
                "content": f"Here is the context of the previous sessions generated so far:\n{history_str}\n\nNow, generate the next two sessions: Session {session_a} and Session {session_b}. Each session must have exactly 10 turns. Keep the turns natural and realistic. Ensure there are memory probes and fact updates (contradictions) that link to facts mentioned in previous sessions. Return ONLY a valid JSON object containing the two sessions, structured like: \n{{\n  \"session_{session_a}\": [ ... turns ... ],\n  \"session_{session_b}\": [ ... turns ... ]\n}}"
            })
        else:
            messages.append({
                "role": "user",
                "content": f"Generate the first two sessions: Session {session_a} and Session {session_b}. Each session must have exactly 10 turns. Weave in user identity facts, preferences, and projects in Session 1, and follow-ups in Session 2. Return ONLY a valid JSON object containing the two sessions, structured like: \n{{\n  \"session_{session_a}\": [ ... turns ... ],\n  \"session_{session_b}\": [ ... turns ... ]\n}}"
            })
            
        raw_output = call_nvidia(messages)
        if not raw_output:
            print("[Error] Failed to generate dataset in this step. Exiting.")
            sys.exit(1)
            
        cleaned_output = clean_json_string(raw_output)
        try:
            sessions_data = json.loads(cleaned_output)
            key_a = f"session_{session_a}"
            key_b = f"session_{session_b}"
            
            turns_a = sessions_data[key_a]
            turns_b = sessions_data[key_b]
            
            print(f"  Successfully parsed Session {session_a} ({len(turns_a)} turns) and Session {session_b} ({len(turns_b)} turns).")
            
            # Save files
            out_dir = "app/src-tauri/tests"
            if not os.path.exists(out_dir):
                out_dir = "tests"
            
            path_a = os.path.join(out_dir, f"dataset_session{session_a}.json")
            path_b = os.path.join(out_dir, f"dataset_session{session_b}.json")
            
            with open(path_a, "w") as f:
                json.dump(turns_a, f, indent=2)
            with open(path_b, "w") as f:
                json.dump(turns_b, f, indent=2)
                
            all_sessions.append({key_a: turns_a})
            all_sessions.append({key_b: turns_b})
            
        except Exception as e:
            print(f"[Error] Failed to parse JSON output: {e}\nRaw output was:\n{raw_output}")
            sys.exit(1)

    print("\n--- Step 6: Running Final Dataset Review and Evaluation ---")
    review_prompt = [
        {"role": "system", "content": SYSTEM_CONTEXT},
        {"role": "user", "content": f"Here are all 10 generated sessions:\n{json.dumps(all_sessions, indent=2)}\n\nPlease review this dataset. Verify and evaluate: \n1. Are there at least 3 distinct USER_SUPERSEDES (fact updates/contradictions) across the sessions? List them.\n2. Are there at least 5 distinct memory probes that test the recall of these facts? List them.\n3. Is the conversation natural, coherent, and non-overfitted? Write a 1-paragraph summary evaluation. Return your review in markdown format."}
    ]
    
    review_output = call_nvidia(review_prompt)
    if review_output:
        out_dir = "app/src-tauri/tests"
        if not os.path.exists(out_dir):
            out_dir = "tests"
        with open(os.path.join(out_dir, "dataset_review.md"), "w") as f:
            f.write(review_output)
        print("\n=== Dataset Review Summary ===")
        print(review_output)
    else:
        print("[Error] Failed to run final review.")
        sys.exit(1)

    print("\n[DatasetGen] Phase 1 completed successfully! Dataset JSON files and review saved.")

if __name__ == "__main__":
    main()
