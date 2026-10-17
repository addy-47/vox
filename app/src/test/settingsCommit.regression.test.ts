import { describe, it, expect, beforeEach } from "vitest";
import { useSettingsStore, type VoxSettings } from "../store/settingsStore";
import {
  setTransport,
  TauriTransport,
  type Transport,
  type UnlistenFn,
} from "../services/transport";
import { minimalSettings, SERVER_ACTIVE } from "@/test/harness/settingsFixtures";

/**
 * settings commit partial-failure isolation regression suite.
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT: commitChanges — domain-scoped patch dispatch, per-key failure
 * isolation (rejected keys stay in the draft and surface on their owning
 * card), and the `llm.servers` vehicle translating to `apply_setting_patch`.
 *
 * Production Entry Seam:
 *   useSettingsStore.getState().commitChanges / updateDraft /
 *   recomputeDomainFlags — the real store actions the settings cards invoke
 *   on save. Transport is the seam production explicitly swappable (services
 *   /transport.ts); `makeTransportStub` in harness/ is a Transport
 *   implementation, not a mock of the seam.
 *
 * Direction Check: the store action is the upstream trigger-side aggregator
 * the UI calls; results are observed in store state and recorded wire calls.
 *
 * Production Path:
 *   settings card (commit button) → commitChanges()
 *   → recomputeDomainFlags → apply_settings_patch per dirty domain
 *   → failure → triggerSaveFailure(keys) → draft restored for retry
 *   → success → settings ← get_settings (server-confirmed state)
 *
 * Observable Exit: failedSaveDomains, draftSettings residue, settings state,
 * and the exact `(cmd, args)` pairs sent.
 *
 * Phase 2a — Testability 4/4: real store constructor + real action +
 * real transport-swap seam; state assembled from the typed wire fixture
 * (no `as unknown as` escapes: field drift is a compile error).
 *
 * Phase 2b — False-Green table (each row a one-line production mutation):
 *   | triggerSaveFailure no-op'd → failedSaveDomains empty        | must fail |
 *   | accepted domain dropped from draft-merge → reverts silently | must fail |
 *   | rejected key pruned from draft → retry impossible           | must fail |
 *   | llm.servers sent as update_setting → zero patch calls       | must fail |
 *   | scoped commit fires all dirty domains → patch count ≠ 1     | must fail |
 *   (verified 2026-10-17: triggerSaveFailure no-op mutant killed)
 *
 * Phase 4 — Governing sentence:
 *   If failureбанner bookkeeping broke, this test would fail because
 *   failedSaveDomains["models"] would be undefined while expecting ["active"].
 */

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

  /** Transport stub over the shared harness: records every cmd, answers patch/get_settings. */
  function settingsTransport(overrides: {
    patchFor?: (patch: Record<string, unknown>) => { applied: true; reload_policy: "hot" | "restart"; message: string; restart_scheduled: boolean } | Error;
  } = {}): { stub: Transport; calls: Array<{ cmd: string; args?: Record<string, unknown> }> } {
    const calls: Array<{ cmd: string; args?: Record<string, unknown> }> = [];
    const stateOnServer: VoxSettings = minimalSettings();
    return {
      calls,
      stub: {
        invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
          calls.push({ cmd, args });
          if (cmd === "apply_settings_patch") {
            const patch = (args as { patch?: Record<string, unknown> })?.patch ?? {};
            const res = overrides.patchFor?.(patch) ?? { applied: true, reload_policy: "hot" as const, message: "ok", restart_scheduled: false };
            if (!(res instanceof Error)) {
              // Backend behavior under test: accepted keys land on the server
              // state get_settings later reports.
              Object.assign(stateOnServer, JSON.parse(JSON.stringify(patch)));
            }
            return (res instanceof Error ? Promise.reject(res) : Promise.resolve(res)) as unknown as Promise<T>;
          }
          if (cmd === "get_settings") {
            return Promise.resolve({ settings: JSON.parse(JSON.stringify(stateOnServer)), models_dir_exists: false, settings_path: "" }) as unknown as Promise<T>;
          }
          if (cmd === "get_model_catalog") {
            return Promise.resolve({
              llm: [], stt: [], tts: [], vad: [], auxiliary: [], model_groups: [], voices: [], preset_colors: [], model_parameters: {},
            }) as unknown as Promise<T>;
          }
          return Promise.resolve(null as unknown as T);
        },
        listen(): Promise<UnlistenFn> {
          return Promise.resolve(() => {});
        },
        convertFileSrc(filePath: string): string {
          return filePath;
        },
      } as Transport,
    };
  }

  it("NEGATIVE: an atomic patch rejection keeps every touched key in the draft, per-domain banners, server state unchanged", async () => {
    const saved = minimalSettings();
    const draft = minimalSettings();
    draft.persona.modular_prompt = "edited-persona";
    draft.llm.active = SERVER_ACTIVE;

    const { stub, calls } = settingsTransport({
      // Backend contract (src-tauri/src/ipc/settings.rs apply_settings_patch):
      // apply_settings_patch_tree validates the whole tree first and rejects
      // atomically — an errored patch leaves server settings untouched, no
      // per-key partial result exists.
      patchFor: () => new Error("Unknown setting: llm.active"),
    });
    setTransport(stub);

    useSettingsStore.setState({ settings: saved, draftSettings: draft });
    useSettingsStore.getState().recomputeDomainFlags();

    await useSettingsStore.getState().commitChanges();

    const state = useSettingsStore.getState();
    // Both touched keys failed — llm maps to the "models" card, persona to its
    // own card (SETTINGS_DOMAIN_TO_UI).
    expect(state.failedSaveDomains["models"]).toEqual(["active"]);
    expect(state.failedSaveDomains["persona"]).toEqual(["modular_prompt"]);
    // Both rejected values stay in the draft so they can be retried…
    expect(state.draftSettings?.llm?.active).toEqual(SERVER_ACTIVE);
    expect(state.draftSettings?.persona?.modular_prompt).toBe("edited-persona");
    // …and the re-read confirms the server never changed (atomic contract).
    expect(state.settings?.persona?.modular_prompt).toBe("a");
    expect(
      calls.filter((c) => c.cmd === "update_setting" && (c.args as { key?: string })?.key === "servers")
    ).toHaveLength(0);
  });

  it("NEGATIVE: domain-scoped rejection routes only that domain's key to its card banner", async () => {
    const saved = minimalSettings();
    const draft = minimalSettings();
    draft.dictation.enabled = true;

    const { stub } = settingsTransport({
      patchFor: () => new Error("Unknown setting: dictation.enabled"),
    });
    setTransport(stub);

    useSettingsStore.setState({ settings: saved, draftSettings: draft });
    useSettingsStore.getState().recomputeDomainFlags();

    await useSettingsStore.getState().commitChanges();

    const state = useSettingsStore.getState();
    // dictation maps to the "interaction" card, per settingsFlags.ts §17.
    expect(state.failedSaveDomains["interaction"]).toEqual(["enabled"]);
    expect(state.draftSettings?.dictation?.enabled).toBe(true);
  });

  it("scoped commit persists one key and leaves other dirty keys alone", async () => {
    const saved = minimalSettings();
    const draft = minimalSettings();
    draft.persona.modular_prompt = "edited-persona";
    draft.llm.cloud_keys = { nvidia: "nv-key" };

    const { stub, calls } = settingsTransport();
    setTransport(stub);

    useSettingsStore.setState({ settings: saved, draftSettings: draft });
    useSettingsStore.getState().recomputeDomainFlags();
    await useSettingsStore.getState().commitChanges({ domain: "llm", key: "cloud_keys" });

    const updates = calls.filter((c) => c.cmd === "apply_settings_patch");
    expect(updates).toHaveLength(1);
    expect(useSettingsStore.getState().draftSettings?.persona?.modular_prompt).toBe("edited-persona");
    expect(useSettingsStore.getState().failedSaveDomains["persona"]).toBeUndefined();
  });

  it("commits the servers vehicle as apply_settings_patch with llm.servers in it", async () => {
    const saved = minimalSettings();
    const draft = minimalSettings();

    const { stub, calls } = settingsTransport();
    setTransport(stub);

    useSettingsStore.setState({ settings: saved, draftSettings: draft });
    useSettingsStore.getState().recomputeDomainFlags();
    useSettingsStore.getState().updateDraft("llm", "servers", [
      { id: "s1", name: "One", url: "http://10.0.0.1:11434", model: "", protocol: "openai", api_key: null },
    ]);
    await useSettingsStore.getState().commitChanges();

    const serverCalls = calls.filter((c) => c.cmd === "apply_settings_patch");
    expect(serverCalls).toHaveLength(1);
    const patch = (serverCalls[0]?.args as { patch?: Record<string, unknown> })?.patch;
    expect(patch).toHaveProperty("llm");
    expect(patch?.llm).toHaveProperty("servers");
    expect(useSettingsStore.getState().failedSaveDomains["models"]).toBeUndefined();
  });
});
