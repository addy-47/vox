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
