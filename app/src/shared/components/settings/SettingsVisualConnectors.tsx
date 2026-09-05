import { memo } from "react";
import { Tooltip } from "@/shared/ui/Tooltip";
import { SETTINGS_COPY } from "@/data/settingsCopy";
import type { SettingsDomain as Domain, SettingsDomainId as DomainId } from "@/data/settingsCopy";

interface HubCenterProps {
  onClick: () => void;
  hasActiveCards: boolean;
}

export const HubCenter = memo(({ onClick, hasActiveCards }: HubCenterProps) => (
  <Tooltip
    label={hasActiveCards ? SETTINGS_COPY.closeAllDomains : SETTINGS_COPY.openAllDomains}
    side="top"
    wrapperClassName="absolute left-1/2 top-1/2 z-30"
    wrapperStyle={{ transform: "translate(-50%, -50%)" }}
  >
    <button
      id="center-node"
      onClick={onClick}
      className="relative w-16 h-16 rounded-full flex items-center justify-center transition-all duration-400 cursor-pointer"
      aria-label={hasActiveCards ? SETTINGS_COPY.closeAllDomains : SETTINGS_COPY.openAllDomains}
    >
      {/* Layer 1 (outermost): A circle ~52px diameter */}
      <div
        className="absolute rounded-full border border-dashed transition-all duration-400"
        style={{
          width: "52px",
          height: "52px",
          borderColor: `rgba(var(--accent), ${hasActiveCards ? 0.35 : 0.20})`,
          animation: hasActiveCards ? "border-rotate 18s linear infinite" : "none",
          background: "transparent",
        }}
      />

      {/* Layer 2: A circle ~38px diameter */}
      <div
        className="absolute rounded-full border transition-all duration-400"
        style={{
          width: "38px",
          height: "38px",
          borderColor: "rgba(var(--accent), 0.40)",
          background: "transparent",
          boxShadow: `inset 0 0 12px rgba(var(--accent), 0.15), 0 0 18px rgba(var(--accent), 0.10)`,
        }}
      />

      {/* Layer 3: A circle ~22px diameter */}
      <div
        className="absolute rounded-full border transition-all duration-400"
        style={{
          width: "22px",
          height: "22px",
          borderColor: "rgba(var(--accent), 0.60)",
          background: "radial-gradient(circle, rgba(var(--accent), 0.25) 0%, transparent 100%)",
          boxShadow: "0 0 16px rgba(var(--accent), 0.35)",
          animation: hasActiveCards ? "reactor-pulse 2.5s ease-in-out infinite" : "none",
        }}
      />

      {/* Layer 4 (innermost dot): Circle 6px */}
      <div
        className="absolute rounded-full transition-all duration-400"
        style={{
          width: "6px",
          height: "6px",
          backgroundColor: "rgb(var(--accent))",
          boxShadow: "0 0 8px rgba(var(--accent), 0.8)",
        }}
      />
    </button>
  </Tooltip>
));
HubCenter.displayName = "HubCenter";

interface SettingsConnectorsOverlayProps {
  domains: readonly Domain[];
  activeDomains: DomainId[];
  lines: Record<string, { x1: number; y1: number; x2: number; y2: number } | null>;
}

export const SettingsConnectorsOverlay = memo(({
  domains,
  activeDomains,
  lines,
}: SettingsConnectorsOverlayProps) => {
  return (
    <svg className="absolute inset-0 w-full h-full pointer-events-none z-10 overflow-visible">
      {domains.map((domain) => {
        if (!activeDomains.includes(domain.id)) return null;
        const line = lines[domain.id];
        if (!line) return null;

        const isVertical = domain.id === "persona" || domain.id === "appearance";
        let pathD = "";

        let nextX = line.x2;
        let nextY = line.y2;

        if (!isVertical) {
          const dx_mid = Math.abs(line.y2 - line.y1);
          if (domain.id === "models" || domain.id === "history") {
            nextX = Math.min(line.x2, line.x1 + dx_mid);
          } else {
            nextX = Math.max(line.x2, line.x1 - dx_mid);
          }
          nextY = line.y2;
        }

        const vx = nextX - line.x1;
        const vy = nextY - line.y1;
        const len = Math.sqrt(vx * vx + vy * vy) || 1;

        const startX = line.x1 + (vx / len) * 20;
        const startY = line.y1 + (vy / len) * 20;

        if (isVertical) {
          pathD = `M ${startX} ${startY} L ${line.x2} ${line.y2}`;
        } else {
          pathD = `M ${startX} ${startY} L ${nextX} ${line.y2} L ${line.x2} ${line.y2}`;
        }

        return (
          <g key={domain.id}>
            <path
              d={pathD}
              fill="none"
              stroke="var(--connection-glow)"
              strokeWidth={4.5}
            />
            <path
              d={pathD}
              fill="none"
              stroke="var(--connection-core)"
              strokeWidth={1.5}
            />
            <path
              d={pathD}
              fill="none"
              stroke="rgb(var(--accent))"
              strokeWidth={1.75}
              strokeLinecap="round"
              strokeDasharray="4 560"
              style={{
                animation: `connector-flow 0.9s ease-out ${domains.indexOf(domain) * 0.12}s forwards`,
              }}
            />
          </g>
        );
      })}
    </svg>
  );
});
SettingsConnectorsOverlay.displayName = "SettingsConnectorsOverlay";
