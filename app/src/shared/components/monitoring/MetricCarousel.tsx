import { useMemo, useState, memo } from "react";
import {
  Mic,
  Brain,
  Volume2,
  Clock,
  ChevronLeft,
  ChevronRight,
  Headphones,
  Database,
} from "lucide-react";
import { type RuntimeSnapshot } from "@/services/pipelineService";
import { MONITORING_COPY } from "@/data/monitoringCopy";
import { type DynamicColors } from "./colorUtils";

interface MetricCarouselProps {
  latest: RuntimeSnapshot | null;
  colors: DynamicColors;
  formatLatency: (ms: number | null) => string;
}

export const MetricCarousel = memo<MetricCarouselProps>(({
  latest,
  colors,
  formatLatency,
}) => {
  const [carouselPage, setCarouselPage] = useState(0);

  const metricCards = useMemo(() => {
    return [
      // Card 1: Speech to Text (Transcription Speed)
      {
        id: "transcription",
        title: "Hearing Delay",
        subtitle: "Speech Recognition",
        icon: Mic,
        value: formatLatency(latest?.stt_latency_ms ?? null),
        color: `rgb(${colors.complementary})`,
      },
      // Card 2: AI First Word Delay
      {
        id: "first_word",
        title: "Thinking Delay",
        subtitle: "First Word Ready",
        icon: Brain,
        value: formatLatency(latest?.ttft_ms ?? null),
        color: `rgb(${colors.primary})`,
      },
      // Card 3: Total Voice Response Time
      {
        id: "total_turnaround",
        title: "Voice Turnaround",
        subtitle: "Full Conversation Delay",
        icon: Clock,
        value: formatLatency(latest?.total_voice_latency_ms ?? null),
        color: `rgb(${colors.primary})`,
      },
      // Card 4: Voice Speech Synthesis Speed Multiplier
      {
        id: "speech_speed",
        title: "Speaking Speed",
        subtitle: "Generation Rate",
        icon: Volume2,
        value: latest?.tts_rtf != null ? `${latest.tts_rtf.toFixed(2)}×` : "--",
        color: `rgb(${colors.complementary})`,
      },
      // Card 5: Audio Output Delay
      {
        id: "audio_playback",
        title: "Audio Playback",
        subtitle: "Speaker Stream Delay",
        icon: Headphones,
        value: formatLatency(latest?.playback_start_ms ?? null),
        color: `rgb(${colors.primary})`,
      },
      // Card 6: Memory Database Health
      {
        id: "memory_db",
        title: "Memory System",
        subtitle: "Knowledge Storage",
        icon: Database,
        value: latest?.is_db_healthy ? "Healthy" : "Standby",
        color: `rgb(${colors.complementary})`,
      },
    ];
  }, [latest, formatLatency, colors]);

  const totalPages = Math.ceil(metricCards.length / 2);

  const handleNextPage = () => {
    setCarouselPage((prev) => (prev + 1) % totalPages);
  };

  const handlePrevPage = () => {
    setCarouselPage((prev) => (prev - 1 + totalPages) % totalPages);
  };

  const visibleCards = metricCards.slice(carouselPage * 2, carouselPage * 2 + 2);

  return (
    <div className="pt-3 pb-3 shrink-0 flex flex-col gap-1.5">
      <div className="relative flex items-center">
        {/* Left Arrow Button */}
        <button
          onClick={handlePrevPage}
          aria-label={MONITORING_COPY.carouselPrev}
          className="absolute -left-1.5 z-20 p-1 rounded-full bg-[rgba(var(--card),0.85)] border border-[rgba(var(--border),0.12)] text-[rgb(var(--foreground))] hover:scale-110 transition-transform shadow-md cursor-pointer backdrop-blur-sm"
        >
          <ChevronLeft size={14} />
        </button>

        {/* Cards Grid Container */}
        <div className="grid grid-cols-2 gap-2.5 w-full px-4">
          {visibleCards.map((card) => {
            const IconComp = card.icon;

            return (
              <div
                key={card.id}
                className="p-3 rounded-2xl bg-[rgba(var(--card),0.4)] border border-[rgba(var(--border),0.08)] flex flex-col justify-between shadow-sm min-h-[72px]"
              >
                <div className="flex items-center gap-1.5 text-[11px] font-bold text-[rgb(var(--foreground-muted))] uppercase">
                  <IconComp size={13} style={{ color: card.color }} />
                  <span className="truncate">{card.title}</span>
                </div>
                <div className="text-[15px] font-mono text-[rgb(var(--foreground))] pt-1">
                  {card.value}
                </div>
                <span className="text-[11px] font-sans text-[rgb(var(--foreground-muted))]/80 leading-none mt-0.5">
                  {card.subtitle}
                </span>
              </div>
            );
          })}
        </div>

        {/* Right Arrow Button */}
        <button
          onClick={handleNextPage}
          aria-label={MONITORING_COPY.carouselNext}
          className="absolute -right-1.5 z-20 p-1 rounded-full bg-[rgba(var(--card),0.85)] border border-[rgba(var(--border),0.12)] text-[rgb(var(--foreground))] hover:scale-110 transition-transform shadow-md cursor-pointer backdrop-blur-sm"
        >
          <ChevronRight size={14} />
        </button>
      </div>

      {/* Carousel Pagination Dots */}
      <div className="flex items-center justify-center gap-1.5 pt-0.5">
        {Array.from({ length: totalPages }).map((_, idx) => (
          <button
            key={idx}
            onClick={() => setCarouselPage(idx)}
            aria-label={`Go to metric page ${idx + 1}`}
            style={{
              backgroundColor:
                carouselPage === idx
                  ? `rgb(${colors.primary})`
                  : "rgba(var(--foreground-muted), 0.25)",
              width: carouselPage === idx ? "16px" : "5px",
            }}
            className="h-1 rounded-full transition-all duration-300 cursor-pointer"
          />
        ))}
      </div>
    </div>
  );
});

MetricCarousel.displayName = "MetricCarousel";
