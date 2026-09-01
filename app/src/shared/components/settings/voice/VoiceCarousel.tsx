import { useState, useEffect, useMemo, memo } from "react";
import { cn } from "@/shared/lib/utils";
import { open } from "@tauri-apps/plugin-dialog";
import { Folder, Mic, Trash2, ChevronLeft, ChevronRight, Search, X, Sparkles } from "lucide-react";
import {
  startBackendRecording,
  stopBackendRecording,
  addVoiceFromFile,
  addVoiceFromRecording,
  renameVoice,
  deleteVoice,
} from "@/services/pipelineService";
import { Edit2, Check } from "lucide-react";
import { Tooltip } from "@/shared/ui/Tooltip";

export const VoiceBars = memo(function VoiceBars({ seed, disabled }: { seed: string; disabled?: boolean }) {
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
    <div className="flex items-end justify-center gap-[3px] h-9 px-2 py-0.5">
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
});

export interface VoiceCarouselProps {
  voices: { id: string; name: string; isCustom?: boolean }[];
  selected: string;
  onChange: (id: string) => void;
  disabled?: boolean;
  onVoicesChanged?: () => void;
  isAdding: boolean;
  setIsAdding: (val: boolean) => void;
  allowClone?: boolean;
}

export const VoiceCarousel = memo(function VoiceCarousel({
  voices,
  selected,
  onChange,
  disabled,
  onVoicesChanged,
  isAdding,
  setIsAdding,
  allowClone = false,
}: VoiceCarouselProps) {
  const [isSearching, setIsSearching] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");

  const filteredVoices = useMemo(() => {
    const q = searchQuery.trim().toLowerCase();
    if (!q) return voices;
    return voices.filter((v) =>
      v.name.toLowerCase().includes(q) || v.id.toLowerCase().includes(q)
    );
  }, [voices, searchQuery]);

  const activeList = filteredVoices.length > 0 ? filteredVoices : voices;
  const index = activeList.findIndex((v) => v.id === selected);
  const activeIndex = index === -1 ? 0 : index;
  const currentVoice = activeList[activeIndex] || activeList[0];

  const cycle = (dir: number) => {
    if (activeList.length <= 1) return;
    let nextIndex = activeIndex + dir;
    if (nextIndex < 0) nextIndex = activeList.length - 1;
    if (nextIndex >= activeList.length) nextIndex = 0;
    onChange(activeList[nextIndex].id);
  };

  // State for cloning
  const [activeTab, setActiveTab] = useState<"upload" | "record">("upload");
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [newVoiceName, setNewVoiceName] = useState("");
  const [cloningStatus, setCloningStatus] = useState<string | null>(null);

  // Recording State
  const [isRecording, setIsRecording] = useState(false);
  const [recordedPcm, setRecordedPcm] = useState<number[] | null>(null);
  const [recordedSampleRate, setRecordedSampleRate] = useState<number>(0);
  const [recordingDuration, setRecordingDuration] = useState(0);
  const [recordingError, setRecordingError] = useState<string | null>(null);

  const handleSelectFile = async () => {
    try {
      const file = await open({
        multiple: false,
        filters: [{ name: "Audio", extensions: ["wav"] }],
      });
      if (file && typeof file === "string") {
        setSelectedFile(file);
      }
    } catch (err) {
      console.error("Failed to select file:", err);
    }
  };

  useEffect(() => {
    let interval: any;
    if (isRecording) {
      interval = setInterval(() => {
        setRecordingDuration((prev) => {
          if (prev >= 30) {
            handleStopRecording();
            return 30;
          }
          return prev + 1;
        });
      }, 1000);
    } else {
      clearInterval(interval);
    }
    return () => clearInterval(interval);
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

  const [editingVoiceId, setEditingVoiceId] = useState<string | null>(null);
  const [editingVoiceName, setEditingVoiceName] = useState("");

  const handleStartRename = (id: string, currentName: string) => {
    setEditingVoiceId(id);
    setEditingVoiceName(currentName);
  };

  const handleSaveRename = async () => {
    if (!editingVoiceId || !editingVoiceName.trim()) {
      setEditingVoiceId(null);
      return;
    }
    try {
      await renameVoice(editingVoiceId, editingVoiceName.trim());
      if (onVoicesChanged) onVoicesChanged();
    } catch (e) {
      console.error("Failed to rename voice:", e);
    } finally {
      setEditingVoiceId(null);
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

  return (
    <div
      className={cn(
        "relative flex flex-col justify-center w-full h-full py-0.5 select-none",
        disabled && "opacity-50 pointer-events-none"
      )}
    >
      {isAdding ? (
        /* ─── Mode A: Clone Voice Form ─── */
        <div className="flex-1 flex flex-col justify-between w-full h-full animate-fade-in">
          <div className="flex items-center gap-2 border-b border-[rgba(var(--accent),0.25)] focus-within:border-[rgb(var(--accent))] pb-1 shrink-0 px-0.5">
            <input
              type="text"
              value={newVoiceName}
              autoFocus
              onChange={(e) => setNewVoiceName(e.target.value)}
              placeholder="Enter voice name..."
              className="flex-1 bg-transparent border-none outline-none text-[12.5px] py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 font-bold"
            />

            <div className="flex items-center gap-1 shrink-0">
              <Tooltip
                label={selectedFile ? `File Selected: ${selectedFile.split(/[/\\]/).pop()}` : "Choose WAV File"}
              >
                <button
                  type="button"
                  onClick={handleSelectFile}
                  disabled={isRecording}
                  className={cn(
                    "p-1 rounded-md transition-colors hover:bg-[rgb(var(--foreground))]/5 cursor-pointer",
                    selectedFile
                      ? "text-emerald-400 bg-emerald-500/10"
                      : "text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))]"
                  )}
                >
                  <Folder size={14} />
                </button>
              </Tooltip>

              <Tooltip label={isRecording ? "Stop Recording" : "Record Voice"}>
                <button
                  type="button"
                  onClick={isRecording ? handleStopRecording : handleStartRecording}
                  className={cn(
                    "p-1 rounded-md transition-colors relative cursor-pointer",
                    isRecording
                      ? "text-rose-400 bg-rose-500/15"
                      : recordedPcm
                      ? "text-emerald-400 bg-emerald-500/10"
                      : "text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgb(var(--foreground))]/5"
                  )}
                >
                  {isRecording && (
                    <span className="animate-ping absolute inline-flex h-2 w-2 rounded-full bg-rose-400 opacity-75 top-0.5 right-0.5"></span>
                  )}
                  <Mic size={14} />
                </button>
              </Tooltip>
            </div>
          </div>

          <div className="flex-1 flex flex-col justify-center min-h-[30px] my-0.5">
            {isRecording && (
              <div className="text-[11px] text-rose-400 font-bold flex items-center justify-center gap-1.5 animate-pulse">
                <span className="w-1.5 h-1.5 rounded-full bg-rose-500"></span>
                Recording... {recordingDuration}s / 30s
              </div>
            )}
            {!isRecording && recordedPcm && (
              <div className="text-[11px] text-emerald-400 font-bold text-center">
                ✓ Audio recorded ({recordingDuration}s)
              </div>
            )}
            {!isRecording && selectedFile && (
              <div className="text-[11px] text-emerald-400 font-bold text-center truncate">
                ✓ {selectedFile.split(/[/\\]/).pop()}
              </div>
            )}
            {recordingError && (
              <div className="text-[11px] text-rose-400 font-medium text-center leading-tight">
                {recordingError}
              </div>
            )}
            {cloningStatus && (
              <div className="text-[11px] text-amber-400 font-bold text-center leading-tight">
                {cloningStatus}
              </div>
            )}
          </div>

          <div className="flex gap-2 mt-1 shrink-0">
            <button
              type="button"
              onClick={resetAddingState}
              className="flex-1 py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.08)] text-[rgb(var(--foreground-muted))]/80 hover:bg-[rgba(var(--foreground),0.05)] transition-all cursor-pointer"
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
              className="flex-[2] py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:brightness-110 active:scale-95 disabled:opacity-40 disabled:pointer-events-none transition-all cursor-pointer"
            >
              {cloningStatus === "Cloning..." ? "Processing..." : "Clone Voice"}
            </button>
          </div>
        </div>
      ) : (
        /* ─── Mode B: Voice Carousel & Soundwave Display ─── */
        <div className="flex-1 flex flex-col justify-between w-full animate-fade-in">
          {/* Top Line: Search Icon + Voice Name or Inline Search Input */}
          <div className="relative w-full h-7 flex items-center justify-center my-0.5 shrink-0 px-1">
            {isSearching ? (
              <div className="flex items-center gap-1.5 w-full bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--accent),0.3)] rounded-lg px-2 py-0.5 animate-fade-in">
                <Search size={12} className="text-[rgb(var(--accent))] shrink-0" />
                <input
                  type="text"
                  value={searchQuery}
                  autoFocus
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search voice..."
                  className="flex-1 bg-transparent border-none outline-none text-[11.5px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/35"
                />
                {searchQuery && (
                  <button
                    type="button"
                    onClick={() => setSearchQuery("")}
                    className="text-[10px] font-bold text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] px-0.5 cursor-pointer"
                  >
                    Clear
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => {
                    setIsSearching(false);
                    setSearchQuery("");
                  }}
                  className="p-0.5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
                  aria-label="Close search"
                >
                  <X size={12} />
                </button>
              </div>
            ) : (
              <div className="flex items-center justify-center gap-1.5 w-full px-1">
                <Tooltip label="Search Voices">
                  <button
                    type="button"
                    onClick={() => setIsSearching(true)}
                    className="p-1 text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] rounded transition-all duration-150 cursor-pointer shrink-0"
                    aria-label="Search Voice"
                  >
                    <Search size={12} />
                  </button>
                </Tooltip>

                {editingVoiceId === currentVoice?.id ? (
                  <div className="flex items-center gap-1 flex-1 max-w-[140px] sm:max-w-[160px]">
                    <input
                      type="text"
                      autoFocus
                      value={editingVoiceName}
                      onChange={(e) => setEditingVoiceName(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") handleSaveRename();
                        if (e.key === "Escape") setEditingVoiceId(null);
                      }}
                      className="w-full bg-[rgba(var(--foreground),0.05)] border border-[rgba(var(--accent),0.4)] rounded px-1.5 py-0.5 text-[12px] font-bold text-[rgb(var(--foreground))] outline-none focus:border-[rgb(var(--accent))]"
                    />
                    <button
                      type="button"
                      onClick={handleSaveRename}
                      className="p-1 text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] rounded transition-colors cursor-pointer shrink-0"
                      aria-label="Save voice name"
                    >
                      <Check size={12} />
                    </button>
                    <button
                      type="button"
                      onClick={() => setEditingVoiceId(null)}
                      className="p-1 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] rounded transition-colors cursor-pointer shrink-0"
                      aria-label="Cancel rename"
                    >
                      <X size={12} />
                    </button>
                  </div>
                ) : (
                  <>
                    <span className="text-[13px] font-black tracking-wide text-[rgb(var(--foreground))] truncate text-center flex-1 max-w-[140px] sm:max-w-[160px]">
                      {currentVoice?.name || "No Voice"}
                    </span>

                    {currentVoice?.isCustom && (
                      <>
                        <Tooltip label="Rename Custom Voice">
                          <button
                            type="button"
                            onClick={() => handleStartRename(currentVoice.id, currentVoice.name)}
                            className="p-1 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:scale-110 transition-all duration-150 cursor-pointer shrink-0"
                            aria-label="Rename voice"
                          >
                            <Edit2 size={12} />
                          </button>
                        </Tooltip>

                        <Tooltip label="Delete Custom Voice">
                          <button
                            type="button"
                            onClick={() => handleDeleteVoice(currentVoice.id)}
                            className="p-1 text-rose-400 hover:text-rose-300 hover:scale-110 transition-all duration-150 cursor-pointer shrink-0"
                            aria-label="Delete voice"
                          >
                            <Trash2 size={12} />
                          </button>
                        </Tooltip>
                      </>
                    )}
                  </>
                )}

                {allowClone && (
                  <Tooltip label="Clone Voice Profile">
                    <button
                      type="button"
                      onClick={() => setIsAdding(true)}
                      className="p-1 text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] rounded transition-all duration-150 cursor-pointer shrink-0"
                    >
                      <Sparkles size={12} />
                    </button>
                  </Tooltip>
                )}
              </div>
            )}
          </div>

          {/* Bottom Line: Voice Carousel Soundwave with Left / Right Navigation */}
          <div className="flex-1 flex items-center justify-between gap-2 w-full px-1 min-h-[38px]">
            <button
              type="button"
              onClick={() => cycle(-1)}
              disabled={disabled || activeList.length <= 1}
              className="p-1.5 rounded-lg hover:bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-all duration-200 shrink-0 disabled:opacity-15 cursor-pointer"
              aria-label="Previous Voice"
            >
              <ChevronLeft size={16} />
            </button>

            <div className="flex-1 flex items-center justify-center min-w-0 h-9">
              <VoiceBars seed={currentVoice?.name || "default"} disabled={disabled} />
            </div>

            <button
              type="button"
              onClick={() => cycle(1)}
              disabled={disabled || activeList.length <= 1}
              className="p-1.5 rounded-lg hover:bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-all duration-200 shrink-0 disabled:opacity-15 cursor-pointer"
              aria-label="Next Voice"
            >
              <ChevronRight size={16} />
            </button>
          </div>
        </div>
      )}
    </div>
  );
});

VoiceCarousel.displayName = "VoiceCarousel";
