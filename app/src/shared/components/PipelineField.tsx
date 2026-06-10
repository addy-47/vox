import React, { useEffect, useState } from "react";

type InteractionState = "Idle" | "Listening" | "UserSpeaking" | "Thinking" | "AssistantSpeaking" | "Interrupted";

interface PipelineFieldProps {
  state: InteractionState;
  volume?: number; // Optional volume parameter for AssistantSpeaking reactive expansion
}

export const PipelineField: React.FC<PipelineFieldProps> = ({ state, volume = 0 }) => {
  const [energy, setEnergy] = useState(0.12);
  const [glowColor, setGlowColor] = useState("rgba(0, 219, 233, "); // Default Accent RGB

  useEffect(() => {
    // Map states to field energy and color shifts
    switch (state) {
      case "Listening":
        setEnergy(0.28);
        setGlowColor("rgba(0, 219, 233, "); // Cyan
        break;
      case "UserSpeaking":
        setEnergy(0.35 + volume * 0.25); // Expand with speech volume
        setGlowColor("rgba(0, 219, 233, ");
        break;
      case "Thinking":
        setEnergy(0.24);
        setGlowColor("rgba(168, 85, 247, "); // Purple
        break;
      case "AssistantSpeaking":
        setEnergy(0.32 + volume * 0.3); // React to output audio
        setGlowColor("rgba(0, 219, 233, ");
        break;
      case "Interrupted":
        setEnergy(0.15);
        setGlowColor("rgba(239, 68, 68, "); // Red warning
        break;
      case "Idle":
      default:
        setEnergy(0.12);
        setGlowColor("rgba(0, 219, 233, ");
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
      {/* Sentient Field Ambient Heatmap */}
      <div
        className="absolute w-[80vw] h-[80vw] max-w-[800px] max-h-[800px] rounded-full blur-[120px] opacity-[var(--field-energy)] transition-all duration-700 ease-out"
        style={{
          left: "50%",
          top: "60%",
          transform: "translate(-50%, -50%)",
          background: `radial-gradient(circle, ${glowColor}0.4) 0%, ${glowColor}0.05) 50%, transparent 70%)`,
          mixBlendMode: "screen",
        }}
      />

      {/* Outer Field Ring Membrane */}
      <div
        className="absolute w-[70vw] h-[70vw] max-w-[700px] max-h-[700px] rounded-full border border-dashed transition-all duration-1000 ease-out"
        style={{
          left: "50%",
          top: "60%",
          transform: `translate(-50%, -50%) scale(${0.8 + energy * 0.4})`,
          borderColor: `${glowColor}${0.05 + energy * 0.1})`,
          opacity: state === "Idle" ? 0.2 : 0.6,
        }}
      />
    </div>
  );
};
