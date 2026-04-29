import React, { useState } from "react";
import { Brain, ChevronDown, Activity } from "lucide-react";

export const Settings: React.FC = () => {
  const [selectedModel, setSelectedModel] = useState("Neural-Omni 4.0 (Latest)");
  const [selectedVoice, setSelectedVoice] = useState("ETHER");
  const [alwaysListening, setAlwaysListening] = useState(true);
  const [overlayUI, setOverlayUI] = useState(false);

  const voices = ["ETHER", "SOLAS", "KRYPTOS", "LYRA"];

  return (
    <div
      className="h-full overflow-y-auto"
      style={{ minHeight: "calc(100vh - 56px)", scrollbarWidth: "none" }}
    >
      {/* Left sidebar panel + right content on desktop */}
      <div className="flex h-full">
        {/* Left mini nav */}
        <div
          className="hidden md:flex flex-col py-6"
          style={{
            width: 220,
            flexShrink: 0,
            borderRight: "1px solid rgba(255,255,255,0.06)",
          }}
        >
          <div className="px-5 mb-6">
            <p className="font-bold" style={{ fontSize: 14, color: "white" }}>
              VOX AI
            </p>
            <p
              className="font-mono uppercase tracking-widest"
              style={{ fontSize: 9, color: "rgba(255,255,255,0.3)" }}
            >
              LOCAL INTELLIGENCE
            </p>
          </div>

          <nav className="flex flex-col px-3 gap-1">
            {[
              { label: "Home", active: false },
              { label: "History", active: false },
              { label: "Settings", active: true },
            ].map((item) => (
              <button
                key={item.label}
                className="flex items-center gap-3 px-3 py-2.5 rounded-xl text-left transition-all"
                style={{
                  background: item.active ? "rgba(0,219,233,0.08)" : "transparent",
                  borderRight: item.active ? "2px solid #00dbe9" : "2px solid transparent",
                  color: item.active ? "white" : "rgba(255,255,255,0.4)",
                  fontSize: 13,
                }}
              >
                {item.label}
              </button>
            ))}
          </nav>
        </div>

        {/* Main settings content */}
        <div className="flex-1 px-8 md:px-12 py-10 flex flex-col" style={{ maxWidth: 900 }}>
          {/* Page header */}
          <div className="mb-10">
            <div className="flex items-center gap-2 mb-3">
              <div
                className="rounded-full"
                style={{ width: 6, height: 6, background: "#00dbe9" }}
              />
              <span
                className="font-mono uppercase tracking-widest"
                style={{ fontSize: 10, color: "#00dbe9" }}
              >
                SYSTEM PREFERENCES
              </span>
            </div>
            <h1
              className="font-bold leading-tight mb-3"
              style={{ fontSize: 42, fontFamily: "'Space Grotesk', sans-serif" }}
            >
              Interface{" "}
              <span style={{ color: "#00dbe9" }}>Intelligence</span>
            </h1>
            <p style={{ fontSize: 15, color: "rgba(255,255,255,0.4)", lineHeight: 1.6 }}>
              Configure your sentient companion's behavioral parameters and auditory profile.
            </p>
          </div>

          {/* Top row: Model Selection + Voice Profile */}
          <div className="grid md:grid-cols-2 gap-5 mb-5">
            {/* Model Selection */}
            <div
              className="rounded-2xl p-6 flex flex-col"
              style={{
                background: "rgba(15,22,28,0.95)",
                border: "1px solid rgba(255,255,255,0.07)",
              }}
            >
              <div className="flex items-center justify-between mb-2">
                <h2 className="font-bold" style={{ fontSize: 18, color: "white" }}>
                  Model Selection
                </h2>
                <Brain size={18} strokeWidth={1.5} style={{ color: "#00dbe9" }} />
              </div>
              <p style={{ fontSize: 13, color: "rgba(255,255,255,0.35)", marginBottom: 20 }}>
                Choose the cognitive architecture driving VOX's responses.
              </p>

              {/* Dropdown */}
              <div
                className="flex items-center justify-between px-4 py-3 rounded-xl mb-5 cursor-pointer"
                style={{
                  background: "rgba(255,255,255,0.04)",
                  border: "1px solid rgba(255,255,255,0.1)",
                }}
                onClick={() => setSelectedModel("Neural-Omni 4.0 (Latest)")}
              >
                <span style={{ fontSize: 14, color: "rgba(255,255,255,0.8)" }}>
                  {selectedModel}
                </span>
                <ChevronDown size={16} strokeWidth={1.5} style={{ color: "rgba(255,255,255,0.3)" }} />
              </div>

              {/* Stats */}
              <div className="grid grid-cols-3 gap-3">
                {[
                  { label: "LATENCY", value: "240ms", highlight: true },
                  { label: "CONTEXT", value: "128k", highlight: false },
                  { label: "STATUS", value: "● OPTIMIZED", highlight: true },
                ].map((stat) => (
                  <div
                    key={stat.label}
                    className="rounded-xl p-3"
                    style={{
                      background: "rgba(255,255,255,0.03)",
                      border: "1px solid rgba(255,255,255,0.06)",
                    }}
                  >
                    <p
                      className="font-mono uppercase tracking-widest mb-1"
                      style={{ fontSize: 9, color: "rgba(255,255,255,0.3)" }}
                    >
                      {stat.label}
                    </p>
                    <p
                      className="font-bold"
                      style={{
                        fontSize: 13,
                        color: stat.highlight ? "#00dbe9" : "rgba(255,255,255,0.7)",
                      }}
                    >
                      {stat.value}
                    </p>
                  </div>
                ))}
              </div>
            </div>

            {/* Voice Profile */}
            <div
              className="rounded-2xl p-6 flex flex-col"
              style={{
                background: "rgba(15,22,28,0.95)",
                border: "1px solid rgba(255,255,255,0.07)",
              }}
            >
              <h2 className="font-bold mb-1" style={{ fontSize: 18, color: "white" }}>
                Voice Profile
              </h2>
              <p style={{ fontSize: 13, color: "rgba(255,255,255,0.35)", marginBottom: 20 }}>
                Auditory personality and tonality.
              </p>

              {/* Voice chips */}
              <div className="flex flex-wrap gap-3 mb-6">
                {voices.map((v) => (
                  <button
                    key={v}
                    onClick={() => setSelectedVoice(v)}
                    className="px-5 py-2 rounded-full font-bold uppercase tracking-widest transition-all"
                    style={{
                      fontSize: 11,
                      background:
                        selectedVoice === v
                          ? "transparent"
                          : "rgba(255,255,255,0.04)",
                      border:
                        selectedVoice === v
                          ? "1.5px solid #00dbe9"
                          : "1.5px solid rgba(255,255,255,0.1)",
                      color: selectedVoice === v ? "#00dbe9" : "rgba(255,255,255,0.4)",
                    }}
                  >
                    {v}
                  </button>
                ))}
              </div>

              {/* Waveform preview */}
              <div
                className="flex-1 flex items-center justify-center rounded-xl"
                style={{
                  background: "rgba(255,255,255,0.02)",
                  border: "1px solid rgba(255,255,255,0.05)",
                  minHeight: 80,
                }}
              >
                <div className="flex items-center gap-px" style={{ height: 40 }}>
                  {[3, 6, 10, 16, 22, 28, 32, 28, 22, 16, 10, 6, 3].map((h, i) => (
                    <div
                      key={i}
                      className="rounded-full"
                      style={{ width: 3, height: h, background: "#00dbe9", opacity: 0.7 }}
                    />
                  ))}
                </div>
              </div>
            </div>
          </div>

          {/* Bottom row: toggles */}
          <div className="grid md:grid-cols-2 gap-5 mb-10">
            {/* Always Listening */}
            <div
              className="rounded-2xl p-6 flex items-center justify-between"
              style={{
                background: "rgba(15,22,28,0.95)",
                border: "1px solid rgba(255,255,255,0.07)",
              }}
            >
              <div>
                <h3 className="font-bold mb-1" style={{ fontSize: 16, color: "white" }}>
                  Always Listening
                </h3>
                <p style={{ fontSize: 12, color: "rgba(255,255,255,0.35)" }}>
                  VOX responds to wake words in real-time.
                </p>
              </div>
              <button
                onClick={() => setAlwaysListening(!alwaysListening)}
                className="relative flex-shrink-0 rounded-full transition-all"
                style={{
                  width: 52,
                  height: 28,
                  background: alwaysListening ? "#00dbe9" : "rgba(255,255,255,0.1)",
                  marginLeft: 16,
                }}
              >
                <div
                  className="absolute top-1 rounded-full transition-all"
                  style={{
                    width: 20,
                    height: 20,
                    background: "white",
                    left: alwaysListening ? 28 : 4,
                  }}
                />
              </button>
            </div>

            {/* Overlay UI */}
            <div
              className="rounded-2xl p-6 flex items-center justify-between"
              style={{
                background: "rgba(15,22,28,0.95)",
                border: "1px solid rgba(255,255,255,0.07)",
              }}
            >
              <div>
                <h3 className="font-bold mb-1" style={{ fontSize: 16, color: "white" }}>
                  Overlay UI
                </h3>
                <p style={{ fontSize: 12, color: "rgba(255,255,255,0.35)" }}>
                  Keep the HUD visible over other applications.
                </p>
              </div>
              <button
                onClick={() => setOverlayUI(!overlayUI)}
                className="relative flex-shrink-0 rounded-full transition-all"
                style={{
                  width: 52,
                  height: 28,
                  background: overlayUI ? "#00dbe9" : "rgba(255,255,255,0.1)",
                  marginLeft: 16,
                }}
              >
                <div
                  className="absolute top-1 rounded-full transition-all"
                  style={{
                    width: 20,
                    height: 20,
                    background: "white",
                    left: overlayUI ? 28 : 4,
                  }}
                />
              </button>
            </div>
          </div>

          {/* Footer actions */}
          <div className="flex items-center justify-between mt-auto pt-6" style={{ borderTop: "1px solid rgba(255,255,255,0.06)" }}>
            <button
              className="flex items-center gap-2 transition-all hover:opacity-70"
              style={{ fontSize: 12, color: "rgba(255,255,255,0.35)" }}
            >
              <Activity size={14} strokeWidth={1.5} />
              <span className="uppercase tracking-widest font-mono" style={{ fontSize: 10 }}>
                RESET TO DEFAULTS
              </span>
            </button>
            <div className="flex items-center gap-4">
              <button
                className="px-6 py-2.5 rounded-xl font-bold uppercase tracking-widest transition-all hover:bg-white/5"
                style={{ fontSize: 12, color: "rgba(255,255,255,0.4)" }}
              >
                CANCEL
              </button>
              <button
                className="px-8 py-2.5 rounded-full font-bold uppercase tracking-widest transition-all hover:opacity-90"
                style={{
                  fontSize: 12,
                  background: "#00dbe9",
                  color: "#050505",
                }}
              >
                SAVE CHANGES
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
