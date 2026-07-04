import json
import os
import urllib.request
import urllib.parse

def load_gemini_key():
    env_path = "temp/.env"
    if os.path.exists(env_path):
        with open(env_path, "r") as f:
            for line in f:
                if line.startswith("GEMINI_API_KEY="):
                    return line.strip().split("=")[1].strip('"\'')
    return os.environ.get("GEMINI_API_KEY", "")

API_KEY = load_gemini_key()
API_URL = f"https://generativelanguage.googleapis.com/v1beta/openai/chat/completions"

def call_gemini(messages, max_tokens=2000):
    headers = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {API_KEY}"
    }
    data = {
        "model": "gemini-2.5-flash",
        "messages": messages,
        "max_tokens": max_tokens
    }
    req = urllib.request.Request(API_URL, data=json.dumps(data).encode("utf-8"), headers=headers)
    try:
        with urllib.request.urlopen(req) as response:
            res_body = response.read().decode("utf-8")
            res_json = json.loads(res_body)
            return res_json["choices"][0]["message"]["content"]
    except Exception as e:
        print(f"Error calling Gemini: {e}")
        return None

# 50 Utterances covering user identity, preferences, technical concepts, sports, Hindi, and follow-ups
USER_UTTERANCES = [
    "Hi, I'm Alex. I am a senior system engineer building Rust applications.", # Turn 1 (Identity)
    "I really hate Python for high-performance backends, Rust is my favorite.", # Turn 2 (Preference)
    "Can you explain how Tokio async runtime handles task scheduling under the hood?", # Turn 3
    "That makes sense. What is the difference between a cooperative worker thread and an OS thread?", # Turn 4
    "My favorite color is teal, by the way.", # Turn 5 (Preference)
    "What is the speed of light in vacuum?", # Turn 6
    "How long does it take for light to travel from Earth to Mars at closest approach?", # Turn 7
    "Let's switch gears. I'm building a voice app called Vox.", # Turn 8 (Project)
    "Vox needs sub-500ms latency for VAD, STT, LLM, and TTS.", # Turn 9 (Constraint)
    "Which STT model would you recommend for local execution?", # Turn 10
    "What is the memory footprint of Nemotron-3.5 FastConformer?", # Turn 11
    "Who directed the movie Interstellar?", # Turn 12
    "What is the score composed by Hans Zimmer in that movie known for?", # Turn 13
    "नमस्ते वॉक्स, क्या आप मुझे भारत की राजधानी के बारे में बता सकते हैं?", # Turn 14 (Hindi)
    "दिल्ली में लाल किला किसने बनवाया था?", # Turn 15 (Hindi)
    "What is the derivative of x^3 + 4x - 7?", # Turn 16
    "What is the indefinite integral of 3x^2 + 4?", # Turn 17
    "Can you summarize Quantum Entanglement in plain English?", # Turn 18
    "Does Quantum Entanglement allow faster-than-light communication?", # Turn 19
    "No, because of the No-Communication Theorem, right?", # Turn 20
    "What is the capital of Japan?", # Turn 21
    "What are the top 3 tech hubs in Japan besides Tokyo?", # Turn 22
    "I prefer using SQLite with rusqlite in Rust for local persistence.", # Turn 23 (Preference)
    "How does SQLite WAL mode improve concurrent reads and writes?", # Turn 24
    "What is the main drawback of WAL mode in shared network filesystems?", # Turn 25
    "Who wrote the book 'Clean Code'?", # Turn 26
    "Why do many Rust developers disagree with some SOLID principles?", # Turn 27
    "What is RAII in Rust and how does it prevent memory leaks?", # Turn 28
    "Can Rust have memory leaks without 'unsafe' blocks?", # Turn 29
    "Yes, via std::mem::forget or Arc cycles, correct?", # Turn 30
    "What is the FIFA World Cup winner of 2018?", # Turn 31
    "Who won the Golden Boot in that tournament?", # Turn 32
    "What is the boiling point of liquid nitrogen in Celsius?", # Turn 33
    "How cold is liquid helium compared to nitrogen?", # Turn 34
    "Let me remind you: what is my name and my preferred programming language?", # Turn 35 (Test Recall)
    "What application am I working on?", # Turn 36 (Test Recall)
    "Can you explain how B-Trees differ from LSM-Trees for database storage engines?", # Turn 37
    "Which database uses LSM-Trees by default?", # Turn 38
    "How does RocksDB perform compaction?", # Turn 39
    "What is the difference between Size-Tiered and Levelled compaction?", # Turn 40
    "What is the atomic number of Gold?", # Turn 41
    "What is the chemical symbol for Silver?", # Turn 42
    "Can you write a short 4-line poem about Rust and concurrency?", # Turn 43
    "What is the half-life of Carbon-14?", # Turn 44
    "What color did I say was my favorite earlier in our chat?", # Turn 45 (Test Recall)
    "What language do I hate for high-performance backends?", # Turn 46 (Test Recall)
    "What is the time complexity of QuickSort in worst case?", # Turn 47
    "How does IntroSort mitigate the O(n^2) worst-case of QuickSort?", # Turn 48
    "Can you summarize what we have discussed so far in 3 bullet points?", # Turn 49
    "Thank you Vox, this concludes our 50-turn session validation." # Turn 50
]

def generate_dataset():
    dataset_path = "app/src-tauri/tests/dataset.json"
    if os.path.exists(dataset_path):
        print(f"Dataset already exists at {dataset_path}, loading...")
        with open(dataset_path, "r") as f:
            return json.load(f)
            
    print("Generating 50-turn dataset using Gemini API (mandating 3-4 line responses)...")
    history = [
        {"role": "system", "content": "You are Vox, a responsive AI voice assistant. Always reply with substantial, multi-sentence answers (at least 3 to 4 lines of clear text) per response."}
    ]
    turns = []
    
    for i, user_text in enumerate(USER_UTTERANCES):
        print(f"Generating Turn {i+1}/50...")
        history.append({"role": "user", "content": user_text})
        assistant_resp = call_gemini(history, max_tokens=2000)
        if not assistant_resp:
            assistant_resp = f"I acknowledge your utterance regarding: {user_text}. Let us explore this further with detailed facts and context."
        
        history.append({"role": "assistant", "content": assistant_resp})
        turns.append({
            "turn": i + 1,
            "user": user_text,
            "assistant": assistant_resp
        })

    with open(dataset_path, "w") as f:
        json.dump(turns, f, indent=2)
    print(f"Saved dataset with 50 turns to {dataset_path}")
    return turns

def evaluate_compaction_prompts(turns):
    print("\n--- Running Compaction Prompt A/B Evaluation ---")
    
    # Format full 50-turn conversation text
    full_conv_str = ""
    for item in turns:
        full_conv_str += f"User: {item['user']}\nAssistant: {item['assistant']}\n\n"

    prompts = {
        "Variant A (Minimal)": "Summarize the conversation concisely while preserving main points.",
        "Variant B (Chronological Structured)": "Provide a structured chronological summary of the conversation, noting key topics, user preferences, explicit facts stated, and unresolved questions.",
        "Variant C (State & Fact Extraction)": "Extract state from the conversation into sections:\n1. User Identity & Preferences\n2. Key Facts & Topics Covered\n3. Unresolved Questions / Active State\nKeep it dense and compact.",
        "Variant D (High-Density Context Engineering)": "Compress the conversation history into a high-density, context-preserving memory block. System requirements:\n- Retain all explicit user identity markers, stated preferences, and project names.\n- Maintain chronological progression of technical decisions and topics.\n- Preserve key facts and Hindi language context.\n- Write in clear, concise prose optimized for LLM context injection."
    }

    results = {}
    for name, prompt_text in prompts.items():
        print(f"Testing {name}...")
        messages = [
            {"role": "system", "content": prompt_text},
            {"role": "user", "content": f"Here is the full conversation history:\n\n{full_conv_str}"}
        ]
        summary = call_gemini(messages, max_tokens=2000)
        results[name] = summary

    # Write evaluation report
    report_path = "app/src-tauri/tests/compaction_prompt_eval_results.md"
    with open(report_path, "w") as f:
        f.write("# Compaction Prompt A/B Evaluation Report\n\n")
        f.write("## Dataset Statistics\n")
        f.write(f"- Total Turns: {len(turns)}\n")
        f.write(f"- Total Raw Text Characters: {len(full_conv_str)}\n\n")
        
        for name, summary in results.items():
            f.write(f"### {name}\n")
            f.write(f"**Length**: {len(summary)} chars\n\n")
            f.write("```\n")
            f.write(summary + "\n")
            f.write("```\n\n")

    print(f"\nSaved evaluation report to {report_path}")

if __name__ == "__main__":
    turns = generate_dataset()
    evaluate_compaction_prompts(turns)
