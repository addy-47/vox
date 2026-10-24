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
      subtitle="A mobile device is requesting permission to control Vox"
      icon={<ShieldAlert className="text-[rgb(var(--accent))] w-5 h-5" />}
      position="global"
      className="max-w-md w-full"
    >
      <div className="flex flex-col p-6 gap-6">
        <div className="flex flex-col gap-2 p-4 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.08)]">
          <div className="flex justify-between items-baseline">
            <span className="text-xs uppercase font-mono tracking-wider text-[rgb(var(--foreground-muted))]">
              Device Name
            </span>
            <span className="text-xs font-mono text-[rgb(var(--foreground-muted))]/60">
              {secondsRemaining}s left
            </span>
          </div>
          <span className="text-base font-bold text-[rgb(var(--foreground))]">
            {pendingPairingPrompt.displayName}
          </span>
          <span className="text-xs font-mono text-[rgb(var(--foreground-muted))]/70 truncate">
            ID: {pendingPairingPrompt.deviceId}
          </span>
        </div>

        <p className="text-xs text-[rgb(var(--foreground-muted))] leading-relaxed">
          Approving will grant this device remote control over Vox sessions, dictation, and speech parameters on your local network.
        </p>

        <div className="flex items-center justify-end gap-3 pt-2">
          <button
            type="button"
            onClick={() => void handleDecision("Reject")}
            className="inline-flex items-center gap-1.5 px-4 py-2 rounded-xl text-xs font-mono border border-[rgba(var(--border),0.25)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--foreground),0.2)] transition-colors cursor-pointer"
          >
            <X size={14} />
            Reject
          </button>
          <button
            type="button"
            onClick={() => void handleDecision("Approve")}
            className="inline-flex items-center gap-1.5 px-5 py-2 rounded-xl text-xs font-bold font-mono bg-[rgb(var(--accent))] text-[rgb(var(--background))] hover:opacity-90 shadow-sm transition-opacity cursor-pointer"
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
