import { memo, useCallback, useEffect, useState } from "react";
import {
  AlertCircle,
  Check,
  FolderUp,
  Loader2,
  Mic,
  RotateCcw,
  Square,
} from "lucide-react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { cn } from "@/shared/lib/utils";
import { ErrorBoundary } from "@/shared/components/common/ErrorBoundary";
import { Modal } from "@/shared/ui/Modal";
import {
  addVoiceFromFile,
  addVoiceFromRecording,
  startBackendRecording,
  stopBackendRecording,
  type UnifiedVoiceDto,
} from "@/services/voicesService";

export interface CloneVoiceDrawerProps {
  isOpen: boolean;
  onClose: () => void;
  onCreated: (newVoice: UnifiedVoiceDto) => void;
}

type Tab = "record" | "upload";

/**
 * @deprecated Use `CloneVoiceCard` instead. Voice cloning is now rendered in-place as a catalog tile.
 */
export const CloneVoiceDrawer = memo(
  ({ isOpen, onClose, onCreated }: CloneVoiceDrawerProps) => {
    const [tab, setTab] = useState<Tab>("record");
    const [voiceName, setVoiceName] = useState("");
    const [isRecording, setIsRecording] = useState(false);
    const [duration, setDuration] = useState(0);
    const [recordedPcm, setRecordedPcm] = useState<number[] | null>(null);
    const [recordedSampleRate, setRecordedSampleRate] = useState(0);
    const [selectedFilePath, setSelectedFilePath] = useState<string | null>(null);
    const [isSubmitting, setIsSubmitting] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const handleStartRecording = useCallback(async () => {
      setError(null);
      setRecordedPcm(null);
      setRecordedSampleRate(0);
      setDuration(0);
      try {
        await startBackendRecording();
        setIsRecording(true);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    }, []);

    const handleStopRecording = useCallback(async () => {
      setIsRecording(false);
      try {
        const [pcm, sampleRate] = await stopBackendRecording();
        setRecordedPcm(pcm);
        setRecordedSampleRate(sampleRate);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    }, []);

    // Timer effect for live recording up to 30s
    useEffect(() => {
      let timer: ReturnType<typeof setInterval> | undefined;
      if (isRecording) {
        timer = setInterval(() => {
          setDuration((d) => (d >= 30 ? 30 : d + 1));
        }, 1000);
      } else {
        clearInterval(timer);
      }
      return () => clearInterval(timer);
    }, [isRecording]);

    // Auto-stop at 30 seconds
    useEffect(() => {
      if (isRecording && duration >= 30) {
        void handleStopRecording();
      }
    }, [isRecording, duration, handleStopRecording]);

    // Cleanup recording on unmount or close
    useEffect(() => {
      if (!isOpen) {
        if (isRecording) {
          void stopBackendRecording().catch(() => {});
        }
        setIsRecording(false);
        setDuration(0);
        setRecordedPcm(null);
        setRecordedSampleRate(0);
        setSelectedFilePath(null);
        setVoiceName("");
        setError(null);
        setIsSubmitting(false);
      }
    }, [isOpen, isRecording]);

    const handlePickFile = useCallback(async () => {
      setError(null);
      try {
        const result = await openDialog({
          multiple: false,
          filters: [{ name: "Audio WAV", extensions: ["wav"] }],
        });
        if (typeof result === "string") {
          setSelectedFilePath(result);
        }
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    }, []);

    const handleClone = useCallback(async () => {
      const trimmed = voiceName.trim();
      if (!trimmed) {
        setError("Please enter a voice name");
        return;
      }

      setError(null);
      setIsSubmitting(true);
      try {
        let created: UnifiedVoiceDto;
        if (tab === "record") {
          if (!recordedPcm || recordedSampleRate === 0) {
            setError("No audio recorded. Please record your voice first.");
            setIsSubmitting(false);
            return;
          }
          created = await addVoiceFromRecording(
            trimmed,
            recordedPcm,
            recordedSampleRate
          );
        } else {
          if (!selectedFilePath) {
            setError("Please select a WAV file to clone.");
            setIsSubmitting(false);
            return;
          }
          created = await addVoiceFromFile(trimmed, selectedFilePath);
        }
        onCreated(created);
        onClose();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setIsSubmitting(false);
      }
    }, [
      voiceName,
      tab,
      recordedPcm,
      recordedSampleRate,
      selectedFilePath,
      onCreated,
      onClose,
    ]);

    if (!isOpen) return null;

    const canSubmit =
      voiceName.trim().length > 0 &&
      !isSubmitting &&
      (tab === "record"
        ? recordedPcm != null && !isRecording
        : selectedFilePath != null);

    return (
      <Modal
        open={isOpen}
        onClose={onClose}
        position="global"
        icon={
          <div className="w-8 h-8 rounded-lg bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] flex items-center justify-center shrink-0">
            <Mic size={18} />
          </div>
        }
        title={
          <div className="min-w-0">
            <h2
              id="clone-voice-title"
              className="font-bold text-[15px] text-[rgb(var(--foreground))] leading-tight"
            >
              Clone New Voice
            </h2>
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]">
              Record a clear 1–30s audio sample
            </p>
          </div>
        }
        ariaLabel="Clone a new voice"
        className="w-[min(480px,94vw)] max-w-[calc(100vw-36px)]"
        bodyClassName="p-4"
        footer={
          <div className="px-4 py-3 border-t border-[rgba(var(--foreground),0.06)] flex items-center justify-end gap-2 bg-[rgba(var(--foreground),0.01)]">
            <button
              type="button"
              onClick={onClose}
              disabled={isSubmitting}
              className="px-4 py-1.5 rounded-lg text-[12px] font-bold text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-all cursor-pointer"
            >
              Cancel
            </button>
            <button
              type="button"
              onClick={handleClone}
              disabled={!canSubmit}
              className={cn(
                "px-4 py-1.5 rounded-lg text-[12px] font-bold transition-all flex items-center gap-1.5 cursor-pointer",
                canSubmit
                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:brightness-110 active:scale-95 shadow-sm"
                  : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]/40 cursor-not-allowed"
              )}
            >
              {isSubmitting ? (
                <>
                  <Loader2 size={13} className="animate-spin" />
                  <span>Processing...</span>
                </>
              ) : (
                <>
                  <Check size={13} strokeWidth={2.5} />
                  <span>Clone Voice</span>
                </>
              )}
            </button>
          </div>
        }
      >
        <ErrorBoundary name="CloneVoiceDialog">
          <div className="flex flex-col gap-4">
            {/* Tabs */}
            <div className="flex rounded-lg bg-[rgba(var(--foreground),0.04)] p-0.5 border border-[rgba(var(--foreground),0.06)]">
              <button
                type="button"
                onClick={() => setTab("record")}
                disabled={isRecording || isSubmitting}
                className={cn(
                  "flex-1 py-1.5 rounded-md text-[12px] font-bold transition-all flex items-center justify-center gap-1.5 cursor-pointer",
                  tab === "record"
                    ? "bg-[rgb(var(--card))] text-[rgb(var(--foreground))] shadow-xs"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                <Mic size={14} />
                <span>Record Audio</span>
              </button>
              <button
                type="button"
                onClick={() => setTab("upload")}
                disabled={isRecording || isSubmitting}
                className={cn(
                  "flex-1 py-1.5 rounded-md text-[12px] font-bold transition-all flex items-center justify-center gap-1.5 cursor-pointer",
                  tab === "upload"
                    ? "bg-[rgb(var(--card))] text-[rgb(var(--foreground))] shadow-xs"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))]"
                )}
              >
                <FolderUp size={14} />
                <span>Upload WAV</span>
              </button>
            </div>

            {/* Tab content */}
            {tab === "record" ? (
              <div className="flex flex-col items-center justify-center p-5 rounded-xl border border-[rgba(var(--foreground),0.08)] bg-[rgba(var(--foreground),0.02)] gap-3">
                {isRecording ? (
                  <div className="flex flex-col items-center gap-3">
                    <div className="relative flex items-center justify-center">
                      <span className="absolute w-16 h-16 rounded-full bg-[rgba(var(--error),0.2)] animate-ping" />
                      <button
                        type="button"
                        onClick={handleStopRecording}
                        className="relative w-14 h-14 rounded-full bg-[rgb(var(--error))] text-white flex items-center justify-center shadow-lg hover:bg-[rgb(var(--error-dark))] transition-all cursor-pointer"
                      >
                        <Square size={20} fill="currentColor" />
                      </button>
                    </div>
                    <span className="font-mono text-[14px] font-bold text-[rgb(var(--error))]">
                      00:{duration.toString().padStart(2, "0")} / 00:30
                    </span>
                    <p className="text-[11px] text-[rgb(var(--foreground-muted))] text-center">
                      Speak clearly into your microphone... Tap square to finish.
                    </p>
                  </div>
                ) : recordedPcm ? (
                  <div className="flex flex-col items-center gap-2">
                    <div className="w-12 h-12 rounded-full bg-[rgba(var(--success),0.15)] text-[rgb(var(--success))] flex items-center justify-center">
                      <Check size={22} strokeWidth={2.5} />
                    </div>
                    <span className="font-bold text-[13px] text-[rgb(var(--foreground))]">
                      Sample Recorded ({duration}s)
                    </span>
                    <button
                      type="button"
                      onClick={handleStartRecording}
                      className="mt-1 px-3 py-1 rounded-lg text-[11px] font-medium text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] border border-[rgba(var(--foreground),0.08)] flex items-center gap-1.5 cursor-pointer"
                    >
                      <RotateCcw size={12} />
                      <span>Record Again</span>
                    </button>
                  </div>
                ) : (
                  <div className="flex flex-col items-center gap-3">
                    <button
                      type="button"
                      onClick={handleStartRecording}
                      className="w-14 h-14 rounded-full bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/25 flex items-center justify-center shadow-sm transition-all cursor-pointer"
                    >
                      <Mic size={24} />
                    </button>
                    <div className="text-center">
                      <span className="font-bold text-[13px] text-[rgb(var(--foreground))]">
                        Ready to Record
                      </span>
                      <p className="text-[11px] text-[rgb(var(--foreground-muted))] mt-0.5 max-w-xs">
                        Read a short paragraph clearly (10–30 seconds recommended).
                      </p>
                    </div>
                  </div>
                )}
              </div>
            ) : (
              <div
                onClick={handlePickFile}
                className="flex flex-col items-center justify-center p-6 rounded-xl border border-dashed border-[rgba(var(--foreground),0.15)] bg-[rgba(var(--foreground),0.02)] hover:border-[rgb(var(--accent))]/50 hover:bg-[rgba(var(--accent),0.02)] transition-all cursor-pointer text-center gap-2"
              >
                <div className="w-12 h-12 rounded-full bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] flex items-center justify-center">
                  <FolderUp size={20} />
                </div>
                {selectedFilePath ? (
                  <div className="flex flex-col items-center">
                    <span className="font-bold text-[13px] text-[rgb(var(--foreground))] break-all max-w-xs">
                      {selectedFilePath.split("/").pop()}
                    </span>
                    <span className="text-[10px] text-[rgb(var(--accent))] mt-0.5">
                      Click to change file
                    </span>
                  </div>
                ) : (
                  <div>
                    <span className="font-bold text-[13px] text-[rgb(var(--foreground))]">
                      Select WAV Audio File
                    </span>
                    <p className="text-[11px] text-[rgb(var(--foreground-muted))] mt-0.5">
                      Supports mono or stereo 16-bit / 24-bit PCM WAV
                    </p>
                  </div>
                )}
              </div>
            )}

            {/* Voice Name Input */}
            <div className="flex flex-col gap-1.5">
              <label
                htmlFor="clone-voice-name"
                className="text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]"
              >
                Voice Name
              </label>
              <input
                id="clone-voice-name"
                type="text"
                value={voiceName}
                onChange={(e) => setVoiceName(e.target.value)}
                placeholder="e.g. My Voice"
                disabled={isSubmitting}
                className="w-full px-3 py-2 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.1)] text-[13px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 outline-none focus:border-[rgb(var(--accent))] focus:ring-1 focus:ring-[rgb(var(--accent))]/30 transition-all"
              />
            </div>

            {/* Error banner */}
            {error && (
              <div className="p-2.5 rounded-lg bg-[rgba(var(--error),0.1)] border border-[rgba(var(--error),0.2)] text-[rgb(var(--error))] text-[11px] flex items-center gap-2">
                <AlertCircle size={14} className="shrink-0" />
                <span className="flex-1">{error}</span>
              </div>
            )}
          </div>
        </ErrorBoundary>
      </Modal>
    );
  }
);

CloneVoiceDrawer.displayName = "CloneVoiceDrawer";
