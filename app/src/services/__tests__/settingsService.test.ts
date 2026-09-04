import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  requestModelCatalog,
  getSettings,
  updateSetting,
  resetSettings,
  checkLlmProviderHealth,
  checkSttProviderHealth,
  checkTtsProviderHealth,
  listLlmModels,
  probeModelCapabilities,
  validateLlmTokenCap,
  listAudioDevices,
  listInputDevices,
  completeSetupWizard,
} from "../settingsService";

describe("settingsService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Settings Boot & Catalog", () => {
    it("should get settings and boot state", async () => {
      const mockBoot = { settings: {}, models_dir_exists: true, settings_path: "/path" };
      mockInvoke.mockResolvedValueOnce(mockBoot);
      const res = await getSettings();
      expect(mockInvoke).toHaveBeenCalledWith("get_settings");
      expect(res).toEqual(mockBoot);
    });

    it("should request model catalog", async () => {
      const mockCatalog = { llm: [], stt: [], tts: [] };
      mockInvoke.mockResolvedValueOnce(mockCatalog);
      const res = await requestModelCatalog();
      expect(mockInvoke).toHaveBeenCalledWith("get_model_catalog");
      expect(res).toEqual(mockCatalog);
    });
  });

  describe("Settings CRUD", () => {
    it("should get settings", async () => {
      const mockSettings = { ui: { theme: "dark" } };
      mockInvoke.mockResolvedValueOnce(mockSettings);
      const res = await getSettings();
      expect(mockInvoke).toHaveBeenCalledWith("get_settings");
      expect(res).toEqual(mockSettings);
    });

    it("should update single setting with domain, key, value", async () => {
      const updateRes = { applied: true, reload_policy: "None", message: "Updated" };
      mockInvoke.mockResolvedValueOnce(updateRes);
      const res = await updateSetting("ui", "theme", "dark");
      expect(mockInvoke).toHaveBeenCalledWith("update_setting", {
        domain: "ui",
        key: "theme",
        value: "dark",
      });
      expect(res).toEqual(updateRes);
    });

    it("should reset settings", async () => {
      const mockSettings = { ui: { theme: "system" } };
      mockInvoke.mockResolvedValueOnce(mockSettings);
      const res = await resetSettings();
      expect(mockInvoke).toHaveBeenCalledWith("reset_settings");
      expect(res).toEqual(mockSettings);
    });
  });

  describe("Provider Health & Model Probing", () => {
    it("should check provider health for LLM, STT, and TTS", async () => {
      mockInvoke.mockResolvedValue(true);

      const llmOk = await checkLlmProviderHealth();
      expect(mockInvoke).toHaveBeenCalledWith("check_provider_health", { kind: "llm", provider: undefined });
      expect(llmOk).toBe(true);

      const sttOk = await checkSttProviderHealth();
      expect(mockInvoke).toHaveBeenCalledWith("check_provider_health", { kind: "stt", provider: undefined });
      expect(sttOk).toBe(true);

      const ttsOk = await checkTtsProviderHealth();
      expect(mockInvoke).toHaveBeenCalledWith("check_provider_health", { kind: "tts", provider: undefined });
      expect(ttsOk).toBe(true);
    });

    it("should list LLM models and probe model capabilities", async () => {
      const mockModels = [{ id: "gpt-4o", name: "GPT-4o" }];
      mockInvoke.mockResolvedValueOnce(mockModels);
      const models = await listLlmModels();
      expect(mockInvoke).toHaveBeenCalledWith("list_llm_models", { provider: undefined });
      expect(models).toEqual(mockModels);

      const mockCaps = { supports_streaming: true, supports_tools: true };
      const mockProbeResult = {
        capabilities: mockCaps,
        validated_cap: null,
        cached_map: {},
      };
      mockInvoke.mockResolvedValueOnce(mockProbeResult);
      const caps = await probeModelCapabilities(undefined, "gpt-4o");
      expect(mockInvoke).toHaveBeenCalledWith("probe_model_capabilities", { provider: undefined, model_id: "gpt-4o", target_cap: undefined });
      expect(caps).toEqual(mockCaps);

      mockInvoke.mockResolvedValueOnce({
        capabilities: mockCaps,
        validated_cap: 4096,
        cached_map: {},
      });
      const ceiling = await validateLlmTokenCap(undefined, "gpt-4o", 2048);
      expect(mockInvoke).toHaveBeenCalledWith("probe_model_capabilities", { provider: undefined, model_id: "gpt-4o", target_cap: 2048 });
      expect(ceiling).toBe(4096);
    });
  });

  describe("Device & Setup Utilities", () => {
    it("should list audio devices and input devices", async () => {
      const mockDevices = [{ name: "Default Mic", is_default: true }];
      mockInvoke.mockResolvedValueOnce(mockDevices);
      const devices = await listAudioDevices("input");
      expect(mockInvoke).toHaveBeenCalledWith("list_audio_devices", { kind: "input" });
      expect(devices).toEqual(mockDevices);

      mockInvoke.mockResolvedValueOnce(mockDevices);
      const inDevices = await listInputDevices();
      expect(mockInvoke).toHaveBeenCalledWith("list_audio_devices", { kind: "input" });
      expect(inDevices).toEqual(mockDevices);
    });

    it("should complete setup wizard", async () => {
      mockInvoke.mockResolvedValueOnce(undefined);
      await completeSetupWizard();
      expect(mockInvoke).toHaveBeenCalledWith("complete_setup_wizard");
    });
  });
});
