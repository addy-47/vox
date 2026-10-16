import { memo } from "react";
import { Network, Server, Terminal, ShieldCheck, Cpu } from "lucide-react";

export interface ProviderArchitectureGraphicProps {
  category?: "STT" | "LLM" | "TTS";
}

export const ProviderArchitectureGraphic = memo(({
  category = "LLM",
}: ProviderArchitectureGraphicProps) => {
  const isTts = category === "TTS";
  const isStt = category === "STT";

  return (
    <div className="w-full flex flex-col gap-3 select-none text-[12px]">
      {/* ─── 1. Architectural SVG Topology Visual ─── */}
      <div className="w-full rounded-xl bg-[rgba(var(--card),0.5)] border border-[rgba(var(--accent),0.15)] p-3 overflow-hidden shadow-xs">
        <svg
          viewBox="0 0 840 156"
          className="w-full h-auto min-h-[140px] text-[rgb(var(--foreground))]"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <defs>
            <linearGradient id="archLineGrad" x1="0" y1="0" x2="1" y2="0">
              <stop offset="0%" stopColor="rgb(var(--accent))" stopOpacity="0.2" />
              <stop offset="50%" stopColor="rgb(var(--accent))" stopOpacity="0.8" />
              <stop offset="100%" stopColor="rgb(var(--accent))" stopOpacity="0.3" />
            </linearGradient>
            <linearGradient id="nodeGrad" x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor="rgba(var(--foreground), 0.05)" />
              <stop offset="100%" stopColor="rgba(var(--foreground), 0.015)" />
            </linearGradient>
            <filter id="accentGlow" x="-20%" y="-20%" width="140%" height="140%">
              <feDropShadow dx="0" dy="0" stdDeviation="4" floodColor="rgb(var(--accent))" floodOpacity="0.35" />
            </filter>
          </defs>

          {/* Connection Pathways */}
          <g stroke="url(#archLineGrad)" strokeWidth="1.8" strokeLinecap="round">
            {/* Vox -> Router */}
            <path d="M 152 78 L 222 78" strokeDasharray="3 3" />
            {/* Router -> On-Device */}
            <path d="M 348 60 C 405 60, 415 32, 455 32 L 470 32" />
            {/* Router -> LAN Server */}
            <path d="M 348 78 L 470 78" />
            {/* Router -> Cloud Gateway */}
            <path d="M 348 96 C 405 96, 415 124, 455 124 L 470 124" />
          </g>

          {/* Moving Flow Pulses */}
          <circle cx="187" cy="78" r="3" fill="rgb(var(--accent))" filter="url(#accentGlow)" />
          <circle cx="420" cy="32" r="2.5" fill="rgb(var(--accent))" />
          <circle cx="420" cy="78" r="2.5" fill="rgb(var(--accent))" />
          <circle cx="420" cy="124" r="2.5" fill="rgb(var(--accent))" />

          {/* Node 1: Vox Client */}
          <g transform="translate(16, 44)">
            <rect width="136" height="68" rx="10" fill="url(#nodeGrad)" stroke="rgba(var(--foreground), 0.12)" strokeWidth="1.2" />
            <rect x="10" y="12" width="24" height="24" rx="6" fill="rgba(var(--accent), 0.12)" stroke="rgba(var(--accent), 0.25)" />
            {/* Mini Waveform */}
            <path d="M 16 24 L 18 20 L 22 28 L 26 22 L 28 24" stroke="rgb(var(--accent))" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
            <text x="42" y="24" fill="rgb(var(--foreground))" fontSize="12" fontWeight="bold" fontFamily="system-ui, sans-serif">Vox Runtime</text>
            <text x="42" y="38" fill="rgb(var(--foreground-muted))" fontSize="9.5" fontFamily="monospace">Native Client</text>
            <rect x="10" y="46" width="116" height="15" rx="4" fill="rgba(var(--foreground), 0.03)" />
            <text x="68" y="57" textAnchor="middle" fill="rgb(var(--accent))" fontSize="8" fontFamily="monospace" fontWeight="600">IN-MEMORY AUDIO & CONTEXT</text>
          </g>

          {/* Node 2: Pipeline Stage Dispatcher */}
          <g transform="translate(222, 44)">
            <rect width="126" height="68" rx="10" fill="url(#nodeGrad)" stroke="rgba(var(--accent), 0.35)" strokeWidth="1.2" filter="url(#accentGlow)" />
            <text x="63" y="22" textAnchor="middle" fill="rgb(var(--accent))" fontSize="9" fontFamily="monospace" fontWeight="bold" letterSpacing="0.08em">PIPELINE ROUTER</text>
            <text x="63" y="42" textAnchor="middle" fill="rgb(var(--foreground))" fontSize="13" fontWeight="bold" fontFamily="system-ui, sans-serif">
              {isTts ? "Speech Engine" : isStt ? "Acoustic Stage" : "Language Core"}
            </text>
            <rect x="12" y="48" width="102" height="14" rx="4" fill="rgba(var(--accent), 0.12)" />
            <text x="63" y="58" textAnchor="middle" fill="rgb(var(--foreground))" fontSize="8" fontFamily="monospace" fontWeight="bold">
              {category} DISPATCHER
            </text>
          </g>

          {/* Node 3A: Destination - On-Device */}
          <g transform="translate(470, 10)">
            <rect width="350" height="44" rx="8" fill="url(#nodeGrad)" stroke="rgba(var(--accent), 0.25)" strokeWidth="1.2" />
            <circle cx="18" cy="22" r="5" fill="rgb(var(--accent))" opacity="0.8" />
            <text x="34" y="20" fill="rgb(var(--foreground))" fontSize="11.5" fontWeight="bold" fontFamily="system-ui, sans-serif">On-Device Engine</text>
            <text x="34" y="33" fill="rgb(var(--accent))" fontSize="9.5" fontFamily="monospace">100% Local · Zero Network Egress · Private</text>
          </g>

          {/* Node 3B: Destination - Local / LAN Server */}
          <g transform="translate(470, 56)">
            <rect width="350" height="44" rx="8" fill="url(#nodeGrad)" stroke="rgba(var(--foreground), 0.1)" strokeWidth="1.2" />
            <rect x="12" y="15" width="14" height="14" rx="3" fill="rgba(var(--foreground), 0.06)" />
            <text x="34" y="22" fill="rgb(var(--foreground))" fontSize="11.5" fontWeight="bold" fontFamily="system-ui, sans-serif">LAN Server / Tailscale</text>
            <text x="34" y="35" fill="rgb(var(--foreground-muted))" fontSize="9.5" fontFamily="monospace">http://127.0.0.1:11434 · OpenAI-Compatible API</text>
          </g>

          {/* Node 3C: Destination - Cloud API */}
          <g transform="translate(470, 102)">
            <rect width="350" height="44" rx="8" fill="url(#nodeGrad)" stroke="rgba(var(--foreground), 0.1)" strokeWidth="1.2" />
            <rect x="12" y="15" width="14" height="14" rx="3" fill="rgba(var(--foreground), 0.06)" />
            <text x="34" y="22" fill="rgb(var(--foreground))" fontSize="11.5" fontWeight="bold" fontFamily="system-ui, sans-serif">Cloud API Gateway</text>
            <text x="34" y="35" fill="rgb(var(--foreground-muted))" fontSize="9.5" fontFamily="monospace">https://api... · TLS 1.3 · Bearer Auth Key</text>
          </g>
        </svg>
      </div>

      {/* ─── 2. Endpoint Anatomy Breakdown ─── */}
      <div className="p-3.5 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.12)] flex flex-col gap-2.5">
        <div className="flex items-center gap-2 text-[rgb(var(--accent))] font-display font-bold text-[12.5px] uppercase tracking-wider">
          <Network size={14} />
          <span>Endpoint URL Anatomy</span>
        </div>

        {/* Visual URL Segmenter */}
        <div className="flex items-stretch gap-1 font-mono text-[11px] p-2 rounded-lg bg-[rgba(var(--card),0.6)] border border-[rgba(var(--foreground),0.06)] overflow-x-auto">
          <span className="text-[rgb(var(--foreground-muted))]/60 px-1 py-0.5 rounded">http://</span>
          <span className="px-1.5 py-0.5 rounded bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold" title="Host machine IP or domain">
            127.0.0.1
          </span>
          <span className="text-[rgb(var(--foreground-muted))]/60">:</span>
          <span className="px-1.5 py-0.5 rounded bg-[rgba(var(--accent),0.2)] text-[rgb(var(--foreground))] font-bold" title="Port number">
            {isTts ? "8000" : "11434"}
          </span>
          <span className="text-[rgb(var(--foreground-muted))]/60 px-1 py-0.5">{isTts ? "/health" : "/v1"}</span>
        </div>

        {/* Breakdown Grid */}
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-2 text-[11px] pt-1">
          <div className="p-2 rounded-lg bg-[rgba(var(--card),0.4)] border border-[rgba(var(--foreground),0.04)]">
            <span className="font-bold text-[rgb(var(--foreground))] block mb-0.5">1. Host Address</span>
            <span className="text-[rgb(var(--foreground-muted))] leading-relaxed">
              Use <code className="font-mono text-[rgb(var(--accent))]">127.0.0.1</code> for this machine, or your LAN/Tailscale IP for external rigs.
            </span>
          </div>

          <div className="p-2 rounded-lg bg-[rgba(var(--card),0.4)] border border-[rgba(var(--foreground),0.04)]">
            <span className="font-bold text-[rgb(var(--foreground))] block mb-0.5">2. Service Port</span>
            <span className="text-[rgb(var(--foreground-muted))] leading-relaxed">
              {isTts ? (
                <>Chatterbox defaults to <code className="font-mono text-[rgb(var(--accent))]">8000</code>.</>
              ) : (
                <>Ollama: <code className="font-mono text-[rgb(var(--accent))]">11434</code> · LM Studio: <code className="font-mono text-[rgb(var(--accent))]">1234</code> · vLLM: <code className="font-mono text-[rgb(var(--accent))]">8000</code>.</>
              )}
            </span>
          </div>

          <div className="p-2 rounded-lg bg-[rgba(var(--card),0.4)] border border-[rgba(var(--foreground),0.04)]">
            <span className="font-bold text-[rgb(var(--foreground))] block mb-0.5">3. Authentication</span>
            <span className="text-[rgb(var(--foreground-muted))] leading-relaxed">
              Local servers do not require keys (leave blank or <code className="font-mono text-[rgb(var(--accent))]">ollama</code>). Cloud APIs require your secret token.
            </span>
          </div>
        </div>
      </div>

      {/* ─── 3. Category-Specific Practical Worksheet ─── */}
      {isTts ? (
        <div className="p-3.5 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--foreground),0.06)] flex flex-col gap-2">
          <div className="flex items-center gap-2 text-[rgb(var(--accent))] font-display font-bold text-[12px] uppercase tracking-wider">
            <Server size={13} />
            <span>Chatterbox Remote GPU Workflow</span>
          </div>
          <p className="text-[11.5px] text-[rgb(var(--foreground-muted))] leading-relaxed">
            Chatterbox remote offload streams 16kHz audio waveforms from a dedicated GPU host over HTTP without taxing your local CPU:
          </p>
          <div className="space-y-1.5 font-mono text-[10.5px]">
            <div className="flex items-center gap-2 p-1.5 rounded bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.05)]">
              <Terminal size={12} className="text-[rgb(var(--accent))] shrink-0" />
              <span className="text-[rgb(var(--foreground))]">Run on GPU server:</span>
              <code className="text-[rgb(var(--accent))]">python server.py --host 0.0.0.0 --port 8000</code>
            </div>
            <div className="flex items-center gap-2 p-1.5 rounded bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.05)]">
              <Cpu size={12} className="text-[rgb(var(--accent))] shrink-0" />
              <span className="text-[rgb(var(--foreground))]">Remote model path:</span>
              <code className="text-[rgb(var(--foreground-muted))]">/opt/chatterbox</code>
            </div>
          </div>
        </div>
      ) : (
        <div className="p-3.5 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--foreground),0.06)] flex flex-col gap-2">
          <div className="flex items-center gap-2 text-[rgb(var(--accent))] font-display font-bold text-[12px] uppercase tracking-wider">
            <ShieldCheck size={13} />
            <span>LAN Network Binding & CORS</span>
          </div>
          <p className="text-[11.5px] text-[rgb(var(--foreground-muted))] leading-relaxed">
            When connecting to an LLM server running on another machine on your Wi-Fi, ensure the service binds to all network interfaces:
          </p>
          <div className="space-y-1.5 font-mono text-[10.5px]">
            <div className="flex items-center gap-2 p-1.5 rounded bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.05)]">
              <Terminal size={12} className="text-[rgb(var(--accent))] shrink-0" />
              <span className="text-[rgb(var(--foreground))]">Ollama LAN startup:</span>
              <code className="text-[rgb(var(--accent))]">OLLAMA_HOST=0.0.0.0 OLLAMA_ORIGINS="*" ollama serve</code>
            </div>
          </div>
        </div>
      )}
    </div>
  );
});

ProviderArchitectureGraphic.displayName = "ProviderArchitectureGraphic";
