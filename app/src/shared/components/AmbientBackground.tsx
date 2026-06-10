import React from "react";

interface AmbientBackgroundProps {
  mood?: "calm" | "active" | "thinking";
}

export const AmbientBackground: React.FC<AmbientBackgroundProps> = ({ mood: _mood = "calm" }) => {
  return (
    <div
      className="fixed inset-0 pointer-events-none"
      style={{ zIndex: 0, willChange: "transform", transform: "translateZ(0)" }}
      aria-hidden="true"
    >
      <style>{`
        @keyframes ambient-blob-1 {
          0%, 100% { border-radius: 60% 40% 30% 70% / 60% 30% 70% 40%; transform: translate(0, 0) scale(1); }
          25% { border-radius: 30% 60% 70% 40% / 50% 60% 30% 60%; transform: translate(30px, -20px) scale(1.05); }
          50% { border-radius: 50% 60% 30% 50% / 40% 60% 50% 60%; transform: translate(-20px, 20px) scale(0.95); }
          75% { border-radius: 40% 50% 60% 30% / 60% 40% 70% 30%; transform: translate(15px, -10px) scale(1.02); }
        }
        @keyframes ambient-blob-2 {
          0%, 100% { border-radius: 40% 60% 50% 50% / 40% 50% 60% 50%; transform: translate(0, 0) scale(1); }
          33% { border-radius: 60% 40% 60% 40% / 50% 60% 40% 50%; transform: translate(-25px, 15px) scale(1.04); }
          66% { border-radius: 50% 50% 40% 60% / 60% 40% 50% 50%; transform: translate(20px, -25px) scale(0.96); }
        }
        @keyframes ambient-blob-3 {
          0%, 100% { border-radius: 50% 60% 40% 50% / 50% 40% 60% 50%; opacity: 0.08; transform: scale(1); }
          50% { border-radius: 40% 50% 60% 40% / 60% 50% 40% 50%; opacity: 0.15; transform: scale(1.08); }
        }
        @media (prefers-reduced-motion: reduce) {
          .ambient-blob { animation: none !important; }
        }
        [data-theme='light'] .ambient-base-dark { opacity: 0; }
        [data-theme='light'] .ambient-base-light { opacity: 1; }
      `}</style>

      {/* Base dark gradient */}
      <div
        className="ambient-base-dark absolute inset-0"
        style={{
          background: "radial-gradient(ellipse at 50% 50%, #0c0d21 0%, #06060c 60%, #020204 100%)",
          transition: "opacity 0.3s ease",
        }}
      />

      {/* Base light gradient (hidden until [data-theme='light']) */}
      <div
        className="ambient-base-light absolute inset-0"
        style={{
          background: "radial-gradient(ellipse at 50% 50%, #f0f3fd 0%, #e4eaf8 60%, #f4f6fc 100%)",
          opacity: 0,
          transition: "opacity 0.3s ease",
        }}
      />

      {/* Blob 1: cool cyan, top-left, 60s slow float + morph */}
      <div
        className="ambient-blob absolute"
        style={{
          top: "10%",
          left: "5%",
          width: "50vw",
          height: "50vw",
          background: "radial-gradient(circle, rgba(0, 219, 233, 0.4) 0%, transparent 70%)",
          filter: "blur(100px)",
          opacity: 0.18,
          animation: "ambient-blob-1 60s ease-in-out infinite",
          willChange: "transform, border-radius",
        }}
      />

      {/* Blob 2: warm purple, bottom-right, 90s slower drift */}
      <div
        className="ambient-blob absolute"
        style={{
          bottom: "5%",
          right: "5%",
          width: "45vw",
          height: "45vw",
          background: "radial-gradient(circle, rgba(216, 186, 255, 0.35) 0%, transparent 70%)",
          filter: "blur(120px)",
          opacity: 0.16,
          animation: "ambient-blob-2 90s ease-in-out infinite",
          willChange: "transform, border-radius",
        }}
      />

      {/* Blob 3: accent cyan, center-right, 45s slow pulse */}
      <div
        className="ambient-blob absolute"
        style={{
          top: "35%",
          right: "10%",
          width: "40vw",
          height: "40vw",
          background: "radial-gradient(circle, rgba(0, 240, 255, 0.3) 0%, transparent 70%)",
          filter: "blur(80px)",
          opacity: 0.12,
          animation: "ambient-blob-3 45s ease-in-out infinite",
          willChange: "transform, border-radius, opacity",
        }}
      />

      {/* Light mode blob overrides */}
      <style>{`
        [data-theme='light'] .ambient-blob { opacity: 0.14 !important; }
      `}</style>

      {/* Subtle noise grain overlay via SVG data URI */}
      <div
        className="absolute inset-0"
        style={{
          backgroundImage: `url("data:image/svg+xml,%3Csvg viewBox='0 0 250 250' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.8' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)' opacity='0.5'/%3E%3C/svg%3E")`,
          backgroundSize: "250px 250px",
          opacity: 0.15,
          mixBlendMode: "overlay",
        }}
        aria-hidden="true"
      />
    </div>
  );
};
