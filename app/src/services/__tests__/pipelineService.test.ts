import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  stopEngine,
  launchEngine,
  startSession,
  endSession,
  pauseSession,
  resumeSession,
  pttStart,
  pttStop,
  pttCancel,
  testClip,
  testClipCancel,
  getRuntimeSnapshot,
  startBackendRecording,
  stopBackendRecording,
  listVoices,
  renameVoice,
  addVoiceFromFile,
  addVoiceFromRecording,
  deleteVoice,
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

    it("should invoke start_session and end_session correctly", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await startSession();
      expect(mockInvoke).toHaveBeenCalledWith("start_session");

      await endSession();
      expect(mockInvoke).toHaveBeenCalledWith("end_session");
    });

    it("should invoke pause_session and resume_session", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await pauseSession();
      expect(mockInvoke).toHaveBeenCalledWith("pause_session");

      await resumeSession();
      expect(mockInvoke).toHaveBeenCalledWith("resume_session");
    });

    it("should invoke ptt_start, ptt_stop, and ptt_cancel correctly", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await pttStart();
      expect(mockInvoke).toHaveBeenCalledWith("ptt_start");

      await pttStop();
      expect(mockInvoke).toHaveBeenCalledWith("ptt_stop");

      await pttCancel();
      expect(mockInvoke).toHaveBeenCalledWith("ptt_cancel");
    });

    it("should invoke test_clip and test_clip_cancel", async () => {
      mockInvoke.mockResolvedValue(undefined);
      await testClip("command.wav");
      expect(mockInvoke).toHaveBeenCalledWith("test_clip", { clip_id: "command.wav" });

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
      expect(mockInvoke).toHaveBeenCalledWith("list_voices", { provider: undefined });
      expect(list).toEqual(mockVoices);

      mockInvoke.mockResolvedValueOnce(undefined);
      await deleteVoice("v1");
      expect(mockInvoke).toHaveBeenCalledWith("delete_voice", { id: "v1" });
    });

    it("should add voice from file and recording", async () => {
      const mockEntry = { id: "v2", name: "Voice 2", source_kind: "file", has_preview: false, created_at: 2000 };
      mockInvoke.mockResolvedValueOnce(mockEntry);
      const resFile = await addVoiceFromFile("Voice 2", "/path/to/sample.wav");
      expect(mockInvoke).toHaveBeenCalledWith("add_voice_from_file", { name: "Voice 2", file_path: "/path/to/sample.wav" });
      expect(resFile).toEqual(mockEntry);

      mockInvoke.mockResolvedValueOnce(mockEntry);
      const resRec = await addVoiceFromRecording("Voice 2", [0.5, -0.5], 16000);
      expect(mockInvoke).toHaveBeenCalledWith("add_voice_from_recording", { name: "Voice 2", pcm_f32: [0.5, -0.5], sample_rate: 16000 });
      expect(resRec).toEqual(mockEntry);
    });

    it("should fetch voices with provider filter and rename voice", async () => {
      const mockEdgeVoices = [{ id: "Aria", name: "Aria", source_kind: "edge", has_preview: true, created_at: 0 }];
      mockInvoke.mockResolvedValueOnce(mockEdgeVoices);
      const res = await listVoices("edge");
      expect(mockInvoke).toHaveBeenCalledWith("list_voices", { provider: "edge" });
      expect(res).toEqual(mockEdgeVoices);

      mockInvoke.mockResolvedValueOnce(undefined);
      await renameVoice("v1", "New Name");
      expect(mockInvoke).toHaveBeenCalledWith("rename_voice", { id: "v1", name: "New Name" });
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
      expect(mockInvoke).toHaveBeenCalledWith("setup_remote_server", {
        connection_string: config.connectionString,
        ssh_port: config.sshPort,
        identity_key_path: config.identityKeyPath,
        remote_path: config.remotePath,
        server_port: config.serverPort,
      });
    });
  });
});
