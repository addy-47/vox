import { memo } from "react";
import { Brain, Mic, Volume2 } from "lucide-react";
import { type RuntimeSnapshot } from "@/services/pipelineService";
import { type DynamicColors } from "./colorUtils";
import { cn } from "@/shared/lib/utils";
import { MONITORING_COPY } from "@/data/monitoringCopy";

export interface ModelResidency {
  llm: string;
  tts: string;
  stt: string;
}

interface ModelResidencyTilesProps {
  latest: RuntimeSnapshot | null;
  colors: DynamicColors;
  variants: ModelResidency;
  isLight: boolean;
}

/**
 * The 3 core-model residency tiles (LLM/STT/TTS residency + loaded state).
 * Moved here from the bottom of LiquidChamber so the chamber keeps only its
 * fluid + counter + edge marks; these tiles now sit above it where the metric
 * carousel used to be.
 */
export const ModelResidencyTiles = memo<ModelResidencyTilesProps>(({
  latest,
  colors,
  variants,
  isLight,
}) => {
  return (
    <div className="relative z-10 grid grid-cols-3 gap-2.5 w-full max-w-md">
      {/* LLM Variant */}
      <div
        style={{
          borderColor: latest?.is_llm_loaded
            ? `rgba(${colors.primary}, 0.65)`
            : "rgba(var(--border), 0.12)",
          boxShadow: latest?.is_llm_loaded
            ? `0 0 16px rgba(${colors.primary}, 0.20), inset 0 1px 1px rgba(var(--card), 0.25)`
            : "none",
        }}
        className={cn(
          "px-3 py-2 rounded-2xl border backdrop-blur-md flex flex-col items-center text-center shadow-md transition-colors duration-300",
          isLight
            ? "bg-[rgba(var(--card),0.55)] hover:bg-[rgba(var(--card),0.75)]"
            : "bg-[rgba(var(--card),0.80)] hover:bg-[rgba(var(--card),0.95)]"
        )}
      >
        <div className="flex items-center gap-1.5 text-[11px] font-mono font-bold text-[rgb(var(--foreground-muted))] uppercase">
          <Brain size={11} style={{ color: `rgb(${colors.primary})` }} />
          <span>{MONITORING_COPY.chamberThinking}</span>
        </div>
        <span
          style={{
            color: latest?.is_llm_loaded ? `rgb(${colors.primary})` : "rgb(var(--foreground))",
          }}
          className="text-[12px] font-sans font-black tracking-wide uppercase mt-0.5 truncate max-w-full"
        >
          {variants.llm}
        </span>
      </div>

      {/* STT Variant */}
      <div
        style={{
          borderColor: latest?.is_stt_loaded
            ? `rgba(${colors.complementary}, 0.65)`
            : "rgba(var(--border), 0.12)",
          boxShadow: latest?.is_stt_loaded
            ? `0 0 16px rgba(${colors.complementary}, 0.20), inset 0 1px 1px rgba(var(--card), 0.25)`
            : "none",
        }}
        className={cn(
          "px-3 py-2 rounded-2xl border backdrop-blur-md flex flex-col items-center text-center shadow-md transition-colors duration-300",
          isLight
            ? "bg-[rgba(var(--card),0.55)] hover:bg-[rgba(var(--card),0.75)]"
            : "bg-[rgba(var(--card),0.80)] hover:bg-[rgba(var(--card),0.95)]"
        )}
      >
        <div className="flex items-center gap-1.5 text-[11px] font-mono font-bold text-[rgb(var(--foreground-muted))] uppercase">
          <Mic size={11} style={{ color: `rgb(${colors.complementary})` }} />
          <span>{MONITORING_COPY.chamberHearing}</span>
        </div>
        <span
          style={{
            color: latest?.is_stt_loaded
              ? `rgb(${colors.complementary})`
              : "rgb(var(--foreground))",
          }}
          className="text-[12px] font-sans font-black tracking-wide uppercase mt-0.5 truncate max-w-full"
        >
          {variants.stt}
        </span>
      </div>

      {/* TTS Variant */}
      <div
        style={{
          borderColor: latest?.is_tts_loaded
            ? `rgba(${colors.primary}, 0.65)`
            : "rgba(var(--border), 0.12)",
          boxShadow: latest?.is_tts_loaded
            ? `0 0 16px rgba(${colors.primary}, 0.20), inset 0 1px 1px rgba(var(--card), 0.25)`
            : "none",
        }}
        className={cn(
          "px-3 py-2 rounded-2xl border backdrop-blur-md flex flex-col items-center text-center shadow-md transition-colors duration-300",
          isLight
            ? "bg-[rgba(var(--card),0.55)] hover:bg-[rgba(var(--card),0.75)]"
            : "bg-[rgba(var(--card),0.80)] hover:bg-[rgba(var(--card),0.95)]"
        )}
      >
        <div className="flex items-center gap-1.5 text-[11px] font-mono font-bold text-[rgb(var(--foreground-muted))] uppercase">
          <Volume2 size={11} style={{ color: `rgb(${colors.primary})` }} />
          <span>{MONITORING_COPY.chamberSpeaking}</span>
        </div>
        <span
          style={{
            color: latest?.is_tts_loaded ? `rgb(${colors.primary})` : "rgb(var(--foreground))",
          }}
          className="text-[12px] font-sans font-black tracking-wide uppercase mt-0.5 truncate max-w-full"
        >
          {variants.tts}
        </span>
      </div>
    </div>
  );
});

ModelResidencyTiles.displayName = "ModelResidencyTiles";
