import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  stopEngine,
  launchEngine,
  engage,
  startRealtimeSession,
  stopRealtimeSession,
  pausePipeline,
  resumePipeline,
  pttStart,
  pttStop,
  testClip,
  testClipCancel,
  getRuntimeSnapshot,
  getRuntimeHistory,
  clearRuntimeHistory,
  getRealtimeSessionCache,
  startBackendRecording,
  stopBackendRecording,
  listVoices,
  addVoiceFromFile,
  addVoiceFromRecording,
  deleteVoice,
  fetchEdgeTtsVoices,
  setupRemoteServer,
} from "../pipelineService";

describe("pipelineService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Engine & Lifecycle Commands", () => {
    it("should invoke stop_engine correctly", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await stopEngine();
      expect(mockInvoke).toHaveBeenCalledWith("stop_engine");
    });

    it("should invoke launch_engine correctly", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await launchEngine();
      expect(mockInvoke).toHaveBeenCalledWith("launch_engine");
    });

    it("should invoke engage correctly", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await engage();
      expect(mockInvoke).toHaveBeenCalledWith("engage");
    });

    it("should invoke start_realtime_session and stop_realtime_session", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await startRealtimeSession();
      expect(mockInvoke).toHaveBeenCalledWith("start_realtime_session");

      await stopRealtimeSession();
      expect(mockInvoke).toHaveBeenCalledWith("stop_realtime_session");
    });

    it("should invoke pause_pipeline and resume_pipeline", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await pausePipeline();
      expect(mockInvoke).toHaveBeenCalledWith("pause_pipeline");

      await resumePipeline();
      expect(mockInvoke).toHaveBeenCalledWith("resume_pipeline");
    });

    it("should invoke ptt_start and ptt_stop with correct owner parameters", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await pttStart("MainWindow");
      expect(mockInvoke).toHaveBeenCalledWith("ptt_start", { owner: "MainWindow" });

      await pttStop("Tray");
      expect(mockInvoke).toHaveBeenCalledWith("ptt_stop", { owner: "Tray" });
    });

    it("should invoke test_clip and test_clip_cancel", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await testClip("command.wav");
      expect(mockInvoke).toHaveBeenCalledWith("test_clip", { clipId: "command.wav" });

      await testClipCancel();
      expect(mockInvoke).toHaveBeenCalledWith("test_clip_cancel");
    });
  });

  describe("Runtime Snapshots & Telemetry", () => {
    it("should fetch runtime snapshot", async () => {
      const mockSnapshot = { pipeline_state: "Idle", current_turn_id: 1 };
      mockInvoke.mockResolvedValueOnce(mockSnapshot);
      const res = await getRuntimeSnapshot();
      expect(mockInvoke).toHaveBeenCalledWith("get_runtime_snapshot");
      expect(res).toEqual(mockSnapshot);
    });

    it("should fetch runtime history and clear it", async () => {
      mockInvoke.mockResolvedValueOnce([{ pipeline_state: "Idle" }]);
      const history = await getRuntimeHistory();
      expect(mockInvoke).toHaveBeenCalledWith("get_runtime_history");
      expect(history).toHaveLength(1);

      mockInvoke.mockResolvedValueOnce(undefined);
      await clearRuntimeHistory();
      expect(mockInvoke).toHaveBeenCalledWith("clear_runtime_history");
    });

    it("should fetch realtime session cache", async () => {
      const mockCache = { has_session: true, provider: "openai", expires_in_seconds: 300, model: "gpt-4o" };
      mockInvoke.mockResolvedValueOnce(mockCache);
      const res = await getRealtimeSessionCache();
      expect(mockInvoke).toHaveBeenCalledWith("get_realtime_session_cache");
      expect(res).toEqual(mockCache);
    });
  });

  describe("Voice Recording & Voice Management", () => {
    it("should manage backend recording start and stop", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await startBackendRecording();
      expect(mockInvoke).toHaveBeenCalledWith("start_backend_recording");

      mockInvoke.mockResolvedValueOnce([[0.1, 0.2], 16000]);
      const res = await stopBackendRecording();
      expect(mockInvoke).toHaveBeenCalledWith("stop_backend_recording");
      expect(res).toEqual([[0.1, 0.2], 16000]);
    });

    it("should list voices and delete voice by id", async () => {
      const mockVoices = [{ id: "v1", name: "Custom Voice", source_kind: "file", has_preview: true, created_at: 1000 }];
      mockInvoke.mockResolvedValueOnce(mockVoices);
      const list = await listVoices();
      expect(mockInvoke).toHaveBeenCalledWith("list_voices");
      expect(list).toEqual(mockVoices);

      mockInvoke.mockResolvedValueOnce(undefined);
      await deleteVoice("v1");
      expect(mockInvoke).toHaveBeenCalledWith("delete_voice", { id: "v1" });
    });

    it("should add voice from file and recording", async () => {
      const mockEntry = { id: "v2", name: "Voice 2", source_kind: "file", has_preview: false, created_at: 2000 };
      mockInvoke.mockResolvedValueOnce(mockEntry);
      const resFile = await addVoiceFromFile("Voice 2", "/path/to/sample.wav");
      expect(mockInvoke).toHaveBeenCalledWith("add_voice_from_file", { name: "Voice 2", filePath: "/path/to/sample.wav" });
      expect(resFile).toEqual(mockEntry);

      mockInvoke.mockResolvedValueOnce(mockEntry);
      const resRec = await addVoiceFromRecording("Voice 2", [0.5, -0.5], 16000);
      expect(mockInvoke).toHaveBeenCalledWith("add_voice_from_recording", { name: "Voice 2", pcmF32: [0.5, -0.5], sampleRate: 16000 });
      expect(resRec).toEqual(mockEntry);
    });

    it("should fetch edge TTS voices", async () => {
      const mockEdgeVoices = [{ name: "en-US-AriaNeural", short_name: "Aria", gender: "Female", locale: "en-US", friendly_name: "Aria" }];
      mockInvoke.mockResolvedValueOnce(mockEdgeVoices);
      const res = await fetchEdgeTtsVoices();
      expect(mockInvoke).toHaveBeenCalledWith("fetch_edge_tts_voices");
      expect(res).toEqual(mockEdgeVoices);
    });
  });

  describe("Remote Deployment", () => {
    it("should setup remote server with configuration payload", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      const config = {
        connectionString: "user@host",
        sshPort: 22,
        identityKeyPath: "/id_rsa",
        remotePath: "/vox",
        serverPort: 8080,
      };
      await setupRemoteServer(config);
      expect(mockInvoke).toHaveBeenCalledWith("setup_remote_server", { ...config });
    });
  });
});
