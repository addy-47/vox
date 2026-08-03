import { useState, useEffect } from "react";
import { cn } from "@/shared/lib/utils";
import { open } from "@tauri-apps/plugin-dialog";
import { Folder, Mic, Trash2, ChevronLeft, ChevronRight } from "lucide-react";
import {
  startBackendRecording,
  stopBackendRecording,
  addVoiceFromFile,
  addVoiceFromRecording,
  deleteVoice,
} from "@/services/pipelineService";

export function VoiceBars({ seed, disabled }: { seed: string; disabled?: boolean }) {
  const hash = Array.from(seed).reduce((acc, char) => acc + char.charCodeAt(0), 0);
  const bars = Array.from({ length: 16 }, (_, i) => ((hash * (i + 1)) % 25) + 10);

  const animations = [
    { dur: "0.40s", delay: "0s" },
    { dur: "0.55s", delay: "-0.3s" },
    { dur: "0.50s", delay: "-0.1s" },
    { dur: "0.35s", delay: "-0.6s" },
    { dur: "0.45s", delay: "-0.7s" },
    { dur: "0.60s", delay: "-0.1s" },
    { dur: "0.80s", delay: "-0.4s" },
    { dur: "0.55s", delay: "-0.2s" },
    { dur: "0.65s", delay: "-0.5s" },
    { dur: "0.45s", delay: "-0.2s" },
    { dur: "0.55s", delay: "-0.4s" },
    { dur: "0.40s", delay: "-0.1s" },
  ];

  return (
    <div className="flex items-end justify-center gap-[3px] h-10 px-3 py-0.5">
      {bars.map((h, i) => {
        const anim = animations[i % animations.length];
        return (
          <div
            key={i}
            className={cn(
              "w-[3px] rounded-full transition-all duration-300",
              disabled
                ? "bg-[rgba(var(--foreground-muted),0.1)]"
                : "bg-gradient-to-t from-[rgba(var(--accent-dark),0.4)] to-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.2)]"
            )}
            style={{
              height: `${h}px`,
              animation: disabled ? "none" : `dynamic-eq ${anim.dur} ease-in-out infinite alternate ${anim.delay}`,
              transformOrigin: "bottom",
            }}
          />
        );
      })}
    </div>
  );
}

export function VoiceCarousel({
  voices,
  selected,
  onChange,
  disabled,
  onVoicesChanged,
  isAdding,
  setIsAdding,
}: {
  voices: { id: string; name: string; isCustom?: boolean }[];
  selected: string;
  onChange: (id: string) => void;
  disabled?: boolean;
  onVoicesChanged?: () => void;
  isAdding: boolean;
  setIsAdding: (val: boolean) => void;
}) {
  const index = voices.findIndex((v) => v.id === selected);
  const activeIndex = index === -1 ? 0 : index;
  const currentVoice = voices[activeIndex];

  const [activeTab, setActiveTab] = useState<"upload" | "record">("upload");
  const [newVoiceName, setNewVoiceName] = useState("");
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [cloningStatus, setCloningStatus] = useState<string | null>(null);

  const [isRecording, setIsRecording] = useState(false);
  const [recordingDuration, setRecordingDuration] = useState(0);
  const [recordedPcm, setRecordedPcm] = useState<number[] | null>(null);
  const [recordedSampleRate, setRecordedSampleRate] = useState<number>(0);
  const [recordingError, setRecordingError] = useState<string | null>(null);

  const cycle = (dir: number) => {
    if (disabled || voices.length === 0) return;
    const next = (activeIndex + dir + voices.length) % voices.length;
    onChange(voices[next].id);
  };

  const handleSelectFile = async () => {
    try {
      const file = await open({
        filters: [{ name: "Audio Files", extensions: ["wav", "mp3", "flac", "m4a", "aac"] }],
        multiple: false,
      });
      if (file) {
        setSelectedFile(typeof file === "string" ? file : (file as any).path || "");
        setActiveTab("upload");
      }
    } catch (e) {
      console.error(e);
    }
  };

  useEffect(() => {
    let interval: any = null;
    if (isRecording) {
      interval = setInterval(() => {
        setRecordingDuration((d) => {
          if (d >= 30) {
            handleStopRecording();
            return 30;
          }
          return d + 1;
        });
      }, 1000);
    }
    return () => {
      if (interval) clearInterval(interval);
    };
  }, [isRecording]);

  const handleStartRecording = async () => {
    setRecordingError(null);
    setRecordedPcm(null);
    setRecordedSampleRate(0);
    setRecordingDuration(0);
    setActiveTab("record");
    try {
      await startBackendRecording();
      setIsRecording(true);
    } catch (err) {
      console.error("Failed to start backend recording:", err);
      setRecordingError(String(err));
    }
  };

  const handleStopRecording = async () => {
    try {
      const [samples, sampleRate] = await stopBackendRecording();
      setRecordedPcm(samples);
      setRecordedSampleRate(sampleRate);
    } catch (err) {
      console.error("Failed to stop backend recording:", err);
      setRecordingError(String(err));
    } finally {
      setIsRecording(false);
    }
  };

  const resetAddingState = () => {
    setIsAdding(false);
    setSelectedFile(null);
    setNewVoiceName("");
    setCloningStatus(null);
    stopBackendRecording().catch(() => {});
    setIsRecording(false);
    setRecordedPcm(null);
    setRecordedSampleRate(0);
    setRecordingDuration(0);
    setRecordingError(null);
  };

  const handleCloneVoice = async () => {
    if (!newVoiceName.trim()) return;
    setCloningStatus("Cloning...");
    try {
      let entry;
      if (activeTab === "upload") {
        if (!selectedFile) return;
        entry = await addVoiceFromFile(newVoiceName.trim(), selectedFile);
      } else {
        if (!recordedPcm || recordedSampleRate === 0) return;
        entry = await addVoiceFromRecording(newVoiceName.trim(), recordedPcm, recordedSampleRate);
      }
      setCloningStatus(null);
      resetAddingState();
      if (onVoicesChanged) onVoicesChanged();
      onChange(entry.id);
    } catch (err) {
      setCloningStatus(String(err));
    }
  };

  const handleDeleteVoice = async (id: string) => {
    if (!confirm("Are you sure you want to delete this custom voice?")) return;
    try {
      await deleteVoice(id);
      if (onVoicesChanged) onVoicesChanged();
      onChange("default");
    } catch (e) {
      console.error(e);
    }
  };

  if (isAdding) {
    return (
      <div
        className={cn(
          "flex flex-col justify-between w-full h-full min-h-[160px] py-1 select-none",
          disabled && "opacity-50 pointer-events-none"
        )}
      >
        <div className="flex items-center gap-3 border-b border-[rgba(var(--border),0.12)] focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-1 mb-2 mt-2">
          <input
            type="text"
            value={newVoiceName}
            onChange={(e) => setNewVoiceName(e.target.value)}
            placeholder="Voice Name"
            className="flex-1 bg-transparent border-none outline-none text-[12px] py-1 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/30 font-bold"
          />

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={handleSelectFile}
              disabled={isRecording}
              className={cn(
                "p-2 rounded-lg transition-all duration-300 hover:bg-[rgb(var(--foreground))]/5",
                selectedFile
                  ? "text-emerald-400 bg-emerald-500/8"
                  : "text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))]"
              )}
              title={selectedFile ? `File Selected: ${selectedFile.split(/[/\\]/).pop()}` : "Choose WAV File"}
            >
              <Folder size={16} />
            </button>

            <button
              type="button"
              onClick={isRecording ? handleStopRecording : handleStartRecording}
              className={cn(
                "p-2 rounded-lg transition-all duration-300 relative",
                isRecording
                  ? "text-rose-400 bg-rose-500/12 hover:bg-rose-500/20"
                  : recordedPcm
                  ? "text-emerald-400 bg-emerald-500/8 hover:bg-emerald-500/15"
                  : "text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--foreground))]/5"
              )}
              title={isRecording ? "Stop Recording" : "Record Voice"}
            >
              {isRecording && (
                <span className="animate-ping absolute inline-flex h-2 w-2 rounded-full bg-rose-400 opacity-75 top-1 right-1"></span>
              )}
              <Mic size={16} />
            </button>
          </div>
        </div>

        <div className="flex-1 flex flex-col justify-center min-h-[40px] my-1">
          {isRecording && (
            <div className="text-[10px] text-rose-400 font-bold flex items-center justify-center gap-1.5 animate-pulse">
              <span className="w-1.5 h-1.5 rounded-full bg-rose-500"></span>
              Recording... {recordingDuration}s / 30s
            </div>
          )}
          {!isRecording && recordedPcm && (
            <div className="text-[10px] text-emerald-400 font-bold text-center">
              ✓ Audio recorded ({recordingDuration}s)
            </div>
          )}
          {!isRecording && selectedFile && (
            <div className="text-[10px] text-emerald-400 font-bold text-center max-w-full truncate px-2" title={selectedFile}>
              ✓ Selected: {selectedFile.split(/[/\\]/).pop()}
            </div>
          )}
          {!isRecording && recordedPcm && recordingDuration < 10 && (
            <div className="text-[9px] text-amber-400 font-medium text-center leading-tight">
              ⚠️ Too short ({recordingDuration}s). Minimum is 10s.
            </div>
          )}
          {recordingError && (
            <div className="text-[9px] text-rose-400 font-medium text-center leading-tight">
              {recordingError}
            </div>
          )}
          {cloningStatus && (
            <div className="text-[10px] text-amber-400 font-bold text-center leading-tight">
              {cloningStatus}
            </div>
          )}
        </div>

        <div className="flex gap-2 mt-2 shrink-0">
          <button
            type="button"
            onClick={resetAddingState}
            className="flex-1 py-2 rounded-xl text-[10px] font-black uppercase tracking-wider bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.08)] text-[rgb(var(--foreground-muted))]/80 hover:bg-[rgba(var(--foreground),0.05)] transition-all duration-300"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={handleCloneVoice}
            disabled={
              !newVoiceName.trim() ||
              (activeTab === "upload" && !selectedFile) ||
              (activeTab === "record" && (!recordedPcm || recordingDuration < 10)) ||
              cloningStatus === "Cloning..." ||
              isRecording
            }
            className="flex-[2] py-2 rounded-xl text-[10px] font-black uppercase tracking-wider bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:scale-[1.01] active:scale-95 disabled:opacity-40 disabled:pointer-events-none transition-all duration-300"
          >
            {cloningStatus === "Cloning..." ? "Processing..." : "Clone"}
          </button>
        </div>
      </div>
    );
  }

  return (
    <div
      className={cn(
        "relative flex flex-col justify-between w-full h-full min-h-[160px] py-1 select-none",
        disabled && "opacity-50 pointer-events-none"
      )}
    >
      {currentVoice?.isCustom && (
        <button
          onClick={() => handleDeleteVoice(currentVoice.id)}
          className="absolute top-0 right-0 p-1.5 rounded-lg text-rose-400 hover:bg-rose-500/10 hover:text-rose-300 transition-all duration-300 z-10"
          title="Delete Custom Voice"
        >
          <Trash2 size={15} />
        </button>
      )}

      <div className="text-center w-full mt-1.5 mb-3 shrink-0">
        <span className="text-[14px] font-black tracking-wide block text-[rgb(var(--foreground))]">
          {currentVoice?.name || "No Voice"}
        </span>
        <span className="text-[9px] block leading-normal mt-0.5 font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/40">
          {currentVoice?.isCustom ? "Custom Clone" : "System Preset"}
        </span>
      </div>

      <div className="flex-1 flex items-center justify-between gap-4 w-full px-2">
        <button
          type="button"
          onClick={() => cycle(-1)}
          disabled={disabled || voices.length <= 1}
          className="p-2 rounded-lg hover:bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-all duration-300 shrink-0 disabled:opacity-10"
          aria-label="Previous Voice"
        >
          <ChevronLeft size={20} />
        </button>

        <div className="flex-1 flex items-center justify-center min-w-0 h-12">
          <VoiceBars seed={currentVoice?.name || "default"} disabled={disabled} />
        </div>

        <button
          type="button"
          onClick={() => cycle(1)}
          disabled={disabled || voices.length <= 1}
          className="p-2 rounded-lg hover:bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-all duration-300 shrink-0 disabled:opacity-10"
          aria-label="Next Voice"
        >
          <ChevronRight size={20} />
        </button>
      </div>
    </div>
  );
}
