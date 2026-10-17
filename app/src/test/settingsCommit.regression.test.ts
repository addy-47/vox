import { describe, it, expect, beforeEach } from "vitest";
import { useSettingsStore, type VoxSettings } from "../store/settingsStore";
import {
  setTransport,
  TauriTransport,
  type Transport,
  type UnlistenFn,
} from "../services/transport";

function minimalSettings(): VoxSettings {
  return {
    appearance: { theme: "dark", accent_seed: "#fff" },
    persona: { modular_prompt: "a", realtime_prompt: "b" },
    llm: {
      active: { category: "embedded", provider: null, model: "qwen" },
      last_selected: { embedded: "qwen" },
      cloud_keys: {},
    },
  } as unknown as VoxSettings;
}

describe("settings commit partial-failure isolation", () => {
  beforeEach(() => {
    setTransport(new TauriTransport());
    useSettingsStore.setState({
      settings: null,
      draftSettings: null,
      failedSaveDomains: {},
      restartKeys: [],
      hasChanges: false,
      isCommitting: false,
    });
  });

  it("keeps only rejected keys in the draft and preserves unrelated edits", async () => {
    const saved = minimalSettings();
    const draft = minimalSettings();
    draft.persona.modular_prompt = "edited-persona";
    draft.llm.active = { category: "server", provider: "srv-1", model: "" };

    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
    const mockTransport: Transport = {
      invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
        calls.push({ cmd, args });
        if (cmd === "apply_settings_patch") {
          const patch = (args as { patch: Record<string, unknown> }).patch;
          if (patch && Object.prototype.hasOwnProperty.call(patch, "llm")) {
            return Promise.reject(new Error("Unknown setting: llm.active")) as unknown as Promise<T>;
          }
          return Promise.resolve({
            applied: true,
            reload_policy: "hot",
            message: "ok",
            restart_scheduled: false,
          } as unknown as T);
        }
        if (cmd === "get_settings") {
          const merged = minimalSettings();
          merged.persona.modular_prompt = "edited-persona";
          return Promise.resolve({
            settings: merged,
            models_dir_exists: false,
            settings_path: "",
          } as unknown as T);
        }
        if (cmd === "get_model_catalog") {
          return Promise.resolve({
            llm: [],
            stt: [],
            tts: [],
            vad: [],
            auxiliary: [],
            model_groups: [],
            voices: [],
            preset_colors: [],
            model_parameters: {},
          } as unknown as T);
        }
        return Promise.resolve(null as unknown as T);
      },
      listen(): Promise<UnlistenFn> {
        return Promise.resolve(() => {});
      },
      convertFileSrc(filePath: string): string {
        return filePath;
      },
    };
    setTransport(mockTransport);

    useSettingsStore.setState({ settings: saved, draftSettings: draft });
    useSettingsStore.getState().recomputeDomainFlags();

    await useSettingsStore.getState().commitChanges();

    const state = useSettingsStore.getState();
    // The rejected llm key is reported on the models card…
    expect(state.failedSaveDomains["models"]).toEqual(["active"]);
    // …stays in the draft for retry…
    expect(state.draftSettings?.llm?.active).toEqual({
      category: "server",
      provider: "srv-1",
      model: "",
    });
    // …while the accepted persona edit lands and is no longer dirty.
    expect(state.settings?.persona?.modular_prompt).toBe("edited-persona");
    expect(state.draftSettings?.persona?.modular_prompt).toBe("edited-persona");
    expect(
      calls.filter(
        (c) => c.cmd === "update_setting" && (c.args as { key: string }).key === "servers"
      )
    ).toHaveLength(0);
  });

  it("scoped commit persists one key and leaves other dirty keys alone", async () => {
    const saved = minimalSettings();
    const draft = minimalSettings();
    draft.persona.modular_prompt = "edited-persona";
    draft.llm.cloud_keys = { nvidia: "nv-key" };

    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
    const mockTransport: Transport = {
      invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
        calls.push({ cmd, args });
        if (cmd === "apply_settings_patch") {
          return Promise.resolve({
            applied: true,
            reload_policy: "restart",
            message: "ok",
            restart_scheduled: false,
          } as unknown as T);
        }
        if (cmd === "get_settings") {
          const merged = minimalSettings();
          merged.llm.cloud_keys = { nvidia: "nv-key" };
          return Promise.resolve({
            settings: merged,
            models_dir_exists: false,
            settings_path: "",
          } as unknown as T);
        }
        if (cmd === "get_model_catalog") {
          return Promise.resolve({
            llm: [],
            stt: [],
            tts: [],
            vad: [],
            auxiliary: [],
            model_groups: [],
            voices: [],
            preset_colors: [],
            model_parameters: {},
          } as unknown as T);
        }
        return Promise.resolve(null as unknown as T);
      },
      listen(): Promise<UnlistenFn> {
        return Promise.resolve(() => {});
      },
      convertFileSrc(filePath: string): string {
        return filePath;
      },
    };
    setTransport(mockTransport);

    useSettingsStore.setState({ settings: saved, draftSettings: draft });
    useSettingsStore.getState().recomputeDomainFlags();
    await useSettingsStore.getState().commitChanges({ domain: "llm", key: "cloud_keys" });

    const updates = calls.filter((c) => c.cmd === "apply_settings_patch");
    expect(updates).toHaveLength(1);
    // The unrelated persona edit is still dirty in the draft, untouched.
    expect(useSettingsStore.getState().draftSettings?.persona?.modular_prompt).toBe(
      "edited-persona"
    );
    expect(useSettingsStore.getState().failedSaveDomains["persona"]).toBeUndefined();
  });

  it("commits the servers vehicle as update_setting(llm, servers)", async () => {
    const saved = minimalSettings();
    const draft = minimalSettings();

    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
    const mockTransport: Transport = {
      invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
        calls.push({ cmd, args });
        if (cmd === "apply_settings_patch") {
          return Promise.resolve({
            applied: true,
            reload_policy: "restart",
            message: "ok",
            restart_scheduled: false,
          } as unknown as T);
        }
        if (cmd === "get_settings") {
          return Promise.resolve({
            settings: minimalSettings(),
            models_dir_exists: false,
            settings_path: "",
          } as unknown as T);
        }
        if (cmd === "get_model_catalog") {
          return Promise.resolve({
            llm: [],
            stt: [],
            tts: [],
            vad: [],
            auxiliary: [],
            model_groups: [],
            voices: [],
            preset_colors: [],
            model_parameters: {},
          } as unknown as T);
        }
        return Promise.resolve(null as unknown as T);
      },
      listen(): Promise<UnlistenFn> {
        return Promise.resolve(() => {});
      },
      convertFileSrc(filePath: string): string {
        return filePath;
      },
    };
    setTransport(mockTransport);

    useSettingsStore.setState({ settings: saved, draftSettings: draft });
    useSettingsStore.getState().recomputeDomainFlags();
    useSettingsStore.getState().updateDraft("llm", "servers", [
      { id: "s1", name: "One", url: "http://10.0.0.1:11434", model: "", protocol: "openai", api_key: null },
    ]);
    await useSettingsStore.getState().commitChanges();

    const serverCalls = calls.filter((c) => c.cmd === "apply_settings_patch");
    expect(serverCalls).toHaveLength(1);
    expect(useSettingsStore.getState().failedSaveDomains["models"]).toBeUndefined();
  });
});
