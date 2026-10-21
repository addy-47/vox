import type { ReactNode } from "react";
import type { ModelCapabilities, ProbeCheck } from "@/store/settingsStore";
import { LLM_CATALOG_COPY } from "@/data/settingsCopy";

/** Human source label for a probed capability set. */
export function provenanceLabel(provenance: ModelCapabilities["provenance"]): string {
  switch (provenance) {
    case "probed_server":
      return LLM_CATALOG_COPY.provenanceMeasured;
    case "catalog_baseline":
      return LLM_CATALOG_COPY.provenanceCatalog;
    case "declared_static":
      return LLM_CATALOG_COPY.provenanceDeclared;
    case "family_baseline":
      return LLM_CATALOG_COPY.provenanceFamily;
    case "user_configured":
      return LLM_CATALOG_COPY.provenanceUser;
    default:
      return LLM_CATALOG_COPY.provenanceUnknown;
  }
}

/** Age of a probe result in whole days from its tested_at_epoch. */
export function probeAgeDays(testedAtEpoch: number): number {
  const nowEpoch = Math.floor(Date.now() / 1000);
  return Math.max(0, Math.floor((nowEpoch - testedAtEpoch) / 86400));
}

/** Probe checks that failed, newest evidence first. */
export function failedProbeChecks(caps: ModelCapabilities | undefined): ProbeCheck[] {
  if (!caps || !Array.isArray(caps.checks)) return [];
  return caps.checks.filter((c) => c.outcome === "failed");
}

/** Whether a probe failure needs surfacing for these capabilities. */
export function hasProbeWarning(
  probeError: string | undefined,
  caps: ModelCapabilities | undefined
): boolean {
  return Boolean(probeError) || failedProbeChecks(caps).length > 0;
}

/** Hover-revealed probe failure detail for the warning icon by the model name. */
export function probeWarningLabel(
  probeError: string | undefined,
  caps: ModelCapabilities | undefined
): ReactNode | null {
  const failed = failedProbeChecks(caps);
  if (!probeError && failed.length === 0) return null;
  return (
    <div className="space-y-2 text-[11px] font-sans w-full max-w-[280px]">
      <div className="font-bold text-[rgb(var(--foreground))] border-b border-[rgba(var(--foreground),0.08)] pb-1">
        {LLM_CATALOG_COPY.probeFailedNote}
      </div>
      {probeError && (
        <div className="text-[rgb(var(--error))] font-mono text-[10.5px] break-words leading-relaxed">{probeError}</div>
      )}
      {failed.map((check) => (
        <div key={check.id} className="space-y-0.5 text-[10.5px]">
          <div className="text-[rgb(var(--foreground-muted))] font-sans font-medium">{check.label}</div>
          <div className="text-[rgb(var(--warn-soft))]/90 font-mono text-[10px] break-words leading-relaxed pl-1.5 border-l-2 border-[rgba(var(--warn-soft),0.3)]">
            {check.detail ?? check.outcome}
          </div>
        </div>
      ))}
    </div>
  );
}
