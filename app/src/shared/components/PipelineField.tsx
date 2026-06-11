import React, { useEffect, useState } from "react";

type InteractionState = "Idle" | "Listening" | "UserSpeaking" | "Thinking" | "AssistantSpeaking" | "Interrupted";

interface PipelineFieldProps {
  state: InteractionState;
  volume?: number; // Optional volume parameter for AssistantSpeaking reactive expansion
}

export const PipelineField: React.FC<PipelineFieldProps> = ({ state, volume = 0 }) => {
  const [energy, setEnergy] = useState(0.12);

  useEffect(() => {
    // Map states to field energy — all use accent color via CSS variable,
    // visual distinction comes from energy level, opacity, and scale.
    switch (state) {
      case "Listening":
        setEnergy(0.28);
        break;
      case "UserSpeaking":
        setEnergy(0.35 + volume * 0.25); // Expand with speech volume
        break;
      case "Thinking":
        setEnergy(0.24);
        break;
      case "AssistantSpeaking":
        setEnergy(0.32 + volume * 0.3); // React to output audio
        break;
      case "Interrupted":
        setEnergy(0.15);
        break;
      case "Idle":
      default:
        setEnergy(0.12);
        break;
    }
  }, [state, volume]);

  return (
    <div
      className="absolute inset-0 pointer-events-none transition-all duration-700 ease-out overflow-hidden"
      style={{
        zIndex: 1,
        // Set CSS custom property dynamically
        ["--field-energy" as any]: energy,
      }}
    >
      {/* Sentient Field Ambient Heatmap — always uses accent color via CSS var */}
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
          transform: `translate(-50%, -50%) scale(${0.8 + energy * 0.4})`,
          borderColor: `rgba(var(--accent), ${0.05 + energy * 0.1})`,
          opacity: state === "Idle" ? 0.2 : 0.6,
        }}
      />
    </div>
  );
};
