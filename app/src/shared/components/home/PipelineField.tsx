import React, { useEffect, useRef } from "react";
import { type InteractionState } from "@/services/eventsService";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";

interface PipelineFieldProps {
  state: InteractionState;
  volume?: number;
}

export const PipelineField = React.memo(({ state, volume = 0 }: PipelineFieldProps) => {
  useMemoryTrace("PipelineField");
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    let targetEnergy: number;

    switch (state) {
      case "Ready":
        targetEnergy = 0.16;
        break;
      case "Listening":
        targetEnergy = 0.35 + volume * 0.25;
        break;
      case "Thinking":
        targetEnergy = 0.24;
        break;
      case "Speaking":
        targetEnergy = 0.32 + volume * 0.3;
        break;
      case "Paused":
        targetEnergy = 0.08;
        break;
      case "Error":
        targetEnergy = 0.05;
        break;
      case "Idle":
      default:
        targetEnergy = 0.12;
        break;
    }

    containerRef.current.style.setProperty("--field-energy", targetEnergy.toString());
    containerRef.current.style.setProperty("--field-scale", (0.8 + targetEnergy * 0.4).toString());
    containerRef.current.style.setProperty("--border-alpha", (0.05 + targetEnergy * 0.1).toString());
  }, [state, volume]);

  return (
    <div
      ref={containerRef}
      className="absolute inset-0 pointer-events-none overflow-hidden"
      style={{
        zIndex: 1,
        ["--field-energy"]: "0.12",
        ["--field-scale"]: "0.85",
        ["--border-alpha"]: "0.06",
      } as React.CSSProperties}
    >
      {/* Sentient Field Ambient Heatmap — zero filter blur, pure gradient falloff */}
      <div
        className="absolute w-[85vw] h-[85vw] max-w-[860px] max-h-[860px] rounded-full opacity-[var(--field-energy)] transition-opacity duration-700 ease-out transform-gpu"
        style={{
          left: "50%",
          top: "55%",
          transform: "translate(-50%, -50%)",
          background:
            "radial-gradient(circle, rgba(var(--accent), 0.28) 0%, rgba(var(--accent), 0.10) 32%, rgba(var(--accent), 0.03) 55%, transparent 72%)",
        }}
      />

      {/* Outer Field Ring Membrane */}
      <div
        className="absolute w-[70vw] h-[70vw] max-w-[700px] max-h-[700px] rounded-full border border-dashed transition-[transform,opacity] duration-1000 ease-out"
        style={{
          left: "50%",
          top: "55%",
          transform: `translate(-50%, -50%) scale(var(--field-scale))`,
          borderColor: `rgba(var(--accent), var(--border-alpha))`,
          opacity: state === "Idle" ? 0.2 : 0.6,
        }}
      />
    </div>
  );
});

PipelineField.displayName = "PipelineField";
