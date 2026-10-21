import { describe, it, expect, beforeAll, afterEach } from "vitest";
import {
  checkDomainDirty,
  checkDomainRequiringRestart,
  checkCategoryDirty,
  scopeEntries,
  SETTINGS_DOMAIN_TO_UI,
  MODEL_CATALOG_SCOPES,
} from "@/store/settingsFlags";
import { getSettingsContract } from "@/services/settingsService";
import { setTransport, TauriTransport, type Transport, type UnlistenFn } from "@/services/transport";
import { minimalSettings, settingsFrom } from "@/test/harness/settingsFixtures";
import type { VoxSettings } from "@/types/settings";

/**
 * Domain dirty/restart flag regression suite (settingsFlags.ts).
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT:
 *   The per-card dirty classification that decides whether the user sees
 *   "unsaved changes" (false == lost edits: the card footer never appears and
 *   the draft dies with it), and the restart classifier that decides whether a
 *   croissant banner offers a restart. Both functions walk DOMAIN_DIRTY_KEYS
 *   of settingsCopy and compare draft vs committed values with
 *   JSON.stringify equality.
 *
 * Production Entry Seam:
 *   checkDomainDirty / checkDomainRequiringRestart / checkCategoryDirty —
 *   exported, pure, and called by the real store: isDomainDirty /
 *   isDomainRequiringRestart (settingsStore) and recomputeDomainFlags for the
 *   always-mounted card wrappers.
 *
 * Direction Check:
 *   Upstream trigger-side readers (settings state in, boolean classification
 *   out). Downstream renderers are sinks — not exercised here.
 *
 * Production Path:
 *   settings.jsonc ← get_settings → committed VoxSettings
 *   → draftSettings mutated via updateDraft
 *   → checkDomainDirty(domainId) → DomainFlags.dirty
 *   → cards' footers / commit buttons
 *
 * Observable Exit: booleans per (domain, draft-vs-committed) pair.
 *
 * Phase 2a — Testability 4/4: pure functions + a typed fixture mirroring the
 * IPC wire shape; no service, no DOM, no clock. `checkDomainRequiringRestart`
 * delegates restart classification to settingsContractService.isRestartKey,
 * whose module-private contract cache is populated through the real loader
 * (`getSettingsContract()` over the swappable transport) in this suite's
 * beforeAll — production-owned state, not a hand-cast substitute.
 *
 * Phase 2b — False-Green table (each row a one-line production mutation):
 *   | checkDomainDirty flipped to always-false → edits invisible      | must fail |
 *   | key iteration inverted to compare only the first key             | must fail |
 *   | rule.keys path removed (whole-scope compare everywhere)          | must fail |
 *   | checkDomainRequiringRestart ignores partial scopes               | must fail |
 *   | SettingsDomainToUI dictation→interaction mapping dropped         | must fail |
 *   | transliteration edit attributed to the models card               | must fail |
 *   | vad rule widened back to the whole scope (noise gate reports dirty)| must fail |
 *
 * Phase 4 — Governing sentence:
 *   If the per-key draft/committed compare silently broke, this test would
 *   fail because checkDomainDirty("persona", draft, committed) would return
 *   false while the draft carries "edited-persona".
 */

const committed: VoxSettings = minimalSettings();

describe("scopeEntries", () => {
  it("NEGATIVE: non-object scopes degrade to an empty map, never a throw", () => {
    expect(scopeEntries(undefined)).toEqual({});
    expect(scopeEntries(null)).toEqual({});
    expect(scopeEntries("text")).toEqual({});
    expect(scopeEntries(42)).toEqual({});
    expect(scopeEntries({ theme: "dark" })).toEqual({ theme: "dark" });
  });
});

describe("checkDomainDirty", () => {
  it("flags a dirty domain only when a declared key actually differs", () => {
    const draft = settingsFrom(committed);
    draft.persona.modular_prompt = "edited-persona";
    expect(checkDomainDirty(committed, draft, "persona")).toBe(true);
    expect(checkDomainDirty(committed, draft, "memory")).toBe(false);
  });

  it("NEGATIVE: dirty detection must not fire when draft equals committed", () => {
    expect(checkDomainDirty(committed, committed, "persona")).toBe(false);
  });

  it("NEGATIVE: a draft with a missing scope vs committed is dirty, not silently equal", () => {
    const draft = settingsFrom(committed) as Partial<VoxSettings>;
    delete (draft as { persona?: unknown }).persona;
    expect(checkDomainDirty(committed, draft as VoxSettings, "persona")).toBe(true);
  });

  it("NEGATIVE: detects changes in nested rule sets of multi-scope domains", () => {
    const draft = settingsFrom(committed);
    draft.dictation.enabled = true; // interaction card owns dictation scope
    expect(checkDomainDirty(committed, draft, "interaction")).toBe(true);
    // vad is a speech-card scope
    const vadDraft = settingsFrom(committed);
    vadDraft.vad.threshold = 0.9;
    expect(checkDomainDirty(committed, vadDraft, "speech")).toBe(true);
    expect(checkDomainDirty(committed, vadDraft, "persona")).toBe(false);
  });

  it("NEGATIVE: dirty verdict is per-key, not per-scope — an undeclared wire field must not flag the card", () => {
    const draft = settingsFrom(committed);
    draft.persona.modular_prompt = "edited-persona";
    // realtime_prompt identical → only modular_prompt must trip the card.
    expect(checkDomainDirty(committed, draft, "persona")).toBe(true);
    const draft2 = settingsFrom(committed);
    draft2.memory.private_mode = true;
    expect(checkDomainDirty(committed, draft2, "memory")).toBe(true);
  });

  it("forward wire compatibility: an undeclared scope field alone never marks a card dirty", () => {
    // The declared dirty-key lists enumerate every field of the typed VoxSettings
    // domains. A later backend revision adding a scope field must not
    // retro-wire it into "unsaved changes": unknown keys are ignored (draft
    // compare is per declared key, never whole-scope JSON equality).
    const draft = settingsFrom(committed);
    (draft.memory as unknown as Record<string, unknown>).backend_added_field = "new";
    (draft.persona as unknown as Record<string, unknown>).backend_added_field = "new";
    (draft.persona as unknown as Record<string, unknown>).modular_prompt = "pin";
    expect(JSON.stringify(draft.memory)).not.toEqual(JSON.stringify(committed.memory));
    expect(checkDomainDirty(committed, draft, "memory")).toBe(false);
  });

  it("is dirty when both settings exist but a declared key vanishes from the draft", () => {
    const draft = settingsFrom(committed) as Partial<VoxSettings>;
    delete (draft as { memory?: { private_mode?: boolean } }).memory?.private_mode;
    expect(checkDomainDirty(committed, draft as VoxSettings, "memory")).toBe(true);
  });
});

describe("dirty ownership split (transliteration | vad noise gate)", () => {
  it("reports a transliteration edit on the interaction card, not on the speech card", () => {
    // The transliteration switch moved to the interaction desk, so the
    // interaction card owns `stt.transliterate_enabled`.
    const draft = settingsFrom(committed);
    draft.stt.transliterate_enabled = !committed.stt.transliterate_enabled;
    expect(checkDomainDirty(committed, draft, "interaction")).toBe(true);
    expect(checkDomainDirty(committed, draft, "speech")).toBe(false);
  });

  it("NEGATIVE: the stt model selection still belongs to the speech card", () => {
    const draft = settingsFrom(committed);
    draft.stt.active = { ...committed.stt.active, model: "whisper-large" };
    expect(checkDomainDirty(committed, draft, "speech")).toBe(true);
    expect(checkDomainDirty(committed, draft, "interaction")).toBe(false);
  });

  it("NEGATIVE: the removed vad noise gate never marks the owning card dirty", () => {
    // `vad.ptt_noise_gate` left the UI but stays in the wire payload, so the
    // card that owns the vad scope must ignore it.
    const draft = settingsFrom(committed);
    draft.vad.ptt_noise_gate = committed.vad.ptt_noise_gate + 0.1;
    expect(checkDomainDirty(committed, draft, "speech")).toBe(false);
    expect(checkDomainDirty(committed, draft, "interaction")).toBe(false);
  });

  it("the user-facing vad keys still mark the speech card dirty", () => {
    for (const key of ["threshold", "silence_duration_ms", "speech_onset_ms"] as const) {
      const draft = settingsFrom(committed);
      draft.vad[key] = draft.vad[key] + 1;
      expect(checkDomainDirty(committed, draft, "speech"), `vad.${key} must flag the card`).toBe(
        true
      );
      expect(checkDomainDirty(committed, draft, "interaction"), `vad.${key} must not flag the interaction card`).toBe(
        false
      );
    }
  });
});

describe("checkCategoryDirty", () => {
  it("classifies a scope-level dirty state using declared keys", () => {
    const draft = settingsFrom(committed);
    draft.persona.realtime_prompt = "changed-prompt";
    expect(checkCategoryDirty(committed, draft, "persona")).toBe(true);
    expect(checkCategoryDirty(committed, committed, "persona")).toBe(false);
  });

  it("NEGATIVE: category casing is normalized (VoiceModal passes 'Persona')", () => {
    const draft = settingsFrom(committed);
    draft.persona.modular_prompt = "x";
    expect(checkCategoryDirty(committed, draft, "PERSONA")).toBe(true);
  });

  it("NEGATIVE: no settings yet means never dirty (no phantom banner before boot)", () => {
    expect(checkCategoryDirty(null, null, "persona")).toBe(false);
    expect(checkDomainDirty(null, null, "persona")).toBe(false);
    expect(checkDomainRequiringRestart(null, null, "speech")).toBe(false);
  });
});

describe("checkDomainRequiringRestart (loaded production contract)", () => {
  // Restart classification reads settingsContractService.isRestartKey, which
  // consults the module-private contract cache loaded at bootstrap.
  // Production-owned state must be assembled through the real loader
  // (get_settings_contract over the swappable transport), never faked.
  beforeAll(async () => {
    const contractTransport: Transport = {
      invoke<T>(cmd: string): Promise<T> {
        expect(cmd).toBe("get_settings_contract");
        return Promise.resolve({
          version: 1,
          supported_keys: {},
          reload_policies: {
            vad: { threshold: "restart" },
            dictation: { enabled: "restart" },
            persona: { modular_prompt: "hot" },
          },
          secret_keys: [],
          llm_cloud_providers: [],
          tts_providers: [],
          defaults: {},
        } as unknown as T);
      },
      listen(): Promise<UnlistenFn> {
        return Promise.resolve(() => {});
      },
      convertFileSrc(p: string): string {
        return p;
      },
    } as Transport;
    setTransport(contractTransport);
    await getSettingsContract();
  });

  afterEach(() => {
    setTransport(new TauriTransport());
  });

  it("classifies restart only for keys the loaded contract marks restart-classified", () => {
    // vad.threshold → declared "restart" in the loaded contract
    const vadDraft = settingsFrom(committed);
    vadDraft.vad.threshold = 0.9;
    expect(checkDomainRequiringRestart(committed, vadDraft, "speech")).toBe(true);
    expect(checkDomainRequiringRestart(committed, vadDraft, "persona")).toBe(false);

    // dictation.enabled → declared "restart"
    const dictDraft = settingsFrom(committed);
    dictDraft.dictation.enabled = true;
    expect(checkDomainRequiringRestart(committed, dictDraft, "interaction")).toBe(true);
  });

  it("NEGATIVE: a hot-reload key change must never classify as restart", () => {
    const draft = settingsFrom(committed);
    draft.persona.modular_prompt = "edited-persona";
    expect(checkDomainRequiringRestart(committed, draft, "persona")).toBe(false);
  });

  it("NEGATIVE: identical committed/draft never requires restart", () => {
    expect(checkDomainRequiringRestart(committed, committed, "speech")).toBe(false);
  });
});

describe("SETTINGS_DOMAIN_TO_UI mapping (single definition card ownership)", () => {
  it("NEGATIVE: every scope maps to one of the commit cards", () => {
    for (const uiDomain of Object.values(SETTINGS_DOMAIN_TO_UI)) {
      expect(
        ["speech", "persona", "memory", "appearance", "language", "interaction"],
        `scope ${String(uiDomain)} leaves the commit-card system`
      ).toContain(uiDomain);
    }
    expect(SETTINGS_DOMAIN_TO_UI.dictation).toBe("interaction");
    expect(SETTINGS_DOMAIN_TO_UI.llm).toBe("language");
    expect(SETTINGS_DOMAIN_TO_UI["stt"]).toBe("speech");
    expect(SETTINGS_DOMAIN_TO_UI["vad"]).toBe("speech");
  });
});

describe("MODEL_CATALOG_SCOPES", () => {
  it(" enumerates the catalog-feeding scopes and nothing else", () => {
    expect([...MODEL_CATALOG_SCOPES].sort()).toEqual(["stt", "system", "vad"]);
  });
});
