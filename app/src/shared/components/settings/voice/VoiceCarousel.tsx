import { useState, useEffect, useMemo, memo } from "react";
import { cn } from "@/shared/lib/utils";
import { open } from "@tauri-apps/plugin-dialog";
import { Folder, Mic, Trash2, ChevronLeft, ChevronRight, Search, X, Sparkles } from "lucide-react";
import {
  startBackendRecording,
  stopBackendRecording,
  addVoiceFromFile,
  addVoiceFromRecording,
  deleteVoice,
} from "@/services/pipelineService";
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
  showRegions?: boolean;
  selectedRegion?: string;
  onSelectRegion?: (region: string) => void;
  regions?: readonly string[];
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
  showRegions = false,
  selectedRegion = "ALL",
  onSelectRegion,
  regions = ["ALL", "US", "UK", "AU", "GLOBAL"],
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
  const currentVoice = activeList[activeIndex];

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
    if (disabled || activeList.length === 0) return;
    const next = (activeIndex + dir + activeList.length) % activeList.length;
    onChange(activeList[next].id);
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

  return (
    <div
      className={cn(
        "relative flex flex-col justify-between w-full h-[145px] py-0.5 select-none",
        disabled && "opacity-50 pointer-events-none"
      )}
    >
      {/* ─── Top Line: Header with Voice Library / Region Tabs & Action Icons ─── */}
      <div className="w-full flex items-center justify-between border-b border-[rgba(var(--border),0.08)] pb-1.5 mb-1.5 shrink-0 px-0.5 min-h-[28px]">
        {showRegions ? (
          <div className="flex items-center justify-between gap-1.5 w-full">
            {isSearching ? (
              <div className="flex items-center gap-1.5 w-full animate-fade-in">
                <Search size={13} className="text-[rgb(var(--accent))] shrink-0" />
                <input
                  type="text"
                  value={searchQuery}
                  autoFocus
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search edge voice..."
                  className="flex-1 bg-transparent border-none outline-none text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/35"
                />
                {searchQuery && (
                  <button
                    type="button"
                    onClick={() => setSearchQuery("")}
                    className="text-[10.5px] font-bold text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] px-1 cursor-pointer"
                  >
                    Clear
                  </button>
                )}
                <Tooltip label="Close Search">
                  <button
                    type="button"
                    onClick={() => {
                      setIsSearching(false);
                      setSearchQuery("");
                    }}
                    className="p-0.5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
                  >
                    <X size={13} />
                  </button>
                </Tooltip>
              </div>
            ) : (
              <>
                <div className="flex-1 grid grid-cols-5 gap-1 items-center">
                  {regions.map((region) => (
                    <button
                      key={region}
                      type="button"
                      onClick={() => onSelectRegion?.(region)}
                      className={cn(
                        "py-0.5 text-[10.5px] sm:text-[11px] font-bold uppercase tracking-wider transition-all duration-150 rounded cursor-pointer text-center",
                        selectedRegion === region
                          ? "text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/12 border border-[rgb(var(--accent))]/30 shadow-[0_0_8px_rgba(var(--accent),0.2)] font-black"
                          : "text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                      )}
                    >
                      {region}
                    </button>
                  ))}
                </div>

                <Tooltip label="Search Voices">
                  <button
                    type="button"
                    onClick={() => setIsSearching(true)}
                    className="p-1 text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] rounded transition-all duration-150 cursor-pointer shrink-0 ml-1"
                    aria-label="Search Voices"
                  >
                    <Search size={13} />
                  </button>
                </Tooltip>
              </>
            )}
          </div>
        ) : (
          <div className="flex items-center justify-between w-full">
            {isSearching ? (
              <div className="flex items-center gap-1.5 w-full animate-fade-in">
                <Search size={13} className="text-[rgb(var(--accent))] shrink-0" />
                <input
                  type="text"
                  value={searchQuery}
                  autoFocus
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search voice name..."
                  className="flex-1 bg-transparent border-none outline-none text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/35"
                />
                {searchQuery && (
                  <button
                    type="button"
                    onClick={() => setSearchQuery("")}
                    className="text-[10.5px] font-bold text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] px-1 cursor-pointer"
                  >
                    Clear
                  </button>
                )}
                <Tooltip label="Close Search">
                  <button
                    type="button"
                    onClick={() => {
                      setIsSearching(false);
                      setSearchQuery("");
                    }}
                    className="p-0.5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
                  >
                    <X size={13} />
                  </button>
                </Tooltip>
              </div>
            ) : (
              <>
                <span className="text-[11.5px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/75">
                  {isAdding ? "Clone Voice" : "Voice Library"}
                </span>

                <div className="flex items-center gap-1">
                  {!isAdding && (
                    <Tooltip label="Search Voices">
                      <button
                        type="button"
                        onClick={() => setIsSearching(true)}
                        className="p-1 text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] rounded transition-all duration-150 cursor-pointer shrink-0"
                        aria-label="Search Voices"
                      >
                        <Search size={13} />
                      </button>
                    </Tooltip>
                  )}

                  {allowClone && (
                    <Tooltip label={isAdding ? "Back to Presets" : "Clone Voice Profile"}>
                      <button
                        type="button"
                        onClick={() => {
                          if (isAdding) {
                            resetAddingState();
                          } else {
                            setIsAdding(true);
                          }
                        }}
                        className="flex items-center gap-1 text-[10.5px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] hover:brightness-125 px-1.5 py-0.5 rounded border border-[rgb(var(--accent))]/25 bg-[rgb(var(--accent))]/5 hover:bg-[rgb(var(--accent))]/10 transition-all cursor-pointer shrink-0 ml-0.5"
                      >
                        <Sparkles size={11} />
                        <span>{isAdding ? "Back" : "Clone"}</span>
                      </button>
                    </Tooltip>
                  )}
                </div>
              </>
            )}
          </div>
        )}
      </div>

      {isAdding ? (
        /* ─── Mode A: Clone Voice Form (Body only, Header preserved) ─── */
        <div className="flex-1 flex flex-col justify-between w-full h-full animate-fade-in">
          {/* Top Line: Input matching voice name position */}
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
              <Tooltip label={selectedFile} className="w-full">
                <div className="text-[11px] text-emerald-400 font-bold text-center max-w-full truncate px-2">
                  ✓ Selected: {selectedFile.split(/[/\\]/).pop()}
                </div>
              </Tooltip>
            )}
            {!isRecording && recordedPcm && recordingDuration < 10 && (
              <div className="text-[11px] text-amber-400 font-medium text-center leading-tight">
                ⚠️ Too short ({recordingDuration}s). Minimum is 10s.
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
              className="flex-1 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--border),0.08)] text-[rgb(var(--foreground-muted))]/80 hover:bg-[rgba(var(--foreground),0.05)] transition-all cursor-pointer"
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
              className="flex-[2] py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:brightness-110 active:scale-95 disabled:opacity-40 disabled:pointer-events-none transition-all cursor-pointer"
            >
              {cloningStatus === "Cloning..." ? "Processing..." : "Clone Voice"}
            </button>
          </div>
        </div>
      ) : (
        /* ─── Mode B: Voice Carousel & Soundwave Display ─── */
        <div className="flex-1 flex flex-col justify-between w-full animate-fade-in">
          {/* Middle Line: Voice Name Display */}
          <div className="relative w-full h-6 flex items-center justify-center my-0.5 shrink-0 px-1">
            <span className="text-[14px] font-black tracking-wide text-[rgb(var(--foreground))] truncate text-center flex-1 px-2">
              {currentVoice?.name || "No Voice"}
            </span>

            {currentVoice?.isCustom && (
              <Tooltip label="Delete Custom Voice">
                <button
                  onClick={() => handleDeleteVoice(currentVoice.id)}
                  className="p-1 text-rose-400 hover:text-rose-300 hover:scale-110 transition-all duration-150 cursor-pointer shrink-0 absolute right-1"
                >
                  <Trash2 size={13} />
                </button>
              </Tooltip>
            )}
          </div>

          {/* Bottom Line: Voice Carousel Soundwave with Left / Right Navigation */}
          <div className="flex-1 flex items-center justify-between gap-3 w-full px-1 min-h-[44px]">
            <button
              type="button"
              onClick={() => cycle(-1)}
              disabled={disabled || activeList.length <= 1}
              className="p-1.5 rounded-lg hover:bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-all duration-200 shrink-0 disabled:opacity-15 cursor-pointer"
              aria-label="Previous Voice"
            >
              <ChevronLeft size={18} />
            </button>

            <div className="flex-1 flex items-center justify-center min-w-0 h-10">
              <VoiceBars seed={currentVoice?.name || "default"} disabled={disabled} />
            </div>

            <button
              type="button"
              onClick={() => cycle(1)}
              disabled={disabled || activeList.length <= 1}
              className="p-1.5 rounded-lg hover:bg-[rgb(var(--foreground))]/5 text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-all duration-200 shrink-0 disabled:opacity-15 cursor-pointer"
              aria-label="Next Voice"
            >
              <ChevronRight size={18} />
            </button>
          </div>
        </div>
      )}
    </div>
  );
});

VoiceCarousel.displayName = "VoiceCarousel";
