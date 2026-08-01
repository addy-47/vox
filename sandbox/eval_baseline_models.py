#!/usr/bin/env python3
"""
============================================================================
eval_baseline_models.py — Empirical Side-by-Side Evaluator: MiniLM-L12 vs NVIDIA NIM
============================================================================
Evaluates local MiniLM-L12 ONNX embedding model vs NVIDIA NIM nv-embedqa-e5-v5
across all 4 splits of sandbox/datasets/vox_embedding_baseline_v1.json.
============================================================================
"""

import json
import math
import os
import sys
import time
import urllib.request
import urllib.error
import numpy as np

DATASET_PATH = "/home/addy/projects/apps/vox/sandbox/datasets/vox_embedding_baseline_v1.json"
ENV_PATH = "/home/addy/projects/apps/vox/temp/.env"
MINILM_MODEL_PATH = os.path.expanduser("~/.vox/models/embedding/minilm-l12-v2/model_int8.onnx")
MINILM_TOKENIZER_PATH = os.path.expanduser("~/.vox/models/embedding/minilm-l12-v2/tokenizer.json")

def get_nvidia_api_key():
    if not os.path.exists(ENV_PATH):
        return None
    with open(ENV_PATH, "r") as f:
        for line in f:
            if line.startswith("NVIDIA_API_KEY="):
                key = line.strip().split("=", 1)[1]
                if key:
                    return key
    return None

def cosine_sim(a, b):
    a = np.array(a, dtype=np.float32)
    b = np.array(b, dtype=np.float32)
    norm_a = np.linalg.norm(a)
    norm_b = np.linalg.norm(b)
    if norm_a == 0 or norm_b == 0:
        return 0.0
    return float(np.dot(a, b) / (norm_a * norm_b))

def call_nvidia_embedding_batch(api_key, texts):
    if not api_key:
        return None
    url = "https://integrate.api.nvidia.com/v1/embeddings"
    headers = {
        "Authorization": f"Bearer {api_key}",
        "Content-Type": "application/json"
    }
    payload = {
        "model": "nvidia/nv-embedqa-e5-v5",
        "input": texts,
        "input_type": "passage",
        "encoding_format": "float"
    }
    req = urllib.request.Request(url, data=json.dumps(payload).encode("utf-8"), headers=headers)
    try:
        with urllib.request.urlopen(req) as resp:
            res = json.loads(resp.read().decode("utf-8"))
            return [d["embedding"] for d in res["data"]]
    except Exception as e:
        print(f"[Warn] NVIDIA NIM Embedding call failed: {e}")
        return None

class LocalMiniLM:
    def __init__(self):
        try:
            import ort
            from tokenizers import Tokenizer
            if not os.path.exists(MINILM_MODEL_PATH) or not os.path.exists(MINILM_TOKENIZER_PATH):
                print(f"[Error] MiniLM model not found at {MINILM_MODEL_PATH}")
                self.session = None
                return
            self.tokenizer = Tokenizer.from_file(MINILM_TOKENIZER_PATH)
            sess_options = ort.SessionOptions()
            sess_options.intra_op_num_threads = 2
            self.session = ort.InferenceSession(MINILM_MODEL_PATH, sess_options)
            self.input_names = [i.name for i in self.session.get_inputs()]
        except Exception as e:
            print(f"[Error] Failed to initialize local MiniLM ONNX: {e}")
            self.session = None

    def embed(self, text):
        if not self.session:
            return None
        encoding = self.tokenizer.encode(text)
        ids = np.array([encoding.ids], dtype=np.int64)
        mask = np.array([encoding.attention_mask], dtype=np.int64)
        
        feed = {"input_ids": ids, "attention_mask": mask}
        if "token_type_ids" in self.input_names:
            type_ids = np.array([encoding.type_ids], dtype=np.int64)
            feed["token_type_ids"] = type_ids
            
        out = self.session.run(None, feed)[0]
        # Mean pooling with mask
        mask_expanded = np.expand_dims(mask, -1)
        sum_embeddings = np.sum(out * mask_expanded, axis=1)
        sum_mask = np.clip(mask_expanded.sum(axis=1), a_min=1e-9, a_max=None)
        mean_pooled = sum_embeddings / sum_mask
        
        # L2 Normalize
        vec = mean_pooled[0]
        norm = np.linalg.norm(vec)
        if norm > 0:
            vec = vec / norm
        return vec.tolist()

def evaluate():
    print("=========================================================================================")
    print("     EMPIRICAL SIDE-BY-SIDE EVALUATION: MINILM-L12 (INT8 ONNX) VS NVIDIA NIM API          ")
    print("=========================================================================================\n")

    with open(DATASET_PATH, "r") as f:
        ds = json.load(f)

    api_key = get_nvidia_api_key()
    minilm = LocalMiniLM()

    print(f"Dataset Loaded: {ds['metadata']['total_samples']} samples across 4 splits.\n")

    # ─── SPLIT 1: SOFT DEDUP EVALUATION (Threshold = 0.95) ────────────────────────
    print("--- Split 1: Soft Deduplication (Threshold >= 0.95) ---")
    soft_dedup = ds.get("soft_dedup", [])
    
    mini_tp, mini_fp, mini_tn, mini_fn = 0, 0, 0, 0
    nv_tp, nv_fp, nv_tn, nv_fn = 0, 0, 0, 0

    for sample in soft_dedup:
        fa, fb = sample["fact_a"], sample["fact_b"]
        is_dup = sample["is_duplicate"]

        # MiniLM
        e_a_m = minilm.embed(fa)
        e_b_m = minilm.embed(fb)
        sim_m = cosine_sim(e_a_m, e_b_m) if e_a_m and e_b_m else 0.0

        if sim_m >= 0.95:
            if is_dup: mini_tp += 1
            else: mini_fp += 1
        else:
            if is_dup: mini_fn += 1
            else: mini_tn += 1

        # NVIDIA NIM
        nv_vecs = call_nvidia_embedding_batch(api_key, [fa, fb]) if api_key else None
        if nv_vecs:
            sim_nv = cosine_sim(nv_vecs[0], nv_vecs[1])
            if sim_nv >= 0.95:
                if is_dup: nv_tp += 1
                else: nv_fp += 1
            else:
                if is_dup: nv_fn += 1
                else: nv_tn += 1

    mini_prec = mini_tp / (mini_tp + mini_fp) if (mini_tp + mini_fp) > 0 else 0.0
    mini_rec = mini_tp / (mini_tp + mini_fn) if (mini_tp + mini_fn) > 0 else 0.0
    mini_f1 = (2 * mini_prec * mini_rec) / (mini_prec + mini_rec) if (mini_prec + mini_rec) > 0 else 0.0

    print(f"MiniLM-L12  | Soft Dedup F1: {mini_f1:.4f} | Precision: {mini_prec:.4f} | Recall: {mini_rec:.4f} | (TP:{mini_tp}, FP:{mini_fp}, TN:{mini_tn}, FN:{mini_fn})")
    
    if api_key and (nv_tp + nv_fp) > 0:
        nv_prec = nv_tp / (nv_tp + nv_fp)
        nv_rec = nv_tp / (nv_tp + nv_fn)
        nv_f1 = (2 * nv_prec * nv_rec) / (nv_prec + nv_rec) if (nv_prec + nv_rec) > 0 else 0.0
        print(f"NVIDIA NIM  | Soft Dedup F1: {nv_f1:.4f} | Precision: {nv_prec:.4f} | Recall: {nv_rec:.4f} | (TP:{nv_tp}, FP:{nv_fp}, TN:{nv_tn}, FN:{nv_fn})")

    # ─── SPLIT 2: INTRA-EDGE FILTER EVALUATION (Cutoff >= 0.40) ─────────────────
    print("\n--- Split 2: Intra-Edge Filter (Candidate Recall @ Cutoff >= 0.40) ---")
    intra_edge = ds.get("intra_edge", [])
    mini_intra_cand_passed = 0
    mini_intra_cand_total = 0

    for sample in intra_edge:
        fa, fb = sample["fact_a"], sample["fact_b"]
        is_cand = sample["is_candidate"]
        if is_cand:
            mini_intra_cand_total += 1
            e_a_m = minilm.embed(fa)
            e_b_m = minilm.embed(fb)
            sim_m = cosine_sim(e_a_m, e_b_m) if e_a_m and e_b_m else 0.0
            if sim_m >= 0.40:
                mini_intra_cand_passed += 1

    mini_intra_rec = mini_intra_cand_passed / mini_intra_cand_total if mini_intra_cand_total > 0 else 0.0
    print(f"MiniLM-L12  | Intra-Edge Candidate Recall @ 0.40: {mini_intra_rec*100:.1f}% ({mini_intra_cand_passed}/{mini_intra_cand_total})")

    # ─── SPLIT 3: INTER-EDGE FILTER EVALUATION (Cutoff >= 0.55) ─────────────────
    print("\n--- Split 3: Inter-Edge Filter (Precision & Recall @ Cutoff >= 0.55) ---")
    inter_edge = ds.get("inter_edge", [])
    mini_inter_tp, mini_inter_fp, mini_inter_tn, mini_inter_fn = 0, 0, 0, 0

    for sample in inter_edge:
        fa, fb = sample["fact_a"], sample["fact_b"]
        is_rel = sample["is_relational"]

        e_a_m = minilm.embed(fa)
        e_b_m = minilm.embed(fb)
        sim_m = cosine_sim(e_a_m, e_b_m) if e_a_m and e_b_m else 0.0

        if sim_m >= 0.55:
            if is_rel: mini_inter_tp += 1
            else: mini_inter_fp += 1
        else:
            if is_rel: mini_inter_fn += 1
            else: mini_inter_tn += 1

    mini_inter_prec = mini_inter_tp / (mini_inter_tp + mini_inter_fp) if (mini_inter_tp + mini_inter_fp) > 0 else 0.0
    mini_inter_rec = mini_inter_tp / (mini_inter_tp + mini_inter_fn) if (mini_inter_tp + mini_inter_fn) > 0 else 0.0
    print(f"MiniLM-L12  | Inter-Edge Precision: {mini_inter_prec:.4f} | Recall: {mini_inter_rec:.4f} | (TP:{mini_inter_tp}, FP:{mini_inter_fp})")

    # ─── SPLIT 4: RAG CUTOFF EVALUATION (Asymmetric Speech Query Search) ────────
    print("\n--- Split 4: RAG Cutoff (Asymmetric Speech Query-vs-Fact Search) ---")
    rag_cutoff = ds.get("rag_cutoff", [])
    
    eng_samples = [s for s in rag_cutoff if s["language"] == "English"]
    hinglish_samples = [s for s in rag_cutoff if s["language"] == "Hinglish"]

    def eval_rag_subsplit(samples, label):
        hits_at_3 = 0
        total_margin = 0.0
        for sample in samples:
            q = sample["query"]
            target = sample["target_fact"]
            distractors = sample["distractor_facts"]

            all_facts = [target] + distractors
            q_emb = minilm.embed(q)
            
            scores = []
            for idx, f in enumerate(all_facts):
                f_emb = minilm.embed(f)
                sim = cosine_sim(q_emb, f_emb) if q_emb and f_emb else 0.0
                scores.append((idx, sim))

            scores.sort(key=lambda x: x[1], reverse=True)
            top_3_indices = [s[0] for s in scores[:3]]
            
            if 0 in top_3_indices:
                hits_at_3 += 1
            
            target_sim = scores[0][1] if scores[0][0] == 0 else [s[1] for s in scores if s[0] == 0][0]
            max_distractor_sim = max([s[1] for s in scores if s[0] != 0])
            margin = target_sim - max_distractor_sim
            total_margin += margin

        acc = (hits_at_3 / len(samples)) * 100 if samples else 0.0
        avg_margin = total_margin / len(samples) if samples else 0.0
        print(f"MiniLM-L12  | RAG {label:<10} | Top-3 Recall: {acc:.1f}% ({hits_at_3}/{len(samples)}) | Avg Cosine Margin: +{avg_margin:.4f}")

    eval_rag_subsplit(eng_samples, "English")
    eval_rag_subsplit(hinglish_samples, "Hinglish")

    print("\n=========================================================================================\n")

if __name__ == "__main__":
    evaluate()
