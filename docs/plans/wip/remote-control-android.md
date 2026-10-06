# Vox Android + Remote Execution

**Status:** WIP  
**Type:** Feature / Product Requirements

## Overview

Bring Vox to Android as a full Vox client, while allowing the Android client to either execute locally on the device or use a configured Vox environment running on another machine.

The Android application should not be treated as a remote microphone or thin audio terminal. It should provide the Vox experience on Android, with the execution environment being configurable.

## Core Concept

The Android Vox client has two execution modes:

**Local mode** -> Android handles Vox execution locally using models/runtime available on the device, with cloud services such as the LLM still available where configured.

**Remote mode** -> Android acts as the Vox client while a configured remote Vox environment, initially the user's laptop, performs the processing and execution.

The user's interaction with Vox should remain conceptually the same regardless of which mode is active.

## Remote Mode

When remote execution is enabled, the Android client connects to the configured remote Vox environment.

The interaction becomes:

`Android Vox -> remote Vox environment -> STT / Harness / LLM / Tools / TTS -> Android Vox`

Audio and relevant runtime events need to be transported between the Android client and the remote environment with sufficiently low latency for conversational use.

The remote environment remains responsible for the core Vox runtime, including its existing conversational state, tools, models, memory, sessions, and other capabilities.

The Android client should therefore not need to duplicate the desktop execution stack merely to support remote operation.

## Local Mode

When remote execution is disabled, Android should be capable of operating as a standalone Vox client.

The architecture should allow Android to use locally available models where practical, while retaining cloud-based components such as the LLM when configured.

The existing Vox model architecture should be considered when determining which models can run on-device. The current model zoo is approximately 3 GB RAM, making local execution on modern Android hardware a viable target to investigate rather than assuming that Android must always depend on the laptop.

## Configuration

Remote execution should be controlled by an explicit configuration such as a `remote_control` setting.

Conceptually:

`remote_control = false -> local Android execution`

`remote_control = true -> configured remote Vox execution`

The remote environment should be configurable rather than hardcoded to a particular laptop or network.

Connection, pairing, discovery, authentication, and reconnection mechanisms remain to be determined during technical exploration.

## Product Goals

The feature should:

- Provide a genuine Android version of Vox rather than a companion microphone application.
- Allow the same Vox interaction model to operate locally or remotely.
- Allow the user's laptop to act as the execution environment when remote mode is enabled.
- Preserve Vox's existing conversational capabilities when operating remotely.
- Make remote execution feel like using Vox rather than controlling a separate application.
- Allow the Android client to eventually operate independently when sufficient local models/runtime are available.

## Audio Requirements

Remote mode requires bidirectional low-latency audio transport.

Android must be able to send microphone audio to the remote Vox environment and receive generated speech audio back.

The transport should be suitable for conversational realtime audio rather than being designed primarily around file transfer.

The existing Vox speaker/TTS ducking behaviour should remain compatible with remote operation.

Vox already handles the current speaker-mode echo problem through its existing ducking/VAD behaviour: when TTS is playing through the speaker, VAD can ignore that audio. This is therefore not considered a fundamental blocker for the remote feature.

A future improvement may make interruption handling more sophisticated, potentially allowing Vox to distinguish TTS audio from user speech using speaker/voice characteristics or embeddings. That is considered a separate, substantially more complex problem and is not required for the initial remote implementation.

## Technical Direction

The feature should first be investigated as an execution-environment abstraction rather than as an audio-streaming feature.

The important architectural question is:

**What boundary allows the Android Vox client to use either a local Android execution environment or a remote Vox execution environment without duplicating the product's interaction model?**

The implementation should build on existing Vox abstractions where possible rather than introducing a separate remote-specific conversational architecture.

Networking concerns such as realtime transport, jitter handling, reconnection, NAT traversal, encryption, and Android lifecycle behaviour are considered established engineering problems. The implementation should investigate suitable existing technologies and Rust/Android ecosystem solutions rather than treating these as problems that Vox needs to solve from first principles.

## Initial Scope

The first implementation should establish:

- Android Vox application/client.
- Local vs remote execution configuration.
- A configured remote Vox environment.
- Bidirectional realtime audio communication.
- Remote execution of the existing Vox conversational pipeline.
- Audio response playback on Android.
- Basic connection lifecycle and failure handling.
- A clean abstraction between the Android Vox client and its execution environment.

## Explicitly Out of Scope for Initial Version

Advanced speaker identification / embedding-based TTS rejection.

Sophisticated barge-in while TTS is playing.

A completely independent Android implementation of every desktop capability.

Inventing a custom realtime networking protocol when an established transport can satisfy the requirements.

## Open Technical Questions

The implementation phase should determine:

1. What is the correct execution-environment boundary between the Android client and Vox runtime?
2. Which existing Vox interfaces can be reused directly?
3. Which capabilities need to be exposed remotely?
4. What realtime transport is appropriate for audio and runtime events?
5. How should pairing, authentication, discovery, and reconnect work?
6. Which Vox models can run effectively on Android?
7. Which parts of the Android application should remain identical between local and remote execution?
8. How should sessions, memory, tools, and other laptop-side state behave when Android is operating remotely?
9. How should the system behave when the remote environment becomes unavailable?
10. Whether the remote execution model should eventually support multiple remote environments/devices.

## Success Criteria

A user should be able to install Vox on Android, configure a remote Vox environment, enable remote execution, and use Vox conversationally from the phone without needing to think about where the actual processing is occurring.

The same Android application should also be capable of operating without the remote environment when local execution is selected.