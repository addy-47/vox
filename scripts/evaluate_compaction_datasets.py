import json
import os
import urllib.request
import sys
import time

def load_env_key():
    candidates = ["temp/.env", "../temp/.env", "../../temp/.env", "app/src-tauri/temp/.env"]
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
MODEL_NAME = "meta/llama-3.1-70b-instruct"

COMPACTION_SYSTEM_PROMPT = """<role>
You are a memory extraction engine. You compress a conversation into structured JSON, nothing else.
</role>

<output_contract>
Return ONLY a single valid JSON object. No prose, no preamble, no markdown, no code fences.
Your response must start with { and end with }.
</output_contract>

<rules>
1. Write all text in English. Translate non-English input to English.
2. Extract only facts explicitly stated. Never infer, assume, or invent.
3. Every one of the 10 keys below must be present, even if its array is empty.
4. Every collection value is a flat array of strings. Never nest objects, maps, or lists inside a collection.
5. Do not invent new top-level keys. Use only the 10 keys listed in <schema>.
6. Each array element is exactly one complete English sentence.
7. No trailing commas. The JSON must parse exactly as written.
</rules>

<schema>
{
  "Identity": [],
  "Constraints": [],
  "Preferences": [],
  "Relationships": [],
  "Skills": [],
  "Projects": [],
  "Experiences": [],
  "Context": [],
  "Tasks": [],
  "Goals": []
}
</schema>

<key_definitions>
Identity: who the user is (name, age, profession, self-descriptors).
Constraints: hard requirements or limits (dietary, physical, temporal, absolute rules).
Preferences: tastes, likes, dislikes, habits, tool/style choices.
Relationships: people mentioned and their connection to the user.
Skills: abilities, languages, technologies, domain expertise.
Projects: initiatives currently being built, designed, or planned.
Experiences: past jobs, life events, places lived, historical facts about the user.
Context: one narrative paragraph on what happened in this conversation and what the user is trying to do.
Tasks: active or upcoming actionable to-dos.
Goals: future aspirations or long-term objectives.
</key_definitions>

<example>
<input>
User: Hey! I'm Sarah, wrapping up a TypeScript project called EcoTrack today.
Assistant: How's it coming along?
User: Good, but I'm beat, coding since 7am. I need a matcha latte - love matcha but I'm dairy-free, so oat milk only.
Assistant: Got it. What's left on EcoTrack?
User: Write the README, push final commits. Also my sister Emma visits tomorrow so I need to clean my desk.
Assistant: Any plans for the rest of the year?
User: Training for my first half-marathon in October, so I need to stick to my running schedule.
</input>
<output>
{
  "Identity": ["The user's name is Sarah."],
  "Constraints": ["The user is dairy-free and must use oat milk instead of dairy."],
  "Preferences": ["The user loves matcha lattes made with oat milk."],
  "Relationships": ["The user has a sister named Emma."],
  "Skills": ["The user has TypeScript programming skills."],
  "Projects": ["The user is building a TypeScript project called EcoTrack."],
  "Experiences": [],
  "Context": ["Sarah gave an update on her EcoTrack project and her plans for the day, including Emma's visit tomorrow and her half-marathon training."],
  "Tasks": ["The user needs to write the README and push final commits for EcoTrack.", "The user needs to clean her desk before Emma visits tomorrow."],
  "Goals": ["The user is training to run her first half-marathon in October."]
}
</output>
</example>

<task>
Process the conversation history provided in the next message. Extract facts into the 10 collections from <schema>, following every rule in <rules>. Return ONLY the JSON object, starting with { and ending with }.
</task>"""

# Python version of the resilient flattener
def clean_json_content(content):
    cleaned = content.strip()
    if cleaned.startswith("```"):
        first_nl = cleaned.find("\n")
        if first_nl != -1:
            cleaned = cleaned[first_nl:].strip()
    if cleaned.endswith("```"):
        cleaned = cleaned[:-3].strip()
    
    # Extract between first '{' and last '}'
    start_idx = cleaned.find("{")
    end_idx = cleaned.rfind("}")
    if start_idx != -1 and end_idx != -1 and start_idx < end_idx:
        cleaned = cleaned[start_idx:end_idx+1]
    return cleaned

def extract_strings(val, results_list):
    if val is None:
        return
    elif isinstance(val, bool):
        results_list.append(str(val).lower())
    elif isinstance(val, (int, float)):
        results_list.append(str(val))
    elif isinstance(val, str):
        trimmed = val.strip()
        if trimmed:
            results_list.append(trimmed)
    elif isinstance(val, list):
        for item in val:
            extract_strings(item, results_list)
    elif isinstance(val, dict):
        found = False
        for possible_key in ["text", "fact", "description", "desc", "content", "value", "name"]:
            if possible_key in val:
                extract_strings(val[possible_key], results_list)
                found = True
                break
        if not found:
            parts = []
            for k, v in val.items():
                sub_strs = []
                extract_strings(v, sub_strs)
                if sub_strs:
                    parts.append(f"{k}: {', '.join(sub_strs)}")
            if parts:
                results_list.append("; ".join(parts))

def resilient_parse_compaction_json(content):
    cleaned = clean_json_content(content)
    try:
        obj = json.loads(cleaned)
    except Exception as e:
        print(f"  [Parser] JSON Syntax Error: {e}")
        return None
    
    if not isinstance(obj, dict):
        print("  [Parser] Root is not a JSON object")
        return None

    known_keys = [
        "Identity", "Constraints", "Preferences", "Relationships",
        "Skills", "Projects", "Experiences", "Context", "Tasks", "Goals"
    ]
    results = {k: [] for k in known_keys}

    for k, v in obj.items():
        lowered = k.lower()
        target_key = None
        if lowered == "identity":
            target_key = "Identity"
        elif lowered == "constraints":
            target_key = "Constraints"
        elif lowered == "preferences":
            target_key = "Preferences"
        elif lowered in ["relationships", "relationship"]:
            target_key = "Relationships"
        elif lowered in ["skills", "skill"]:
            target_key = "Skills"
        elif lowered in ["projects", "project"]:
            target_key = "Projects"
        elif lowered in ["experiences", "experience"]:
            target_key = "Experiences"
        elif lowered == "context":
            target_key = "Context"
        elif lowered in ["tasks", "task"]:
            target_key = "Tasks"
        elif lowered in ["goals", "goal"]:
            target_key = "Goals"

        if target_key:
            extract_strings(v, results[target_key])
    
    return results

def call_nvidia(system_prompt, user_content):
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {API_KEY}"
    }
    data = {
        "model": MODEL_NAME,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_content}
        ],
        "temperature": 0.1,
        "max_tokens": 4096
    }
    req = urllib.request.Request(API_URL, data=json.dumps(data).encode("utf-8"), headers=headers)
    
    try:
        with urllib.request.urlopen(req, timeout=90) as response:
            res_data = response.read().decode("utf-8")
            parsed = json.loads(res_data)
            return parsed["choices"][0]["message"]["content"]
    except Exception as e:
        print(f"  [API Error] {e}")
        return None

def process_dataset(dataset_path):
    print(f"\n==================================================")
    print(f"Processing Dataset: {os.path.basename(dataset_path)}")
    print(f"==================================================")
    
    with open(dataset_path, "r") as f:
        turns_data = json.load(f)
    
    print(f"Loaded {len(turns_data)} conversational turns.")
    
    # Compile 100 turns history
    history_lines = []
    for turn in turns_data:
        history_lines.append(f"User: {turn['user']}")
        history_lines.append(f"Assistant: {turn['assistant']}")
    
    user_content = "\n".join(history_lines)
    
    print(f"Sending entire 100 turns history ({len(user_content)} chars) to {MODEL_NAME}...")
    start_time = time.time()
    raw_response = call_nvidia(COMPACTION_SYSTEM_PROMPT, user_content)
    elapsed = time.time() - start_time
    
    if not raw_response:
        print("  [FAIL] Failed to receive response from NVIDIA API.")
        return False
        
    print(f"  [SUCCESS] Received response in {elapsed:.1f}s.")
    
    # Verify the resilient parser on the raw response
    parsed_facts = resilient_parse_compaction_json(raw_response)
    if not parsed_facts:
        print("  [FAIL] Resilient parser failed to extract facts.")
        print(f"Raw Response: {raw_response[:500]}...")
        return False
        
    print("  [PASS] Resilient parser successfully extracted category facts:")
    for cat, facts in parsed_facts.items():
        print(f"    - {cat}: {len(facts)} facts extracted.")
        
    # Save the extracted facts
    output_path = dataset_path.replace(".json", "_extracted.json")
    with open(output_path, "w") as f:
        json.dump(parsed_facts, f, indent=2)
    print(f"Saved extracted facts to {os.path.basename(output_path)}.")
    return True

if __name__ == "__main__":
    datasets = [
        "app/src-tauri/tests/dataset_session1.json",
        "app/src-tauri/tests/dataset_session2.json",
        "app/src-tauri/tests/dataset_session3.json"
    ]
    success_all = True
    for d in datasets:
        if not process_dataset(d):
            success_all = False
            
    if success_all:
        print("\nAll datasets evaluated successfully!")
        sys.exit(0)
    else:
        print("\nSome dataset evaluations failed.")
        sys.exit(1)
