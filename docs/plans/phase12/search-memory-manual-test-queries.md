---
title: "Manual search_memory recall and precision test queries"
audience: "Internal — Vox UI and memory evaluation"
last_updated: 2026-09-25
owners: "memory-evaluation"
related_docs:
  - "docs/specs/tools-specs/search-memory.md — retrieval behavior contract"
---

# Manual `search_memory` Test Queries

## How to read this doc
- **Audience:** Manual UI testers.
- **Scope:** Recall, precision, contradiction handling, filtering, and answer-grounding tests.
- **Convention:** Run each query in a fresh turn when possible; record the tool observation, final answer, and any false positives.
- **Non-goals:** This is not a benchmark or a quality score; it is a repeatable manual probe set.
- **SSOT:** Tool behavior is defined by `docs/specs/tools-specs/search-memory.md`.

## Ground-truth anchors

The recovered eval sidecar contains these consolidated personal-memory anchors:

- Severe tree-nut allergy, specifically walnuts and cashews, with anaphylactic risk; baked goods and Doughvid must remain nut-free.
- Owns Doughvid, a sourdough starter; considers Pixel a cat.
- Plays acoustic guitar; practicing `Blackbird` and considering `Drifting`; likes ambient/instrumental acoustic music.
- Enjoys hard sci-fi; read `Red Mars`; `Green Mars` is on the list; mentions Children of Memory, The Expanse, Alastair Reynolds, and Orbital Mechanics for Engineering Students.
- Studying Spanish and Japanese; decided to prioritize Spanish; practiced Spanish in Mexico City.
- Based in Chicago, specifically Logan Square; prefers the local farmers market.
- Pre-diabetic; reducing refined sugar; prefers dark chocolate with at least 70% cacao.
- Coffee is pour-over, unsweetened, medium-roast whole beans; last ordered Ethiopian Yirgacheffe.
- Roommate/partner is Jamie; Jamie enjoys baked goods; Jamie’s birthday is in early July.
- Works on a Rust project involving memory management, buffer allocation, lifetimes, multi-threading, and a circular-buffer networking layer.
- Uses Neovim with Telescope.
- Was planning a Mexico City trip; recent evidence says the trip was about three days away and a lesson was scheduled for 3 PM.

## Query set

### Direct recall

1. What is the name of my sourdough starter?
2. What pet do I have besides the starter?
3. Which foods must I avoid because of my allergy?
4. What guitar song am I practicing?
5. Where do I live?
6. Which two languages am I studying?
7. Which language did I decide to prioritize?
8. What city did I visit or plan to visit for the Spanish practice?
9. What book did I finish recently?
10. What sci-fi book is still on my reading list?
11. What type of coffee do I prefer?
12. What percentage cacao do I prefer in dark chocolate?
13. Which editor and plugin do I use for buffer management?
14. What is Jamie’s birthday timing?
15. What is the main technical focus of my Rust project?

### Vague and paraphrased

16. What should I never put in the bread recipe?
17. Tell me about the companion in the apartment who likes baked goods.
18. What kind of music helps me while I code?
19. Where am I based, and what nearby place do I prefer?
20. What am I learning for an upcoming trip?
21. How should I handle sugar if I want to be healthier?
22. What Rust work have I been doing lately?
23. Which books sound like my kind of reading?
24. What breakfast or drink setup do I normally choose?

### Multi-fact and temporal

25. What do I know about my baking and dietary safety?
26. What have I been doing with languages and Mexico City?
27. What current projects and tools am I using?
28. What recent trip details and lesson schedule should I remember?
29. Which interests connect music, books, and my creative hobbies?
30. Give me a concise profile of my health constraints and preferences.

### Contradictory, stale, and precision traps

31. Do I live in Chicago or have I moved to Mexico City?
32. Should I use walnuts in the next sourdough recipe?
33. Did I say my partner is Jamie, or was that a different person?
34. Is my current sci-fi book Red Mars or Green Mars?
35. Is Pixel a cat, a dog, or a sourdough starter?
36. Should I drink sweetened coffee with breakfast?
37. Did I finish updating the Rust project tracker, or was that only a plan?
38. Did I already set the Spanish lesson reminder, or is the lesson simply scheduled?
39. Am I mainly focusing on Spanish, Japanese, or both equally?
40. What exactly is the current status of the Mexico City trip: planned, booked, or completed?

## Manual pass criteria

For every query, mark:

- **Recall:** Did the final answer contain the relevant anchor facts?
- **Precision:** Did it avoid unsupported facts and stale alternatives?
- **Contradiction handling:** Did it state uncertainty or temporal context when evidence conflicted?
- **Tool grounding:** Could every factual claim be traced to the `search_memory` observation in that turn?
- **Scope control:** Did the answer stay within the retrieved memory rather than inventing related details?

## Expected high-risk cases

- `11`–`12`: health and allergy constraints must not be weakened by paraphrases.
- `18`–`19`: vague queries should retrieve context without returning unrelated candidates.
- `31`–`40`: the answer should preserve uncertainty and avoid resolving conflicts by guessing.
- `37`–`38`: the model must not convert a plan or reminder discussion into completed action.
