import { memo } from "react";
import { Network, Info, Check, RefreshCw } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface RemoteServerSetupProps {
  sshConnectionString: string;
  setSshConnectionString: (val: string) => void;
  sshPort: string;
  setSshPort: (val: string) => void;
  sshIdentityKey: string;
  setSshIdentityKey: (val: string) => void;
  setupStatus: any;
  triggerRemoteSetup: () => void;
  isRemoteTtsHealthy: boolean | null;
}

export const RemoteServerSetup = memo(({
  sshConnectionString,
  setSshConnectionString,
  sshPort,
  setSshPort,
  sshIdentityKey,
  setSshIdentityKey,
  setupStatus,
  triggerRemoteSetup,
  isRemoteTtsHealthy,
}: RemoteServerSetupProps) => {
  return (
    <div className="space-y-4">
      {/* Description Banner */}
      <div className="border border-[rgba(var(--accent),0.15)] bg-[rgba(var(--accent),0.02)] rounded-xl p-4 space-y-2">
        <div className="flex items-center gap-2 text-[rgb(var(--accent))]">
          <Info size={16} />
          <span className="font-bold text-[12px] uppercase tracking-[0.1em]">
            Chatterbox Remote Deployment
          </span>
        </div>
        <p className="text-[12px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed font-medium">
          Deploy Chatterbox on a remote CUDA-accelerated GPU host (e.g. RunPod, Vast.ai, or homelab) to offload memory-intensive flow-matching voice synthesis. Enter your SSH connection info below to automatically sync the codebase, download GGUF models, and run the server.
        </p>
      </div>

      {/* Setup Panel */}
      <div className="border border-[rgba(var(--accent),0.15)] bg-[rgba(var(--accent),0.02)] rounded-xl p-3 animate-fade-in space-y-3">
        <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1.5">
          <span className="font-bold text-[12px] text-[rgb(var(--foreground))] flex items-center gap-1.5">
            <Network size={14} className="text-[rgb(var(--accent))]" />
            Setup Remote GPU Server (SSH Setup Required)
          </span>
          <span className={cn(
            "text-[11px] font-black uppercase px-1.5 py-0.5 rounded border",
            isRemoteTtsHealthy
              ? "bg-emerald-500/10 text-emerald-400 border-emerald-500/20"
              : "bg-rose-500/10 text-rose-400 border-rose-500/20"
          )}>
            {isRemoteTtsHealthy ? "Online / Connected" : "Offline / Unconfigured"}
          </span>
        </div>

        <div className="grid grid-cols-[2.5fr_1fr_2.5fr] gap-2.5">
          <div className="space-y-1">
            <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75">
              SSH Host / Profile
            </label>
            <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
              <input
                type="text"
                value={sshConnectionString}
                onChange={(e) => setSshConnectionString(e.target.value)}
                placeholder="user@hostname"
                className="w-full bg-transparent border-none outline-none text-[12px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
              />
            </div>
          </div>
          <div className="space-y-1">
            <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75">
              SSH Port
            </label>
            <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
              <input
                type="text"
                value={sshPort}
                onChange={(e) => setSshPort(e.target.value)}
                placeholder="22"
                className="w-full bg-transparent border-none outline-none text-[12px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
              />
            </div>
          </div>
          <div className="space-y-1">
            <label className="text-[11px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75">
              Identity Key Path
            </label>
            <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
              <input
                type="text"
                value={sshIdentityKey}
                onChange={(e) => setSshIdentityKey(e.target.value)}
                placeholder="~/.ssh/id_rsa"
                className="w-full bg-transparent border-none outline-none text-[12px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
              />
            </div>
          </div>
        </div>

        {setupStatus && (
          <div className="space-y-1.5 pt-1">
            <div className="flex items-center justify-between text-[11px]">
              <span className="font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">
                {setupStatus.step === "initiating" && "Initializing Setup..."}
                {setupStatus.step === "connecting" && "Testing SSH Connection..."}
                {setupStatus.step === "deploying" && "Configuring Remote Server..."}
                {setupStatus.step === "starting_service" && "Starting Chatterbox Service..."}
                {setupStatus.step === "verifying" && "Verifying Health Endpoint..."}
                {setupStatus.step === "complete" && "Setup Completed Successfully"}
                {setupStatus.step === "failed" && "Setup Failed"}
              </span>
              <span className="font-mono text-[rgb(var(--accent))]">{setupStatus.progress}%</span>
            </div>
            <div className="w-full h-1 bg-[rgba(var(--foreground),0.06)] rounded-full overflow-hidden">
              <div
                className={cn(
                  "h-full transition-all duration-300 rounded-full",
                  setupStatus.step === "failed" ? "bg-rose-500" : "bg-[rgb(var(--accent))]"
                )}
                style={{ width: `${setupStatus.progress}%` }}
              />
            </div>
            {setupStatus.log_line && (
              <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/70 truncate">
                {setupStatus.log_line}
              </p>
            )}
          </div>
        )}

        <div className="flex items-center justify-between pt-1">
          <p className="text-[11px] text-[rgb(var(--foreground-muted))]/60">
            {setupStatus?.step === "complete" ? "Ready to synthesize flow-matching audio." : "Syncs scripts and installs PyTorch CUDA on remote host."}
          </p>
          <button
            type="button"
            onClick={triggerRemoteSetup}
            disabled={setupStatus && setupStatus.step !== "failed" && setupStatus.step !== "complete"}
            className={cn(
              "px-3 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300 flex items-center gap-1.5 border shadow-[0_0_12px_rgba(var(--accent),0.15)]",
              setupStatus?.step === "complete"
                ? "bg-emerald-500/10 border-emerald-500/30 text-emerald-400 hover:bg-emerald-500/20"
                : "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] border-[rgba(var(--accent),0.2)] hover:scale-[1.02] active:scale-95"
            )}
          >
            {setupStatus?.step === "complete" ? (
              <>
                <Check size={12} />
                Deployed & Active
              </>
            ) : setupStatus && setupStatus.step !== "failed" ? (
              <>
                <RefreshCw size={12} className="animate-spin" />
                Deploying...
              </>
            ) : (
              "Deploy Chatterbox Server"
            )}
          </button>
        </div>
      </div>
    </div>
  );
});

RemoteServerSetup.displayName = "RemoteServerSetup";
