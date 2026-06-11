import React, {
  useState,
  useCallback,
  memo,
  useMemo,
} from "react";
import { RotateCcw, Mic, Brain, Palette, Eye, Database, MessageSquare } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettings } from "@/shared/context/SettingsContext";
import { CoreSettings } from "@/shared/components/CoreSettings";
import { ModelSettings } from "@/shared/components/ModelSettings";
import { TraySettings } from "@/shared/components/TraySettings";
import { GlassSkeleton } from "@/shared/components/GlassSkeleton";
import { AnimatePresence, motion } from "framer-motion";

// ─── Domain types ─────────────────────────────────────────────────────────────

type DomainId = "voice" | "models" | "interface" | "tray" | "memory" | "assistant";

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
  { id: "voice",     label: "Voice",     sublabel: "Signal & interaction",   icon: Mic,          angle: -90  },
  { id: "models",    label: "Models",    sublabel: "Intelligence engines",   icon: Brain,        angle: -30  },
  { id: "interface", label: "Interface", sublabel: "Appearance & accent",    icon: Palette,      angle: 30   },
  { id: "tray",      label: "Tray",      sublabel: "HUD & overlay",          icon: Eye,          angle: 90   },
  { id: "memory",    label: "Memory",    sublabel: "Persistence & privacy",  icon: Database,     angle: 150  },
  { id: "assistant", label: "Assistant", sublabel: "Prompts & language",     icon: MessageSquare,angle: -150 },
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

// Renders the actual settings panel for each domain.
// We reuse the existing well-tested sub-components.
const DomainContent = memo(({ domain }: { domain: DomainId }) => {
  switch (domain) {
    case "voice":
      return <CoreSettings />;
    case "models":
      return <ModelSettings />;
    case "interface":
      // AppearanceCard + InteractionCard are embedded inside CoreSettings already.
      // For the interface domain we render CoreSettings which contains the Appearance and Interaction cards.
      return <CoreSettings />;
    case "tray":
      return <TraySettings />;
    case "memory":
      return <MemorySettings />;
    case "assistant":
      return <AssistantSettings />;
    default:
      return null;
  }
});
DomainContent.displayName = "DomainContent";

// ─── Memory Settings ──────────────────────────────────────────────────────────

const MemorySettings = memo(() => {
  const { draftSettings, updateDraft } = useSettings();
  if (!draftSettings) return null;

  return (
    <div className="space-y-8 max-w-lg">
      <div className="space-y-5">
        <SectionLabel label="Session Storage" />
        <SettingRow
          label="Persist sessions"
          description="Save voice sessions to local database"
        >
          <Toggle
            value={draftSettings.persistence.enabled}
            onChange={(v) => updateDraft("persistence", "enabled", v)}
            aria="Toggle session persistence"
          />
        </SettingRow>
        <SettingRow
          label="Private mode"
          description="Prevent session data from being stored"
        >
          <Toggle
            value={draftSettings.persistence.private_mode}
            onChange={(v) => updateDraft("persistence", "private_mode", v)}
            aria="Toggle private mode"
          />
        </SettingRow>
      </div>

      <div className="space-y-5">
        <SectionLabel label="Retention" />
        <SettingRow
          label="Max sessions"
          description={`Keep ${draftSettings.persistence.max_sessions} sessions on disk`}
        >
          <SliderInput
            value={draftSettings.persistence.max_sessions}
            min={5}
            max={500}
            step={5}
            onChange={(v) => updateDraft("persistence", "max_sessions", v)}
            format={(v) => `${v}`}
          />
        </SettingRow>
        <SettingRow
          label="Retention period"
          description={`Auto-delete sessions older than ${draftSettings.persistence.retention_days} days`}
        >
          <SliderInput
            value={draftSettings.persistence.retention_days}
            min={1}
            max={365}
            step={1}
            onChange={(v) => updateDraft("persistence", "retention_days", v)}
            format={(v) => `${v}d`}
          />
        </SettingRow>
      </div>
    </div>
  );
});
MemorySettings.displayName = "MemorySettings";

// ─── Assistant Settings ───────────────────────────────────────────────────────

const AssistantSettings = memo(() => {
  const { draftSettings, updateDraft } = useSettings();
  if (!draftSettings) return null;

  return (
    <div className="space-y-8 max-w-2xl">
      <div className="space-y-4">
        <SectionLabel label="System Prompt" />
        <p className="text-[12px] text-[rgb(var(--foreground-muted))]/60 leading-relaxed">
          This prompt shapes Vox's personality and behavior. Emotion tags{" "}
          <code className="text-[rgb(var(--accent))]/80 bg-[rgba(var(--accent),0.08)] px-1.5 py-0.5 rounded text-[11px]">
            &lt;laugh&gt;
          </code>{" "}
          are automatically injected when supported.
        </p>
        <textarea
          value={draftSettings.assistant.system_prompt}
          onChange={(e) => updateDraft("assistant", "system_prompt", e.target.value)}
          rows={5}
          className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-4 py-3 text-[13px] text-[rgb(var(--foreground))]/80 font-mono leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.35)] transition-colors placeholder:text-[rgb(var(--foreground-muted))]/30"
          placeholder="You are Vox, a calm and intelligent voice assistant..."
          spellCheck={false}
        />
      </div>

      <div className="space-y-4">
        <SectionLabel label="Language Prompts" />
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-2">
            <label className="text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]/50">
              Hindi / Hinglish
            </label>
            <textarea
              value={draftSettings.assistant.hindi_prompt}
              onChange={(e) => updateDraft("assistant", "hindi_prompt", e.target.value)}
              rows={4}
              className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-4 py-3 text-[13px] text-[rgb(var(--foreground))]/80 font-mono leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.35)] transition-colors"
              spellCheck={false}
            />
          </div>
          <div className="space-y-2">
            <label className="text-[10px] font-bold uppercase tracking-widest text-[rgb(var(--foreground-muted))]/50">
              English
            </label>
            <textarea
              value={draftSettings.assistant.english_prompt}
              onChange={(e) => updateDraft("assistant", "english_prompt", e.target.value)}
              rows={4}
              className="w-full bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--accent),0.12)] rounded-xl px-4 py-3 text-[13px] text-[rgb(var(--foreground))]/80 font-mono leading-relaxed resize-none focus:outline-none focus:border-[rgba(var(--accent),0.35)] transition-colors"
              spellCheck={false}
            />
          </div>
        </div>
      </div>
    </div>
  );
});
AssistantSettings.displayName = "AssistantSettings";

// ─── Reusable mini-components ─────────────────────────────────────────────────

const SectionLabel: React.FC<{ label: string }> = ({ label }) => (
  <div className="flex items-center gap-3">
    <span className="text-[10px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/60">
      {label}
    </span>
    <div className="flex-1 h-px bg-[rgba(var(--accent),0.06)]" />
  </div>
);

const SettingRow: React.FC<{
  label: string;
  description?: string;
  children: React.ReactNode;
}> = ({ label, description, children }) => (
  <div className="flex items-center justify-between gap-6">
    <div className="flex-1 min-w-0">
      <div className="text-[13px] font-medium text-[rgb(var(--foreground))]/80">{label}</div>
      {description && (
        <div className="text-[11px] text-[rgb(var(--foreground-muted))]/50 mt-0.5 leading-relaxed">
          {description}
        </div>
      )}
    </div>
    <div className="shrink-0">{children}</div>
  </div>
);

const Toggle: React.FC<{
  value: boolean;
  onChange: (v: boolean) => void;
  aria: string;
}> = ({ value, onChange, aria }) => (
  <button
    onClick={() => onChange(!value)}
    className={cn(
      "group relative flex items-center h-5 w-9 px-0.5 rounded-full transition-all duration-300",
      value ? "bg-[rgb(var(--accent))]" : "bg-black/35 border border-[rgba(var(--accent),0.2)]"
    )}
    aria-label={aria}
    role="switch"
    aria-checked={value}
  >
    <div
      className={cn(
        "w-4 h-4 rounded-full bg-white transition-transform duration-300",
        value ? "translate-x-4" : "translate-x-0"
      )}
    />
  </button>
);

const SliderInput: React.FC<{
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
  format: (v: number) => string;
}> = ({ value, min, max, step, onChange, format }) => (
  <div className="flex items-center gap-3 w-40">
    <input
      type="range"
      min={min}
      max={max}
      step={step}
      value={value}
      onChange={(e) => onChange(Number(e.target.value))}
      className="flex-1"
    />
    <span className="text-[12px] font-mono font-bold text-[rgb(var(--accent))] w-10 text-right shrink-0">
      {format(value)}
    </span>
  </div>
);

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
      onClick={() => onSelect(domain.id)}
      className={cn(
        "absolute flex flex-col items-center gap-1.5 group transition-all duration-400",
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
      <span className="text-[9px] font-bold uppercase tracking-[0.15em] leading-none">
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
        "absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 w-16 h-16 rounded-full border flex items-center justify-center transition-all duration-400",
        hasSelection
          ? "border-[rgba(var(--accent),0.15)] bg-[rgba(var(--accent),0.04)] cursor-pointer hover:border-[rgba(var(--accent),0.3)]"
          : "border-[rgba(var(--accent),0.25)] bg-[rgba(var(--accent),0.06)] cursor-default"
      )}
      aria-label={hasSelection ? "Return to hub" : "Configuration hub"}
    >
      {/* Pulsing center dot */}
      <span
        className={cn(
          "w-2 h-2 rounded-full bg-[rgb(var(--accent))] transition-all duration-400",
          hasSelection ? "opacity-30" : "opacity-80 shadow-[0_0_10px_rgba(var(--accent),0.6)]"
        )}
        style={!hasSelection ? { animation: "pulse-slow 3s ease-in-out infinite" } : {}}
      />
    </button>
  )
);
HubCenter.displayName = "HubCenter";

// ─── Pill navigation (compact domain selector after selection) ────────────────

interface PillNavProps {
  domains: Domain[];
  activeDomain: DomainId;
  onSelect: (id: DomainId) => void;
  onReset: () => void;
}

const PillNav = memo(({ domains, activeDomain, onSelect, onReset }: PillNavProps) => (
  <div className="flex items-center justify-center gap-1 px-4 py-2 rounded-full glass-surface glass-base border border-[rgba(var(--accent),0.08)] flex-wrap">
    {domains.map((d) => {
      const Icon = d.icon;
      const isActive = d.id === activeDomain;
      return (
        <button
          key={d.id}
          onClick={() => onSelect(d.id)}
          className={cn(
            "flex items-center gap-1.5 px-3 py-1.5 rounded-full text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
            isActive
              ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] shadow-md"
              : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--accent),0.06)]"
          )}
          aria-label={`${d.label} settings`}
          aria-current={isActive ? "true" : undefined}
        >
          <Icon size={11} strokeWidth={isActive ? 2.5 : 1.5} />
          {d.label}
        </button>
      );
    })}
    <button
      onClick={onReset}
      className="flex items-center justify-center w-6 h-6 rounded-full text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--foreground-muted))] transition-colors ml-1"
      aria-label="Return to hub overview"
    >
      <RotateCcw size={10} />
    </button>
  </div>
));
PillNav.displayName = "PillNav";

// ─── Connector lines (SVG, drawn from center to each node) ───────────────────

const HubConnectors: React.FC<{ activeDomain: DomainId | null }> = ({ activeDomain }) => {
  const size = HUB_RADIUS * 2 + 120; // viewBox size
  const cx = size / 2;
  const cy = size / 2;

  return (
    <svg
      width={size}
      height={size}
      className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 pointer-events-none"
      aria-hidden="true"
    >
      {DOMAINS.map((d) => {
        const pos = polarToCartesian(d.angle, HUB_RADIUS);
        const isActive = activeDomain === d.id;
        return (
          <line
            key={d.id}
            x1={cx}
            y1={cy}
            x2={cx + pos.x}
            y2={cy + pos.y}
            stroke={`rgba(var(--accent), ${isActive ? 0.4 : 0.08})`}
            strokeWidth={isActive ? 1.5 : 1}
            strokeDasharray={isActive ? "0" : "3 4"}
            style={{ transition: "stroke-opacity 0.4s ease, stroke-width 0.4s ease" }}
          />
        );
      })}
    </svg>
  );
};

// ─── Main Settings Component ──────────────────────────────────────────────────

export const Settings: React.FC = () => {
  const { draftSettings } = useSettings();
  const [activeDomain, setActiveDomain] = useState<DomainId | null>(null);

  const handleSelect = useCallback((id: DomainId) => {
    setActiveDomain(id);
  }, []);

  const handleReset = useCallback(() => {
    setActiveDomain(null);
  }, []);

  const activeDomainData = useMemo(
    () => DOMAINS.find((d) => d.id === activeDomain),
    [activeDomain]
  );

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

  return (
    <div className="flex-1 flex flex-col min-w-0 z-10 h-full relative overflow-hidden bg-transparent select-none">

      {/* ── Main content area ──────────────────────────────────────────── */}
      <div className="flex-1 flex flex-col items-center justify-start min-h-0 overflow-hidden">

        <AnimatePresence mode="wait">
          {/* ── RADIAL HUB view ─────────────────────────────────────────── */}
          {!activeDomain ? (
            <motion.div
              key="hub"
              initial={{ opacity: 0, scale: 0.95 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.95 }}
              transition={{ duration: 0.3, ease: [0.16, 1, 0.3, 1] }}
              className="flex flex-col items-center justify-center flex-1 w-full gap-4"
            >

              {/* Radial hub container */}
              <div
                className="relative"
                style={{
                  width: HUB_RADIUS * 2 + 160,
                  height: HUB_RADIUS * 2 + 160,
                }}
              >
                {/* SVG connector lines */}
                <HubConnectors activeDomain={activeDomain} />

                {/* Domain nodes */}
                {DOMAINS.map((domain) => (
                  <RadialNode
                    key={domain.id}
                    domain={domain}
                    isActive={false}
                    onSelect={handleSelect}
                  />
                ))}

                {/* Center node */}
                <HubCenter onClick={() => {}} hasSelection={false} />
              </div>
            </motion.div>
          ) : (
            /* ── DOMAIN CONTENT view ─────────────────────────────────── */
            <motion.div
              key="content"
              initial={{ opacity: 0, y: 12 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: 8 }}
              transition={{ duration: 0.28, ease: [0.16, 1, 0.3, 1] }}
              className="flex flex-col items-center w-full h-full min-h-0"
            >
              {/* Pill nav header */}
              <div className="pt-5 pb-4 px-6 shrink-0 w-full flex justify-center">
                <PillNav
                  domains={DOMAINS}
                  activeDomain={activeDomain}
                  onSelect={handleSelect}
                  onReset={handleReset}
                />
              </div>

              {/* Domain title */}
              {activeDomainData && (
                <div className="pb-4 px-8 w-full shrink-0 max-w-[1400px]">
                  <div className="flex items-center gap-3">
                    <activeDomainData.icon
                      size={18}
                      className="text-[rgb(var(--accent))]"
                      strokeWidth={2}
                    />
                    <div>
                      <h1 className="text-[15px] font-bold text-[rgb(var(--foreground))]">
                        {activeDomainData.label}
                      </h1>
                      <p className="text-[10px] text-[rgb(var(--foreground-muted))]/50 uppercase tracking-widest">
                        {activeDomainData.sublabel}
                      </p>
                    </div>
                  </div>
                </div>
              )}

              {/* Scrollable content area */}
              <div className="flex-1 w-full overflow-y-auto custom-scrollbar px-8 pb-28 min-h-0 max-w-[1400px]">
                <AnimatePresence mode="wait">
                  <motion.div
                    key={activeDomain}
                    initial={{ opacity: 0, x: 10 }}
                    animate={{ opacity: 1, x: 0 }}
                    exit={{ opacity: 0, x: -6 }}
                    transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
                    className="h-full"
                  >
                    <DomainContent domain={activeDomain} />
                  </motion.div>
                </AnimatePresence>
              </div>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

    </div>
  );
};
