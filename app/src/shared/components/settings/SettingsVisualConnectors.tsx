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
      className="relative w-16 h-16 rounded-full flex items-center justify-center transition-colors duration-200 cursor-pointer"
      aria-label={hasActiveCards ? SETTINGS_COPY.closeAllDomains : SETTINGS_COPY.openAllDomains}
    >
      {/* Layer 1 (outermost): A circle ~52px diameter */}
      <div
        className="absolute rounded-full border border-dashed transition-colors duration-200"
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
        className="absolute rounded-full border transition-colors duration-200"
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
        className="absolute rounded-full border transition-colors duration-200"
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
        className="absolute rounded-full transition-colors duration-200"
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
  isSideCarousel?: boolean;
}

export const SettingsConnectorsOverlay = memo(({
  domains,
  activeDomains,
  lines,
  isSideCarousel = false,
}: SettingsConnectorsOverlayProps) => {
  if (isSideCarousel) {
    // ── Side Carousel Mode: Mathematically symmetric converging diagonal paths ──
    const pathsToRender: { id: string; pathD: string; animDelay: number }[] = [];

    // 1. Persona (12:00) vertical line
    if (activeDomains.includes("persona") && lines.persona) {
      const l = lines.persona;
      pathsToRender.push({
        id: "persona",
        pathD: `M ${l.x1} ${l.y1 - 20} L ${l.x2} ${l.y2}`,
        animDelay: 0,
      });
    }

    // 2. Appearance (6:00) vertical line
    if (activeDomains.includes("appearance") && lines.appearance) {
      const l = lines.appearance;
      pathsToRender.push({
        id: "appearance",
        pathD: `M ${l.x1} ${l.y1 + 20} L ${l.x2} ${l.y2}`,
        animDelay: 0.36,
      });
    }

    // 3. Left Side: 10:00 (Interaction) and 8:00 (Personal Memory) - Permanent converging fork
    const line10 = lines.interaction;
    const line8 = lines.personal_memory;
    const leftActive = activeDomains.includes("interaction") || activeDomains.includes("personal_memory");
    if (leftActive && (line10 || line8)) {
      const nodeX = line10?.x1 ?? line8!.x1;
      const cardX = line10?.x2 ?? line8!.x2;
      const cy = (line10 && line8) ? (line10.y1 + line8.y1) / 2 : (line10 ? line10.y1 + 45 : line8!.y1 - 45);
      const halfH = line10 ? Math.abs(cy - line10.y1) : 45;
      const deltaX = Math.min(42, Math.max(26, Math.abs(nodeX - cardX) * 0.35));
      const mergeX = nodeX - deltaX;
      const diagLen = Math.hypot(deltaX, halfH);
      const offX = (deltaX / diagLen) * 20;
      const offY = (halfH / diagLen) * 20;

      const start10X = nodeX - offX;
      const start10Y = cy - halfH + offY;
      const start8X = nodeX - offX;
      const start8Y = cy + halfH - offY;

      // Both diagonal arms converge to (mergeX, cy), then single horizontal feeder to card
      pathsToRender.push({
        id: "left-fork-10",
        pathD: `M ${start10X} ${start10Y} L ${mergeX} ${cy} L ${cardX} ${cy}`,
        animDelay: 0.12,
      });
      pathsToRender.push({
        id: "left-fork-8",
        pathD: `M ${start8X} ${start8Y} L ${mergeX} ${cy} L ${cardX} ${cy}`,
        animDelay: 0.24,
      });
    }

    // 4. Right Side: 2:00 (Models) and 4:00 (Working Memory) - Permanent converging fork
    const line2 = lines.models;
    const line4 = lines.working_memory;
    const rightActive = activeDomains.includes("models") || activeDomains.includes("working_memory");
    if (rightActive && (line2 || line4)) {
      const nodeX = line2?.x1 ?? line4!.x1;
      const cardX = line2?.x2 ?? line4!.x2;
      const cy = (line2 && line4) ? (line2.y1 + line4.y1) / 2 : (line2 ? line2.y1 + 45 : line4!.y1 - 45);
      const halfH = line2 ? Math.abs(cy - line2.y1) : 45;
      const deltaX = Math.min(42, Math.max(26, Math.abs(cardX - nodeX) * 0.35));
      const mergeX = nodeX + deltaX;
      const diagLen = Math.hypot(deltaX, halfH);
      const offX = (deltaX / diagLen) * 20;
      const offY = (halfH / diagLen) * 20;

      const start2X = nodeX + offX;
      const start2Y = cy - halfH + offY;
      const start4X = nodeX + offX;
      const start4Y = cy + halfH - offY;

      // Both diagonal arms converge to (mergeX, cy), then single horizontal feeder to card
      pathsToRender.push({
        id: "right-fork-2",
        pathD: `M ${start2X} ${start2Y} L ${mergeX} ${cy} L ${cardX} ${cy}`,
        animDelay: 0.12,
      });
      pathsToRender.push({
        id: "right-fork-4",
        pathD: `M ${start4X} ${start4Y} L ${mergeX} ${cy} L ${cardX} ${cy}`,
        animDelay: 0.24,
      });
    }

    return (
      <svg className="absolute inset-0 w-full h-full pointer-events-none z-0 overflow-visible">
        {pathsToRender.map(({ id, pathD, animDelay }) => (
          <g key={id}>
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
                animation: `connector-flow 0.9s ease-out ${animDelay}s forwards`,
              }}
            />
          </g>
        ))}
      </svg>
    );
  }

  return (
    <svg className="absolute inset-0 w-full h-full pointer-events-none z-0 overflow-visible">
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
          if (domain.id === "models" || domain.id === "working_memory") {
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
