# Phase 13 Backlog — Cloud TTS UI/UX, Quota Management & Guide Redesign

## 1. Cloud TTS Quota & Usage Transparency (Google Cloud & Azure AI Speech)
- **Problem**: Cloud TTS providers (Google Cloud TTS Chirp 3 HD, Azure AI Speech) have strict monthly free tier limits (e.g., 1,000,000 characters/month for Google Chirp 3 HD, 500,000 characters/month for Azure Speech). Beyond these limits, user accounts with active billing can incur pay-as-you-go charges.
- **Requirements**:
  - **Usage Metrics Tracking**: Track monthly character counts synthesized across cloud providers in the local SQLite database (`~/.vox/vox.db`).
  - **Visual Usage Meter & Transparency**: Display monthly usage progress bars in TTS provider settings, showing consumed characters vs. monthly free limit.
  - **Threshold Warnings & Safeguards**: Provide configurable alert banners and soft/hard budget ceilings in the UI when reaching 80%, 90%, and 100% of the free quota.
  - **Reset Cycle Sync**: Align tracking resets with monthly calendar boundaries or user-configured billing cycles.

## 2. Complete Azure AI Speech Integration & End-to-End User Flow
- **Problem**: Azure TTS preview synthesis is currently paused to prioritize Google Cloud TTS.
- **Requirements**:
  - **Credentials Management**: Support Azure region selection and API subscription keys in the interaction settings desk.
  - **Voice Discovery & Filtering**: Integrate Azure neural voice catalog browsing and filtering by locale, gender, and style.
  - **Streaming Synthesis & Benchmark**: Verify end-to-end low-latency streaming and execute automated TTS benchmarks across top Azure voices (`en-US-AvaMultilingualNeural`, `en-US-AndrewMultilingualNeural`, `en-US-EmmaMultilingualNeural`, `en-US-BrianMultilingualNeural`).
  - **Proactive Health Probing**: Health check verification endpoint validating region and subscription key validity on boot.

## 3. Dedicated Setup & Guide Full Modal Overhaul
- **Problem**: Currently, clicking "Setup and Guide" in the provider modal only replaces the inner grid section with a partial guide view, which is cramped and lacks dedicated navigational depth.
- **Requirements**:
  - **Full Modal State Replacement**: Clicking "Setup and Guide" transforms the modal into a dedicated walkthrough interface.
  - **Dynamic Left Rail Navigation**: The left facet rail replaces filter categories with dedicated guide topics:
    - *Google Cloud TTS*: ADC terminal authentication (`gcloud auth application-default login`), project setup, quota project linking.
    - *Azure AI Speech*: Azure portal resource creation, Speech Services key & region retrieval.
    - *Remote Server Setup*: SSH tunnel setup, remote Python/FastAPI environment, Chatterbox remote daemon configuration.
    - *Local Embedded TTS*: On-device ONNX runtime prerequisites, Kokoro / Supertonic weight management.
  - **Rich Interactive Content Panel**: The main body section displays structured prerequisites, terminal copy-paste snippets with inline copy buttons, diagnostics, and step-by-step troubleshooting.

## 4. Remote Chatterbox Server Provisioning (excluded from the Speech card)
- **Problem**: The Speech card lists the seven engines it can run without extra setup: four on-device (`supertonic`, `kokoro`, `zipvoice`, `chatterbox`) and three cloud (`edge_tts`, `google_tts`, `azure_tts`). The self-hosted Chatterbox engine (`chatterbox_remote`) is excluded because it needs a provisioned GPU host first. The previous `RemoteServerSetup` component was deleted: it was never reachable from the new card and its flow was incomplete.
- **Requirements**:
  - **Reintroduce the engine behind a gate**: show the remote engine in the grid only when at least one server is configured, or always with a `Configure server` action instead of a tier badge.
  - **Provisioning flow**: SSH target entry, codebase sync, GGUF model pull, and daemon start, with progress and failure states. This replaces the deleted component.
  - **Voice level**: the remote engine has its own voice inventory; it must reuse the same per-engine voice level the other engines use.
  - **Health**: per-server reachability and model-load status on the card, with a retry path.

## 5. Cloud TTS Credentials in the Speech Card
- **Problem**: The Speech card renders the three cloud engines as plain cards. Selecting one applies it, but the card offers no place to enter the Google ADC project or the Azure key/region, so a misconfigured cloud engine fails silently at synthesis time.
- **Requirements**:
  - **Configured state**: read the existing configured signal (`Google` = ADC present, `Azure` = stored key + region) and show it on the card without hiding the card.
  - **Credential entry**: a `Configure` action per cloud engine that opens a key/region form; no provider concept in the grid, only per-engine credentials.
  - **Failure surfacing**: a synthesis failure for a cloud engine must name the missing credential and link to the form.

## 6. Remote Control mDNS Subsystem & Dynamic IP Resolution (Ticket 4 Part B)
- **Problem**: Spec §6.1 mandates mDNS service advertisement (`_vox._tcp.local`) for automatic host discovery and dynamic IP address resolution. When a paired desktop host changes local IP address (e.g. DHCP lease renewal), the controller currently fails to reconnect unless the address is manually overridden.
- **Requirements**:
  - **Service Advertisement (`_vox._tcp.local`)**: Desktop host registers and maintains mDNS service publication carrying TXT records for `paired_peer` (device ID) and port.
  - **Controller Resolver Daemon**: Mobile controller listens for `_vox._tcp.local` mDNS service updates and dynamically updates `hints` in `paired_devices` on IP change.
  - **Subsystem Isolation**: Implement dedicated actor/daemon separated from `RemoteSessionManager` so network transport and discovery lifecycles remain orthogonal.
  - **Cross-Platform Compatibility**: Verify compatibility across Linux desktop (`zeroconf` / pure-Rust multicast DNS), macOS, Windows, and Android (`NsdManager` / native multicast socket).
