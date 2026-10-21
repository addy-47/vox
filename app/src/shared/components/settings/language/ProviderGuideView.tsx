import { memo } from "react";
import { ArrowLeft, BookOpen, Globe, HardDrive, Key, Network, Server } from "lucide-react";
import { PROVIDER_GUIDE_COPY } from "@/data/settingsCopy";

export interface ProviderGuideViewProps {
  onBackToProviders: () => void;
  category?: "STT" | "LLM" | "TTS";
}

export const ProviderGuideView = memo(function ProviderGuideView({
  onBackToProviders,
  category = "LLM",
}: ProviderGuideViewProps) {
  const { guideTitle, guideSubtitle, backToProviders, sections, steps } = PROVIDER_GUIDE_COPY;

  const sectionIcons: Record<string, typeof Globe> = {
    provider: Globe,
    url: Network,
    port: Server,
    apikey: Key,
  };

  return (
    <div
      className="h-full overflow-y-auto overscroll-contain custom-scrollbar p-1.5 sm:p-2.5 pr-2.5 sm:pr-4 select-none"
    >
      <div className="flex flex-col gap-4 animate-fade-in pb-4">
      {/* Top Banner / Navigation */}
      <div className="flex items-center justify-between gap-3 p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.12)]">
        <div className="flex items-center gap-2.5 min-w-0">
          <div className="w-8 h-8 rounded-lg bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] flex items-center justify-center shrink-0">
            <BookOpen size={16} />
          </div>
          <div className="min-w-0">
            <h3 className="font-display text-[14px] sm:text-[15px] font-bold text-[rgb(var(--foreground))] tracking-tight truncate">
              {category === "TTS" ? "Voice Synthesis Connection Guide" : guideTitle}
            </h3>
            <p className="text-[11.5px] text-[rgb(var(--foreground-muted))] truncate">
              {category === "TTS"
                ? "Configure Microsoft Edge TTS or connect a high-throughput Chatterbox GPU node."
                : guideSubtitle}
            </p>
          </div>
        </div>

        <button
          type="button"
          onClick={onBackToProviders}
          className="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[12px] font-bold text-[rgb(var(--accent))] bg-[rgba(var(--accent),0.08)] hover:bg-[rgba(var(--accent),0.18)] transition-colors cursor-pointer shrink-0"
        >
          <ArrowLeft size={13} strokeWidth={2.5} />
          <span>{backToProviders}</span>
        </button>
      </div>

      {/* Plain-Language Connection Cards */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
        {[
          { icon: Globe, title: "Cloud Providers", body: "Add an API key from your provider (OpenRouter, NVIDIA, etc.) and save." },
          { icon: Server, title: "Custom Servers", body: "Connect self-hosted Ollama, vLLM, or LM Studio endpoints on your local network." },
          { icon: HardDrive, title: "On-Device Models", body: "Run private, zero-telemetry local models directly on your hardware." },
        ].map((card) => (
          <div
            key={card.title}
            className="p-3.5 rounded-xl bg-[rgba(var(--card),0.5)] border border-[rgba(var(--foreground),0.06)] flex flex-col gap-2"
          >
            <div className="flex items-center gap-2 text-[rgb(var(--accent))] font-display font-bold text-[13px]">
              <card.icon size={15} />
              <span>{card.title}</span>
            </div>
            <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed">
              {card.body}
            </p>
          </div>
        ))}
      </div>

      {/* 2x2 Concept Explainer Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
        {sections.map((sec) => {
          const IconComponent = sectionIcons[sec.id] || Globe;
          return (
            <div
              key={sec.id}
              className="p-3.5 rounded-xl bg-[rgba(var(--card),0.5)] border border-[rgba(var(--foreground),0.06)] flex flex-col gap-2"
            >
              <div className="flex items-center gap-2 text-[rgb(var(--accent))] font-display font-bold text-[13px]">
                <IconComponent size={15} />
                <span>{sec.title}</span>
              </div>
              <p className="text-[12px] text-[rgb(var(--foreground-muted))] leading-relaxed">
                {sec.description}
              </p>
            </div>
          );
        })}
      </div>

      {/* Step-by-Step Setup Sequence */}
      <div className="p-4 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] flex flex-col gap-3">
        <h4 className="font-display text-[13px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">
          Quick Connection Steps
        </h4>
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
          {steps.map((st) => (
            <div
              key={st.number}
              className="flex flex-col gap-1.5 p-3 rounded-lg bg-[rgba(var(--card),0.4)] border border-[rgba(var(--foreground),0.05)]"
            >
              <div className="flex items-center gap-2">
                <span className="w-5 h-5 rounded-full bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[11px] font-mono font-bold flex items-center justify-center shrink-0">
                  {st.number}
                </span>
                <span className="font-display text-[12px] font-bold text-[rgb(var(--foreground))]">
                  {st.title}
                </span>
              </div>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] leading-relaxed pl-7">
                {st.body}
              </p>
            </div>
          ))}
        </div>
      </div>
    </div>
  </div>
  );
});

ProviderGuideView.displayName = "ProviderGuideView";
