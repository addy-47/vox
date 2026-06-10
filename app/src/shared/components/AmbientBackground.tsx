import React from "react";

interface AmbientBackgroundProps {
  mood?: "calm" | "active" | "thinking";
}

export const AmbientBackground: React.FC<AmbientBackgroundProps> = ({ mood = "calm" }) => {
  // Map mood to opacity and wave animation speed/scaling parameters
  const isLight = typeof document !== 'undefined' && document.documentElement.getAttribute('data-theme') === 'light';
  
  // Wave configurations based on mood
  const moodConfig = {
    calm: {
      energy: 0.12,
      opacity: 0.08,
      speedMultiplier: 1.0,
      scaleY: 1.0,
    },
    active: {
      energy: 0.35,
      opacity: 0.18,
      speedMultiplier: 2.2,
      scaleY: 1.8,
    },
    thinking: {
      energy: 0.25,
      opacity: 0.15,
      speedMultiplier: 1.5,
      scaleY: 1.4,
    }
  }[mood] || { energy: 0.12, opacity: 0.08, speedMultiplier: 1.0, scaleY: 1.0 };

  return (
    <div
      className="fixed inset-0 pointer-events-none overflow-hidden select-none"
      style={{ zIndex: 0, willChange: "transform", transform: "translateZ(0)" }}
      aria-hidden="true"
    >
      <style>{`
        @keyframes wave-drift-1 {
          0% { transform: translate3d(0, 0, 0) scaleY(${moodConfig.scaleY}); }
          50% { transform: translate3d(-40px, 15px, 0) scaleY(${moodConfig.scaleY * 1.15}); }
          100% { transform: translate3d(0, 0, 0) scaleY(${moodConfig.scaleY}); }
        }
        @keyframes wave-drift-2 {
          0% { transform: translate3d(0, 0, 0) scaleY(${moodConfig.scaleY * 0.9}); }
          50% { transform: translate3d(30px, -20px, 0) scaleY(${moodConfig.scaleY * 1.2}); }
          100% { transform: translate3d(0, 0, 0) scaleY(${moodConfig.scaleY * 0.9}); }
        }
        @keyframes wave-drift-3 {
          0% { transform: translate3d(0, 0, 0) scaleY(${moodConfig.scaleY * 1.1}); }
          50% { transform: translate3d(-20px, -10px, 0) scaleY(${moodConfig.scaleY * 0.85}); }
          100% { transform: translate3d(0, 0, 0) scaleY(${moodConfig.scaleY * 1.1}); }
        }
        @media (prefers-reduced-motion: reduce) {
          .sentient-wave { animation: none !important; transform: scaleY(1) !important; }
        }
        [data-theme='light'] .ambient-base-dark { opacity: 0; }
        [data-theme='light'] .ambient-base-light { opacity: 1; }
        [data-theme='light'] .sentient-wave { stroke: rgba(var(--signal), 0.18) !important; }
      `}</style>

      {/* Base dark gradient */}
      <div
        className="ambient-base-dark absolute inset-0"
        style={{
          background: "radial-gradient(ellipse at 50% 60%, #080915 0%, #030307 70%, #010103 100%)",
          transition: "opacity 0.5s cubic-bezier(0.16, 1, 0.3, 1)",
        }}
      />

      {/* Base light gradient */}
      <div
        className="ambient-base-light absolute inset-0"
        style={{
          background: "radial-gradient(ellipse at 50% 60%, #f3f6fe 0%, #e8edf9 70%, #f4f6fc 100%)",
          opacity: 0,
          transition: "opacity 0.5s cubic-bezier(0.16, 1, 0.3, 1)",
        }}
      />

      {/* Sentient Field Wave Topology (SVG Overlay) */}
      <div 
        className="absolute inset-0 flex items-center justify-center"
        style={{ opacity: moodConfig.opacity, transition: "opacity 0.5s ease" }}
      >
        <svg 
          width="120%" 
          height="120%" 
          viewBox="0 0 1000 600" 
          preserveAspectRatio="none"
          className="absolute min-w-full min-h-full opacity-60"
        >
          {/* Wave 1: Cyan/Accent topology line */}
          <path
            className="sentient-wave"
            d="M -100 300 C 150 150, 350 450, 500 300 C 650 150, 850 450, 1100 300"
            fill="none"
            stroke="rgb(var(--accent))"
            strokeWidth="1.5"
            strokeOpacity="0.4"
            style={{
              transformOrigin: "center",
              animation: `wave-drift-1 ${45 / moodConfig.speedMultiplier}s ease-in-out infinite`,
            }}
          />
          {/* Wave 2: Purple/Deep offset topology line */}
          <path
            className="sentient-wave"
            d="M -100 280 C 200 400, 300 200, 500 320 C 700 440, 800 180, 1100 280"
            fill="none"
            stroke={mood === 'thinking' ? "rgb(var(--accent))" : "rgb(168, 85, 247)"}
            strokeWidth="1"
            strokeOpacity="0.3"
            style={{
              transformOrigin: "center",
              animation: `wave-drift-2 ${60 / moodConfig.speedMultiplier}s ease-in-out infinite`,
            }}
          />
          {/* Wave 3: Secondary highlight line */}
          <path
            className="sentient-wave"
            d="M -100 320 C 100 250, 400 350, 500 280 C 600 210, 900 380, 1100 320"
            fill="none"
            stroke="rgb(var(--accent))"
            strokeWidth="0.8"
            strokeOpacity="0.25"
            style={{
              transformOrigin: "center",
              animation: `wave-drift-3 ${35 / moodConfig.speedMultiplier}s ease-in-out infinite`,
            }}
          />
        </svg>
      </div>

      {/* Reactive Glow Core */}
      <div
        className="absolute left-1/2 top-1/2 -translate-x-1/2 -translate-y-1/2 rounded-full pointer-events-none"
        style={{
          width: "70vw",
          height: "70vh",
          background: `radial-gradient(circle, rgba(var(--accent), ${moodConfig.energy * 0.15}) 0%, transparent 70%)`,
          mixBlendMode: "screen",
          opacity: 0.8,
          transition: "background 0.5s ease, opacity 0.5s ease",
        }}
      />

      {/* Subtle Noise Grain Overlay */}
      <div
        className="absolute inset-0"
        style={{
          backgroundImage: `url("data:image/svg+xml,%3Csvg viewBox='0 0 250 250' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.8' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.5'/%3E%3C/svg%3E")`,
          backgroundSize: "250px 250px",
          opacity: isLight ? 0.08 : 0.12,
          mixBlendMode: isLight ? "multiply" : "overlay",
        }}
        aria-hidden="true"
      />
    </div>
  );
};
