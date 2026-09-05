#!/usr/bin/env python3
import json
import sqlite3
import os
import sys
import time

DB_PATH = os.path.expanduser("~/.vox/vox.db")
DATASET_PATH = os.path.join(
    os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
    "sandbox/datasets/gate3_v7_ontology_6000p.json"
)

TARGET_FACTS = 3000

def seed_db():
    if not os.path.exists(DATASET_PATH):
        print(f"Error: dataset not found at {DATASET_PATH}", file=sys.stderr)
        sys.exit(1)

    if not os.path.exists(DB_PATH):
        print(f"Error: DB not found at {DB_PATH}", file=sys.stderr)
        sys.exit(1)

    with open(DATASET_PATH, "r", encoding="utf-8") as f:
        data = json.load(f)

    pairs = data.get("pairs", [])
    print(f"Loaded {len(pairs)} ontology pairs from dataset.")

    conn = sqlite3.connect(DB_PATH)
    cur = conn.cursor()

    # Retain existing test identity facts if present
    cur.execute("SELECT id, type, collection, fact, source, status, session_id, turn_id, created_at FROM memory_facts WHERE id IN ('id_1', 'id_2')")
    existing_preserved = cur.fetchall()

    cur.execute("DELETE FROM memory_relations")
    cur.execute("DELETE FROM memory_facts")

    for row in existing_preserved:
        cur.execute(
            "INSERT INTO memory_facts (id, type, collection, fact, source, status, session_id, turn_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            row
        )

    fact_to_id = {}
    fact_records = []
    relation_records = []

    # Map existing preserved
    for row in existing_preserved:
        fact_to_id[row[3]] = row[0]

    now_ms = int(time.time() * 1000)

    # Valid collections according to ontology spec
    valid_collections = {"Identity", "Profile", "Directives", "Constraints", "Entities"}

    fact_counter = 1000

    for pair in pairs:
        fact_a = pair.get("fact_a", "").strip()
        fact_b = pair.get("fact_b", "").strip()
        label = pair.get("expected_label", "NONE").strip()

        domain_a = pair.get("source_domain", "Entities").strip()
        domain_b = pair.get("target_domain", "Entities").strip()

        if domain_a not in valid_collections:
            domain_a = "Entities"
        if domain_b not in valid_collections:
            domain_b = "Entities"

        # Register Fact A
        if fact_a and fact_a not in fact_to_id:
            if len(fact_to_id) < TARGET_FACTS:
                fact_counter += 1
                f_id = f"fact_{fact_counter:05d}"
                fact_to_id[fact_a] = f_id
                fact_records.append((
                    f_id,
                    "Fact",
                    domain_a,
                    fact_a,
                    "LLM",
                    "active",
                    "",
                    "",
                    now_ms - (len(fact_records) * 60000)
                ))

        # Register Fact B
        if fact_b and fact_b not in fact_to_id:
            if len(fact_to_id) < TARGET_FACTS:
                fact_counter += 1
                f_id = f"fact_{fact_counter:05d}"
                fact_to_id[fact_b] = f_id
                fact_records.append((
                    f_id,
                    "Fact",
                    domain_b,
                    fact_b,
                    "LLM",
                    "active",
                    "",
                    "",
                    now_ms - (len(fact_records) * 60000)
                ))

        # Add Relation if both facts exist and label is not NONE
        if fact_a in fact_to_id and fact_b in fact_to_id:
            id_a = fact_to_id[fact_a]
            id_b = fact_to_id[fact_b]
            if id_a != id_b and label and label != "NONE":
                relation_records.append((
                    id_a,
                    id_b,
                    label,
                    "NLI",
                    now_ms
                ))

        if len(fact_to_id) >= TARGET_FACTS:
            break

    print(f"Preparing to insert {len(fact_records)} facts and {len(relation_records)} relations...")

    cur.executemany(
        "INSERT OR IGNORE INTO memory_facts (id, type, collection, fact, source, status, session_id, turn_id, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        fact_records
    )

    cur.executemany(
        "INSERT OR IGNORE INTO memory_relations (from_id, to_id, relation, source, created_at) VALUES (?, ?, ?, ?, ?)",
        relation_records
    )

    conn.commit()

    # Query collection distribution
    cur.execute("SELECT collection, count(*) FROM memory_facts GROUP BY collection ORDER BY count(*) DESC")
    print("Facts per collection:")
    for col, count in cur.fetchall():
        print(f"  - {col}: {count}")

    cur.execute("SELECT relation, count(*) FROM memory_relations GROUP BY relation ORDER BY count(*) DESC")
    print("Relations per type:")
    for rel, count in cur.fetchall():
        print(f"  - {rel}: {count}")

    cur.execute("SELECT count(*) FROM memory_facts")
    total_facts = cur.fetchone()[0]
    cur.execute("SELECT count(*) FROM memory_relations")
    total_relations = cur.fetchone()[0]

    print(f"Done! Successfully seeded {total_facts} total facts and {total_relations} total relations into {DB_PATH}")
    conn.close()

if __name__ == "__main__":
    seed_db()
