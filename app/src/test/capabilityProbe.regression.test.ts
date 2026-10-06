import { describe, it, expect } from "vitest";
import {
  provenanceLabel,
  probeAgeDays,
  failedProbeChecks,
  hasProbeWarning,
} from "@/shared/components/settings/models/LlmCatalogView";
import type { ModelCapabilities } from "@/store/settingsStore";

/**
 * Capability-probe transparency regression suite — LLM catalog labels.
 *
 * SUT: the exact label functions the catalog rows call (exported from
 * LlmCatalogView). A measured value and a baseline guess must never render
 * with the same words; a failed check must surface its detail; a stale
 * result must be computable as stale.
 *
 * Production Entry Seams (all real production signatures):
 *   provenanceLabel / probeAgeDays / failedProbeChecks
 *     (shared/components/settings/models/LlmCatalogView.tsx — the same
 *     functions RemoteModelCard and the inline capabilities tooltip call)
 *
 * False-Green table (each row is a one-line production mutation):
 *   | provenance switch collapsed to one string → catalog baseline test | must fail |
 *   | failedProbeChecks filter dropped → failed check invisible         | must fail |
 *   | probeAgeDays off-by-1000x (ms vs s) → fresh probe reads stale    | must fail |
 *
 * Testability gap (reported, not faked): full RemoteModelCard rendering is
 * not covered — it needs the zustand settings store plus Tauri invoke
 * backing. These unit tests cover every branch the rows evaluate; the
 * remaining risk is JSX wiring, which the invariants suite guards
 * statically (no testing tile, no vision filter, copy-table strings).
 */

function capsWith(overrides: Partial<ModelCapabilities>): ModelCapabilities {
  return {
    model_id: "test-model",
    provider_kind: "server",
    supports_tools: "unknown",
    supports_latin: "unknown",
    supports_devanagari: "unknown",
    context_window: null,
    max_output_tokens: null,
    provenance: "unknown",
    tps: null,
    ttft_ms: null,
    server_has_gpu: false,
    is_gpu_accelerated: false,
    gpu_status: "",
    vram_bytes: null,
    parameter_size: null,
    quantization: null,
    family: null,
    tested_at_epoch: Math.floor(Date.now() / 1000),
    checks: [],
    ...overrides,
  };
}

describe("provenanceLabel", () => {
  it("labels every provenance tier with a distinct string", () => {
    const labels = new Set(
      (
        [
          "catalog_baseline",
          "family_baseline",
          "probed_server",
          "declared_static",
          "user_configured",
          "unknown",
        ] as const
      ).map(provenanceLabel)
    );
    expect(labels.size).toBe(6);
  });

  it("never calls a baseline guess a live measurement", () => {
    expect(provenanceLabel("catalog_baseline")).not.toContain("live");
    expect(provenanceLabel("catalog_baseline")).not.toBe(provenanceLabel("probed_server"));
    expect(provenanceLabel("family_baseline")).not.toBe(provenanceLabel("probed_server"));
  });

  it("falls back to the unknown label for unexpected values", () => {
    expect(
      provenanceLabel("invented_tier" as ModelCapabilities["provenance"])
    ).toBe(provenanceLabel("unknown"));
  });
});

describe("probeAgeDays", () => {
  it("reports today for a fresh probe", () => {
    expect(probeAgeDays(Math.floor(Date.now() / 1000))).toBe(0);
  });

  it("reports whole days for old probes", () => {
    const tenDaysAgo = Math.floor(Date.now() / 1000) - 10 * 86400;
    expect(probeAgeDays(tenDaysAgo)).toBe(10);
  });

  it("clamps future timestamps to zero instead of going negative", () => {
    const tomorrow = Math.floor(Date.now() / 1000) + 86400;
    expect(probeAgeDays(tomorrow)).toBe(0);
  });
});

describe("failedProbeChecks", () => {
  it("returns failed checks with their detail, never skipped ones", () => {
    const caps = capsWith({
      checks: [
        { id: "streaming", label: "Streaming", outcome: "measured", detail: null, duration_ms: 41 },
        { id: "tool_calls", label: "Tools", outcome: "failed", detail: "HTTP 429", duration_ms: 12 },
        { id: "ollama_gpu", label: "GPU", outcome: "skipped", detail: "not ollama", duration_ms: 1 },
      ],
    });
    const failed = failedProbeChecks(caps);
    expect(failed.map((c) => c.id)).toEqual(["tool_calls"]);
    expect(failed[0].detail).toBe("HTTP 429");
  });

  it("returns nothing for a clean probe or missing capabilities", () => {
    expect(failedProbeChecks(capsWith({}))).toEqual([]);
    expect(failedProbeChecks(undefined)).toEqual([]);
  });

  it("tolerates a missing checks array from older cache entries", () => {
    const legacy = capsWith({});
    delete (legacy as Partial<ModelCapabilities>).checks;
    expect(failedProbeChecks(legacy)).toEqual([]);
  });
});

describe("hasProbeWarning", () => {
  it("warns on invoke errors and on failed checks, never on skipped ones", () => {
    expect(hasProbeWarning("boom", capsWith({}))).toBe(true);
    expect(
      hasProbeWarning(
        undefined,
        capsWith({
          checks: [
            { id: "tool_calls", label: "Tools", outcome: "failed", detail: "HTTP 429", duration_ms: 1 },
          ],
        })
      )
    ).toBe(true);
    expect(
      hasProbeWarning(
        undefined,
        capsWith({
          checks: [
            { id: "gpu", label: "GPU", outcome: "skipped", detail: "not ollama", duration_ms: 1 },
          ],
        })
      )
    ).toBe(false);
    expect(hasProbeWarning(undefined, capsWith({}))).toBe(false);
    expect(hasProbeWarning(undefined, undefined)).toBe(false);
  });
});
