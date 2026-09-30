# Vox Dictation Speech Refinement Engine

Status: Concept / Research & Exploration
Scope: Dictation v2
Audience: ML Engineer, Backend Engineer, Test Engineer

## 1. Problem

Vox dictation currently performs the core speech-to-text path successfully: speech is captured, sent through local STT, and the resulting transcript is routed directly to the OS output layer. The existing architecture intentionally keeps dictation outside the LLM/TTS cognitive path.

The remaining problem is that acoustic transcription is not necessarily suitable as written text.

Natural speech contains disfluencies, repetitions, corrections, spoken forms of numbers and dates, missing punctuation, inconsistent capitalization, filler words, and speech repairs.

For example:

`I want to do this on Friday oh no no not Friday Saturday`

should ideally become:

`I want to do this on Saturday.`

The system therefore needs a refinement stage between STT and output that converts raw spoken-language transcription into usable written text while preserving the user's intended meaning.

The refinement system should be treated primarily as a constrained text-transformation problem rather than as unrestricted text generation.

## 2. Goal

Introduce a local, low-latency Dictation Refinement Engine that receives finalized STT output and produces a polished transcript before Vox commits text to the target application.

The intended capability set includes:

* Speech repairs and backtracking
* Repetition and disfluency removal
* Filler removal
* Punctuation
* Capitalization / truecasing
* Quotes and other spoken formatting constructs
* Inverse text normalization
* Numbers, dates, currencies and similar written representations
* Context-dependent corrections where simple deterministic processing is insufficient

The system should initially remain separate from the conversational LLM/Harness path. The existing dictation architecture explicitly treats dictation as a zero-LLM/TTS path.

## 3. Current Architecture

The existing production path is broadly:

`Speech/PTT -> VAD window -> STT -> TranscriptFinal -> Dictation transcript handler -> OutputRouter`

The current integration contract verifies that dictation reaches the output router without dispatching work to the LLM.

The refinement engine would conceptually become:

`Speech/PTT -> VAD -> STT -> Refinement -> OutputRouter`

The exact event/state representation is intentionally open for investigation. The existing `TranscriptFinal` contract should not be changed merely for architectural aesthetics; the ML/backend investigation should determine the cleanest integration seam.

## 4. Proposed User Experience

The important UX distinction is between transcription and commitment.

While the user is speaking, Vox may continue exposing live/interim transcription where appropriate.

After speech ends, the transcript becomes a staging candidate rather than immediately becoming committed output.

Conceptually:

`Live speech -> raw/interim transcript -> speech ends -> refinement/staging -> polished transcript -> OS output`

The existing dictation lifecycle already has a post-speech processing phase before returning to `Ready`, so this architecture should investigate whether that existing lifecycle can naturally accommodate refinement rather than introducing an independent orchestration mechanism.

The existing specification also explicitly identifies a future neural refinement engine between acoustic STT and the Output Router.

## 5. Architectural Invariants

These are the things the implementation should preserve unless investigation demonstrates a fundamental architectural reason to change them.

### Dictation remains independent from the conversational LLM

The refinement system must not turn dictation into an invocation of the normal conversational Harness/LLM pipeline.

`Dictation -> local refinement -> OutputRouter`

remains conceptually separate from:

`Assistant -> Harness -> LLM -> TTS`

The existing integration tests explicitly treat zero LLM dispatch as a dictation invariant.

### Rules must not become the primary intelligence layer

Deterministic rules are useful for genuinely deterministic transformations and safety validation.

They should not become a large hand-written linguistic system containing decisions about repairs, meaning, context or intent.

A useful principle is:

`Model decides -> deterministic layer executes/validates`

rather than:

`large rule system decides -> models handle exceptions`

The ML investigation should challenge this formulation if a better architecture is found, but the system should avoid recreating a brittle rule-based NLP engine.

### No fabricated latency assumptions

Latency must be measured on the actual Vox deployment environment.

The existing speech-finalization window may provide some processing budget, but it should not be assumed that the entire window is available to refinement. STT, orchestration and other work consume part of the available time.

Latency targets therefore need to emerge from profiling rather than being selected as arbitrary constants.

### No model should be selected because of model size alone

Model candidates should be evaluated on the actual task, including quality, false edits, latency, memory, CPU/GPU/NPU behavior, deployment complexity and fine-tuning suitability.

A smaller model is not automatically preferable if it creates unacceptable correction errors.

### Research precedes implementation commitment

The ML engineer should investigate the proposed architecture and candidate models before implementation and should be encouraged to replace the proposed approach where evidence supports a better solution.

## 6. Working Architecture Hypothesis

The current hypothesis is a two-stage neural refinement architecture.

`STT transcript -> Edit Planner -> deterministic execution/normalization -> contextual Resolver -> validation -> final transcript`

This is a hypothesis to benchmark, not a locked implementation.

### Stage A — Edit Planner

The first model would analyse the transcript and identify where and what type of transformation is required.

The original idea was a token-level tagger with labels such as:

`KEEP / EDIT / DELETE / PUNCTUATE / CAPITALIZE / NUMBER / CURRENCY`

The ML investigation should not assume this label design is correct.

A more expressive formulation may instead involve structured edit operations such as:

`KEEP -> DELETE(span) -> REPLACE(span) -> NORMALIZE(span) -> INSERT(boundary) -> SPLIT/MERGE`

with attributes describing the required transformation.

The agent should investigate token classification, span classification, boundary prediction, structured edit prediction and other non-autoregressive approaches.

The objective is to identify edits rather than generate the entire sentence.

### Stage B — Deterministic Processing

A deterministic layer may execute transformations that are unambiguous and/or validate model output.

Potential responsibilities include:

* Applying explicitly selected formatting operations
* Safe ITN transformations
* Structural validation
* Number/entity preservation
* Detecting malformed output
* Rejecting unsafe or implausible edits
* Providing no-op behavior when appropriate

This layer should remain deliberately narrow.

### Stage C — Contextual Resolver

Some corrections cannot be reliably expressed as isolated token operations.

Speech repairs are the clearest example.

`Let's meet Friday, wait no, Saturday`

requires understanding that `Friday` was superseded by `Saturday`.

The resolver therefore receives broader context and selected edit regions and produces bounded corrections.

It should be investigated as a contextual editor rather than simply asking an autoregressive model to regenerate the entire transcript.

The ML engineer should also investigate whether the resolver is actually necessary for all tasks, whether one model can perform both stages effectively, or whether another architecture provides better quality/latency.

## 7. Candidate Planner Models

The current investigation should include, but not be limited to:

* ModernBERT-style encoder models
* Laya / System-One-style decision models
* GLiNER2.5-Decide or related structured decision/span models
* Other current non-autoregressive encoder or edit-planning architectures

One important research clarification is that Laya should not simply be treated as a fundamentally different encoder alternative to ModernBERT. Its English implementation is itself based on a ModernBERT-large backbone; the interesting distinction is its decision-oriented interface/training approach.

The agent should verify the current model landscape rather than relying on this initial shortlist.

## 8. Candidate Resolver Models

The resolver investigation should include small local SLMs across a range of model sizes.

The candidate pool should explicitly include:

* SmallLM around the 130M class
* LFM2.5-230M
* LFM2.5-350M
* Other small LFM-family variants
* Qwen small models
* Other compact models discovered through research

The purpose is not to select the smallest model.

The objective is to find the smallest model that provides acceptable contextual correction quality while meeting the actual deployment constraints.

## 9. Important Alternative Architectures to Investigate

The ML agent should compare the proposed two-stage architecture against at least these alternatives:

1. A single multi-task encoder performing punctuation, casing, ITN and disfluency/edit detection.

2. An encoder/tagging stage followed by a small generative resolver, similar in principle to two-stage neural speech-text normalization architectures.

3. A single small SLM performing complete transcript refinement.

4. A structured edit model that directly predicts spans and transformations without a separate resolver.

5. A hybrid architecture where deterministic normalization handles clearly safe cases and neural models handle only ambiguous transformations.

6. Architectures incorporating ASR confidence, word timestamps or N-best hypotheses if those materially improve correction quality.

The agent should recommend the architecture based on evidence rather than treating the currently proposed two-stage design as predetermined.

## 10. ASR Information

The initial system can operate on finalized text alone.

However, the architecture should investigate whether additional STT information can materially improve refinement:

`1-best transcript`

versus:

`1-best + confidence`

versus:

`N-best hypotheses + confidence/timing`

This is particularly relevant to semantic correction. A text-only refiner cannot recover information that the STT system completely failed to recognize unless alternative hypotheses or acoustic information are available.

This does not necessarily need to be part of the first implementation, but the interface should avoid unnecessarily preventing it later.

## 11. Dataset Strategy

Dataset development is a major part of this project and should happen before model selection is finalized.

The dataset should represent the actual transformation Vox needs rather than relying exclusively on generic text-generation datasets.

### Clean -> Spoken/ASR-like -> Refined

A useful training structure is:

`Clean written text -> synthetic spoken/ASR corruption -> expected refined text`

Synthetic corruption should cover phenomena such as:

* Missing punctuation
* Missing capitalization
* Spoken numbers
* Spoken dates
* Spoken currencies
* Fillers
* Repetitions
* False starts
* Speech repairs
* Backtracking
* Partial phrases
* Common ASR-style formatting errors
* Context-dependent replacements

The corruption generator should avoid producing unrealistic speech patterns.

### Real conversational speech

Public conversational/disfluency datasets should be investigated, including corpora containing repairs, repetitions and disfluencies.

These provide important examples that synthetic corruption may not reproduce accurately.

### Hard negatives / no-op examples

The dataset must contain clean and ambiguous examples where the correct action is to make no change.

This is critical.

A refiner that aggressively "improves" already-correct text can be more damaging than one that occasionally misses a formatting opportunity.

### Edit annotations

Where practical, the dataset should contain both:

`raw transcript -> final transcript`

and structured edit information describing what changed.

This allows the planner to be trained and evaluated independently from the resolver.

## 12. Dataset Splitting

The ML investigation should ensure that evaluation does not leak speakers, sessions or near-duplicate examples between training and evaluation sets.

The test set should deliberately contain difficult examples rather than being a random sample dominated by easy formatting cases.

The agent should recommend the exact split strategy after examining available datasets.

## 13. Evaluation

Evaluation should measure more than final text similarity.

Important metrics to investigate include:

* Edit precision / recall / F1
* Unnecessary-edit rate
* Exact-match or normalized text accuracy
* Punctuation accuracy
* Capitalization accuracy
* ITN accuracy
* Speech-repair resolution accuracy
* Semantic preservation
* Number/date/currency preservation
* Named-entity preservation
* No-op accuracy
* Resolver abstention quality
* End-to-end latency
* P50/P95 latency
* Memory consumption
* CPU/GPU/NPU utilization

The exact acceptance thresholds should be established after creating a representative benchmark and baseline.

No arbitrary threshold should be treated as a requirement before the benchmark exists.

## 14. Safety / Validation

The refinement engine should be conservative when confidence is low.

Potential validation mechanisms to investigate include:

* Number preservation
* Date/currency preservation
* Named-entity preservation
* URL/email/code preservation where applicable
* Length-change constraints
* Structural validity
* Confidence thresholds
* No-op / abstain outputs
* Comparison between raw and refined text
* Rejecting malformed model-produced edits

The existing dictation specification already identifies deterministic runtime guards such as number conservation and length-ratio validation as a direction for this future system.

The ML engineer should determine which guards are actually useful and whether additional validation is required.

## 15. End-to-End Workflow

The intended workflow should be evaluated as:

`Audio -> VAD -> STT -> raw transcript -> refinement planning -> deterministic transformations -> contextual resolution -> validation -> OutputRouter`

The user-facing behavior is:

`Speech -> live/raw transcript -> speech ends -> refinement -> final committed text`

The backend integration should preserve the existing dictation ownership, lifecycle and zero-LLM characteristics.

## 16. Proposed Research Order

The first phase should not begin by implementing models.

First, the ML agent should inspect the current dictation/STT interfaces and determine exactly what information is available at the refinement boundary.
The code and specs are avl in the vox/ dir already , main relevant file is dictation-spec.md in docs/specs .

Then establish a representative evaluation dataset and baseline raw-STT outputs.

Then research current architectures for speech disfluency removal, ITN, punctuation/casing, ASR correction and structured edit prediction.

Then benchmark candidate planner architectures.

Then benchmark candidate resolver architectures.

Then compare the proposed two-stage design against simpler alternatives.

Only after these results should the architecture and model combination be selected for implementation.

The implementation should then proceed around a clearly defined refinement contract, followed by integration testing and end-to-end latency measurement.

## 17. Initial Engineering Contract to Investigate

The agent should investigate whether the refinement subsystem should expose a contract conceptually equivalent to:

`RawTranscript -> RefinementRequest -> EditPlan -> RefinedTranscript`

The exact structures, events and ownership should be proposed after inspecting the current codebase.

The contract should ideally make it possible to observe:

* Raw transcript
* Planned edits
* Applied edits
* Final transcript
* Confidence/abstention information
* Refinement latency
* Validation failures

This will make both model development and production debugging substantially easier.

## 18. Implementation Scope

The initial implementation should focus on English.

The current dictation specification explicitly places Devanagari outside the initial refinement scope, with transliteration handled separately.

The first implementation should establish the refinement pipeline and evaluation methodology before expanding language coverage.

UI concepts such as a floating caret overlay are separate future work and should not be coupled to the refinement engine itself.

## 19. What Is Currently Finalised vs Open

### Architectural direction currently established

Dictation should remain separate from the conversational LLM pipeline.

The refinement stage belongs between STT and OutputRouter.

The system should favour constrained transformation over unrestricted regeneration.

Rules should remain a narrow deterministic execution/validation layer rather than becoming the primary linguistic intelligence.

The system must be benchmark-driven rather than based on assumed latency or model-size targets.

English is the initial refinement scope.

### Strong working hypothesis

A planner + deterministic layer + contextual resolver architecture is currently the leading design hypothesis.

Structured edit/span prediction is a promising direction for the planner.

A small local SLM is a promising direction for context-dependent resolution.

SmallLM-class models, LFM-family models, Qwen-family models and other compact models should be considered.

### Explicitly not finalised

The exact planner architecture.

The exact planner label/operation schema.

Whether the planner should be token-, span-, boundary- or edit-oriented.

Whether a separate resolver is actually necessary.

Whether the resolver should be autoregressive.

The exact resolver model.

Whether SmallLM 130M is sufficient.

Whether a larger model provides a worthwhile quality improvement.

The division of work between neural models and deterministic processing.

Whether ASR N-best/confidence information is required.

The exact staging behavior and integration point in the existing event/state system.

Latency targets.

Quality thresholds.

Dataset composition and weighting.

Production rollout criteria.

All of these should be determined through research, experiments and benchmarks.

## 20. ML Agent Mandate

The ML agent should treat this document as a research direction, not an implementation prescription.

For each major design decision it should:

`Inspect existing system -> research current approaches -> establish baseline -> benchmark alternatives -> identify tradeoffs -> recommend an approach`

The agent should explicitly challenge the proposed architecture where evidence suggests a simpler, faster or more reliable solution.

It should not select a model because it appears fashionable, small, fast according to a vendor claim, or conceptually aligned with the proposal.

It should produce measurable evidence for quality, false edits, latency, resource consumption and deployment feasibility.

The most important outcome of this phase is therefore not "build the proposed two-model pipeline."

It is to determine what architecture can reliably transform raw Vox dictation into useful written text while preserving user intent, remaining local and low-latency, and avoiding unnecessary modification of correct speech.
