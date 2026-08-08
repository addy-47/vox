import { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { SETTINGS_DOMAINS as DOMAINS, type SettingsDomainId as DomainId, type SettingsDomain as Domain } from "@/data/settingsDomains";

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

  return (
    <button
      id={`node-${domain.id}`}
      onClick={() => onSelect(domain.id)}
      className={cn(
        "absolute w-10 h-10 rounded-full flex items-center justify-center border transition-all duration-400 group z-25",
        isActive
          ? "text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] border-[rgba(var(--accent),0.4)]"
          : "text-[rgb(var(--foreground-muted))] dark:text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.15)] dark:border-[rgba(var(--border),0.08)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--accent),0.06)]"
      )}
      style={{
        left: "50%",
        top: "50%",
        transform: `translate(calc(-50% + ${pos.x}px), calc(-50% + ${pos.y}px))`,
      }}
      aria-label={`${domain.label} settings`}
    >
      <Icon size={20} strokeWidth={isActive ? 2.5 : 1.5} />
      <span 
        className={cn(
          "absolute left-1/2 -translate-x-1/2 text-[11px] font-bold uppercase tracking-[0.15em] leading-none whitespace-nowrap pointer-events-none text-center transition-all duration-400",
          isUpper ? "bottom-[calc(100%+8px)]" : "top-[calc(100%+8px)]"
        )}
      >
        {domain.label}
      </span>
    </button>
  );
});
RadialNode.displayName = "RadialNode";

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
        
        const t35 = 0.35;
        const px35 = x1 + dx * t35;
        const py35 = y1 + dy * t35;
        const halfLen35 = isActive ? 4 : 3;
        const t35_1_x = px35 + nx * halfLen35;
        const t35_1_y = py35 + ny * halfLen35;
        const t35_2_x = px35 - nx * halfLen35;
        const t35_2_y = py35 - ny * halfLen35;
        
        const t65 = 0.65;
        const px65 = x1 + dx * t65;
        const py65 = y1 + dy * t65;
        const halfLen65 = isActive ? 5.5 : 4.5;
        const t65_1_x = px65 + nx * halfLen65;
        const t65_1_y = py65 + ny * halfLen65;
        const t65_2_x = px65 - nx * halfLen65;
        const t65_2_y = py65 - ny * halfLen65;
        
        return (
          <g key={d.id} className="transition-all duration-400">
            <line
              x1={lineX1}
              y1={lineY1}
              x2={lineX2}
              y2={lineY2}
              className="transition-all duration-400"
              stroke={isActive ? "rgba(var(--accent), var(--hub-connector-active-opacity, 0.55))" : "rgba(var(--accent), 0.12)"}
              strokeWidth={isActive ? 1.5 : 1}
              strokeDasharray={isActive ? "none" : "3 5"}
            />
            <line
              x1={t35_1_x}
              y1={t35_1_y}
              x2={t35_2_x}
              y2={t35_2_y}
              className="transition-all duration-400"
              stroke={isActive ? "rgba(var(--accent), var(--hub-connector-tick35-opacity, 0.45))" : "rgba(var(--accent), 0.25)"}
              strokeWidth={1}
              opacity={isActive ? 1 : 0.4}
            />
            <line
              x1={t65_1_x}
              y1={t65_1_y}
              x2={t65_2_x}
              y2={t65_2_y}
              className="transition-all duration-400"
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
