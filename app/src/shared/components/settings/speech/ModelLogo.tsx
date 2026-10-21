import { memo } from "react";
import {
  Activity,
  Cloud,
  Cpu,
  Ear,
  Globe,
  Grid3x3,
  Heart,
  Languages,
  MessageCircle,
  Sparkles,
  Waves,
  Zap,
  type LucideIcon,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";

/**
 * Mark for a speech model family.
 *
 * Deliberately never an official brand logo: those live only in the language
 * model provider card. Here every family gets one stroke glyph in the accent
 * colour, so a grid of mixed vendors still scans as one system.
 *
 * Keys are family fragments, never manifest group ids: the frontend stays
 * model-agnostic (lint-arch invariant 6), and a renamed group still matches.
 */
const GLYPH_MARKS: Record<string, { Icon: LucideIcon }> = {
  // hexgrad Kokoro — heart in Japanese.
  koko: { Icon: Heart },
  // Resemble AI Chatterbox — a talkative voice.
  chatt: { Icon: MessageCircle },
  // Supertone Supertonic — clean, bright tone.
  super: { Icon: Sparkles },
  // ZipVoice — fast, zip.
  zipv: { Icon: Zap },
  // Silero — a voice-activity trace.
  siler: { Icon: Activity },
  // TEN Framework.
  ten_: { Icon: Grid3x3 },
  // NVIDIA Nemotron — hardware-tuned recognition.
  nvidia: { Icon: Cpu },
  // Qwen ASR — multilingual recognition.
  qwen: { Icon: Languages },
  // Google Cloud TTS.
  google: { Icon: Cloud },
  // Azure AI Speech.
  azure: { Icon: Waves },
  // Edge Neural voices — internet voices.
  edge: { Icon: Globe },
};

export interface ModelLogoProps {
  modelId: string;
  size?: number;
  className?: string;
}

export const ModelLogo = memo(({ modelId, size = 40, className }: ModelLogoProps) => {
  const id = modelId.toLowerCase();
  const glyph = Object.entries(GLYPH_MARKS).find(([key]) => id.includes(key))?.[1];
  const Icon = glyph?.Icon ?? (id.includes("ear") ? Ear : null);

  return (
    <span
      className={cn(
        "grid shrink-0 place-items-center rounded-xl border",
        "border-[rgba(var(--foreground),0.07)] bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--accent))]",
        className
      )}
      style={{ width: size, height: size }}
      aria-hidden="true"
    >
      {Icon ? (
        <Icon size={Math.round(size * 0.5)} strokeWidth={1.9} />
      ) : (
        <span className="font-display font-black leading-none" style={{ fontSize: size * 0.4 }}>
          {modelId.slice(0, 1).toUpperCase()}
        </span>
      )}
    </span>
  );
});

ModelLogo.displayName = "ModelLogo";
