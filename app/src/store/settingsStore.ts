import { create } from "zustand";
import {
  getSettings,
  getRestartStatus,
  updateSetting,
  applySettingsPatch,
  resetSettings,
} from "@/services/settingsService";
import { useCatalogStore } from "@/store/catalogStore";
import { applyTheme } from "@/shared/theme";
import { ipcErrorMessage } from "@/shared/lib/ipcError";
import {
  DOMAIN_DIRTY_KEYS,
  SETTINGS_SCOPE_KEYS,
  type SettingsDomainId,
  type SettingsScope,
} from "@/data/settingsCopy";
import {
  settingsContractService,
  type SettingsContract,
} from "@/services/settingsService";
import {
  CLOUD_PROVIDERS,
  updateCloudProvidersFromContract,
  updateTtsProvidersFromContract,
} from "@/data/providersCopy";
import { getEffectiveCloudApiKey } from "@/shared/lib/activeSelection";

export type * from "@/types/settings";
import type {
  VoxSettings,
  DomainFlags,
  DraftKeyMap,
  LlmProviderConfig,
  ModelCatalog,
  ModelCapabilities,
  LlmModelInfo,
} from "@/types/settings";
import {
  scopeEntries,
  SETTINGS_DOMAIN_TO_UI,
  COMMIT_DOMAIN_IDS,
  MODEL_CATALOG_SCOPES,
  isRestartKey,
  EMPTY_DOMAIN_FLAGS,
  checkDomainDirty,
  checkDomainRequiringRestart,
  checkCategoryDirty,
} from "@/store/settingsFlags";

export { SETTINGS_DOMAIN_TO_UI, COMMIT_DOMAIN_IDS, isRestartKey };

export type TtsVoiceSource = "catalog" | "custom" | "edge" | "none";

export interface ParamRange {
  min: number;
  max: number;
  step: number;
}

export interface ProviderCaps {
  voices: TtsVoiceSource;
  clone: boolean;
  speed_range: ParamRange;
}

export interface SettingsState {
  settings: VoxSettings | null;
  draftSettings: VoxSettings | null;
  modelCatalog: ModelCatalog | null;
  remoteModels: LlmModelInfo[];
  loadingRemoteModels: boolean;
  remoteModelsError: string | null;
  remoteModelsFetchedKey: string | null;
  capabilitiesCache: Record<string, ModelCapabilities>;
  capabilitiesCacheError: string | null;
  isLoading: boolean;
  hasChanges: boolean;
  /**
   * `domain.key` entries the backend classified `SettingReloadPolicy::Restart`
   * during the most recent commit. Authoritative: the backend executed the
   * rebuild. The UI must never reconstruct this from a local key list.
   */
  restartKeys: string[];
  /** True while the backend's coalesced engine restart is running. */
  restartInFlight: boolean;
  error: string | null;
  /** Canonical settings contract from backend (supported keys, reload policies, providers, defaults). */
  contract: SettingsContract | null;

  loadContract: () => Promise<SettingsContract | null>;
  loadSettings: () => Promise<void>;
  loadModelCatalog: () => Promise<void>;
  loadRemoteModels: (providerConfig?: LlmProviderConfig, force?: boolean) => Promise<void>;
  clearRemoteModelsError: () => void;
  loadCapabilitiesCache: () => Promise<void>;
  patchRemoteModelCapabilities: (modelId: string, caps: ModelCapabilities) => void;
  updateDraft: <D extends keyof VoxSettings>(
    domain: D,
    key: DraftKeyMap[D],
    value: unknown,
    explicitDomainId?: SettingsDomainId
  ) => void;
  /**
   * Persists draft changes. Without `only`, commits every dirty key.
   * With `only`, commits exactly one key (e.g. an inventory delete from a
   * trash icon) so unrelated half-typed edits are never swept along.
   */
  commitChanges: (only?: { domain: keyof VoxSettings; key: string }) => Promise<void>;
  /**
   * Commit any pending autosave immediately (same guards as the 600ms timer).
   * Called when a settings card closes so hot changes survive instead of
   * dying with the timer. Restart-classified dirty state is left untouched
   * in the draft — survive, not apply, not prompt.
   */
  flushPendingAutosave: () => void;
  trackRestartCompletion: () => void;
  discardChanges: () => void;
  isDomainDirty: (domainId: string) => boolean;
  isDomainRequiringRestart: (domainId: string) => boolean;
  discardDomainChanges: (domainId: string) => void;
  isCategoryDirty: (category: string) => boolean;
  discardCategoryChanges: (category: string) => void;
  restoreDefaults: () => Promise<void>;
  toggleTheme: () => void;
  isCommitting: boolean;
  autoSavedDomain: string | null;
  lastSavedTimestamp: number;
  triggerAutoSaveToast: (domainId: string) => void;
  /**
   * Domains whose most recent commit was rejected by the backend.
   * Value is the rejected key list for that domain — it was a single flat
   * `failedSaveKeys` array shared by every domain, so two simultaneous
   * failures made one card's banner print the other card's key names.
   */
  failedSaveDomains: Record<string, string[]>;
  triggerSaveFailure: (domainId: string, keys: string[]) => void;
  /**
   * Transient desk-level error (provider health / model-list failure) shown
   * in the card footer like every other banner — never inline inside the
   * desk, which clutters the card. Owned by the desk that sets it; the card
   * wrapper only renders the entry matching its own domain.
   */
  deskFooter: { domain: string; title: string; detail: string } | null;
  setDeskFooter: (entry: { domain: string; title: string; detail: string } | null) => void;
  /**
   * Per-domain dirty / restart-required flags, recomputed ONCE per store
   * mutation instead of once per subscriber.
   *
   * `isDomainDirty` and `isDomainRequiringRestart` are `JSON.stringify` walks
   * over ~57 declared keys plus a whole-scope TTS comparison. They were invoked
   * directly as Zustand selectors, so every one of the six always-mounted
   * `SettingsCardWrapper`s ran both on every `set()` — roughly 1,100 stringify
   * calls per keystroke, inside the same task as a theme flip. Selectors are
   * now O(1) lookups against this snapshot.
   */
  domainFlags: Record<string, DomainFlags>;
  /** Recomputes `domainFlags` + `hasChanges` from current state. Call once per
   *  mutation, after the state it describes has been written. */
  recomputeDomainFlags: () => Record<string, DomainFlags>;
}

let appearanceDebounceTimer: ReturnType<typeof setTimeout> | null = null;
let appearanceDebounceGeneration = 0;
let settingsAutoSaveTimer: ReturnType<typeof setTimeout> | null = null;
let autoSaveToastTimer: ReturnType<typeof setTimeout> | null = null;
let saveFailureTimers = new Map<string, ReturnType<typeof setTimeout>>();

let restartPollInterval: ReturnType<typeof setInterval> | null = null;

/** Poll cadence for mirroring the backend's restart progress. */
const RESTART_POLL_INTERVAL_MS = 500;
/** Upper bound so a failed restart cannot wedge the UI in "restarting" (30 * 500ms = 15s). */
const RESTART_POLL_MAX_TICKS = 30;

export const useSettingsStore = create<SettingsState>((set, get) => ({
  settings: null,
  draftSettings: null,
  modelCatalog: null,
  remoteModels: [],
  loadingRemoteModels: false,
  remoteModelsError: null,
  remoteModelsFetchedKey: null,
  capabilitiesCache: {},
  capabilitiesCacheError: null,
  isLoading: true,
  hasChanges: false,
  restartKeys: [],
  restartInFlight: false,
  error: null,
  contract: null,
  domainFlags: EMPTY_DOMAIN_FLAGS,

  loadContract: async () => {
    try {
      const contract = await settingsContractService.getSettingsContract();
      if (contract) {
        updateCloudProvidersFromContract(contract.llm_cloud_providers);
        updateTtsProvidersFromContract(contract.tts_providers);
        set({ contract });
      }
      return contract;
    } catch (err) {
      console.warn("[SettingsStore] Failed to load settings contract:", err);
      return null;
    }
  },

  /**
   * Recomputes the per-domain dirty/restart snapshot from the CURRENT state.
   * Called once per mutation, never inside a `set()` updater — reading the
   * store from an updater would be a side effect in a pure callback.
   */
  recomputeDomainFlags: () => {
    const flags: Record<string, DomainFlags> = {};
    for (const id of COMMIT_DOMAIN_IDS) {
      flags[id] = {
        dirty: get().isDomainDirty(id),
        requiresRestart: get().isDomainRequiringRestart(id),
      };
    }
    const hasChanges = COMMIT_DOMAIN_IDS.some((id) => flags[id].dirty);
    set({ domainFlags: flags, hasChanges });
    return flags;
  },

  loadSettings: async () => {
    try {
      get().loadContract().catch((err) => {
        console.warn("[SettingsStore] Non-blocking contract load error:", err);
      });

      const bootState = await getSettings();
      const fetched = bootState.settings;
      const cloned = structuredClone(fetched);

      set((state) => {
        if (state.draftSettings && state.draftSettings.appearance && cloned.appearance) {
          cloned.appearance.theme = state.draftSettings.appearance.theme;
          cloned.appearance.accent_seed = state.draftSettings.appearance.accent_seed;
        }
        return {
          settings: fetched,
          draftSettings: state.hasChanges ? state.draftSettings : cloned,
          isLoading: false,
          hasChanges: state.hasChanges,
          error: null,
        };
      });
      get().recomputeDomainFlags();
      applyTheme(fetched.appearance, { animate: false });
    } catch (err) {
      const reason = ipcErrorMessage(err);
      console.error("Failed to load settings:", err);
      set({ isLoading: false, error: reason || "Failed to load settings" });
    }
  },

  loadModelCatalog: async () => {
    await useCatalogStore.getState().loadModelCatalog();
  },

  loadRemoteModels: async (providerConfig?: LlmProviderConfig, force = false) => {
    await useCatalogStore.getState().loadRemoteModels(providerConfig, force);
  },

  clearRemoteModelsError: () => {
    useCatalogStore.getState().clearRemoteModelsError();
  },

  loadCapabilitiesCache: async () => {
    await useCatalogStore.getState().loadCapabilitiesCache();
  },

  patchRemoteModelCapabilities: (modelId, caps) => {
    useCatalogStore.getState().patchCapabilities(modelId, caps);
  },

  lastSavedTimestamp: 0,
  autoSavedDomain: null as string | null,
  failedSaveDomains: {} as Record<string, string[]>,
  deskFooter: null as { domain: string; title: string; detail: string } | null,
  setDeskFooter: (entry) => {
    const cur = get().deskFooter;
    if (
      cur?.domain === entry?.domain &&
      cur?.title === entry?.title &&
      cur?.detail === entry?.detail
    ) {
      return;
    }
    set({ deskFooter: entry });
  },
  triggerSaveFailure: (domainId: string, keys: string[]) => {
    set((state) => ({
      failedSaveDomains: { ...state.failedSaveDomains, [domainId]: keys },
    }));
    // Re-arming without clearing the old timer let an earlier failure's timer
    // clear a newer failure's banner early.
    const existing = saveFailureTimers.get(domainId);
    if (existing) clearTimeout(existing);
    saveFailureTimers.set(
      domainId,
      setTimeout(() => {
        saveFailureTimers.delete(domainId);
        set((state) => {
          if (!state.failedSaveDomains[domainId]) return {};
          const next = { ...state.failedSaveDomains };
          delete next[domainId];
          return { failedSaveDomains: next };
        });
      }, 6000)
    );
  },
  triggerAutoSaveToast: (domainId: string) => {
    set({ autoSavedDomain: domainId, lastSavedTimestamp: Date.now() });
    // Same re-arm bug as above: toggling the theme twice inside 1800ms had the
    // first timer clear the second toast 800ms early, so the "Changes Saved"
    // footer visibly truncated on rapid edits.
    if (autoSaveToastTimer) clearTimeout(autoSaveToastTimer);
    autoSaveToastTimer = setTimeout(() => {
      autoSaveToastTimer = null;
      set((state) =>
        state.autoSavedDomain === domainId ? { autoSavedDomain: null } : {}
      );
    }, 1800);
  },

  updateDraft: <D extends keyof VoxSettings>(
    domain: D,
    key: DraftKeyMap[D],
    value: unknown,
    explicitDomainId?: SettingsDomainId
  ) => {
    const { settings, draftSettings } = get();
    if (!draftSettings || !settings) return;

    const currentVal = scopeEntries(draftSettings[domain])[key];
    if (JSON.stringify(currentVal) === JSON.stringify(value)) return;

    const newDraft = {
      ...draftSettings,
      [domain]: {
        ...scopeEntries(draftSettings[domain]),
        [key]: value,
      },
    };

    if (domain === "appearance" && (key === "theme" || key === "accent_seed")) {
      applyTheme(newDraft.appearance);

      if (appearanceDebounceTimer) {
        clearTimeout(appearanceDebounceTimer);
      }
      // Generation stamp: a second edit while the first IPC is in flight must
      // not have the older response overwrite `settings` with a stale value.
      // The old code nulled the timer handle inside the fired callback, so a
      // concurrent update started an unguarded second timer, and a late
      // resolution left the card permanently dirty.
      const generation = ++appearanceDebounceGeneration;
      appearanceDebounceTimer = setTimeout(() => {
        appearanceDebounceTimer = null;
        updateSetting("appearance", key, value)
          .then(() => {
            if (generation !== appearanceDebounceGeneration) return;
            const curSettings = get().settings;
            if (curSettings) {
              set({
                settings: {
                  ...curSettings,
                  appearance: {
                    ...curSettings.appearance,
                    [key]: value,
                  },
                },
              });
            }
            get().triggerAutoSaveToast("appearance");
          })
          .catch(console.error);
      }, 200);

      // Derived flags first, then a single write, so subscribers see the draft
      // and its dirty state in one render rather than two.
      set({ draftSettings: newDraft });
      get().recomputeDomainFlags();
      return;
    }

    const clearsRemoteError = key === "active" && Boolean(get().remoteModelsError);
    set({
      draftSettings: newDraft,
      ...(clearsRemoteError ? { remoteModelsError: null } : {}),
    });
    const flags = get().recomputeDomainFlags();

    if (!flags || !COMMIT_DOMAIN_IDS.some((d) => flags[d]?.dirty)) {
      if (settingsAutoSaveTimer) {
        clearTimeout(settingsAutoSaveTimer);
        settingsAutoSaveTimer = null;
      }
      return;
    }

    const targetDomainId = explicitDomainId || SETTINGS_DOMAIN_TO_UI[domain as string] || "models";
    const needsRestart = flags[targetDomainId]?.requiresRestart ?? false;

    // If this change or domain requires an engine restart, do NOT auto-commit.
    // Leave the card dirty with "Apply & Restart" button visible.
    if (needsRestart) {
      if (settingsAutoSaveTimer) {
        clearTimeout(settingsAutoSaveTimer);
        settingsAutoSaveTimer = null;
      }
      return;
    }

    // Hot or WorkerCommand: Automatically commit with 600ms debounce and flash "Saved" toast on that specific card
    if (settingsAutoSaveTimer) {
      clearTimeout(settingsAutoSaveTimer);
    }
    settingsAutoSaveTimer = setTimeout(() => {
      settingsAutoSaveTimer = null;
      const anyNeedsRestart = COMMIT_DOMAIN_IDS.some(
        (d) => get().domainFlags[d]?.requiresRestart
      );
      if (anyNeedsRestart) {
        return;
      }
      get()
        .commitChanges()
        .then(() => {
          get().triggerAutoSaveToast(targetDomainId);
        })
        .catch(console.error);
    }, 600);
  },

  isDomainDirty: (domainId: string) =>
    checkDomainDirty(get().settings, get().draftSettings, domainId),

  isDomainRequiringRestart: (domainId: string) =>
    checkDomainRequiringRestart(get().settings, get().draftSettings, domainId),

  isCategoryDirty: (category: string) =>
    checkCategoryDirty(get().settings, get().draftSettings, category),

  discardCategoryChanges: (category: string) => {
    const { settings, updateDraft } = get();
    if (!settings) return;

    const scope = category.toLowerCase() as keyof VoxSettings;
    const savedScope = scopeEntries(settings?.[scope]);
    if (!savedScope) return;

    const contractKeys = settingsContractService.getSupportedKeys(scope);
    const keys = contractKeys && contractKeys.length > 0 ? contractKeys : SETTINGS_SCOPE_KEYS[scope as SettingsScope];
    if (keys && keys.length > 0) {
      keys.forEach((k: string) => updateDraft(scope, k as DraftKeyMap[typeof scope], savedScope[k]));
    } else {
      Object.keys(savedScope).forEach((k) => updateDraft(scope, k as DraftKeyMap[typeof scope], savedScope[k]));
    }
  },

  discardDomainChanges: (domainId: string) => {
    if (settingsAutoSaveTimer) {
      clearTimeout(settingsAutoSaveTimer);
      settingsAutoSaveTimer = null;
    }

    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return;

    const dirtyKeys = DOMAIN_DIRTY_KEYS[domainId as SettingsDomainId];
    if (!dirtyKeys) return;

    const newDraft = structuredClone(draftSettings);

    for (const rule of dirtyKeys) {
      const scope = rule.scope as keyof VoxSettings;
      const savedScope = scopeEntries(settings[scope]);
      if (savedScope) {
        if (!newDraft[scope]) {
          (newDraft as unknown as Record<string, Record<string, unknown>>)[scope] = {};
        }
        if (rule.keys) {
          rule.keys.forEach((k: string) => {
            (newDraft as unknown as Record<string, Record<string, unknown>>)[scope][k] = structuredClone(savedScope[k]);
          });
        } else {
          (newDraft as unknown as Record<string, unknown>)[scope] = structuredClone(savedScope);
        }
      }
    }

    if (domainId === "appearance") {
      applyTheme(settings.appearance);
    }

    set({ draftSettings: newDraft, autoSavedDomain: null });
    get().recomputeDomainFlags();
  },

  isCommitting: false,
  commitChanges: async (only) => {
    const { settings, draftSettings } = get();
    if (!settings || !draftSettings) return;

    set({ isCommitting: true });
    const restartKeys: string[] = [];
    const failures: { domain: string; key: string; reason: string }[] = [];
    const touchedScopes = new Set<string>();
    const touchedKeys = new Set<string>();
    let restartScheduled = false;

    const canonicalDomains: (keyof VoxSettings)[] = [
      "vad",
      "stt",
      "llm",
      "tts",
      "realtime",
      "interaction",
      "dictation",
      "working_memory",
      "appearance",
      "personal_memory",
      "persona",
      "system",
    ];

    const dirtyTree: Record<string, Record<string, unknown>> = {};

    for (const domain of canonicalDomains) {
      const draftObj = draftSettings[domain];
      const savedObj = settings[domain];
      if (!draftObj || typeof draftObj !== "object") continue;

      const savedMap = new Map<string, unknown>(
        savedObj && typeof savedObj === "object" ? Object.entries(savedObj) : []
      );

      for (const [key, val] of Object.entries(draftObj)) {
        const oldVal = savedMap.get(key);

        if (JSON.stringify(val) === JSON.stringify(oldVal)) continue;
        if (only && (domain !== only.domain || key !== only.key)) continue;

        touchedScopes.add(domain);
        touchedKeys.add(`${domain}.${key}`);
        if (!dirtyTree[domain]) dirtyTree[domain] = {};
        dirtyTree[domain][key] = val;
      }
    }

    try {
      if (Object.keys(dirtyTree).length > 0) {
        const res = await applySettingsPatch(dirtyTree);
        if (res?.reload_policy === "restart") {
          restartKeys.push(...touchedKeys);
        }
        if (res?.restart_scheduled) {
          restartScheduled = true;
        }
      }

      if (failures.length > 0) {
        // Re-read from the backend so the draft reflects what was actually
        // stored, but keep ONLY the rejected keys from the pre-commit draft so
        // they can be retried. Every other key landed and must not linger as
        // dirty — and unrelated edits must never be discarded with them.
        const bootState = await getSettings();
        const fetched = bootState.settings;
        const merged = structuredClone(fetched);
        for (const f of failures) {
          const draftScope = scopeEntries(draftSettings[f.domain as keyof VoxSettings]);
          const mergedScope = scopeEntries(merged[f.domain as keyof VoxSettings]);
          if (draftScope && mergedScope && f.key in draftScope) {
            mergedScope[f.key] = structuredClone(draftScope[f.key]);
          }
        }
        set({
          settings: fetched,
          draftSettings: merged as VoxSettings,
          hasChanges: true,
          isLoading: false,
        });
        get().recomputeDomainFlags();
        const failedByUiDomain = new Map<string, string[]>();
        for (const f of failures) {
          const uiDomain = SETTINGS_DOMAIN_TO_UI[f.domain] ?? "models";
          const keys = failedByUiDomain.get(uiDomain);
          if (keys) keys.push(f.key);
          else failedByUiDomain.set(uiDomain, [f.key]);
        }
        for (const [uiDomain, keys] of failedByUiDomain) {
          get().triggerSaveFailure(uiDomain, keys);
        }
        return;
      }

      set({ hasChanges: false });
      const bootState = await getSettings();
      const fetched = bootState.settings;
      const cloned = structuredClone(fetched);
      if (only) {
        // A scoped commit must not discard other dirty keys: restore every
        // pre-commit draft value that differs from the fresh snapshot except
        // the key just committed.
        const preDraft = draftSettings;
        for (const domain of canonicalDomains) {
          const preScope = scopeEntries(preDraft[domain]);
          const savedScope = scopeEntries(settings[domain]);
          const clonedScope = scopeEntries(cloned[domain]);
          if (!preScope || !clonedScope) continue;
          for (const key of Object.keys(preScope)) {
            if (domain === only.domain && key === only.key) continue;
            if (JSON.stringify(preScope[key]) !== JSON.stringify(savedScope?.[key])) {
              clonedScope[key] = structuredClone(preScope[key]);
            }
          }
        }
      }
      set({ settings: fetched, draftSettings: cloned, hasChanges: false, isLoading: false });
      get().recomputeDomainFlags();

      // Only re-read the catalog when a committed key actually feeds it. The
      // catalog is manifest + voices + model parameters + the TTS
      // active_voice_id; provider inventory, selections, keys, and prompts
      // never appear in it, so reloading for those is a full manifest
      // re-parse plus database round trips that remount the catalog views
      // (the visible flicker after every server save).
      const catalogAffected =
        touchedKeys.has("tts.active_voice_id") ||
        [...touchedScopes].some((scope) => MODEL_CATALOG_SCOPES.has(scope));
      if (catalogAffected) {
        await get().loadModelCatalog().catch(console.error);
      }

      // Replace wholesale rather than merging: a key that stopped being
      // restart-classified must not linger in the banner on the next commit.
      set({ restartKeys, restartInFlight: restartScheduled });
      if (restartScheduled) {
        // The restart is asynchronous and coalesced. Poll until the backend's
        // runner has finished so the "restarting" state cannot get stuck on.
        get().trackRestartCompletion();
      }
    } catch (err) {
      console.error("[Settings] Failed to persist settings patch:", err);
      const reason = ipcErrorMessage(err);
      for (const key of touchedKeys) {
        const [domain, k] = key.split(".");
        failures.push({ domain, key: k, reason });
      }
      const bootState = await getSettings();
      const fetched = bootState.settings;
      const merged = structuredClone(fetched);
      for (const f of failures) {
        const draftScope = scopeEntries(draftSettings[f.domain as keyof VoxSettings]);
        const mergedScope = scopeEntries(merged[f.domain as keyof VoxSettings]);
        if (draftScope && mergedScope && f.key in draftScope) {
          mergedScope[f.key] = structuredClone(draftScope[f.key]);
        }
      }
      set({
        settings: fetched,
        draftSettings: merged as VoxSettings,
        hasChanges: true,
        isLoading: false,
      });
      get().recomputeDomainFlags();
      const failedByUiDomain = new Map<string, string[]>();
      for (const f of failures) {
        const uiDomain = SETTINGS_DOMAIN_TO_UI[f.domain] ?? "models";
        const keys = failedByUiDomain.get(uiDomain);
        if (keys) keys.push(f.key);
        else failedByUiDomain.set(uiDomain, [f.key]);
      }
      for (const [uiDomain, keys] of failedByUiDomain) {
        get().triggerSaveFailure(uiDomain, keys);
      }
      return;
    } finally {
      set({ isCommitting: false });
    }
  },

  flushPendingAutosave: () => {
    if (!settingsAutoSaveTimer) return;
    clearTimeout(settingsAutoSaveTimer);
    settingsAutoSaveTimer = null;
    const anyNeedsRestart = COMMIT_DOMAIN_IDS.some(
      (d) => get().domainFlags[d]?.requiresRestart
    );
    if (anyNeedsRestart) {
      return;
    }
    // No toast here: the card is closing, so there is nothing to flash it on.
    // The persisted values are visible when the card reopens.
    get()
      .commitChanges()
      .catch(console.error);
  },

  /**
   * Mirrors the backend's `restart_in_flight` / `restart_requested` flag into the store.
   *
   * Polls `getRestartStatus()` every 500ms while restarting is in flight. Clears
   * `restartInFlight` as soon as the engine has restarted or after a 15-second
   * safety timeout so a failed restart cannot wedge the UI in "restarting".
   */
  trackRestartCompletion: () => {
    if (restartPollInterval) {
      clearInterval(restartPollInterval);
      restartPollInterval = null;
    }

    let ticks = 0;
    restartPollInterval = setInterval(async () => {
      ticks += 1;
      if (ticks > RESTART_POLL_MAX_TICKS || !get().restartInFlight) {
        if (restartPollInterval) {
          clearInterval(restartPollInterval);
          restartPollInterval = null;
        }
        if (get().restartInFlight) {
          set({ restartInFlight: false });
        }
        return;
      }

      try {
        const status = await getRestartStatus();
        if (!status.restarting) {
          if (restartPollInterval) {
            clearInterval(restartPollInterval);
            restartPollInterval = null;
          }
          if (get().restartInFlight) {
            set({ restartInFlight: false });
          }
        }
      } catch (err) {
        console.warn("[SettingsStore] Failed to poll restart status:", err);
      }
    }, RESTART_POLL_INTERVAL_MS);
  },

  discardChanges: () => {
    if (settingsAutoSaveTimer) {
      clearTimeout(settingsAutoSaveTimer);
      settingsAutoSaveTimer = null;
    }
    const { settings } = get();
    if (!settings) return;
    const cloned = structuredClone(settings);
    applyTheme(settings.appearance);
    set({ draftSettings: cloned, hasChanges: false, autoSavedDomain: null });
    get().recomputeDomainFlags();
  },

  restoreDefaults: async () => {
    try {
      const res = await resetSettings();
      const defaults = res.settings;
      const cloned = structuredClone(defaults);
      applyTheme(defaults.appearance);
      // `restart_scheduled` is the backend's own verdict; `["all"]` is only a
      // display placeholder for a reset that touched every domain.
      const restartKeys = res.reload_policy === "restart" ? ["all"] : [];
      set({
        settings: defaults,
        draftSettings: cloned,
        hasChanges: false,
        restartKeys,
        restartInFlight: res.restart_scheduled,
      });
      get().recomputeDomainFlags();
      // A reset restores every default, so the catalog is always affected.
      await get().loadModelCatalog().catch(console.error);
      if (res.restart_scheduled) {
        get().trackRestartCompletion();
      }
    } catch (err) {
      console.error("Failed to restore defaults:", err);
    }
  },

  toggleTheme: () => {
    const { draftSettings } = get();
    if (!draftSettings?.appearance) return;
    const currentTheme = draftSettings.appearance.theme;
    const newTheme = currentTheme === "dark" ? "light" : "dark";
    get().updateDraft("appearance", "theme", newTheme);
  },

  clearRestartKeys: () => {
    if (restartPollInterval) {
      clearInterval(restartPollInterval);
      restartPollInterval = null;
    }
    set({ restartKeys: [], restartInFlight: false });
  },
}));

export function selectActiveLlmProvider(state: SettingsState): LlmProviderConfig {
  const active = state.draftSettings?.llm?.active ?? state.settings?.llm?.active;
  const kind = active?.category ?? "embedded";
  if (kind === "embedded") return { kind: "embedded" };
  if (kind === "server") {
    const servers = state.draftSettings?.llm?.servers ?? state.settings?.llm?.servers ?? [];
    const s = servers.find((entry) => entry.id === active?.provider);
    return {
      kind: "server",
      provider_id: s?.id,
      base_url: s?.url || "",
      model: active?.model || "",
      api_key: s?.api_key ?? undefined,
      provider_name: s?.name,
    };
  }
  const cloudId = active?.provider ?? "nvidia";
  const contractEntry = state.contract?.llm_cloud_providers?.find((p) => p.id === cloudId);
  const cloudEntry = contractEntry || CLOUD_PROVIDERS.find((p) => p.id === cloudId);
  const apiKey = getEffectiveCloudApiKey(state.draftSettings, state.settings, cloudId);
  return {
    kind: "cloud",
    provider_id: cloudId,
    base_url: contractEntry?.base_url || cloudEntry?.url || "",
    model: active?.model || "",
    api_key: apiKey,
    provider_name: cloudEntry?.name || cloudId,
  };
}
