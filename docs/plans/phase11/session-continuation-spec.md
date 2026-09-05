# Session Persistence, Continuation & Structured Stream Routing Spec

**Type:** Feature spec (single feature: persistent sessions, restore-to-continue, general stream-routing primitive with automatic titles as first consumer).
**Status:** Draft — awaiting approval.

---

## Name & Concept

Persistent conversation sessions that the user can reopen from a conversation list and continue from their latest persisted state, powered by a general routing layer that separates user-facing response content from tagged orchestration content (automatic session titles being the first use of that layer).

## Purpose

A user can leave a conversation, return later, pick it from a visible list, and carry on where they left off — with the assistant remembering what was previously discussed, each session carrying a recognizable name earned with zero user effort, and the interface clearly communicating when past context is loaded. The routing layer exists so the title (and later capabilities such as tool calls, execution instructions, structured actions, and internal agent state) can travel inside the same generation as the normal reply without ever leaking into what the user sees or hears, and without ever slowing that reply down.

## Must Be True

### A. Discovery and selection

1. Persisted sessions are accessible through a conversation list opened from the home screen, presented as a side rail.
2. Every session entry shows its title (generated title once available, otherwise a neutral untitled placeholder) and enough recency information for the user to tell sessions apart.
3. Entries are ordered newest-first by last activity, so the most recently touched session appears at the top.
4. Selecting a session makes it the active session; all subsequent turns in the current interaction are appended to that session until the user starts a new conversation or selects a different session.
5. Starting an explicit new conversation creates a new empty session with an untitled placeholder and does not inherit turns or titles from any prior session.
6. Once a title is persisted, the list entry for that session shows the new title without requiring the user to reselect the session, reload the list, or restart the app.

### B. Restoration and continuation

7. Selecting a session loads its persisted turns in original order with no turns missing, duplicated, reordered, or mixed in from any other session.
8. The restored visible transcript matches exactly what was persisted for that session — no extra system messages, summaries, or orchestration content appear as chat turns.
9. Switching to a session while a response from a previous session is still generating cancels the prior generation, and none of its late-arriving content (response or title) is appended to the newly selected session or written to the wrong session.
10. Selecting the already-active session is a no-op: no duplicate load, no transcript reset, no repeated animation.
11. When a session is restored, the next user turn is generated with the normal system context plus the latest successfully persisted summary for that session included as background context.
12. A session with no persisted summary restores and continues normally with the normal context alone.
13. A failure to load the persisted summary must never block restoration: the transcript still loads, the session still becomes active, and the next turn proceeds with whatever context is available.
14. The persisted summary is used only as background context for generation; it is never rendered as visible chat turns and never read aloud.
15. Every successful restore triggers a single reverse-flow ambient animation toward the central orb, communicating that previous context is being ingested; it runs once per restore, does not block interaction, and does not replay on subsequent turns within the same session.
16. A fresh new conversation never triggers the reverse-flow animation.

### C. General stream-routing primitive

17. All generation output passes through a single routing layer before reaching any user-facing destination (visible transcript, speech synthesis); there is no path by which raw generation output reaches the user without passing through it.
18. Generation output consists of an ordered stream of segments; each segment is either user-facing response content (the default when no tag is present) or a tagged segment explicitly marked with its purpose.
19. A single generation may contain zero, one, or many tagged segments alongside user-facing content, in any order, including tagged segments appearing after the user-facing response is complete.
20. Tag markers are routing instructions, not content: the routing layer consumes them and they never reach any user-facing destination.
21. User-facing segments flow to the existing response path (incremental visible streaming, chunking, speech synthesis) preserving their original relative order and wording, with timing indistinguishable from a generation that contained no tagged segments.
22. Each tagged segment is delivered in full to exactly one orchestration consumer determined by its tag purpose, and is withheld from all user-facing destinations (not displayed, not spoken, not stored as chat transcript).
23. User-facing delivery never waits for orchestration delivery: a slow or failing orchestration consumer must not block, delay, reorder, or interrupt the visible or spoken response.
24. A tag purpose with no registered consumer is safely discarded — the segment is dropped, the user-facing response is unaffected, and the generation as a whole still succeeds.
25. Malformed or unparseable tagged content is dropped rather than leaked to the user, and the user-facing response still completes.
26. The routing layer contains no logic specific to any single tag purpose; all purpose-specific handling lives in that purpose's consumer, so adding a new purpose (tool calls, execution instructions, structured actions, internal agent state) requires no change to existing routing or to the user-facing path, and a failure in one consumer does not affect delivery to any other consumer or to the user.

### D. Automatic titles as first consumer

27. Title generation is attempted only for a session that has no title yet, during its first meaningful turn — the first turn in which the user provided real input and the assistant produced a non-empty user-facing response. Turns with empty input, cancelled or failed generation, or filler-only or error-only output do not consume the attempt.
28. The generation for that turn contains the normal user-facing response followed by one distinct title segment; the response comes first so user-facing delivery can begin before the title is complete.
29. The title segment is delivered only to the title consumer via the routing layer; it never appears in the visible transcript, never enters speech synthesis, and is never stored as chat transcript.
30. The title consumer accepts the title only if it is non-empty after trimming surrounding whitespace; an overlong title is shortened to the agreed display length rather than rejected.
31. An accepted title is persisted against its session asynchronously, off the response path; the response completes, plays, and stays interactive regardless of how long persistence takes.
32. The title is display-only metadata: it never alters assistant behavior, never enters the context of future turns, and never changes what was said or heard.
33. A missing, empty, or invalid title segment leaves the session with its untitled placeholder; the user-facing response still succeeds and the turn is still persisted normally.
34. A failure to persist a valid title is silent to the conversation: no error interrupts the response, no retry blocks playback, and the turn remains continuable later.
35. An already-titled session never receives a new automatic title from later turns — later title segments for a titled session are discarded.

## Must Not Happen

- Selecting an existing session must not create a new session or fork a copy; restoring a session must not delete, overwrite, or reorder its persisted turns; turns from one session must never appear inside another.
- The interface must never show one session's transcript while recording new turns against a different session.
- The persisted summary must never be displayed as a user or assistant message, spoken aloud, or counted as a chat turn.
- Orchestration content of any kind — including title segments, tag markers, partial tag fragments, and errors from orchestration consumers — must never appear in the visible transcript, the streamed visible output, or the spoken output.
- The routing layer must never alter user-facing wording, reorder user-facing segments, inject orchestration-derived text into the response, or fail, cancel, or retry the user-facing response because an orchestration consumer failed, was slow, or was missing.
- Title handling must never block, delay, interrupt, or degrade the user-facing response, and a title failure must never fail the turn, lose the transcript, or prevent later continuation.
- An automatic title must never overwrite an already-stored title, be written to the wrong session, or be fabricated when none was emitted (no placeholder text, input echo, or response fragment stored as a title).
- A failure in summary loading, title loading, title persistence, or animation playback must not fail the restore or leave the session unselectable.

## Out of Scope

- Manual session management (rename, delete, search, filter, pin), storage limits, retention policy, and pagination or lazy loading of very long transcripts.
- The visual design of the rail itself (width, breakpoints, gestures) beyond the behaviors stated above.
- Prompt wording and tag vocabulary used to instruct the model to emit tags, and transport details such as buffering, chunk boundaries, retry policy, observability, and logging.
- Title content policy beyond the emptiness and length validation above (style guide, localization), and whether titles ever update as a conversation drifts.
- Future tag purposes beyond titles (tool calls, execution instructions, structured actions, internal agent state), which will each define their own payload contract against the routing primitive established here.

## Open Questions

1. What exactly defines "latest persisted state" — persisted turns only, or turns plus summary plus pending notifications, and which source wins on disagreement?
2. Where does the active-session pointer live, and what restores on app restart — last active session or always a fresh session?
3. How many turns load on restore for very long sessions (full vs. windowed with lazy history), and what does the user see while older turns load?
4. What happens to an in-flight turn (recording, transcribing, generating) when the user selects a different session mid-turn — cancel silently, finish-then-switch, or block the switch? Is a late-arriving title for the previous session still persisted or discarded?
5. What does selecting a deleted or unloadable session show — error state, empty state, or fallback to a new session?
6. Does restoring a session restore any per-session settings snapshot, or do current global settings always apply?
7. What is the tag syntax and framing so it survives incremental streaming, never collides with normal prose, supports tags split across output chunks, and degrades to dropped-not-leaked when malformed? May tagged segments interleave mid-sentence or only follow the response?
8. What are the delivery semantics per purpose (at-most-once vs. exactly-once), size and count limits per generation, and how are unknown purposes surfaced for future development without leaking to the user?
9. What are the title content constraints (maximum display length, shortening rule, allowed characters, language matching), what instructs the model on title style without leaking into the response, and if the first meaningful turn yields no valid title, may a later untitled turn retry or is the session untitled until renamed manually?
