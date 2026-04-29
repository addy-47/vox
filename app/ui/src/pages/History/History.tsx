import React, { useState } from "react";
import { MessageSquare, Mic, Send, Activity } from "lucide-react";

interface Message {
  id: number;
  role: "ai" | "user";
  content: string;
  time: string;
}

const MOCK_MESSAGES: Message[] = [
  {
    id: 1,
    role: "ai",
    content: "Initializing local intelligence modules... I am VOX. How can I assist your workflow today?",
    time: "JUST NOW",
  },
  {
    id: 2,
    role: "user",
    content: "Compare the architectural patterns of micro-services vs monoliths in high-frequency trading environments.",
    time: "12:42 PM",
  },
  {
    id: 3,
    role: "ai",
    content: "In high-frequency trading (HFT), the choice between microservices and monoliths is dictated by nanosecond latency requirements.",
    time: "12:43 PM",
  },
  {
    id: 4,
    role: "user",
    content: "Visualize the latency trade-off.",
    time: "12:45 PM",
  },
];

const CARD_ITEMS = [
  {
    title: "Monolithic Pattern",
    desc: "Shared memory space minimizes inter-process communication (IPC) overhead. Preferred for the core execution engine.",
  },
  {
    title: "Microservices Pattern",
    desc: "Used for non-critical paths like risk reporting, market data ingestion, and post-trade analytics.",
  },
];

export const History: React.FC = () => {
  const [input, setInput] = useState("");

  return (
    <div
      className="flex h-full"
      style={{ minHeight: "calc(100vh - 56px)" }}
    >
      {/* ===== LEFT: Nav/Sessions ===== */}
      <div
        className="hidden md:flex flex-col py-6"
        style={{
          width: 220,
          flexShrink: 0,
          borderRight: "1px solid rgba(255,255,255,0.06)",
        }}
      >
        <div className="px-5 mb-6">
          <div className="flex items-center gap-2 mb-1">
            <div
              className="rounded-full"
              style={{ width: 8, height: 8, background: "#00dbe9", boxShadow: "0 0 6px #00dbe9" }}
            />
            <span className="font-bold" style={{ fontSize: 14, color: "white" }}>
              VOX AI
            </span>
          </div>
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
            { label: "History", active: true },
            { label: "Settings", active: false },
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
              <MessageSquare size={14} strokeWidth={1.5} />
              {item.label}
            </button>
          ))}
        </nav>

        {/* User profile at bottom */}
        <div
          className="mt-auto mx-3 p-3 rounded-xl flex items-center gap-3"
          style={{
            background: "rgba(255,255,255,0.04)",
            border: "1px solid rgba(255,255,255,0.07)",
          }}
        >
          <div
            className="rounded-full flex items-center justify-center flex-shrink-0"
            style={{
              width: 32,
              height: 32,
              background: "rgba(0,219,233,0.15)",
              border: "1px solid rgba(0,219,233,0.3)",
            }}
          >
            <span style={{ fontSize: 12, color: "#00dbe9" }}>A</span>
          </div>
          <div>
            <p style={{ fontSize: 12, color: "rgba(255,255,255,0.8)" }}>Admin Account</p>
            <p
              className="font-mono uppercase"
              style={{ fontSize: 9, color: "#00dbe9" }}
            >
              Pro Node Active
            </p>
          </div>
        </div>
      </div>

      {/* ===== RIGHT: Chat Panel ===== */}
      <div className="flex-1 flex flex-col">
        {/* Top bar */}
        <div
          className="flex items-center justify-end px-6 py-3"
          style={{ borderBottom: "1px solid rgba(255,255,255,0.05)" }}
        >
          <div className="flex items-center gap-3">
            <Activity size={18} strokeWidth={1.5} style={{ color: "#00dbe9" }} />
            <div
              className="rounded-full overflow-hidden flex items-center justify-center"
              style={{
                width: 32,
                height: 32,
                background: "rgba(0,219,233,0.15)",
                border: "1px solid rgba(0,219,233,0.3)",
              }}
            >
              <span style={{ fontSize: 12, color: "#00dbe9" }}>A</span>
            </div>
          </div>
        </div>

        {/* Message thread */}
        <div
          className="flex-1 overflow-y-auto px-8 py-6 flex flex-col gap-5"
          style={{ scrollbarWidth: "none" }}
        >
          {MOCK_MESSAGES.map((msg) => (
            <div key={msg.id}>
              {msg.role === "ai" ? (
                <div style={{ maxWidth: 600 }}>
                  <div
                    className="rounded-2xl p-5"
                    style={{
                      background: "rgba(15,25,28,0.85)",
                      border: "1px solid rgba(255,255,255,0.07)",
                      fontSize: 14,
                      color: "rgba(255,255,255,0.8)",
                      lineHeight: 1.65,
                    }}
                  >
                    {msg.content}

                    {/* Show comparison cards after 3rd message */}
                    {msg.id === 3 && (
                      <div className="flex gap-3 mt-4">
                        {CARD_ITEMS.map((card) => (
                          <div
                            key={card.title}
                            className="flex-1 p-4 rounded-xl"
                            style={{
                              background: "rgba(0,219,233,0.05)",
                              border: "1px solid rgba(0,219,233,0.15)",
                            }}
                          >
                            <p
                              className="font-bold mb-2"
                              style={{ fontSize: 12, color: "#00dbe9" }}
                            >
                              {card.title}
                            </p>
                            <p style={{ fontSize: 11, color: "rgba(255,255,255,0.45)", lineHeight: 1.6 }}>
                              {card.desc}
                            </p>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                  <p
                    className="mt-1 px-1 font-mono uppercase tracking-widest"
                    style={{ fontSize: 9, color: "rgba(255,255,255,0.2)" }}
                  >
                    VOX AI · {msg.time}
                  </p>
                </div>
              ) : (
                <div className="flex justify-end">
                  <div style={{ maxWidth: 520 }}>
                    <div
                      className="rounded-2xl px-5 py-4"
                      style={{
                        background: "rgba(0,50,60,0.7)",
                        border: "1px solid rgba(0,219,233,0.15)",
                        fontSize: 14,
                        color: "rgba(255,255,255,0.85)",
                        lineHeight: 1.6,
                      }}
                    >
                      {msg.content}
                    </div>
                    <p
                      className="mt-1 px-1 font-mono uppercase tracking-widest text-right"
                      style={{ fontSize: 9, color: "rgba(255,255,255,0.2)" }}
                    >
                      YOU · {msg.time}
                    </p>
                  </div>
                </div>
              )}
            </div>
          ))}

          {/* Chart placeholder for last message */}
          <div style={{ maxWidth: 600 }}>
            <div
              className="rounded-2xl p-4"
              style={{
                background: "rgba(15,25,28,0.85)",
                border: "1px solid rgba(255,255,255,0.07)",
                height: 160,
                display: "flex",
                alignItems: "center",
                justifyContent: "center",
              }}
            >
              <div className="flex items-end gap-px" style={{ height: 100, width: "100%" }}>
                {Array.from({ length: 48 }).map((_, i) => {
                  const h = Math.sin(i * 0.4) * 30 + Math.random() * 40 + 10;
                  return (
                    <div
                      key={i}
                      className="flex-1 rounded-t-sm"
                      style={{
                        height: h,
                        background:
                          i > 30
                            ? `rgba(0,219,233,${0.3 + (i - 30) * 0.025})`
                            : "rgba(0,80,100,0.3)",
                      }}
                    />
                  );
                })}
              </div>
            </div>
          </div>
        </div>

        {/* Input bar */}
        <div
          className="px-6 py-4"
          style={{ borderTop: "1px solid rgba(255,255,255,0.05)" }}
        >
          <div
            className="flex items-center gap-3 rounded-2xl px-5 py-3"
            style={{
              background: "rgba(15,20,22,0.9)",
              border: "1px solid rgba(255,255,255,0.1)",
            }}
          >
            <div
              className="rounded-full flex-shrink-0"
              style={{ width: 8, height: 8, background: "#00dbe9" }}
            />
            <input
              type="text"
              placeholder="ASK VOX..."
              value={input}
              onChange={(e) => setInput(e.target.value)}
              className="flex-1 bg-transparent outline-none"
              style={{
                fontSize: 13,
                color: "rgba(255,255,255,0.7)",
                fontFamily: "'Space Grotesk', sans-serif",
              }}
            />
            <button
              className="flex items-center justify-center rounded-xl transition-all hover:bg-[#00dbe9]/20"
              style={{
                width: 34,
                height: 34,
                background: "rgba(0,219,233,0.1)",
                border: "1px solid rgba(0,219,233,0.25)",
                color: "#00dbe9",
              }}
            >
              <Mic size={15} strokeWidth={1.5} />
            </button>
            <button
              className="flex items-center justify-center rounded-xl transition-all"
              style={{
                width: 34,
                height: 34,
                background: "#00dbe9",
                color: "#050505",
              }}
            >
              <Send size={14} strokeWidth={2} />
            </button>
          </div>

          {/* Bottom tabs */}
          <div className="flex items-center justify-around mt-3">
            {[
              { icon: MessageSquare, label: "CHAT", active: true },
              { icon: Activity, label: "MODELS", active: false },
            ].map((tab) => (
              <button
                key={tab.label}
                className="flex flex-col items-center gap-1 px-6 py-1 transition-all"
                style={{ color: tab.active ? "#00dbe9" : "rgba(255,255,255,0.3)" }}
              >
                <tab.icon size={16} strokeWidth={1.5} />
                <span className="font-bold uppercase tracking-widest" style={{ fontSize: 9 }}>
                  {tab.label}
                </span>
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
};
