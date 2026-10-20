import { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import {
  SETTINGS_DOMAINS as DOMAINS,
  type SettingsDomainId as DomainId,
  type SettingsDomain as Domain,
  SETTINGS_COPY,
} from "@/data/settingsCopy";

/* ─── Radial Node ──────────────────────────────────────────────────────────── */

interface RadialNodeProps {
  domain: Domain;
  isActive: boolean;
  onSelect: (id: DomainId) => void;
  radiusX: number;
  radiusY: number;
}

export const RadialNode = memo(({ domain, isActive, onSelect, radiusX, radiusY }: RadialNodeProps) => {
  const rad = (domain.angle * Math.PI) / 180;
  const pos = {
    x: radiusX * Math.cos(rad),
    y: radiusY * Math.sin(rad),
  };
  const Icon = domain.icon;
  const isUpper = domain.angle < 0;
  const tooltipLabel = (isActive ? SETTINGS_COPY.closeDomain : SETTINGS_COPY.openDomain).replace("{label}", domain.label);

  return (
    <Tooltip
      label={tooltipLabel}
      side={isUpper ? "bottom" : "top"}
      wrapperClassName="absolute left-1/2 top-1/2 z-30"
      wrapperStyle={{ transform: `translate(calc(-50% + ${pos.x}px), calc(-50% + ${pos.y}px))` }}
    >
      <button
        id={`node-${domain.id}`}
        onClick={() => onSelect(domain.id)}
        className={cn(
          "relative w-10 h-10 rounded-full flex items-center justify-center border transition-colors duration-200 hover:scale-[1.06]",
          isActive
            ? "text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] border-[rgba(var(--accent),0.4)] shadow-[0_0_12px_rgba(var(--accent),0.2)]"
            : "text-[rgb(var(--foreground-muted))] dark:text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.15)] dark:border-[rgba(var(--border),0.08)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--accent),0.06)]"
        )}
        aria-label={tooltipLabel}
      >
        <Icon size={20} strokeWidth={isActive ? 2.5 : 1.5} />
        <span
          className={cn(
            "absolute left-1/2 -translate-x-1/2 text-[10.5px] sm:text-[11px] font-bold uppercase tracking-[0.16em] leading-none whitespace-nowrap pointer-events-none text-center transition-opacity duration-200 px-1.5 py-0.5",
            isUpper ? "bottom-[calc(100%+10px)]" : "top-[calc(100%+10px)]",
            isActive ? "opacity-0 scale-90" : "opacity-100 scale-100"
          )}
        >
          {domain.label}
        </span>
      </button>
    </Tooltip>
  );
});
RadialNode.displayName = "RadialNode";

/* ─── Hub Center ───────────────────────────────────────────────────────────── */

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

/* ─── Hub Connectors (Center to Radial Nodes) ──────────────────────────────── */

interface HubConnectorsProps {
  activeDomains: DomainId[];
  radiusX: number;
  radiusY: number;
}

export const HubConnectors = memo(({ activeDomains, radiusX, radiusY }: HubConnectorsProps) => {
  const maxRadius = Math.max(radiusX, radiusY);
  const size = maxRadius * 2 + 120;
  const cx = size / 2;
  const cy = size / 2;

  return (
    <svg
      width={size}
      height={size}
      className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 pointer-events-none z-5 overflow-visible"
      aria-hidden="true"
    >
      {DOMAINS.map((d) => {
        const rad = (d.angle * Math.PI) / 180;
        const pos = {
          x: radiusX * Math.cos(rad),
          y: radiusY * Math.sin(rad),
        };
        const isActive = activeDomains.includes(d.id);
        const x1 = cx;
        const y1 = cy;
        const x2 = cx + pos.x;
        const y2 = cy + pos.y;
        const dx = x2 - x1;
        const dy = y2 - y1;
        const len = Math.sqrt(dx * dx + dy * dy) || 1;
        const nx = -dy / len;
        const ny = dx / len;

        const cosAngle = pos.x / len;
        const sinAngle = pos.y / len;
        const R_center = 26;
        const R_node = 20;

        const lineX1 = cx + cosAngle * R_center;
        const lineY1 = cy + sinAngle * R_center;
        const lineX2 = cx + pos.x - cosAngle * R_node;
        const lineY2 = cy + pos.y - sinAngle * R_node;

        const px35 = x1 + dx * 0.35;
        const py35 = y1 + dy * 0.35;
        const halfLen35 = isActive ? 4 : 3;

        const px65 = x1 + dx * 0.65;
        const py65 = y1 + dy * 0.65;
        const halfLen65 = isActive ? 5.5 : 4.5;

        return (
          <g key={d.id}>
            <line
              x1={lineX1}
              y1={lineY1}
              x2={lineX2}
              y2={lineY2}
              className="transition-[stroke] duration-200"
              stroke={isActive ? "rgba(var(--accent), var(--hub-connector-active-opacity, 0.55))" : "rgba(var(--accent), 0.12)"}
              strokeWidth={isActive ? 1.5 : 1}
              strokeDasharray={isActive ? undefined : "3 5"}
            />
            <line
              x1={px35 + nx * halfLen35}
              y1={py35 + ny * halfLen35}
              x2={px35 - nx * halfLen35}
              y2={py35 - ny * halfLen35}
              className="transition-[stroke,opacity] duration-200"
              stroke={isActive ? "rgba(var(--accent), var(--hub-connector-tick35-opacity, 0.45))" : "rgba(var(--accent), 0.25)"}
              strokeWidth={1}
              opacity={isActive ? 1 : 0.4}
            />
            <line
              x1={px65 + nx * halfLen65}
              y1={py65 + ny * halfLen65}
              x2={px65 - nx * halfLen65}
              y2={py65 - ny * halfLen65}
              className="transition-[stroke,opacity] duration-200"
              stroke={isActive ? "rgba(var(--accent), var(--hub-connector-tick65-opacity, 0.35))" : "rgba(var(--accent), 0.20)"}
              strokeWidth={1}
              opacity={isActive ? 1 : 0.4}
            />
          </g>
        );
      })}
    </svg>
  );
});
HubConnectors.displayName = "HubConnectors";

/* ─── Unified Radial Hub Component ─────────────────────────────────────────── */

export interface RadialHubProps {
  activeDomains: DomainId[];
  radiusX: number;
  radiusY: number;
  onSelect: (id: DomainId) => void;
  onCenterClick: () => void;
  hasSelection: boolean;
}

export const RadialHub = memo(({
  activeDomains,
  radiusX,
  radiusY,
  onSelect,
  onCenterClick,
  hasSelection,
}: RadialHubProps) => {
  const size = Math.max(radiusX, radiusY) * 2 + 100;
  return (
    <div className="relative shrink-0" style={{ width: size, height: size }}>
      <HubConnectors activeDomains={activeDomains} radiusX={radiusX} radiusY={radiusY} />
      {DOMAINS.map((domain) => (
        <RadialNode
          key={domain.id}
          domain={domain}
          isActive={activeDomains.includes(domain.id)}
          onSelect={onSelect}
          radiusX={radiusX}
          radiusY={radiusY}
        />
      ))}
      <HubCenter onClick={onCenterClick} hasActiveCards={hasSelection} />
    </div>
  );
});
RadialHub.displayName = "RadialHub";

/* ─── Settings Connectors Overlay (Nodes to Open Cards) ─────────────────────── */

export interface SettingsConnectorsOverlayProps {
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
    const pathsToRender: { id: string; pathD: string; animDelay: number }[] = [];

    if (activeDomains.includes("persona") && lines.persona) {
      pathsToRender.push({
        id: "persona",
        pathD: `M ${lines.persona.x1} ${lines.persona.y1 - 20} L ${lines.persona.x2} ${lines.persona.y2}`,
        animDelay: 0,
      });
    }

    if (activeDomains.includes("appearance") && lines.appearance) {
      pathsToRender.push({
        id: "appearance",
        pathD: `M ${lines.appearance.x1} ${lines.appearance.y1 + 20} L ${lines.appearance.x2} ${lines.appearance.y2}`,
        animDelay: 0.36,
      });
    }

    const line10 = lines.interaction;
    const line8 = lines.speech;
    const leftActive = activeDomains.includes("interaction") || activeDomains.includes("speech");
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

      pathsToRender.push({
        id: "left-fork-10",
        pathD: `M ${nodeX - offX} ${cy - halfH + offY} L ${mergeX} ${cy} L ${cardX} ${cy}`,
        animDelay: 0.12,
      });
      pathsToRender.push({
        id: "left-fork-8",
        pathD: `M ${nodeX - offX} ${cy + halfH - offY} L ${mergeX} ${cy} L ${cardX} ${cy}`,
        animDelay: 0.24,
      });
    }

    // The right side holds Memory (2:00) and Language (4:00). Both converge on
    // the same carousel card, so draw a mirrored fork; a single open card keeps
    // its own elbow.
    const line2 = lines.memory;
    const line4 = lines.language;
    const rightActive =
      activeDomains.includes("memory") || activeDomains.includes("language");
    if (rightActive && (line2 || line4)) {
      const nodeX = line2?.x1 ?? line4!.x1;
      const cardX = line2?.x2 ?? line4!.x2;
      const cy = line2 && line4 ? (line2.y1 + line4.y1) / 2 : line2 ? line2.y1 + 45 : line4!.y1 - 45;
      const halfH = line2 ? Math.abs(cy - line2.y1) : 45;
      const deltaX = Math.min(42, Math.max(26, Math.abs(nodeX - cardX) * 0.35));
      const mergeX = nodeX + deltaX;
      const diagLen = Math.hypot(deltaX, halfH) || 1;
      const offX = (deltaX / diagLen) * 20;
      const offY = (halfH / diagLen) * 20;

      pathsToRender.push({
        id: "right-fork-2",
        pathD: `M ${nodeX + offX} ${cy - halfH + offY} L ${mergeX} ${cy} L ${cardX} ${cy}`,
        animDelay: 0.12,
      });
      pathsToRender.push({
        id: "right-fork-4",
        pathD: `M ${nodeX + offX} ${cy + halfH - offY} L ${mergeX} ${cy} L ${cardX} ${cy}`,
        animDelay: 0.24,
      });
    }

    return (
      <svg className="absolute inset-0 w-full h-full pointer-events-none z-0 overflow-visible">
        {pathsToRender.map(({ id, pathD, animDelay }) => (
          <g key={id}>
            <path d={pathD} fill="none" stroke="var(--connection-glow)" strokeWidth={4.5} />
            <path d={pathD} fill="none" stroke="var(--connection-core)" strokeWidth={1.5} />
            <path
              d={pathD}
              fill="none"
              stroke="rgb(var(--accent))"
              strokeWidth={1.75}
              strokeLinecap="round"
              strokeDasharray="4 560"
              style={{ animation: `connector-flow 0.9s ease-out ${animDelay}s forwards` }}
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
        let nextX = line.x2;

        if (!isVertical) {
          const dx_mid = Math.abs(line.y2 - line.y1);
          if (domain.id === "memory" || domain.id === "language") {
            nextX = Math.min(line.x2, line.x1 + dx_mid);
          } else {
            nextX = Math.max(line.x2, line.x1 - dx_mid);
          }
        }

        const vx = nextX - line.x1;
        const vy = line.y2 - line.y1;
        const len = Math.sqrt(vx * vx + vy * vy) || 1;
        const startX = line.x1 + (vx / len) * 20;
        const startY = line.y1 + (vy / len) * 20;

        const pathD = isVertical
          ? `M ${startX} ${startY} L ${line.x2} ${line.y2}`
          : `M ${startX} ${startY} L ${nextX} ${line.y2} L ${line.x2} ${line.y2}`;

        return (
          <g key={domain.id}>
            <path d={pathD} fill="none" stroke="var(--connection-glow)" strokeWidth={4.5} />
            <path d={pathD} fill="none" stroke="var(--connection-core)" strokeWidth={1.5} />
            <path
              d={pathD}
              fill="none"
              stroke="rgb(var(--accent))"
              strokeWidth={1.75}
              strokeLinecap="round"
              strokeDasharray="4 560"
              style={{ animation: `connector-flow 0.9s ease-out ${domains.indexOf(domain) * 0.12}s forwards` }}
            />
          </g>
        );
      })}
    </svg>
  );
});
SettingsConnectorsOverlay.displayName = "SettingsConnectorsOverlay";
