import { memo } from "react";
import { Network, Info, Check, RefreshCw, Terminal } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { UnderlineInput } from "@/shared/ui";
import { Tooltip } from "@/shared/ui/Tooltip";
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
    <div className="flex-1 min-h-0 w-full h-full flex flex-col justify-between overflow-y-auto custom-scrollbar pr-1 animate-fade-in gap-3">
      {/* Top Section: Header & Structured Grid Form */}
      <div className="space-y-3.5">
        {/* Header Bar */}
        <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-2">
          <div className="flex items-center gap-1.5 min-w-0">
            <Network size={13} className="text-[rgb(var(--accent))] shrink-0" />
            <span className="font-bold text-[11px] uppercase tracking-wider text-[rgb(var(--foreground))] truncate">
              {REMOTE_SERVER_COPY.panelTitle}
            </span>
            <Tooltip
              label={`${REMOTE_SERVER_COPY.bannerTitle}: ${REMOTE_SERVER_COPY.bannerBody}`}
              side="bottom"
            >
              <span className="text-[rgb(var(--foreground-muted))]/50 hover:text-[rgb(var(--accent))] transition-colors cursor-help inline-flex items-center p-0.5 shrink-0">
                <Info size={12} />
              </span>
            </Tooltip>
          </div>

          <Tooltip
            label={isRemoteTtsHealthy ? REMOTE_SERVER_COPY.online : REMOTE_SERVER_COPY.offline}
            side="left"
          >
            <div className="flex items-center gap-1.5 shrink-0 px-1 py-0.5 cursor-help">
              <span
                className={cn(
                  "w-2 h-2 rounded-full transition-all duration-300",
                  isRemoteTtsHealthy
                    ? "bg-[rgb(var(--accent))] shadow-[0_0_8px_rgba(var(--accent),0.6)]"
                    : "bg-[rgba(var(--foreground),0.2)]"
                )}
              />
              <span className="text-[10px] uppercase font-mono tracking-wider text-[rgb(var(--foreground-muted))]/60">
                {isRemoteTtsHealthy ? "Online" : "Offline"}
              </span>
            </div>
          </Tooltip>
        </div>

        {/* Inputs Layout: 2 rows with natural breathing room */}
        <div className="grid grid-cols-3 gap-x-3 gap-y-3">
          <div className="col-span-2">
            <UnderlineInput
              label={REMOTE_SERVER_COPY.hostLabel}
              value={sshConnectionString}
              onChange={(e) => setSshConnectionString(e.target.value)}
              placeholder={REMOTE_SERVER_COPY.hostPlaceholder}
            />
          </div>
          <div className="col-span-1">
            <UnderlineInput
              label={REMOTE_SERVER_COPY.portLabel}
              value={sshPort}
              onChange={(e) => setSshPort(e.target.value)}
              placeholder={REMOTE_SERVER_COPY.portPlaceholder}
            />
          </div>
          <div className="col-span-3">
            <UnderlineInput
              label={REMOTE_SERVER_COPY.keyLabel}
              value={sshIdentityKey}
              onChange={(e) => setSshIdentityKey(e.target.value)}
              placeholder={REMOTE_SERVER_COPY.keyPlaceholder}
            />
          </div>
        </div>
      </div>

      {/* Bottom Section: Progress Tracking & Action Bar */}
      <div className="space-y-2.5 pt-1">
        {setupStatus && (
          <div className="space-y-1.5 p-2 rounded-lg bg-[rgba(var(--accent),0.03)] border border-[rgba(var(--accent),0.08)]">
            <div className="flex items-center justify-between text-[10px]">
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
              <p className="text-[10px] font-mono text-[rgb(var(--foreground-muted))]/70 truncate">
                {setupStatus.log_line}
              </p>
            )}
          </div>
        )}

        <div className="flex items-center justify-between gap-3 border-t border-[rgba(var(--accent),0.08)] pt-2">
          <p className="text-[10.5px] text-[rgb(var(--foreground-muted))]/60 leading-tight">
            {setupStatus?.step === "complete" ? REMOTE_SERVER_COPY.footerReady : REMOTE_SERVER_COPY.footerBusy}
          </p>
          <button
            type="button"
            onClick={triggerRemoteSetup}
            disabled={Boolean(setupStatus && setupStatus.step !== "failed" && setupStatus.step !== "complete")}
            className={cn(
              "px-3 py-1.5 rounded-lg text-[10.5px] font-bold uppercase tracking-wider transition-all duration-300 flex items-center gap-1.5 border shadow-[0_0_12px_rgba(var(--accent),0.15)] shrink-0",
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
              <>
                <Terminal size={12} />
                {REMOTE_SERVER_COPY.deploy}
              </>
            )}
          </button>
        </div>
      </div>
    </div>
  );
});

RemoteServerSetup.displayName = "RemoteServerSetup";
