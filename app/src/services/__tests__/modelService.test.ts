import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  fetchManifest,
  checkModelExists,
  downloadOptionalModel,
  deleteModel,
  startModelSetup,
  cancelModelSetup,
  completeSetupWizard,
  revealWizard,
  checkForUpdates,
  checkForModelUpdates,
  getRuntimeReport,
  getOnboardingStatus,
} from "../modelService";

describe("modelService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Manifest & Model Lifecycle Operations", () => {
    it("should fetch manifest", async () => {
      const mockManifest = { models_version: "1.0", total_size_bytes: 1000, model_groups: [] };
      mockInvoke.mockResolvedValueOnce(mockManifest);
      const res = await fetchManifest();
      expect(mockInvoke).toHaveBeenCalledWith("fetch_manifest");
      expect(res).toEqual(mockManifest);
    });

    it("should check model exists, download optional model, and delete model", async () => {
      mockInvoke.mockResolvedValueOnce(true);
      const exists = await checkModelExists("m1");
      expect(mockInvoke).toHaveBeenCalledWith("manage_models", { payload: { action: "exists", model_id: "m1" } });
      expect(exists).toBe(true);

      mockInvoke.mockResolvedValueOnce(undefined);
      await downloadOptionalModel("m1");
      expect(mockInvoke).toHaveBeenCalledWith("manage_models", { payload: { action: "download", model_id: "m1" } });

      mockInvoke.mockResolvedValueOnce(undefined);
      await deleteModel("m1");
      expect(mockInvoke).toHaveBeenCalledWith("manage_models", { payload: { action: "delete", model_id: "m1" } });
    });
  });

  describe("Setup Wizard Controls", () => {
    it("should start, cancel, complete, and reveal setup wizard", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await startModelSetup(["m1", "m2"]);
      expect(mockInvoke).toHaveBeenCalledWith("manage_models", { payload: { action: "start_setup", selected_ids: ["m1", "m2"] } });

      await cancelModelSetup();
      expect(mockInvoke).toHaveBeenCalledWith("manage_models", { payload: { action: "cancel" } });

      await completeSetupWizard();
      expect(mockInvoke).toHaveBeenCalledWith("complete_setup_wizard");

      await revealWizard();
      expect(mockInvoke).toHaveBeenCalledWith("reveal_wizard");
    });
  });

  describe("Updates & System Reports", () => {
    it("should check for app updates and model updates", async () => {
      const mockAppUpdate = { current_version: "0.8.7", latest_version: "0.8.7", update_available: false, release_notes: [], update_command: "" };
      mockInvoke.mockResolvedValueOnce({ app: mockAppUpdate });
      const appReport = await checkForUpdates();
      expect(mockInvoke).toHaveBeenCalledWith("check_updates", { scope: "app" });
      expect(appReport).toEqual(mockAppUpdate);

      const mockModelUpdate = { local_version: "1.0", remote_version: "1.0", update_available: false, outdated_models: [] };
      mockInvoke.mockResolvedValueOnce({ models: mockModelUpdate });
      const modelReport = await checkForModelUpdates();
      expect(mockInvoke).toHaveBeenCalledWith("check_updates", { scope: "models" });
      expect(modelReport).toEqual(mockModelUpdate);
    });

    it("should get runtime report and onboarding status", async () => {
      const mockRuntime = { write_access: true, available_space_gb: 50, setup_completed: true };
      mockInvoke.mockResolvedValueOnce(mockRuntime);
      const report = await getRuntimeReport();
      expect(mockInvoke).toHaveBeenCalledWith("get_runtime_report");
      expect(report).toEqual(mockRuntime);

      mockInvoke.mockResolvedValueOnce(true);
      const status = await getOnboardingStatus();
      expect(mockInvoke).toHaveBeenCalledWith("get_onboarding_status");
      expect(status).toBe(true);
    });
  });
});
