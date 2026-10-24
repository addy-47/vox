import { memo, useState, useEffect, useCallback, useRef } from "react";
import { ShieldAlert, Check, X } from "lucide-react";
import { Modal } from "@/shared/ui/Modal";
import { useRemoteStore } from "@/store/remoteStore";

export const PairingPromptModal = memo(() => {
  const pendingPairingPrompt = useRemoteStore((s) => s.pendingPairingPrompt);
  const approvePairing = useRemoteStore((s) => s.approvePairing);
  const dismissPairingPrompt = useRemoteStore((s) => s.dismissPairingPrompt);

  const [secondsRemaining, setSecondsRemaining] = useState(30);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const handleDecision = useCallback(
    async (decision: "Approve" | "Reject") => {
      clearTimer();
      try {
        await approvePairing(decision);
      } catch (e) {
        console.warn("[PairingPromptModal] approvePairing error:", e);
        dismissPairingPrompt();
      }
    },
    [approvePairing, dismissPairingPrompt, clearTimer]
  );

  useEffect(() => {
    if (pendingPairingPrompt) {
      let remaining = 30;
      setSecondsRemaining(30);
      clearTimer();

      timerRef.current = setInterval(() => {
        remaining -= 1;
        setSecondsRemaining(remaining);
        if (remaining <= 0) {
          clearTimer();
          void handleDecision("Reject");
        }
      }, 1000);
    } else {
      clearTimer();
    }

    return () => clearTimer();
  }, [pendingPairingPrompt, clearTimer, handleDecision]);

  if (!pendingPairingPrompt) {
    return null;
  }

  return (
    <Modal
      open={true}
      onClose={() => void handleDecision("Reject")}
      title="Remote Connection Request"
      subtitle="A mobile device is requesting permission to control Vox."
      icon={<ShieldAlert className="text-[rgb(var(--accent))] w-5 h-5" />}
      position="global"
      fitContent
      className="max-w-md w-full"
    >
      <div className="flex flex-col p-6 gap-5">
        <div className="flex flex-col gap-1.5 p-4 rounded-2xl bg-[rgba(var(--card),0.65)] border border-[rgba(var(--border),0.08)]">
          <div className="flex justify-between items-center">
            <span className="text-[11px] uppercase font-mono font-bold tracking-wider text-[rgb(var(--foreground-muted))]">
              Device Identity
            </span>
            <span className="text-xs font-mono font-semibold text-[rgb(var(--accent))]">
              {secondsRemaining}s remaining
            </span>
          </div>
          <span className="text-base font-bold text-[rgb(var(--foreground))] mt-0.5">
            {pendingPairingPrompt.displayName}
          </span>
          <span className="text-xs font-mono text-[rgb(var(--foreground-muted))]/75 truncate">
            ID: {pendingPairingPrompt.deviceId}
          </span>
        </div>

        <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
          Approving will grant this device remote control over Vox sessions,
          live audio, and assistant settings on your local network.
        </p>

        <div className="flex items-center justify-end gap-2.5 pt-1">
          <button
            type="button"
            onClick={() => void handleDecision("Reject")}
            className="h-10 inline-flex items-center gap-1.5 px-4 rounded-xl text-xs font-semibold border border-[rgba(var(--border),0.12)] bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--border),0.22)] transition-colors cursor-pointer"
          >
            <X size={14} />
            Reject
          </button>
          <button
            type="button"
            onClick={() => void handleDecision("Approve")}
            className="h-10 inline-flex items-center gap-1.5 px-5 rounded-xl text-xs font-bold bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:opacity-90 shadow-sm transition-opacity cursor-pointer"
          >
            <Check size={14} strokeWidth={2.5} />
            Approve Device
          </button>
        </div>
      </div>
    </Modal>
  );
});

PairingPromptModal.displayName = "PairingPromptModal";