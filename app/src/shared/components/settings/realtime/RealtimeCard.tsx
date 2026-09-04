import { memo, useState, useMemo } from "react";
import { useSettingsStore, type VoxSettings } from "@/store/settingsStore";
import {
  Search,
  Cpu,
  Mic,
  Speaker,
  Radio,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";



const VOICE_OPTIONS = ["Aoede", "Charon", "Fenrir", "Kore", "Puck"];

const VOICE_INFO: Record<string, { desc: string }> = {
  Aoede: { desc: "Warm & expressive" },
  Charon: { desc: "Deep & resonant" },
  Fenrir: { desc: "Bold & powerful" },
  Kore: { desc: "Bright & clear" },
  Puck: { desc: "Playful & light" },
};


const PipelineFlow = ({ active }: { active: boolean }) => {
  // Generate 3 particles per connection with unique delays/drifts
  const particles = useMemo(
    () =>
      Array.from({ length: 3 }, (_, i) => ({
        delay: `${i * 0.8}s`,
        drift: `${(i % 2 === 0 ? 1 : -1) * (6 + i * 3)}px`,
      })),
    [],
  );
  const returnParticles = useMemo(
    () =>
      Array.from({ length: 2 }, (_, i) => ({
        delay: `${0.5 + i * 1.2}s`,
        drift: `${(i % 2 === 0 ? -1 : 1) * (4 + i * 2)}px`,
      })),
    [],
  );

  return (
    <div className="relative w-full overflow-hidden rounded-xl border border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.02)]">
      <div className="flex items-center justify-between relative z-10 px-3 py-2.5">
        {/* Stage 1: Capture */}
        <div className="flex flex-col items-center gap-1 shrink-0">
          <div
            className={cn(
              "w-[34px] h-[34px] rounded-full flex items-center justify-center border-2 transition-all duration-500",
              active
                ? "border-[rgba(var(--accent),0.35)] node-glow-active"
                : "border-[rgba(var(--border),0.1)]",
            )}
          >
            <Mic
              size={13}
              className={cn(
                active
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/40",
              )}
            />
          </div>
          <span
            className={cn(
              "text-[11px] font-bold uppercase transition-colors",
              active
                ? "text-[rgb(var(--accent))]/80"
                : "text-[rgb(var(--foreground-muted))]/40",
            )}
          >
            Capture
          </span>
        </div>

        {/* Connection 1 → 2 */}
        <div className="flex-1 relative h-[34px] mx-2 flex items-center">
          {/* Energy ribbon track */}
          <div
            className={cn(
              "absolute inset-x-0 h-[2px] rounded-full",
              active
                ? "bg-[rgba(var(--accent),0.1)]"
                : "bg-[rgba(var(--border),0.06)]",
            )}
          />
          {/* Flowing gradient ribbon */}
          {active && (
            <div
              className="absolute inset-x-0 h-[2px] rounded-full"
              style={{
                background:
                  "linear-gradient(90deg, transparent 0%, rgba(var(--accent-rgb),0.6) 30%, rgba(var(--accent-rgb),0.3) 60%, transparent 100%)",
                backgroundSize: "200% 100%",
                animation: "ribbon-flow 2s linear infinite",
                filter: "blur(2px)",
                height: "6px",
                top: "-2px",
              }}
            />
          )}
          {/* Forward particles */}
          {active &&
            particles.map((p, i) => (
              <span
                key={i}
                className="absolute w-1 h-1 rounded-full bg-[rgb(var(--accent))]"
                style={
                  {
                    animation: `particle-drift 2.4s ease-in-out ${p.delay} infinite`,
                    "--drift": p.drift,
                    left: "0%",
                  } as React.CSSProperties
                }
              />
            ))}
          {/* Return particles (duplex) */}
          {active &&
            returnParticles.map((p, i) => (
              <span
                key={`r${i}`}
                className="absolute w-[3px] h-[3px] rounded-full bg-[rgb(var(--accent))]/50"
                style={
                  {
                    animation: `particle-return 2.8s ease-in-out ${p.delay} infinite`,
                    "--drift": p.drift,
                    left: "0%",
                  } as React.CSSProperties
                }
              />
            ))}
        </div>

        {/* Stage 2: S2S Gateway */}
        <div className="flex flex-col items-center gap-1 shrink-0 relative">
          <div
            className={cn(
              "w-[34px] h-[34px] rounded-full flex items-center justify-center border-2 transition-all duration-500",
              active
                ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.06)] node-glow-active"
                : "border-[rgba(var(--border),0.1)]",
            )}
          >
            <Radio
              size={13}
              className={cn(
                active && "animate-pulse",
                active
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/40",
              )}
            />
          </div>
          <span
            className={cn(
              "text-[11px] font-bold uppercase transition-colors",
              active
                ? "text-[rgb(var(--accent))]"
                : "text-[rgb(var(--foreground-muted))]/40",
            )}
          >
            Think
          </span>
          {active && (
            <span className="absolute -top-0.5 -right-0.5 w-[6px] h-[6px] rounded-full bg-[rgb(var(--accent))]">
              <span className="absolute inset-0 rounded-full bg-[rgb(var(--accent))] animate-ping opacity-40" />
            </span>
          )}
        </div>

        {/* Connection 2 → 3 */}
        <div className="flex-1 relative h-[34px] mx-2 flex items-center">
          <div
            className={cn(
              "absolute inset-x-0 h-[2px] rounded-full",
              active
                ? "bg-[rgba(var(--accent),0.1)]"
                : "bg-[rgba(var(--border),0.06)]",
            )}
          />
          {active && (
            <div
              className="absolute inset-x-0 h-[2px] rounded-full"
              style={{
                background:
                  "linear-gradient(90deg, transparent 0%, rgba(var(--accent-rgb),0.6) 30%, rgba(var(--accent-rgb),0.3) 60%, transparent 100%)",
                backgroundSize: "200% 100%",
                animation: "ribbon-flow 2s linear infinite",
                filter: "blur(2px)",
                height: "6px",
                top: "-2px",
              }}
            />
          )}
          {active &&
            particles.map((p, i) => (
              <span
                key={`c2-${i}`}
                className="absolute w-1 h-1 rounded-full bg-[rgb(var(--accent))]"
                style={
                  {
                    animation: `particle-drift 2.4s ease-in-out ${p.delay} infinite`,
                    "--drift": p.drift,
                    left: "0%",
                  } as React.CSSProperties
                }
              />
            ))}
          {active &&
            returnParticles.map((p, i) => (
              <span
                key={`cr2-${i}`}
                className="absolute w-[3px] h-[3px] rounded-full bg-[rgb(var(--accent))]/50"
                style={
                  {
                    animation: `particle-return 2.8s ease-in-out ${p.delay} infinite`,
                    "--drift": p.drift,
                    left: "0%",
                  } as React.CSSProperties
                }
              />
            ))}
        </div>

        {/* Stage 3: Render */}
        <div className="flex flex-col items-center gap-1 shrink-0">
          <div
            className={cn(
              "w-[34px] h-[34px] rounded-full flex items-center justify-center border-2 transition-all duration-500",
              active
                ? "border-[rgba(var(--accent),0.35)] node-glow-active"
                : "border-[rgba(var(--border),0.1)]",
            )}
          >
            <Speaker
              size={13}
              className={cn(
                active
                  ? "text-[rgb(var(--accent))]"
                  : "text-[rgb(var(--foreground-muted))]/40",
              )}
            />
          </div>
          <span
            className={cn(
              "text-[11px] font-bold uppercase transition-colors",
              active
                ? "text-[rgb(var(--accent))]/80"
                : "text-[rgb(var(--foreground-muted))]/40",
            )}
          >
            Speak
          </span>
        </div>
      </div>

      {/* End of Pipeline Flow */}
    </div>
  );
};


function Input({
  label,
  value,
  onChange,
  placeholder,
  disabled,
}: {
  label?: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  disabled?: boolean;
}) {
  return (
    <div
      className={cn(
        "bg-transparent border border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.12)] transition-all duration-300 rounded-xl px-3.5 py-2 flex flex-col justify-center gap-0.5 min-h-[58px]",
        disabled && "opacity-50",
      )}
    >
      {label && (
        <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 block leading-none">
          {label}
        </span>
      )}
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        className={cn(
          "w-full bg-transparent border-none outline-none text-[13px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25",
          disabled ? "cursor-not-allowed" : "",
        )}
      />
    </div>
  );
}

function TemperatureSlider({
  label,
  value,
  onChange,
  disabled,
}: {
  label?: string;
  value: number;
  onChange: (v: number) => void;
  disabled?: boolean;
}) {
  return (
    <div
      className={cn(
        "bg-transparent border border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.12)] transition-all duration-300 rounded-xl px-3.5 py-2.5 flex flex-col justify-center gap-1.5 min-h-[58px]",
        disabled && "opacity-50",
      )}
    >
      <div className="flex items-center justify-between">
        {label && (
          <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 leading-none">
            {label}
          </span>
        )}
        <span className="text-[11px] font-mono font-bold px-1.5 py-0.5 rounded-md bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/20 text-[rgb(var(--accent))] leading-none">
          {value.toFixed(2)}
        </span>
      </div>
      <input
        type="range"
        min="0.0"
        max="1.0"
        step="0.05"
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        disabled={disabled}
        className={cn(
          "w-full h-1 rounded-lg appearance-none cursor-pointer mt-1",
          disabled ? "cursor-not-allowed" : "",
        )}
      />
    </div>
  );
}

function ToggleRow({
  label,
  sub,
  enabled,
  onChange,
  icon,
  disabled,
}: {
  label: string;
  sub?: string;
  enabled: boolean;
  onChange: () => void;
  icon?: React.ReactNode;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      onClick={onChange}
      disabled={disabled}
      className={cn(
        "bg-transparent border border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.12)] transition-all duration-300 rounded-xl px-3.5 py-2 flex items-center justify-between text-left min-h-[58px] w-full",
        disabled ? "opacity-50 cursor-not-allowed" : "cursor-pointer",
      )}
    >
      <div className="flex flex-col gap-0.5">
        <span className="text-[11px] font-bold uppercase tracking-wider flex items-center gap-1.5 text-[rgb(var(--foreground))]/90">
          {icon}
          {label}
        </span>
        {sub && (
          <span className="text-[11px] text-[rgb(var(--foreground-muted))]/60 leading-tight">
            {sub}
          </span>
        )}
      </div>

      {/* Sleek Switch */}
      <div
        className={cn(
          "w-8 h-5 rounded-full relative shrink-0 transition-colors duration-300",
          enabled
            ? "bg-[rgb(var(--accent))]"
            : "bg-[rgba(var(--foreground),0.1)]",
        )}
      >
        <span
          className={cn(
            "w-4 h-4 rounded-full bg-white absolute top-0.5 transition-all duration-300 shadow-sm",
            enabled ? "left-[14px]" : "left-0.5",
          )}
        />
      </div>
    </button>
  );
}


function VoiceBars({ seed, disabled }: { seed: string; disabled?: boolean }) {
  const bars = useMemo(() => {
    const base = [...seed].reduce((a, c) => a + c.charCodeAt(0), 0);
    return Array.from({ length: 12 }, (_, i) => {
      const h = (base + i * 17) % 100;
      return 12 + (h % 13); // range 12–24px
    });
  }, [seed]);

  const animations = [
    { dur: "0.65s", delay: "-0.1s" },
    { dur: "0.85s", delay: "-0.5s" },
    { dur: "0.55s", delay: "-0.3s" },
    { dur: "0.75s", delay: "-0.7s" },
    { dur: "0.95s", delay: "-0.2s" },
    { dur: "0.60s", delay: "-0.6s" },
    { dur: "0.80s", delay: "-0.4s" },
    { dur: "0.50s", delay: "-0.8s" },
    { dur: "0.70s", delay: "-0.1s" },
    { dur: "0.90s", delay: "-0.5s" },
    { dur: "0.65s", delay: "-0.3s" },
    { dur: "0.75s", delay: "-0.6s" },
  ];

  return (
    <div className="flex items-end justify-center gap-[4px] h-8 px-4 py-0.5">
      {bars.map((h, i) => {
        const anim = animations[i % animations.length];
        return (
          <div
            key={i}
            className={cn(
              "w-[4px] rounded-full transition-all duration-300",
              disabled
                ? "bg-[rgba(var(--foreground-muted),0.1)]"
                : "bg-gradient-to-t from-[rgba(var(--accent-dark),0.4)] to-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.2)]",
            )}
            style={{
              height: `${h}px`,
              animation: disabled
                ? "none"
                : `dynamic-eq ${anim.dur} ease-in-out infinite alternate ${anim.delay}`,
              transformOrigin: "bottom",
            }}
          />
        );
      })}
    </div>
  );
}

function VoiceCarousel({
  selected,
  onChange,
  disabled,
}: {
  selected: string;
  onChange: (id: string) => void;
  disabled?: boolean;
}) {
  const [index, setIndex] = useState(() =>
    Math.max(0, VOICE_OPTIONS.indexOf(selected)),
  );

  // Keep internal index in sync when parent changes selected voice
  const selectedIndex = VOICE_OPTIONS.indexOf(selected);
  if (selectedIndex !== -1 && selectedIndex !== index) {
    setIndex(selectedIndex);
  }

  const currentVoice = VOICE_OPTIONS[index] || VOICE_OPTIONS[0];
  const info = VOICE_INFO[currentVoice];

  const cycle = (dir: number) => {
    if (disabled) return;
    const next = (index + dir + VOICE_OPTIONS.length) % VOICE_OPTIONS.length;
    setIndex(next);
    onChange(VOICE_OPTIONS[next]);
  };

  return (
    <div
      className={cn(
        "bg-transparent border border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.12)] transition-all duration-300 rounded-xl p-3.5 flex flex-col justify-between h-full w-full",
        disabled && "opacity-50",
      )}
    >
      {/* Voice Title */}
      <span className="text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 block text-center leading-none">
        Select Voice
      </span>


      {/* Carousel name + arrows */}
      <div className="flex items-center justify-between gap-1 my-2">
        <button
          type="button"
          onClick={() => cycle(-1)}
          disabled={disabled}
          className="p-1.5 rounded-lg hover:bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-all duration-300 shrink-0 disabled:opacity-20 disabled:cursor-not-allowed"
          aria-label="Previous Voice"
        >
          <ChevronLeft size={16} />
        </button>

        <div className="flex-1 text-center min-w-0">
          <span
            className={cn(
              "text-[15px] font-black tracking-wide block truncate text-[rgb(var(--foreground))]",
            )}
          >
            {currentVoice}
          </span>
          {info && (
            <span
              className={cn(
                "text-[11px] block leading-normal mt-0.5 truncate text-[rgb(var(--foreground-muted))]/70",
              )}
            >
              {info.desc}
            </span>
          )}
        </div>

        <button
          type="button"
          onClick={() => cycle(1)}
          disabled={disabled}
          className="p-1.5 rounded-lg hover:bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-all duration-300 shrink-0 disabled:opacity-20 disabled:cursor-not-allowed"
          aria-label="Next Voice"
        >
          <ChevronRight size={16} />
        </button>
      </div>

      {/* 12-bar organic visualizer */}
      <div className="flex items-center justify-center py-2 shrink-0">
        <VoiceBars seed={currentVoice} disabled={disabled} />
      </div>

      {/* Dots indicator */}
      <div className="flex items-center justify-center gap-1.5 shrink-0 mt-1">
        {VOICE_OPTIONS.map((v, i) => (
          <span
            key={v}
            className={cn(
              "rounded-full transition-all duration-300",
              i === index
                ? "w-2.5 h-1 bg-[rgb(var(--accent))] rounded-full"
                : "w-1 h-1 bg-[rgba(var(--foreground-muted),0.25)]",
            )}
          />
        ))}
      </div>
    </div>
  );
}


function UnifiedConfig({
  subkey,
  draftSettings,
  updateDraft,
  disabled,
  layoutMode = "full-max",
}: {
  subkey: string;
  draftSettings: VoxSettings;
  updateDraft: (section: any, key: any, value: any) => void;
  disabled: boolean;
  layoutMode?: "full-max" | "full-min" | "small";
}) {
  const isDeepgram = subkey === "deepgram_voice_agent" || subkey === "deepgram";
  const isOpenAi = subkey === "openai_realtime" || subkey === "openai";
  const isElevenLabs = subkey === "elevenlabs_convai" || subkey === "elevenlabs";
  const isGemini = !isDeepgram && !isOpenAi && !isElevenLabs;

  const canonicalSubkey = isDeepgram
    ? "deepgram_voice_agent"
    : isOpenAi
    ? "openai_realtime"
    : isElevenLabs
    ? "elevenlabs_convai"
    : "gemini_live";

  const realtime = draftSettings.realtime;
  const config: Record<string, any> =
    (realtime as any)?.[canonicalSubkey] ||
    (realtime as any)?.[subkey] ||
    (isGemini ? realtime?.gemini : isDeepgram ? realtime?.deepgram : {}) ||
    {};

  const voiceField = isGemini ? "voice_name" : "voice";
  const currentVoice = (config[voiceField] as string) || VOICE_OPTIONS[0];

  return (
    <div
      className={cn(
        "w-full items-stretch",
        layoutMode === "small"
          ? "flex flex-col gap-3"
          : "flex flex-row gap-3.5",
        disabled && "opacity-60 pointer-events-none select-none",
      )}
    >
      {/* Left column: Model, Temperature, Toggle (vertical) */}
      <div className="flex-[3] flex flex-col gap-3 min-w-0">
        {/* Model ID — default shows the model name */}
        <Input
          label="Model"
          value={config.model || ""}
          onChange={(v) => {
            if (!disabled)
              updateDraft("realtime", canonicalSubkey, { ...config, model: v });
          }}
          placeholder={"Model ID"}
          disabled={disabled}
        />

        {/* Temperature */}
        <TemperatureSlider
          label="Temperature"
          value={config.temperature ?? 0.7}
          onChange={(v) => {
            if (!disabled)
              updateDraft("realtime", canonicalSubkey, { ...config, temperature: v });
          }}
          disabled={disabled}
        />

        {/* Toggle (only rendered when provider has supported boolean flags) */}
        {(isGemini || isDeepgram) && (
          <ToggleRow
            label={isGemini ? "Google Search" : "Agent Mode"}
            sub={isGemini ? "Live web grounding" : "AI agent routing"}
            enabled={isGemini ? Boolean(config.enable_web_search) : Boolean(config.agent_mode)}
            onChange={() => {
              if (disabled) return;
              if (isGemini) {
                updateDraft("realtime", "gemini_live", {
                  ...config,
                  enable_web_search: !config.enable_web_search,
                });
              } else if (isDeepgram) {
                updateDraft("realtime", "deepgram_voice_agent", {
                  ...config,
                  agent_mode: !config.agent_mode,
                });
              }
            }}
            icon={
              isGemini ? (
                <Search size={11} className="text-[rgb(var(--accent))]" />
              ) : undefined
            }
            disabled={disabled}
          />
        )}
      </div>

      {/* Right column: Voice carousel */}
      <div
        className={cn(
          "shrink-0",
          layoutMode === "small" ? "w-full" : "w-2/5 min-w-[100px]",
        )}
      >
        <VoiceCarousel
          selected={currentVoice}
          onChange={(v) => {
            if (disabled) return;
            updateDraft("realtime", canonicalSubkey, { ...config, [voiceField]: v });
          }}
          disabled={disabled}
        />
      </div>
    </div>
  );
}


interface RealtimeCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const RealtimeCard = memo(
  ({ layoutMode = "full-max" }: RealtimeCardProps) => {
    const draftSettings = useSettingsStore((s) => s.draftSettings);
    const updateDraft = useSettingsStore((s) => s.updateDraft);

    if (!draftSettings) return null;

    const providerId =
      draftSettings.realtime?.active ||
      draftSettings.realtime?.provider ||
      "gemini_live";

    const subkeyMap: Record<string, string> = {
      gemini_live: "gemini_live",
      gemini: "gemini_live",
      openai_realtime: "openai_realtime",
      openai: "openai_realtime",
      deepgram_voice_agent: "deepgram_voice_agent",
      deepgram: "deepgram_voice_agent",
      elevenlabs_convai: "elevenlabs_convai",
      elevenlabs: "elevenlabs_convai",
    };
    const subkey = subkeyMap[providerId] || "gemini_live";
    const disabled =
      (providerId as string) !== "gemini_live" &&
      (providerId as string) !== "deepgram_voice_agent";

    return (
      <div
        className={cn(
          "w-full h-auto flex flex-col text-[14px] gap-3 leading-relaxed text-[rgb(var(--foreground))]/85 select-none",
          layoutMode === "small"
            ? "bg-transparent p-0"
            : cn(
                "glass-card p-5",
                layoutMode === "full-min"
                  ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]"
                  : "lg:w-[520px]",
              ),
        )}
      >
        {/* ── Header ──────────────────────────────────────────────────────── */}
        {layoutMode !== "small" && (
          <div className="flex items-center justify-between shrink-0">
            <div className="flex items-center gap-2">
              <Cpu className="text-[rgb(var(--accent))]" size={16} />
              <span className="font-display text-[12px] font-bold uppercase tracking-[0.2em] text-[rgb(var(--accent))]/80">
                Realtime Hub
              </span>
            </div>
            <span className="text-[11px] font-bold uppercase text-[rgb(var(--foreground-muted))]/60">
              Live Mode
            </span>
          </div>
        )}

        {/* ── Pipeline Flow (transparent container) ──────────────────────── */}
        <PipelineFlow active={true} />

        {/* ── Config workspace: Unified Glass Desk Container ─────────── */}
        <div className="w-full flex flex-col shrink-0 rounded-xl p-3 relative border border-[rgba(var(--accent),0.06)] bg-[rgba(var(--foreground),0.02)]">
          <UnifiedConfig
            subkey={subkey}
            draftSettings={draftSettings}
            updateDraft={updateDraft}
            disabled={disabled}
            layoutMode={layoutMode}
          />
        </div>
      </div>
    );
  },
);

RealtimeCard.displayName = "RealtimeCard";
