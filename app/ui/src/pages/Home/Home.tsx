import React, { useState } from "react";
import { VoxOrb } from "../../shared/ui/AdvancedOrb";
import { Mic, Activity, Shield } from "lucide-react";

type OrbState = "IDLE" | "LISTENING" | "SPEAKING";

export const Home: React.FC = () => {
  const [orbState, setOrbState] = useState<OrbState>("IDLE");

  const statusLabel: Record<OrbState, string> = {
    IDLE: "NEURAL_SYNC: STABLE",
    LISTENING: "LISTENING...",
    SPEAKING: "PROCESSING_CORE",
  };

  return (
    <div
      className="flex h-full"
      style={{ minHeight: "calc(100vh - 56px)", position: "relative" }}
    >
      {/* ===== LEFT: ORB PANEL ===== */}
      <div
        className="flex-1 flex flex-col items-center justify-center relative"
        style={{ paddingBottom: 80 }}
      >
        {/* Status badge */}
        <div
          className="absolute flex items-center gap-2 px-3 py-1.5 rounded-full"
          style={{
            top: 32,
            left: "50%",
            transform: "translateX(-50%)",
            background: "rgba(0,219,233,0.08)",
            border: "1px solid rgba(0,219,233,0.2)",
          }}
        >
          <div
            className="rounded-full"
            style={{
              width: 6,
              height: 6,
              background: orbState === "IDLE" ? "#00dbe9" : "#22c55e",
              boxShadow: `0 0 6px ${orbState === "IDLE" ? "#00dbe9" : "#22c55e"}`,
            }}
          />
          <span
            className="font-mono uppercase tracking-widest"
            style={{ fontSize: 10, color: "rgba(255,255,255,0.6)" }}
          >
            {statusLabel[orbState]}
          </span>
        </div>

        {/* Orb container */}
        <div
          className="relative flex items-center justify-center"
          style={{ width: 400, height: 400 }}
        >
          {/* Outer dark ring */}
          <div
            className="absolute inset-0 rounded-full"
            style={{
              background:
                "radial-gradient(circle, rgba(10,20,25,0.95) 60%, rgba(0,40,50,0.4) 100%)",
              border: "1px solid rgba(0,219,233,0.1)",
              boxShadow: "0 0 80px rgba(0,219,233,0.06) inset, 0 0 40px rgba(0,0,0,0.8)",
            }}
          />
          {/* Orb */}
          <div className="absolute inset-0 rounded-full overflow-hidden flex items-center justify-center">
            <VoxOrb />
          </div>
        </div>

        {/* Processing indicator */}
        {orbState !== "IDLE" && (
          <div
            className="absolute flex items-center gap-2 px-3 py-1.5 rounded-lg"
            style={{
              bottom: 100,
              background: "rgba(10,20,25,0.85)",
              border: "1px solid rgba(255,255,255,0.08)",
            }}
          >
            <span
              className="font-mono uppercase tracking-widest"
              style={{ fontSize: 9, color: "rgba(255,255,255,0.35)" }}
            >
              PROCESSING_CORE
            </span>
            <span className="font-mono" style={{ fontSize: 11, color: "#00dbe9" }}>
              0.0042ms
            </span>
          </div>
        )}

        {/* Bottom control bar */}
        <div
          className="absolute bottom-8 flex items-center justify-around w-full max-w-xs"
          style={{ paddingBottom: 16 }}
        >
          <button
            className="flex flex-col items-center gap-1.5 transition-all"
            style={{ color: orbState === "LISTENING" ? "#00dbe9" : "rgba(255,255,255,0.3)" }}
            onClick={() => setOrbState(orbState === "LISTENING" ? "IDLE" : "LISTENING")}
          >
            <Mic size={20} strokeWidth={1.5} />
            <span
              className="font-bold uppercase tracking-widest"
              style={{ fontSize: 9 }}
            >
              LISTEN
            </span>
          </button>

          <button
            className="flex flex-col items-center gap-1.5 transition-all"
            style={{
              color: orbState === "SPEAKING" ? "#00dbe9" : "rgba(255,255,255,0.3)",
            }}
            onClick={() => setOrbState(orbState === "SPEAKING" ? "IDLE" : "SPEAKING")}
          >
            <div
              className="flex items-center justify-center rounded-2xl"
              style={{
                width: 52,
                height: 52,
                background:
                  orbState === "SPEAKING"
                    ? "rgba(0,219,233,0.2)"
                    : "rgba(0,219,233,0.08)",
                border: `1.5px solid ${orbState === "SPEAKING" ? "#00dbe9" : "rgba(0,219,233,0.2)"}`,
              }}
            >
              <Activity size={22} strokeWidth={1.5} color="#00dbe9" />
            </div>
            <span className="font-bold uppercase tracking-widest" style={{ fontSize: 9, color: "#00dbe9" }}>
              ANALYZE
            </span>
          </button>

          <button
            className="flex flex-col items-center gap-1.5 transition-all"
            style={{ color: "rgba(255,255,255,0.3)" }}
          >
            <Shield size={20} strokeWidth={1.5} />
            <span className="font-bold uppercase tracking-widest" style={{ fontSize: 9 }}>
              COMMAND
            </span>
          </button>
        </div>
      </div>

      {/* ===== RIGHT: INFO PANELS ===== */}
      <div
        className="hidden lg:flex flex-col gap-4 py-10 pr-8"
        style={{ width: 340, flexShrink: 0 }}
      >
        {/* Voice Recognition Card */}
        <div
          className="rounded-2xl p-5"
          style={{
            background: "rgba(15,20,22,0.9)",
            border: "1px solid rgba(255,255,255,0.07)",
          }}
        >
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center gap-2">
              <Mic size={14} strokeWidth={1.5} color="#00dbe9" />
              <span
                className="font-bold uppercase tracking-widest"
                style={{ fontSize: 10, color: "#00dbe9" }}
              >
                VOICE RECOGNITION
              </span>
            </div>
            <span
              className="font-mono uppercase tracking-widest"
              style={{ fontSize: 9, color: "rgba(255,255,255,0.3)" }}
            >
              SAMPLING...
            </span>
          </div>
          <p
            className="italic leading-relaxed"
            style={{
              fontSize: 18,
              color: "rgba(255,255,255,0.85)",
              fontFamily: "'Inter', sans-serif",
              fontStyle: "italic",
            }}
          >
            "Analyze architectural patterns for the next iteration..."
          </p>
          <div
            className="mt-4"
            style={{ height: 1, background: "#00dbe9", width: "60%" }}
          />
        </div>

        {/* System Architecture Card */}
        <div
          className="rounded-2xl p-5"
          style={{
            background: "rgba(15,20,22,0.9)",
            border: "1px solid rgba(255,255,255,0.07)",
          }}
        >
          <div className="flex items-center gap-2 mb-5">
            <Activity size={14} strokeWidth={1.5} color="#00dbe9" />
            <span
              className="font-bold uppercase tracking-widest"
              style={{ fontSize: 10, color: "rgba(255,255,255,0.6)" }}
            >
              SYSTEM ARCHITECTURE
            </span>
          </div>
          {[
            { label: "Active Model", value: "VOX-CORE-8", highlight: true },
            { label: "Cluster Link", value: "XENON-04", highlight: false },
            { label: "Encrypted Feed", value: "● SECURED", highlight: true },
          ].map((row) => (
            <div
              key={row.label}
              className="flex items-center justify-between py-3"
              style={{ borderBottom: "1px solid rgba(255,255,255,0.05)" }}
            >
              <span style={{ fontSize: 13, color: "rgba(255,255,255,0.5)" }}>{row.label}</span>
              <span
                className="font-mono font-bold"
                style={{
                  fontSize: 12,
                  color: row.highlight ? "#00dbe9" : "rgba(255,255,255,0.7)",
                  letterSpacing: "0.05em",
                }}
              >
                {row.value}
              </span>
            </div>
          ))}
        </div>

        {/* Neural Activity Card */}
        <div
          className="rounded-2xl p-5"
          style={{
            background: "rgba(15,20,22,0.9)",
            border: "1px solid rgba(255,255,255,0.07)",
          }}
        >
          <div className="flex items-center gap-2 mb-4">
            <Shield size={14} strokeWidth={1.5} color="#00dbe9" />
            <span
              className="font-bold uppercase tracking-widest"
              style={{ fontSize: 10, color: "rgba(255,255,255,0.6)" }}
            >
              NEURAL ACTIVITY
            </span>
          </div>
          {[
            {
              label: "Contextual map 'Project_Nova' indexed",
              time: "045 AGO",
              active: true,
            },
            {
              label: "Background telemetry synced",
              time: "12M AGO",
              active: false,
            },
          ].map((item, i) => (
            <div key={i} className="flex gap-3 py-2.5">
              <div
                className="mt-1.5 rounded-full flex-shrink-0"
                style={{
                  width: 6,
                  height: 6,
                  background: item.active ? "#00dbe9" : "rgba(255,255,255,0.15)",
                }}
              />
              <div>
                <p style={{ fontSize: 12, color: "rgba(255,255,255,0.65)" }}>{item.label}</p>
                <p
                  className="font-mono uppercase"
                  style={{ fontSize: 9, color: "rgba(255,255,255,0.25)", marginTop: 2 }}
                >
                  {item.time}
                </p>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Audio waveform strip at bottom */}
      <div
        className="absolute bottom-0 left-0 right-0 flex items-end justify-center gap-px opacity-30"
        style={{ height: 40, overflow: "hidden" }}
      >
        {Array.from({ length: 60 }).map((_, i) => (
          <div
            key={i}
            className="flex-shrink-0 rounded-t-sm"
            style={{
              width: 2,
              height: Math.random() * 24 + 4,
              background: "#00dbe9",
            }}
          />
        ))}
      </div>
    </div>
  );
};
