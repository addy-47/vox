import { memo } from "react";
import { Network, Server, Terminal, ShieldCheck, Cpu } from "lucide-react";

export interface ProviderArchitectureGraphicProps {
  category?: "STT" | "LLM" | "TTS";
}

export const ProviderArchitectureGraphic = memo(({
  category = "LLM",
}: ProviderArchitectureGraphicProps) => {
  const isTts = category === "TTS";

  return (
    <div className="w-full flex flex-col gap-3 py-2 select-none text-[12px]">
      {/* ─── 1. Endpoint Anatomy Breakdown ─── */}
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

      {/* ─── 2. Category-Specific Practical Worksheet ─── */}
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
              <code className="text-[rgb(var(--foreground-muted))]">/opt/chatterbox</code> (containing GGUF files)
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
