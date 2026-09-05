import { memo } from "react";
import { Network, Info, Check, RefreshCw } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { UnderlineInput } from "@/shared/ui";
import { REMOTE_SERVER_COPY } from "@/data/settingsCopy";

export interface RemoteSetupStatus {
  step: "initiating" | "connecting" | "deploying" | "starting_service" | "verifying" | "complete" | "failed" | string;
  progress: number;
  log_line?: string | null;
  error?: string | null;
}

export interface RemoteServerSetupProps {
  sshConnectionString: string;
  setSshConnectionString: (val: string) => void;
  sshPort: string;
  setSshPort: (val: string) => void;
  sshIdentityKey: string;
  setSshIdentityKey: (val: string) => void;
  setupStatus: RemoteSetupStatus | null;
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
            {REMOTE_SERVER_COPY.bannerTitle}
          </span>
        </div>
        <p className="text-[12px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed font-medium">
          {REMOTE_SERVER_COPY.bannerBody}
        </p>
      </div>

      {/* Setup Panel */}
      <div className="border border-[rgba(var(--accent),0.15)] bg-[rgba(var(--accent),0.02)] rounded-xl p-3 animate-fade-in space-y-3">
        <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1.5">
          <span className="font-bold text-[12px] text-[rgb(var(--foreground))] flex items-center gap-1.5">
            <Network size={14} className="text-[rgb(var(--accent))]" />
            {REMOTE_SERVER_COPY.panelTitle}
          </span>
          <span className={cn(
            "text-[11px] font-black uppercase px-1.5 py-0.5 rounded border",
            isRemoteTtsHealthy
              ? "bg-emerald-500/10 text-emerald-400 border-emerald-500/20"
              : "bg-rose-500/10 text-rose-400 border-rose-500/20"
          )}>
            {isRemoteTtsHealthy ? REMOTE_SERVER_COPY.online : REMOTE_SERVER_COPY.offline}
          </span>
        </div>

        <div className="grid grid-cols-[2.5fr_1fr_2.5fr] gap-3">
          <UnderlineInput
            label={REMOTE_SERVER_COPY.hostLabel}
            value={sshConnectionString}
            onChange={(e) => setSshConnectionString(e.target.value)}
            placeholder={REMOTE_SERVER_COPY.hostPlaceholder}
          />
          <UnderlineInput
            label={REMOTE_SERVER_COPY.portLabel}
            value={sshPort}
            onChange={(e) => setSshPort(e.target.value)}
            placeholder={REMOTE_SERVER_COPY.portPlaceholder}
          />
          <UnderlineInput
            label={REMOTE_SERVER_COPY.keyLabel}
            value={sshIdentityKey}
            onChange={(e) => setSshIdentityKey(e.target.value)}
            placeholder={REMOTE_SERVER_COPY.keyPlaceholder}
          />
        </div>

        {setupStatus && (
          <div className="space-y-1.5 pt-1">
            <div className="flex items-center justify-between text-[11px]">
              <span className="font-bold text-[rgb(var(--foreground))] uppercase tracking-wider">
                {REMOTE_SERVER_COPY.steps[setupStatus.step] ?? setupStatus.step}
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
            {setupStatus?.step === "complete" ? REMOTE_SERVER_COPY.footerReady : REMOTE_SERVER_COPY.footerBusy}
          </p>
          <button
            type="button"
            onClick={triggerRemoteSetup}
            disabled={Boolean(setupStatus && setupStatus.step !== "failed" && setupStatus.step !== "complete")}
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
                {REMOTE_SERVER_COPY.deployed}
              </>
            ) : setupStatus && setupStatus.step !== "failed" ? (
              <>
                <RefreshCw size={12} className="animate-spin" />
                {REMOTE_SERVER_COPY.deploying}
              </>
            ) : (
              REMOTE_SERVER_COPY.deploy
            )}
          </button>
        </div>
      </div>
    </div>
  );
});

RemoteServerSetup.displayName = "RemoteServerSetup";
