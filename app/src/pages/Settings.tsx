import { useState, useCallback, memo, useEffect, useMemo, useRef } from "react";
import { Brain, Palette, Eye, Database, UserCircle, Sliders, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettings } from "@/shared/context/SettingsContext";
import { GlassSkeleton } from "@/shared/components/GlassSkeleton";
import { AnimatePresence, motion } from "framer-motion";

// Import custom cards
import { PersonaCard } from "@/shared/components/settings/cards/PersonaCard";
import { ModelsCard } from "@/shared/components/settings/cards/ModelsCard";
import { TrayCard } from "@/shared/components/settings/cards/TrayCard";
import { MemoryCard } from "@/shared/components/settings/cards/MemoryCard";
import { AppearanceCard } from "@/shared/components/settings/cards/AppearanceCard";
import { InteractionCard } from "@/shared/components/settings/cards/InteractionCard";

// ─── Domain types ─────────────────────────────────────────────────────────────

type DomainId = "persona" | "models" | "tray" | "memory" | "appearance" | "interaction";

interface Domain {
  id: DomainId;
  label: string;
  sublabel: string;
  icon: React.ElementType;
  /** Angle in degrees around the radial hub — 0° = top */
  angle: number;
}

// ─── Domain definitions ───────────────────────────────────────────────────────

const DOMAINS: Domain[] = [
  { id: "persona",     label: "Persona",     sublabel: "Prompts & identity",     icon: UserCircle,   angle: -90  },
  { id: "models",      label: "Models",      sublabel: "Intelligence engines",   icon: Brain,        angle: -30  },
  { id: "tray",        label: "Tray",        sublabel: "HUD & overlay settings", icon: Eye,          angle: 30   },
  { id: "memory",      label: "Memory",      sublabel: "Database & retention",   icon: Database,     angle: 90   },
  { id: "appearance",  label: "Appearance",  sublabel: "Visual theme & colors",  icon: Palette,      angle: 150  },
  { id: "interaction", label: "Interaction", sublabel: "Activation & cloud key", icon: Sliders,      angle: -150 },
];

// ─── Radial hub geometry ──────────────────────────────────────────────────────

const HUB_RADIUS = 120; // px — distance from center to domain label center

function polarToCartesian(angleDeg: number, radius: number) {
  const rad = (angleDeg * Math.PI) / 180;
  return {
    x: radius * Math.cos(rad),
    y: radius * Math.sin(rad),
  };
}

// ─── Domain content map ───────────────────────────────────────────────────────

const DomainContent = memo(({ domain }: { domain: DomainId }) => {
  switch (domain) {
    case "persona":
      return <PersonaCard />;
    case "models":
      return <ModelsCard />;
    case "tray":
      return <TrayCard />;
    case "memory":
      return <MemoryCard />;
    case "appearance":
      return <AppearanceCard />;
    case "interaction":
      return <InteractionCard />;
    default:
      return null;
  }
});
DomainContent.displayName = "DomainContent";

// ─── Radial Hub Node ──────────────────────────────────────────────────────────

interface RadialNodeProps {
  domain: Domain;
  isActive: boolean;
  onSelect: (id: DomainId) => void;
}

const RadialNode = memo(({ domain, isActive, onSelect }: RadialNodeProps) => {
  const pos = polarToCartesian(domain.angle, HUB_RADIUS);
  const Icon = domain.icon;

  return (
    <button
      id={`node-${domain.id}`}
      onClick={() => onSelect(domain.id)}
      className={cn(
         "absolute flex flex-col items-center gap-1.5 group transition-all duration-400 z-25",
         isActive
           ? "text-[rgb(var(--accent))]"
           : "text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))]"
      )}
      style={{
        left: "50%",
        top: "50%",
        transform: `translate(calc(-50% + ${pos.x}px), calc(-50% + ${pos.y}px))`,
      }}
      aria-label={`${domain.label} settings`}
    >
      {/* Icon circle */}
      <div
        className={cn(
          "w-10 h-10 rounded-full flex items-center justify-center border transition-all duration-400",
          isActive
             ? "bg-[rgba(var(--accent),0.15)] border-[rgba(var(--accent),0.4)] shadow-[0_0_18px_rgba(var(--accent),0.25)]"
             : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--border),0.08)] group-hover:border-[rgba(var(--accent),0.25)] group-hover:bg-[rgba(var(--accent),0.06)]"
        )}
      >
        <Icon size={16} strokeWidth={isActive ? 2.5 : 1.5} />
      </div>
      {/* Label */}
      <span className="text-[11px] font-bold uppercase tracking-[0.15em] leading-none">
        {domain.label}
      </span>
    </button>
  );
});
RadialNode.displayName = "RadialNode";

// ─── Hub center ───────────────────────────────────────────────────────────────

const HubCenter = memo(
  ({ onClick, hasSelection }: { onClick: () => void; hasSelection: boolean }) => (
    <button
      onClick={onClick}
      className={cn(
        "absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 w-16 h-16 rounded-full border flex items-center justify-center transition-all duration-400 z-10",
        hasSelection
          ? "border-[rgba(var(--accent),0.15)] bg-[rgba(var(--accent),0.04)] cursor-pointer hover:border-[rgba(var(--accent),0.3)]"
          : "border-[rgba(var(--accent),0.25)] bg-[rgba(var(--accent),0.06)] cursor-default"
      )}
      aria-label={hasSelection ? "Clear all selections" : "Configuration hub"}
    >
      {/* Pulsing center dot */}
      <span
        className={cn(
          "w-2 h-2 rounded-full bg-[rgb(var(--accent))] transition-all duration-400",
          hasSelection ? "opacity-65" : "opacity-80 shadow-[0_0_10px_rgba(var(--accent),0.6)]"
        )}
        style={!hasSelection ? { animation: "pulse-slow 3s ease-in-out infinite" } : {}}
      />
    </button>
  )
);
HubCenter.displayName = "HubCenter";

// ─── Settings Card Wrapper Component (For Desktop & Tablet) ───────────
interface SettingsCardWrapperProps {
  domain: Domain;
  isActive: boolean;
}

const SettingsCardWrapper: React.FC<SettingsCardWrapperProps> = memo(({ domain, isActive }) => {
  return (
    <AnimatePresence>
      {isActive && (
        <motion.div
          initial={{ opacity: 0, scale: 0.96 }}
          animate={{ opacity: 1, scale: 1 }}
          exit={{ opacity: 0, scale: 0.96 }}
          transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
          className="w-full h-full flex items-center justify-center pointer-events-auto"
        >
          <div id={`card-${domain.id}`} className="shrink-0">
            {/* Actual Card content */}
            <DomainContent domain={domain.id} />
          </div>
        </motion.div>
      )}
    </AnimatePresence>
  );
});
SettingsCardWrapper.displayName = "SettingsCardWrapper";

// ─── Main Settings Component ──────────────────────────────────────────────────

export const Settings: React.FC = () => {
  const { draftSettings } = useSettings();
  const [activeDomains, setActiveDomains] = useState<DomainId[]>([]);
  const [isCompact, setIsCompact] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const [lines, setLines] = useState<Record<DomainId, { x1: number; y1: number; x2: number; y2: number } | null>>({
    persona: null,
    models: null,
    tray: null,
    memory: null,
    appearance: null,
    interaction: null,
  });

  // Resize listener to check if we are in compact mobile/tablet mode (< 1024px)
  useEffect(() => {
    const checkSize = () => {
      setIsCompact(window.innerWidth < 1024);
    };
    checkSize();
    window.addEventListener("resize", checkSize);
    return () => window.removeEventListener("resize", checkSize);
  }, []);

  // Outside click handler to pop active domains (LIFO order)
  useEffect(() => {
    const handleOutsideClick = (e: MouseEvent) => {
      if (activeDomains.length === 0) return;

      const target = e.target as HTMLElement;

      const clickedInsideNodeOrCard = DOMAINS.some((domain) => {
        const nodeEl = document.getElementById(`node-${domain.id}`);
        const cardEl = document.getElementById(`card-${domain.id}`);
        return (
          (nodeEl && nodeEl.contains(target)) ||
          (cardEl && cardEl.contains(target))
        );
      });

      const centerNodeEl = document.getElementById("center-node");
      const clickedCenter = centerNodeEl && centerNodeEl.contains(target);

      if (!clickedInsideNodeOrCard && !clickedCenter) {
        setActiveDomains((prev) => prev.slice(0, -1));
      }
    };

    document.addEventListener("mousedown", handleOutsideClick);
    return () => document.removeEventListener("mousedown", handleOutsideClick);
  }, [activeDomains]);

  // Calculate dynamic line positions between active nodes and cards
  useEffect(() => {
    if (isCompact || activeDomains.length === 0) {
      setLines({
        persona: null,
        models: null,
        tray: null,
        memory: null,
        appearance: null,
        interaction: null,
      });
      return;
    }

    let active = true;
    const updateLines = () => {
      if (!active || !containerRef.current) return;
      const containerRect = containerRef.current.getBoundingClientRect();
      const newLines = { ...lines };
      let changed = false;

      DOMAINS.forEach((domain) => {
        if (!activeDomains.includes(domain.id)) {
          if (newLines[domain.id] !== null) {
            newLines[domain.id] = null;
            changed = true;
          }
          return;
        }

        const nodeEl = document.getElementById(`node-${domain.id}`);
        const cardEl = document.getElementById(`card-${domain.id}`);

        if (nodeEl && cardEl) {
          const nodeRect = nodeEl.getBoundingClientRect();
          const cardRect = cardEl.getBoundingClientRect();

          const x1 = (nodeRect.left + nodeRect.right) / 2 - containerRect.left;
          const y1 = (nodeRect.top + nodeRect.bottom) / 2 - containerRect.top;

          let x2 = 0;
          let y2 = 0;

          // Connect to the edge of the card nearest to the node
          switch (domain.id) {
            case "persona":
              x2 = (cardRect.left + cardRect.right) / 2 - containerRect.left;
              y2 = cardRect.bottom - containerRect.top;
              break;
            case "memory":
              x2 = (cardRect.left + cardRect.right) / 2 - containerRect.left;
              y2 = cardRect.top - containerRect.top;
              break;
            case "models":
            case "tray":
              x2 = cardRect.left - containerRect.left;
              y2 = (cardRect.top + cardRect.bottom) / 2 - containerRect.top;
              break;
            case "appearance":
            case "interaction":
              x2 = cardRect.right - containerRect.left;
              y2 = (cardRect.top + cardRect.bottom) / 2 - containerRect.top;
              break;
          }

          if (!isNaN(x1) && !isNaN(y1) && !isNaN(x2) && !isNaN(y2)) {
            const existing = newLines[domain.id];
            if (
              !existing ||
              Math.abs(existing.x1 - x1) > 0.5 ||
              Math.abs(existing.y1 - y1) > 0.5 ||
              Math.abs(existing.x2 - x2) > 0.5 ||
              Math.abs(existing.y2 - y2) > 0.5
            ) {
              newLines[domain.id] = { x1, y1, x2, y2 };
              changed = true;
            }
          }
        } else {
          if (newLines[domain.id] !== null) {
            newLines[domain.id] = null;
            changed = true;
          }
        }
      });

      if (changed) {
        setLines(newLines);
      }

      if (active) {
        requestAnimationFrame(updateLines);
      }
    };

    updateLines();

    return () => {
      active = false;
    };
  }, [activeDomains, isCompact]);

  const handleSelect = useCallback((id: DomainId) => {
    setActiveDomains((prev) => {
      if (prev.includes(id)) {
        return prev.filter((d) => d !== id);
      } else {
        if (isCompact) {
          return [id];
        }
        return [...prev, id];
      }
    });
  }, [isCompact]);

  const handleClearAll = useCallback(() => {
    setActiveDomains([]);
  }, []);

  // Use the last active domain for mobile/tablet panel view fallback
  const lastActiveDomain = useMemo(() => {
    return activeDomains.length > 0 ? activeDomains[activeDomains.length - 1] : null;
  }, [activeDomains]);

  if (!draftSettings) {
    return (
      <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent px-6 md:px-10 py-6 md:py-10">
        <div className="w-full max-w-md mx-auto space-y-6">
          <GlassSkeleton variant="card" />
          <GlassSkeleton variant="card" />
        </div>
      </div>
    );
  }

  const hasSelection = activeDomains.length > 0;

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent select-none p-6">
      
      {/* ── Desktop & Tablet Hexagon/Grid Layout (>= 1024px) ────────────────── */}
      {!isCompact ? (
        <div ref={containerRef} className="flex-1 w-full grid grid-cols-12 grid-rows-6 gap-4 items-stretch relative min-h-0">
          
          {/* Dynamic SVG Overlay for Node-to-Card connections */}
          <svg className="absolute inset-0 w-full h-full pointer-events-none z-10 overflow-visible">
            {DOMAINS.map((domain) => {
              const line = lines[domain.id];
              if (!line) return null;

              const isVertical = domain.id === "persona" || domain.id === "memory";
              let pathD = "";

              if (isVertical) {
                pathD = `M ${line.x1} ${line.y1} L ${line.x2} ${line.y2}`;
              } else {
                const dx = Math.abs(line.y2 - line.y1);
                let xMid = 0;
                if (domain.id === "models" || domain.id === "tray") {
                  // Card is on the right
                  xMid = Math.min(line.x2, line.x1 + dx);
                } else {
                  // Card is on the left (appearance, interaction)
                  xMid = Math.max(line.x2, line.x1 - dx);
                }
                pathD = `M ${line.x1} ${line.y1} L ${xMid} ${line.y2} L ${line.x2} ${line.y2}`;
              }

              return (
                <g key={domain.id}>
                  {/* Outer glow line */}
                  <path
                    d={pathD}
                    fill="none"
                    stroke="rgba(var(--accent), 0.15)"
                    strokeWidth={4.5}
                  />
                  {/* Sharp core line */}
                  <path
                    d={pathD}
                    fill="none"
                    stroke="rgba(var(--accent), 0.45)"
                    strokeWidth={1.5}
                  />
                </g>
              );
            })}
          </svg>

          {/* Top-Left Slot (Col 1-4, Row 1-3) -> 10:00 (Interaction Card) */}
          <div className="col-start-1 col-span-4 row-start-1 row-span-3 flex items-end justify-end p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[5]} isActive={activeDomains.includes("interaction")} />
          </div>

          {/* Top-Center Slot (Col 5-8, Row 1-2) -> 12:00 (Persona Card) */}
          <div className="col-start-5 col-span-4 row-start-1 row-span-2 flex items-end justify-center p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[0]} isActive={activeDomains.includes("persona")} />
          </div>

          {/* Top-Right Slot (Col 9-12, Row 2-4) -> 2:00 (Models Card) */}
          <div className="col-start-9 col-span-4 row-start-2 row-span-3 flex items-center justify-start p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[1]} isActive={activeDomains.includes("models")} />
          </div>

          {/* Middle-Left Slot (Col 1-4, Row 4-6) -> 8:00 (Appearance Card) */}
          <div className="col-start-1 col-span-4 row-start-4 row-span-3 flex items-start justify-end p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[4]} isActive={activeDomains.includes("appearance")} />
          </div>

          {/* Middle-Center Slot (Col 5-8, Row 3-4) -> Radial Hub Center Grid Cell */}
          <div className="col-start-5 col-span-4 row-start-3 row-span-2 flex items-center justify-center p-2 z-20">
            <div
              className="relative shrink-0"
              style={{
                width: HUB_RADIUS * 2 + 100,
                height: HUB_RADIUS * 2 + 100,
              }}
            >
              {/* Domain nodes */}
              {DOMAINS.map((domain) => (
                <RadialNode
                  key={domain.id}
                  domain={domain}
                  isActive={activeDomains.includes(domain.id)}
                  onSelect={handleSelect}
                />
              ))}

              {/* Center node */}
              <HubCenter onClick={handleClearAll} hasSelection={hasSelection} />
            </div>
          </div>

          {/* Middle-Right Slot (Col 9-12, Row 4-6) -> 4:00 (Tray Card) */}
          <div className="col-start-9 col-span-4 row-start-4 row-span-3 flex items-start justify-start p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[2]} isActive={activeDomains.includes("tray")} />
          </div>

          {/* Bottom-Center Slot (Col 5-8, Row 5-6) -> 6:00 (Memory Card) */}
          <div className="col-start-5 col-span-4 row-start-5 row-span-2 flex items-start justify-center p-2 relative">
            <SettingsCardWrapper domain={DOMAINS[3]} isActive={activeDomains.includes("memory")} />
          </div>

        </div>
      ) : (
        /* ── Mobile & Compact Layout Fallback (< 1024px) ───────────────────── */
        <div className="flex-1 flex flex-col items-center justify-center relative min-h-0 w-full animate-fade-in">
          <div
            className="relative transition-all duration-500 ease-in-out shrink-0"
            style={{
              width: HUB_RADIUS * 2 + 160,
              height: HUB_RADIUS * 2 + 160,
            }}
          >
            {DOMAINS.map((domain) => (
              <RadialNode
                key={domain.id}
                domain={domain}
                isActive={activeDomains.includes(domain.id)}
                onSelect={handleSelect}
              />
            ))}

            <HubCenter onClick={handleClearAll} hasSelection={hasSelection} />
          </div>

          <AnimatePresence>
            {lastActiveDomain && (
              <motion.div
                key={lastActiveDomain}
                initial={{ opacity: 0, scale: 0.9, y: 20 }}
                animate={{ opacity: 1, scale: 1, y: 0 }}
                exit={{ opacity: 0, scale: 0.9, y: 20 }}
                transition={{ duration: 0.3, ease: [0.16, 1, 0.3, 1] }}
                className="fixed left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 z-40 w-[calc(100%-32px)] max-w-md bg-black/95 backdrop-blur-2xl border border-[rgba(var(--accent),0.15)] shadow-2xl p-5 overflow-y-auto custom-scrollbar rounded-2xl max-h-[70vh] flex flex-col justify-start pointer-events-auto"
              >
                <div className="relative w-full">
                  {/* Explicit Close Button */}
                  <button
                    onClick={() => handleSelect(lastActiveDomain)}
                    className="absolute -top-1.5 -right-1.5 w-7 h-7 rounded-full border border-[rgba(var(--accent),0.18)] bg-black/40 flex items-center justify-center text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--accent))] transition-colors z-50 cursor-pointer"
                    aria-label="Close settings panel"
                  >
                    <X size={14} />
                  </button>
                  
                  {/* Domain Card Content */}
                  <DomainContent domain={lastActiveDomain} />
                </div>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      )}
    </div>
  );
};
