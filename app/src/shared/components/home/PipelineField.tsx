import React, { useEffect, useRef } from "react";
import { type InteractionState } from "@/services/eventsService";

interface PipelineFieldProps {
  state: InteractionState;
  volume?: number;
}

export const PipelineField = React.memo(({ state, volume = 0 }: PipelineFieldProps) => {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!containerRef.current) return;
    let targetEnergy = 0.12;

    switch (state) {
      case "Listening":
        targetEnergy = 0.28;
        break;
      case "UserSpeaking":
        targetEnergy = 0.35 + volume * 0.25;
        break;
      case "Thinking":
        targetEnergy = 0.24;
        break;
      case "AssistantSpeaking":
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
      className="absolute inset-0 pointer-events-none transition-all duration-700 ease-out overflow-hidden"
      style={{
        zIndex: 1,
        ["--field-energy" as any]: "0.12",
        ["--field-scale" as any]: "0.85",
        ["--border-alpha" as any]: "0.06",
      }}
    >
      {/* Sentient Field Ambient Heatmap */}
      <div
        className="absolute w-[80vw] h-[80vw] max-w-[800px] max-h-[800px] rounded-full blur-[120px] opacity-[var(--field-energy)] transition-all duration-700 ease-out"
        style={{
          left: "50%",
          top: "55%",
          transform: "translate(-50%, -50%)",
          background: `radial-gradient(circle, rgba(var(--accent), 0.4) 0%, rgba(var(--accent), 0.05) 50%, transparent 70%)`,
          mixBlendMode: "screen",
        }}
      />

      {/* Outer Field Ring Membrane */}
      <div
        className="absolute w-[70vw] h-[70vw] max-w-[700px] max-h-[700px] rounded-full border border-dashed transition-all duration-1000 ease-out"
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
