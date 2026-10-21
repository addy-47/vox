import { DOMAIN_DIRTY_KEYS, SETTINGS_SCOPE_KEYS, type SettingsDomainId, type SettingsScope } from "@/data/settingsCopy";
import { settingsContractService } from "@/services/settingsService";
import type { VoxSettings, DomainFlags } from "@/types/settings";

/** Reads a settings scope as a key-value map for dynamic key access. The
 * single guarded cast in the store; every dynamic read funnels through here. */
export function scopeEntries(scope: unknown): Record<string, unknown> {
  if (typeof scope !== "object" || scope === null) return {};
  return scope as Record<string, unknown>;
}

/**
 * Maps a settings scope (the IPC `domain`) to the settings card that owns it.
 * Single definition: the auto-save toast and the save-failure banner must
 * agree on which card a rejected key belongs to.
 */
export const SETTINGS_DOMAIN_TO_UI: Record<string, SettingsDomainId> = {
  persona: "persona",
  memory: "memory",
  appearance: "appearance",
  interaction: "interaction",
  dictation: "interaction",
  realtime: "speech",
  vad: "speech",
  stt: "speech",
  llm: "language",
  tts: "speech",
  system: "speech",
};

/** The card domains that own a commit footer. */
export const COMMIT_DOMAIN_IDS = [
  "speech",
  "persona",
  "memory",
  "appearance",
  "language",
  "interaction",
] as const;

/**
 * Catalog-affecting scopes for `commitChanges`. The catalog is manifest +
 * voices + model parameters + the TTS `active_voice_id`; `llm`/`tts`
 * inventory, selections, and keys never appear in it.
 */
export const MODEL_CATALOG_SCOPES: ReadonlySet<string> = new Set([
  "vad",
  "stt",
  "system",
]);

/** Checks if a setting key requires an engine restart. */
export function isRestartKey(scope: string, key: string): boolean {
  return settingsContractService.isRestartKey(scope, key);
}

/** Empty flag set used before any settings are loaded. */
export const EMPTY_DOMAIN_FLAGS: Record<string, DomainFlags> = {};

export function checkDomainDirty(
  settings: VoxSettings | null,
  draftSettings: VoxSettings | null,
  domainId: string
): boolean {
  if (!settings || !draftSettings) return false;

  const dirtyKeys = DOMAIN_DIRTY_KEYS[domainId as SettingsDomainId];
  if (!dirtyKeys || dirtyKeys.length === 0) return false;

  for (const rule of dirtyKeys) {
    const scope = rule.scope as keyof VoxSettings;
    const draftScope = scopeEntries(draftSettings[scope]);
    const savedScope = scopeEntries(settings[scope]);

    if (!draftScope && !savedScope) continue;
    if (!draftScope || !savedScope) return true;

    if (rule.keys && rule.keys.length > 0) {
      for (const k of rule.keys) {
        const draftVal = draftScope[k];
        const savedVal = savedScope[k];
        if (JSON.stringify(draftVal) !== JSON.stringify(savedVal)) {
          return true;
        }
      }
    } else {
      if (JSON.stringify(draftScope) !== JSON.stringify(savedScope)) {
        return true;
      }
    }
  }

  return false;
}

export function checkDomainRequiringRestart(
  settings: VoxSettings | null,
  draftSettings: VoxSettings | null,
  domainId: string
): boolean {
  if (!settings || !draftSettings) return false;

  const dirtyKeys = DOMAIN_DIRTY_KEYS[domainId as SettingsDomainId];
  if (!dirtyKeys || dirtyKeys.length === 0) return false;

  for (const rule of dirtyKeys) {
    const scope = rule.scope as keyof VoxSettings;
    const draftScope = scopeEntries(draftSettings[scope]);
    const savedScope = scopeEntries(settings[scope]);

    if (!draftScope || !savedScope) continue;

    const keysToCheck = rule.keys || Object.keys(draftScope);
    for (const k of keysToCheck) {
      if (isRestartKey(scope, k)) {
        const draftVal = draftScope[k];
        const savedVal = savedScope[k];
        if (JSON.stringify(draftVal) !== JSON.stringify(savedVal)) {
          return true;
        }
      }
    }
  }

  return false;
}

export function checkCategoryDirty(
  settings: VoxSettings | null,
  draftSettings: VoxSettings | null,
  category: string
): boolean {
  if (!settings || !draftSettings) return false;

  const scope = category.toLowerCase() as keyof VoxSettings;
  const draftScope = scopeEntries(draftSettings[scope]);
  const savedScope = scopeEntries(settings[scope]);

  if (!draftScope && !savedScope) return false;
  if (!draftScope || !savedScope) return true;

  const contractKeys = settingsContractService.getSupportedKeys(scope);
  const keys = contractKeys && contractKeys.length > 0 ? contractKeys : SETTINGS_SCOPE_KEYS[scope as SettingsScope];
  if (keys && keys.length > 0) {
    for (const k of keys) {
      const draftVal = draftScope[k];
      const savedVal = savedScope[k];
      if (JSON.stringify(draftVal) !== JSON.stringify(savedVal)) {
        return true;
      }
    }
    return false;
  }
  return JSON.stringify(draftScope) !== JSON.stringify(savedScope);
}
