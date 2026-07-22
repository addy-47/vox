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

# Let's support selecting the model and dataset via command-line arguments
model_name = sys.argv[1] if len(sys.argv) > 1 else "meta/llama-3.1-70b-instruct"
dataset_path = sys.argv[2] if len(sys.argv) > 2 else "app/src-tauri/tests/dataset_session1.json"

# We use the original COMPACTION_SYSTEM_PROMPT from constants.rs
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

def call_nvidia(system_prompt, user_content):
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {API_KEY}"
    }
    data = {
        "model": model_name,
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

def main():
    print(f"Model: {model_name}")
    print(f"Dataset: {dataset_path}")
    
    if not os.path.exists(dataset_path):
        print(f"Error: Dataset path {dataset_path} does not exist.")
        sys.exit(1)
        
    with open(dataset_path, "r") as f:
        turns_data = json.load(f)
        
    # Compile 100 turns history
    history_lines = []
    for turn in turns_data:
        history_lines.append(f"User: {turn['user']}")
        history_lines.append(f"Assistant: {turn['assistant']}")
    
    user_content = "\n".join(history_lines)
    
    print(f"Sending 100 turns to NVIDIA API using {model_name}...")
    start_time = time.time()
    raw_response = call_nvidia(COMPACTION_SYSTEM_PROMPT, user_content)
    elapsed = time.time() - start_time
    
    if not raw_response:
        print("Failed to get response.")
        sys.exit(1)
        
    print(f"Received response in {elapsed:.1f}s. Saving response to files...")
    
    # Save raw response
    raw_output_path = "temp/raw_llm_compaction_response.txt"
    os.makedirs("temp", exist_ok=True)
    with open(raw_output_path, "w") as f:
        f.write(raw_response)
        
    print(f"Saved RAW LLM Response to: {raw_output_path}")
    
    # Try parsing and print preview
    try:
        # Strip code fences if any
        clean_resp = raw_response.strip()
        if clean_resp.startswith("```"):
            # strip start fence
            lines = clean_resp.split("\n")
            if lines[0].startswith("```"):
                lines = lines[1:]
            if lines[-1].startswith("```"):
                lines = lines[:-1]
            clean_resp = "\n".join(lines).strip()
            
        parsed_data = json.loads(clean_resp)
        print("\nParsed JSON successfully! Here is a preview of the extracted categories:")
        for k, v in parsed_data.items():
            print(f"  {k}: {len(v)} items")
            for item in v[:2]:
                print(f"    - {item}")
    except Exception as e:
        print(f"\nWarning: Could not parse response directly as JSON: {e}")
        print("The raw file remains available for manual check.")

if __name__ == "__main__":
    main()
