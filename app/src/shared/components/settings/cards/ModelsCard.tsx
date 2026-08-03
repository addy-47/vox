import { useState, useEffect, useCallback, memo } from "react";
import { useSettings } from "@/shared/context/SettingsContext";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { 
  Brain, Volume2, Database, Trash2,
  Activity, Sparkles, Check, ArrowLeft,
  Download, RefreshCw, Info, AlertCircle, Network,
  ChevronLeft, ChevronRight, Loader2, Folder, Mic,
  Layers, Globe, AlertTriangle
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { LlmModelInfo, ModelCapabilities } from "@/store/settingsStore";



interface ModelStatus {
  step: 'idle' | 'downloading' | 'extracting' | 'verifying' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  bytesDownloaded: number;
  totalBytes: number;
  error?: string;
}

interface ModelEntry {
  id: string;
  path: string;
  size: number;
  required: boolean;
}

interface ModelGroup {
  id: string;
  name: string;
  category: string;
  version: string;
  files: ModelEntry[];
}

interface VoxManifest {
  models_version: string;
  release_notes?: string[];
  total_size_bytes: number;
  model_groups: ModelGroup[];
}

const pulseStyles = `
@keyframes premium-pulse-red {
  0%, 100% { border-color: rgba(239, 68, 68, 0.25); box-shadow: 0 0 4px rgba(239, 68, 68, 0.15); }
  50% { border-color: rgba(239, 68, 68, 0.75); box-shadow: 0 0 12px rgba(239, 68, 68, 0.4); }
}
@keyframes premium-pulse-purple {
  0%, 100% { border-color: rgba(168, 85, 247, 0.25); box-shadow: 0 0 4px rgba(168, 85, 247, 0.15); }
  50% { border-color: rgba(168, 85, 247, 0.75); box-shadow: 0 0 12px rgba(168, 85, 247, 0.4); }
}
@keyframes dynamic-eq {
  0% { transform: scaleY(0.15); }
  100% { transform: scaleY(1.0); }
}
.pulse-missing {
  animation: premium-pulse-red 2s infinite ease-in-out;
  border-width: 1px !important;
}
.pulse-update {
  animation: premium-pulse-purple 2s infinite ease-in-out;
  border-width: 1px !important;
}
.tooltip-container:hover .tooltip-content {
  display: block !important;
}
`;



function VoiceBars({ seed, disabled }: { seed: string; disabled?: boolean }) {
  const hash = Array.from(seed).reduce((acc, char) => acc + char.charCodeAt(0), 0);
  const bars = Array.from({ length: 16 }, (_, i) => {
    const val = ((hash * (i + 1)) % 25) + 10;
    return val;
  });

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

function VoiceCarousel({
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
  const index = voices.findIndex(v => v.id === selected);
  const activeIndex = index === -1 ? 0 : index;
  const currentVoice = voices[activeIndex];

  const [activeTab, setActiveTab] = useState<'upload' | 'record'>('upload');
  const [newVoiceName, setNewVoiceName] = useState("");
  const [selectedFile, setSelectedFile] = useState<string | null>(null);
  const [cloningStatus, setCloningStatus] = useState<string | null>(null);

  // Recording states
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

  // Recording counter/auto-stop
  useEffect(() => {
    let interval: any = null;
    if (isRecording) {
      interval = setInterval(() => {
        setRecordingDuration(d => {
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
      await invoke("start_backend_recording");
      setIsRecording(true);
    } catch (err) {
      console.error("Failed to start backend recording:", err);
      setRecordingError(String(err));
    }
  };

  const handleStopRecording = async () => {
    try {
      const [samples, sampleRate] = await invoke<[number[], number]>("stop_backend_recording");
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
    invoke("stop_backend_recording").catch(() => {});
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
        entry = await invoke<any>("add_voice_from_file", {
          name: newVoiceName.trim(),
          filePath: selectedFile,
        });
      } else {
        if (!recordedPcm || recordedSampleRate === 0) return;
        entry = await invoke<any>("add_voice_from_recording", {
          name: newVoiceName.trim(),
          pcmF32: recordedPcm,
          sampleRate: recordedSampleRate,
        });
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
      await invoke("delete_voice", { id });
      if (onVoicesChanged) onVoicesChanged();
      onChange("default");
    } catch (e) {
      console.error(e);
    }
  };

  if (isAdding) {
    return (
      <div className={cn(
        "flex flex-col justify-between w-full h-full min-h-[160px] py-1 select-none",
        disabled && "opacity-50 pointer-events-none"
      )}>
        {/* Underlined Name Input & Inline Triggers */}
        <div className="flex items-center gap-3 border-b border-[rgba(var(--border),0.12)] focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-1 mb-2 mt-2">
          <input
            type="text"
            value={newVoiceName}
            onChange={(e) => setNewVoiceName(e.target.value)}
            placeholder="Voice Name"
            className="flex-1 bg-transparent border-none outline-none text-[12px] py-1 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/30 font-bold"
          />
          
          <div className="flex items-center gap-1.5 shrink-0">
            {/* Folder Icon (Upload File) */}
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

            {/* Mic Icon (Record Mic) */}
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

        {/* Dynamic Status Display */}
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

        {/* Action buttons */}
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
              (activeTab === 'upload' && !selectedFile) ||
              (activeTab === 'record' && (!recordedPcm || recordingDuration < 10)) ||
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
    <div className={cn(
      "relative flex flex-col justify-between w-full h-full min-h-[160px] py-1 select-none",
      disabled && "opacity-50 pointer-events-none"
    )}>
      {/* Delete / Dustbin icon on top right for custom voices */}
      {currentVoice?.isCustom && (
        <button
          onClick={() => handleDeleteVoice(currentVoice.id)}
          className="absolute top-0 right-0 p-1.5 rounded-lg text-rose-400 hover:bg-rose-500/10 hover:text-rose-300 transition-all duration-300 z-10"
          title="Delete Custom Voice"
        >
          <Trash2 size={15} />
        </button>
      )}

      {/* Voice Name on Top */}
      <div className="text-center w-full mt-1.5 mb-3 shrink-0">
        <span className="text-[14px] font-black tracking-wide block text-[rgb(var(--foreground))]">
          {currentVoice?.name || "No Voice"}
        </span>
        <span className="text-[9px] block leading-normal mt-0.5 font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]/40">
          {currentVoice?.isCustom ? "Custom Clone" : "System Preset"}
        </span>
      </div>

      {/* Chevrons with Waveform in the center */}
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

        {/* Bigger Center Waveform */}
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

interface SubModelCardProps {
  id: string;
  name: string;
  description: string;
  parameters: string;
  ramUsage?: string;
  tradeoffs?: string;
  isDownloaded: boolean;
  isActive: boolean;
  isRequired: boolean;
  layoutMode: "full-max" | "full-min" | "small";
  onSelect: () => void;
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  downloadStatus?: { step: string; progress: number };
  startDownload: () => void;
  deleteModel: () => void;
  showTooltip?: boolean;
}

const SubModelCard: React.FC<SubModelCardProps> = ({
  id,
  name,
  description,
  parameters,
  ramUsage,
  tradeoffs,
  isDownloaded,
  isActive,
  isRequired,
  layoutMode,
  onSelect,
  confirmDeleteId,
  setConfirmDeleteId,
  downloadStatus,
  startDownload,
  deleteModel,
  showTooltip = false,
}) => {
  const isConfirmingDelete = confirmDeleteId === id;

  const renderAction = () => {
    if (!isDownloaded) {
      if (downloadStatus) {
        return (
          <span className="text-[11px] font-mono text-[rgb(var(--accent))] font-bold shrink-0">
            {Math.round(downloadStatus.progress)}%
          </span>
        );
      }
      return (
        <button
          onClick={(e) => {
            e.stopPropagation();
            startDownload();
          }}
          className="px-2.5 py-1 rounded bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] text-[11px] font-bold uppercase tracking-wider shrink-0 hover:scale-[1.02] active:scale-95 transition-all"
        >
          Get
        </button>
      );
    }

    if (isRequired) return null;

    if (isConfirmingDelete) {
      return (
        <div className="flex items-center gap-1 transition-all duration-300 shrink-0">
          <span className="text-[10px] text-red-500 font-bold uppercase tracking-wider mr-0.5">Delete?</span>
          <button
            onClick={(e) => {
              e.stopPropagation();
              deleteModel();
              setConfirmDeleteId(null);
            }}
            className="p-1 rounded-lg bg-red-500/20 text-red-500 hover:bg-red-500/35 transition-colors border border-red-500/30 flex items-center justify-center"
            aria-label="Confirm Delete"
          >
            <Check size={14} className="font-bold" />
          </button>
          <button
            onClick={(e) => {
              e.stopPropagation();
              setConfirmDeleteId(null);
            }}
            className="p-1 rounded-lg bg-[rgb(var(--foreground))]/[0.05] text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--foreground))] hover:bg-[rgb(var(--foreground))]/[0.08] transition-colors border border-[rgba(var(--border),0.1)] flex items-center justify-center"
            aria-label="Cancel"
          >
            <ArrowLeft size={14} />
          </button>
        </div>
      );
    }

    return (
      <button
        onClick={(e) => {
          e.stopPropagation();
          setConfirmDeleteId(id);
        }}
        className="p-1.5 rounded-lg bg-red-500/10 text-red-500 border border-red-500/20 hover:bg-red-500/20 hover:border-red-500/30 transition-colors shrink-0"
        aria-label="Delete weights"
      >
        <Trash2 size={16} />
      </button>
    );
  };

  const hasTooltip = showTooltip && !!(description || parameters || ramUsage || tradeoffs);

  return (
    <div
      onClick={() => {
        if (isDownloaded && !isActive) {
          onSelect();
        }
      }}
      className={cn(
        "p-4 rounded-lg border transition-all duration-300 flex flex-col justify-between gap-2.5 glass min-h-[105px]",
        isDownloaded && !isActive && "cursor-pointer hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--accent),0.02)]",
        isActive && "border-[rgb(var(--accent))] bg-[rgb(var(--accent))]/5"
      )}
    >
      {/* Top Section */}
      <div className="space-y-0.5">
        <div className="flex items-start justify-between gap-2">
          <span className={cn("text-[12px] font-bold text-[rgb(var(--foreground))]", layoutMode === "small" ? "" : "truncate max-w-[170px]")} title={name}>
            {name}
          </span>
          
          {hasTooltip && (
            <div className="relative tooltip-container inline-block shrink-0 mt-0.5">
              <Info size={16} className="text-[rgb(var(--foreground-muted))]/70 hover:text-[rgb(var(--accent))] transition-colors cursor-help" />
              <div className="absolute right-full top-0 mr-2 hidden tooltip-content w-52 p-2.5 rounded-lg bg-[rgb(var(--background))]/95 border border-[rgba(var(--accent),0.25)] text-[11px] text-[rgb(var(--foreground-muted))]/80 shadow-2xl leading-normal z-50">
                <div className="space-y-1">
                  <div className="flex justify-between border-b border-[rgba(var(--accent),0.06)] pb-0.5 mb-1 font-bold">
                    <span className="text-[9px] text-[rgb(var(--accent))] uppercase tracking-wider">Specs</span>
                    <span className="font-mono text-[9px] text-[rgb(var(--foreground-muted))]/60">{parameters}</span>
                  </div>
                  {description && <div className="text-[10px] text-[rgb(var(--foreground))]/80 leading-normal mb-1">{description}</div>}
                  {ramUsage && (
                    <div className="text-[9px] text-[rgb(var(--foreground-muted))]/70 font-mono">
                      RAM: {ramUsage}
                    </div>
                  )}
                  {tradeoffs && (
                    <div className="text-[9px] text-[rgb(var(--foreground-muted))]/70 italic border-t border-[rgba(var(--accent),0.06)] pt-1 mt-1 leading-normal">
                      {tradeoffs}
                    </div>
                  )}
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Subtext metadata */}
        {description && (
          !showTooltip ? (
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 leading-normal">
              {description}
              {ramUsage && ` · RAM: ${ramUsage}`}
              {parameters && ` · ${parameters}`}
            </p>
          ) : (
            <p className="text-[11px] text-[rgb(var(--foreground-muted))]/70 leading-normal line-clamp-2">
              {description}
            </p>
          )
        )}
      </div>

      {/* Bottom Section */}
      <div className="flex items-center justify-between pt-1.5 border-t border-[rgba(var(--border),0.05)] h-6 shrink-0">
        <span className={cn(
          "text-[11px] font-bold uppercase tracking-wider",
          isActive ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/70"
        )}>
          {isActive ? "Active" : "Ready"}
        </span>
        {renderAction()}
      </div>
    </div>
  );
};

interface ModelsCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

export const ModelsCard = memo(({ layoutMode = "full-max" }: ModelsCardProps) => {
  const { settings, draftSettings, updateDraft, modelCatalog } = useSettings();
  const [downloadStatuses, setDownloadStatuses] = useState<Record<string, ModelStatus>>({});
  const [modelPresence, setModelPresence] = useState<Record<string, boolean>>({});
  const [activePipelineTab, setActivePipelineTab] = useState<"vad" | "asr" | "llm" | "tts" | "auxiliary">("llm");
  const [activeCategoryTab, setActiveCategoryTab] = useState<"model" | "settings">("model");
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);
  const [outdatedModels, setOutdatedModels] = useState<string[]>([]);
  const [manifest, setManifest] = useState<VoxManifest | null>(null);

  interface CustomVoice {
    id: string;
    name: string;
    source_kind: string;
    has_preview: boolean;
    created_at: number;
  }

  const [customVoices, setCustomVoices] = useState<CustomVoice[]>([]);
  const [chatterboxIsAdding, setChatterboxIsAdding] = useState(false);

  const loadCustomVoices = useCallback(async () => {
    try {
      const list = await invoke<CustomVoice[]>("list_voices");
      setCustomVoices(list);
    } catch (e) {
      console.error("Failed to list voices", e);
    }
  }, []);

  useEffect(() => {
    if (activePipelineTab === "tts") {
      loadCustomVoices();
    }
  }, [activePipelineTab, loadCustomVoices]);

  // Chatterbox Remote setup & health states
  const [isRemoteTtsHealthy, setIsRemoteTtsHealthy] = useState<boolean | null>(null);
  const [checkingTtsHealth, setCheckingTtsHealth] = useState(false);
  const [sshConnectionString, setSshConnectionString] = useState(() => localStorage.getItem("vox_ssh_conn") || "root@localhost");
  const [sshPort, setSshPort] = useState(() => localStorage.getItem("vox_ssh_port") || "22");
  const [sshIdentityKey, setSshIdentityKey] = useState(() => localStorage.getItem("vox_ssh_key") || "~/.ssh/id_rsa");
  const [setupStatus, setSetupStatus] = useState<any>(null);

  const [edgeTtsVoices, setEdgeTtsVoices] = useState<Array<{ name: string; short_name: string; gender: string; locale: string; friendly_name: string }>>([]);
  const [edgeTtsError, setEdgeTtsError] = useState<string | null>(null);
  const [loadingEdgeVoices, setLoadingEdgeVoices] = useState<boolean>(false);

  const loadEdgeVoices = useCallback(async () => {
    setLoadingEdgeVoices(true);
    setEdgeTtsError(null);
    try {
      const list = await invoke<any[]>("fetch_edge_tts_voices");
      setEdgeTtsVoices(list);
    } catch (err: any) {
      console.error("Failed to fetch Edge TTS voices:", err);
      setEdgeTtsError(String(err));
    } finally {
      setLoadingEdgeVoices(false);
    }
  }, []);

  useEffect(() => {
    if (draftSettings?.tts?.provider?.kind === "edge_tts" && edgeTtsVoices.length === 0 && !loadingEdgeVoices) {
      loadEdgeVoices();
    }
  }, [draftSettings?.tts?.provider?.kind, edgeTtsVoices.length, loadingEdgeVoices, loadEdgeVoices]);

  useEffect(() => {
    if (draftSettings?.tts?.provider?.kind !== "chatterbox_remote") {
      setIsRemoteTtsHealthy(null);
      return;
    }

    const checkHealth = async () => {
      if (!draftSettings?.tts?.provider) return;
      setCheckingTtsHealth(true);
      try {
        const healthy = await invoke<boolean>("check_tts_provider_health", {
          provider: draftSettings.tts.provider
        });
        setIsRemoteTtsHealthy(healthy);
      } catch (_) {
        setIsRemoteTtsHealthy(false);
      } finally {
        setCheckingTtsHealth(false);
      }
    };

    checkHealth();
    const interval = setInterval(checkHealth, 5000);
    return () => clearInterval(interval);
  }, [draftSettings?.tts?.provider]);

  useEffect(() => {
    const unlistenPromise = listen<any>("remote_setup_status", (event) => {
      setSetupStatus(event.payload);
      if (event.payload?.step === "complete" && draftSettings?.tts?.provider) {
        invoke<boolean>("check_tts_provider_health", {
          provider: draftSettings.tts.provider
        }).then(healthy => setIsRemoteTtsHealthy(healthy));
      }
    });
    return () => {
      unlistenPromise.then(fn => fn());
    };
  }, [draftSettings?.tts?.provider]);

  useEffect(() => {
    localStorage.setItem("vox_ssh_conn", sshConnectionString);
  }, [sshConnectionString]);

  useEffect(() => {
    localStorage.setItem("vox_ssh_port", sshPort);
  }, [sshPort]);

  useEffect(() => {
    localStorage.setItem("vox_ssh_key", sshIdentityKey);
  }, [sshIdentityKey]);

  useEffect(() => {
    const handleSync = (e: Event) => {
      const tab = (e as CustomEvent).detail;
      if (tab === "stt") {
        setActivePipelineTab("asr");
      } else if (tab === "llm" || tab === "tts") {
        setActivePipelineTab(tab);
      }
    };
    window.addEventListener("sync_pipeline_tab", handleSync);
    return () => window.removeEventListener("sync_pipeline_tab", handleSync);
  }, []);

  useEffect(() => {
    let cat = "";
    if (activePipelineTab === "asr") cat = "STT";
    else if (activePipelineTab === "llm") cat = "LLM";
    else if (activePipelineTab === "tts") cat = "TTS";
    if (cat) {
      const event = new CustomEvent("sync_interaction_category", { detail: cat });
      window.dispatchEvent(event);
    }
  }, [activePipelineTab]);

  const triggerRemoteSetup = async () => {
    if (!draftSettings?.tts?.provider) return;
    setSetupStatus({ progress: 10, step: "initiating", log_line: "Starting connection..." });
    try {
      const endpoint = draftSettings.tts.provider.kind === "chatterbox_remote" 
        ? draftSettings.tts.provider.endpoint 
        : "http://127.0.0.1:7860";
      const remotePath = draftSettings.tts.provider.kind === "chatterbox_remote"
        ? draftSettings.tts.provider.remote_path
        : "~/.vox";

      let srvPort = 7860;
      try {
        const urlObj = new URL(endpoint);
        srvPort = urlObj.port ? parseInt(urlObj.port) : 7860;
      } catch (_) {
        const parts = (endpoint || "").replace("http://", "").replace("https://", "").split(":");
        if (parts.length > 1) {
          srvPort = parseInt(parts[parts.length - 1]) || 7860;
        }
      }
      
      await invoke("setup_remote_server", {
        connectionString: sshConnectionString,
        sshPort: sshPort ? parseInt(sshPort) : null,
        identityKeyPath: sshIdentityKey || null,
        remotePath: remotePath || "~/.vox",
        serverPort: srvPort
      });
    } catch (err) {
      setSetupStatus({ progress: 0, step: "failed", log_line: `Error: ${err}`, error: String(err) });
    }
  };

  // Remote LLM models catalog live state
  const [remoteModels, setRemoteModels] = useState<LlmModelInfo[]>([]);
  const [loadingRemoteModels, setLoadingRemoteModels] = useState(false);
  const [remoteModelsError, setRemoteModelsError] = useState<string | null>(null);
  const [probingMap, setProbingMap] = useState<Record<string, { status: 'idle' | 'testing' | 'success' | 'error'; capabilities?: ModelCapabilities; error?: string }>>({});

  // Fix: Base layout decisions on committed settings to prevent uncommitted leaks
  const savedProvider = settings?.llm?.provider;
  const isRemoteLlm = savedProvider?.kind === "open_ai_compat";
  const provider = (draftSettings?.llm?.provider?.kind === savedProvider?.kind)
    ? draftSettings?.llm?.provider
    : savedProvider;

  const handleProbeCapabilities = useCallback(async (modelId?: string) => {
    if (!provider) return;
    const targetId = modelId || (provider.kind === "open_ai_compat" ? provider.model : "embedded");
    if (!targetId) return;

    setProbingMap(prev => ({
      ...prev,
      [targetId]: { status: 'testing' }
    }));

    try {
      const caps = await invoke<ModelCapabilities>("probe_model_capabilities", {
        provider,
        modelId: targetId
      });

      setProbingMap(prev => ({
        ...prev,
        [targetId]: { status: 'success', capabilities: caps }
      }));

      setRemoteModels(prev => prev.map(m => m.id === targetId ? { ...m, capabilities: caps } : m));
    } catch (err) {
      console.error("[CapabilityProbe] Failed to probe model:", err);
      setProbingMap(prev => ({
        ...prev,
        [targetId]: { status: 'error', error: String(err) }
      }));
    }
  }, [provider]);



  const [customModelId, setCustomModelId] = useState("");
  const [customModelStatus, setCustomModelStatus] = useState<'idle' | 'valid' | 'invalid' | 'checking'>('idle');

  const getFilteredModels = useCallback(() => {
    if (!provider || !provider.provider_name) return remoteModels;
    const name = provider.provider_name.toLowerCase();
    
    let filtered: LlmModelInfo[] = [];
    if (remoteModels && remoteModels.length > 0) {
      if (name.includes("openai")) {
        filtered = remoteModels.filter(m => 
          m.id.toLowerCase().includes("gpt") && 
          !m.id.toLowerCase().includes("instruct") && 
          !m.id.toLowerCase().includes("embedding") && 
          !m.id.toLowerCase().includes("audio")
        );
      } else if (name.includes("gemini") || name.includes("google")) {
        filtered = remoteModels.filter(m => 
          m.id.toLowerCase().includes("gemini") && 
          !m.id.toLowerCase().includes("embedding")
        );
      } else if (name.includes("anthropic")) {
        filtered = remoteModels.filter(m => 
          m.id.toLowerCase().includes("claude")
        );
      } else if (name.includes("nvidia")) {
        filtered = remoteModels.filter(m => 
          !m.id.toLowerCase().includes("embedding") && 
          !m.id.toLowerCase().includes("rerank") &&
          !m.id.toLowerCase().includes("clip") &&
          !m.id.toLowerCase().includes("guard")
        );
      } else if (name.includes("groq")) {
        filtered = remoteModels.filter(m => 
          (m.id.toLowerCase().includes("llama") || m.id.toLowerCase().includes("mixtral") || m.id.toLowerCase().includes("gemma")) && 
          !m.id.toLowerCase().includes("whisper")
        );
      } else {
        filtered = [...remoteModels];
      }

      // Sort: newer versions first (e.g. 2.5 > 2.0 > 1.5)
      filtered.sort((a, b) => {
        const aId = a.id.toLowerCase();
        const bId = b.id.toLowerCase();
        
        // Put experimental or preview models at the bottom
        const aExp = aId.includes("exp") || aId.includes("preview");
        const bExp = bId.includes("exp") || bId.includes("preview");
        if (aExp && !bExp) return 1;
        if (!aExp && bExp) return -1;

        // Compare numbers if present
        const aNum = parseFloat(aId.match(/\d+(\.\d+)?/)?.[0] || "0");
        const bNum = parseFloat(bId.match(/\d+(\.\d+)?/)?.[0] || "0");
        if (bNum !== aNum) {
          return bNum - aNum; // Higher version number first
        }
        return aId.localeCompare(bId);
      });

      // Strip models/ prefix from names
      return filtered.map(m => ({
        ...m,
        name: m.name.replace(/^models\//, "")
      })).slice(0, 4);
    }
    return [];
  }, [provider?.provider_name, remoteModels]);



  useEffect(() => {
    if (provider?.model) {
      const filtered = getFilteredModels();
      const isFiltered = filtered.some(m => m.id === provider.model);
      if (!isFiltered) {
        setCustomModelId(provider.model);
      } else {
        setCustomModelId("");
      }
    } else {
      setCustomModelId("");
    }
    setCustomModelStatus('idle');
  }, [provider?.base_url, provider?.provider_name, provider?.model, remoteModels, getFilteredModels]);

  const handleValidateCustomModel = useCallback(() => {
    if (!customModelId.trim() || !provider) return;
    setCustomModelStatus('checking');
    const modelToUse = customModelId.trim();
    
    const exists = remoteModels.some(m => m.id.toLowerCase() === modelToUse.toLowerCase());
    if (exists) {
      setCustomModelStatus('valid');
      updateDraft("llm", "provider", {
        ...provider,
        model: modelToUse
      });
    } else {
      if (remoteModels.length > 0) {
        setCustomModelStatus('invalid');
        updateDraft("llm", "provider", {
          ...provider,
          model: modelToUse
        });
      } else {
        setCustomModelStatus('valid');
        updateDraft("llm", "provider", {
          ...provider,
          model: modelToUse
        });
      }
    }
  }, [customModelId, provider, remoteModels, updateDraft]);

  useEffect(() => {
    if (activePipelineTab === "llm" && activeCategoryTab === "model" && isRemoteLlm && provider) {
      const fetchRemoteModels = async () => {
        setLoadingRemoteModels(true);
        setRemoteModelsError(null);
        try {
          const list = await invoke<LlmModelInfo[]>("list_llm_models", {
            provider
          });
          setRemoteModels(list);
        } catch (err) {
          console.error(err);
          setRemoteModelsError("Failed to fetch remote models list");
        } finally {
          setLoadingRemoteModels(false);
        }
      };
      fetchRemoteModels();
    }
  }, [activePipelineTab, activeCategoryTab, isRemoteLlm, provider?.base_url, provider?.api_key, provider?.provider_name]);

  const getGroupIdForFile = useCallback((fileId: string): string => {
    if (!manifest) {
      if (fileId.startsWith("vad")) return "ten_vad";
      if (fileId.startsWith("translit")) return "vox_translit_rnn";
      if (fileId.startsWith("stt_nemotron")) return "nvidia_nemotron";
      if (fileId.startsWith("stt_")) return "qwen3_asr";
      if (fileId.startsWith("tts_supertonic")) return "supertonic_tts";
      if (fileId.startsWith("tts_chatterbox")) return "chatterbox_tts";
      return fileId;
    }
    for (const group of manifest.model_groups) {
      if (group.id === fileId || group.files.some(f => f.id === fileId)) {
        return group.id;
      }
    }
    return fileId;
  }, [manifest]);

  const isGroupRequired = useCallback((groupId: string): boolean => {
    if (!manifest) return groupId === "ten_vad" || groupId === "vox_translit_rnn" || groupId === "qwen3_asr" || groupId === "nvidia_nemotron";
    const group = manifest.model_groups.find(g => g.id === groupId);
    return group ? group.files.some(f => f.required) : false;
  }, [manifest]);



  const checkOutdated = useCallback(async () => {
    try {
      const res = await invoke<any>("check_for_model_updates");
      if (res && res.update_available) {
        setOutdatedModels(res.outdated_models);
      } else {
        setOutdatedModels([]);
      }
    } catch (e) {
      console.warn("Failed to check outdated models:", e);
    }
  }, []);

  const checkPresence = useCallback(async () => {
    if (!modelCatalog || !draftSettings) return;
    const presence: Record<string, boolean> = {};

    checkOutdated();

    const groups = manifest?.model_groups || [];
    const checkIds = groups.length > 0 
      ? groups.map(g => g.id)
      : [
          "ten_vad",
          "vox_translit_rnn",
          "qwen3_asr",
          "nvidia_nemotron",
          "gemma_4_reasoning",
          "llama_3_2_reasoning",
          "gemma_4_uncensored",
          "supertonic_tts",
          "chatterbox_tts"
        ];

    for (const id of checkIds) {
      try {
        const exists = await invoke<boolean>("check_model_exists", { modelId: id });
        presence[id] = exists;
      } catch (err) {
        presence[id] = false;
      }
    }

    presence["earshot"] = true;
    presence["edge_tts"] = true;
    setModelPresence(presence);
  }, [modelCatalog, draftSettings, checkOutdated, manifest]);

  useEffect(() => {
    const loadManifest = async () => {
      try {
        const data = await invoke<VoxManifest>("fetch_manifest");
        setManifest(data);
      } catch (err) {
        console.error("Failed to fetch manifest:", err);
      }
    };
    loadManifest();
  }, []);

  useEffect(() => {
    setActiveCategoryTab("model");
  }, [activePipelineTab]);

  useEffect(() => {
    checkPresence();

    const unlistenStatus = listen<{
      model_id: string;
      step: string;
      progress: number;
      bytes_downloaded: number;
      total_bytes: number;
      error?: string;
    }>("model_setup_status", (event) => {
      const fileId = event.payload.model_id;
      const groupId = getGroupIdForFile(fileId);
      setDownloadStatuses(prev => ({
        ...prev,
        [groupId]: {
          step: event.payload.step as any,
          progress: event.payload.progress,
          bytesDownloaded: event.payload.bytes_downloaded,
          totalBytes: event.payload.total_bytes,
          error: event.payload.error
        }
      }));
    });

    const unlistenComplete = listen<string>("optional_model_complete", (event) => {
      checkPresence();
      setDownloadStatuses(prev => {
        const next = { ...prev };
        delete next[event.payload];
        return next;
      });
    });

    return () => {
      unlistenStatus.then(u => u());
      unlistenComplete.then(u => u());
    };
  }, [checkPresence, getGroupIdForFile]);

  if (!draftSettings || !modelCatalog) return null;

  const startDownload = (modelId: string) => {
    setDownloadStatuses(prev => ({
      ...prev,
      [modelId]: { step: 'idle', progress: 0, bytesDownloaded: 0, totalBytes: 0 }
    }));
    invoke("download_optional_model", { modelId });
  };

  const deleteModel = async (modelId: string) => {
    try {
      await invoke("delete_model", { modelId });
      checkPresence();
    } catch (err) {
      console.error("Failed to delete model:", err);
    }
  };

  const activeVadBackend = draftSettings.vad.vad_backend;
  const isVadVerified = activeVadBackend === "earshot" || modelPresence["ten_vad"];

  const selectedAsrId = draftSettings.asr.model;
  const isAsrVerified = modelPresence[selectedAsrId];

  const selectedLlmId = draftSettings.llm.model;
  const isLlmDownloaded = modelPresence[selectedLlmId];

  const isTtsVerified = modelPresence["supertonic_tts"] || modelPresence["chatterbox_tts"];

  const isVadCategoryMissing = activeVadBackend === "ten_vad" && !modelPresence["ten_vad"];
  const isAsrCategoryMissing = !modelPresence[selectedAsrId];
  const isLlmCategoryMissing = !modelPresence[selectedLlmId];
  const isTtsCategoryMissing = !modelPresence["supertonic_tts"] && !modelPresence["chatterbox_tts"];

  const hasVadUpdate = outdatedModels.includes("ten_vad");
  const hasAsrUpdate = outdatedModels.includes(selectedAsrId);
  const hasLlmUpdate = outdatedModels.includes(selectedLlmId);
  const hasTtsUpdate = outdatedModels.includes("supertonic_tts") || outdatedModels.includes("chatterbox_tts");

  const getPulseClass = (isMissing: boolean, hasUpdate: boolean) => {
    if (isMissing) return "pulse-missing border-red-500/35";
    if (hasUpdate) return "pulse-update border-purple-500/35";
    return "";
  };

  const renderOverlayIcon = (isMissing: boolean, hasUpdate: boolean) => {
    if (!isMissing && !hasUpdate) return null;
    const Icon = isMissing ? Download : RefreshCw;
    const colorClass = isMissing ? "text-[rgb(var(--accent))]/75 animate-bounce" : "text-[rgb(var(--accent))] animate-spin";
    return (
      <div className="absolute top-0.5 right-0.5 p-0.5 rounded-full bg-[rgba(var(--foreground),0.08)] dark:bg-[rgba(var(--foreground),0.2)] backdrop-blur-sm z-10">
        <Icon size={12} className={colorClass} style={{ animationDuration: isMissing ? "2s" : "4s" }} />
      </div>
    );
  };


  return (
    <div className={cn(
      "w-full h-auto flex flex-col text-[13px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none",
      layoutMode === "small"
        ? "bg-transparent p-0"
        : cn(
            "glass-card p-5",
            layoutMode === "full-min" ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]" : "lg:w-[520px]"
          )
    )}>
      <style>{pulseStyles}</style>
      <div className="flex flex-col gap-4">
        
        {/* Header */}
        {layoutMode === "small" ? (
          (activePipelineTab === "vad" || activePipelineTab === "llm" || activePipelineTab === "tts") && (() => {
            const isRemoteTtsSetupNotDone = activePipelineTab === "tts" &&
              draftSettings?.tts?.provider?.kind === "chatterbox_remote" &&
              isRemoteTtsHealthy !== true;

            if (isRemoteTtsSetupNotDone && activeCategoryTab === "settings") {
              setTimeout(() => setActiveCategoryTab("model"), 0);
            }

            return (
              <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
                <span className="text-[10px] font-semibold tracking-wider text-[rgb(var(--foreground-muted))]/70 uppercase">CATALOG VIEW</span>
                <div className="flex glass p-0.5 rounded-lg border border-[rgba(var(--accent),0.08)]">
                  <button
                    onClick={() => setActiveCategoryTab("model")}
                    className={cn(
                      "px-2 py-0.5 rounded-md text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                      activeCategoryTab === "model"
                        ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                        : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    Model
                  </button>
                  {!isRemoteTtsSetupNotDone && (
                    <button
                      onClick={() => setActiveCategoryTab("settings")}
                      className={cn(
                        "px-2 py-0.5 rounded-md text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                        activeCategoryTab === "settings"
                          ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                          : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
                      )}
                    >
                      Settings
                    </button>
                  )}
                </div>
              </div>
            );
          })()
        ) : (
          <div className="flex items-center justify-between mb-3 shrink-0 border-b border-[rgba(var(--accent),0.08)] pb-2 w-full">
            <div className="flex items-center gap-2">
              <Database className="text-[rgb(var(--accent))]" size={18} />
              <span className="text-[12px] font-black uppercase tracking-[0.22em] text-[rgb(var(--foreground))]">
                Model Hub
              </span>
            </div>
            {/* Small Category Tabs */}
            {(activePipelineTab === "vad" || activePipelineTab === "llm" || activePipelineTab === "tts") && (() => {
              const isRemoteTtsSetupNotDone = activePipelineTab === "tts" &&
                draftSettings?.tts?.provider?.kind === "chatterbox_remote" &&
                isRemoteTtsHealthy !== true;

              if (isRemoteTtsSetupNotDone && activeCategoryTab === "settings") {
                setTimeout(() => setActiveCategoryTab("model"), 0);
              }

              return (
                <div className="flex glass p-0.5 rounded-lg border border-[rgba(var(--accent),0.08)]">
                  <button
                    onClick={() => setActiveCategoryTab("model")}
                    className={cn(
                      "px-2 py-0.5 rounded-md text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                      activeCategoryTab === "model"
                        ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                        : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
                    )}
                  >
                    Model
                  </button>
                  {!isRemoteTtsSetupNotDone && (
                    <button
                      onClick={() => setActiveCategoryTab("settings")}
                      className={cn(
                        "px-2 py-0.5 rounded-md text-[10px] font-bold uppercase tracking-wider transition-all duration-300",
                        activeCategoryTab === "settings"
                          ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                          : "text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
                      )}
                    >
                      Settings
                    </button>
                  )}
                </div>
              );
            })()}
          </div>
        )}

        {/* Topology Pipeline Map */}
        <div className={cn(
          "gap-1 shrink-0 p-1 rounded-xl glass overflow-visible mb-1 bg-[rgba(var(--foreground),0.02)]",
          layoutMode === "small"
            ? "flex overflow-x-auto snap-x no-scrollbar scrollbar-none w-full scroll-smooth"
            : "grid grid-cols-5"
        )}>
          
          {/* NODE 1: VAD */}
          <button
            onClick={() => setActivePipelineTab("vad")}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden",
              activePipelineTab === "vad"
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              layoutMode === "small" && "min-w-[75px] snap-center flex-1 py-1.5 px-1",
              getPulseClass(isVadCategoryMissing, hasVadUpdate)
            )}
          >
            {renderOverlayIcon(isVadCategoryMissing, hasVadUpdate)}
            <Activity size={18} className={cn("transition-colors shrink-0", activePipelineTab === "vad" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">VAD</span>
            <span className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isVadVerified ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "bg-[rgb(var(--accent))]/30"
            )} />
          </button>

          {/* NODE 2: STT */}
          <button
            onClick={() => setActivePipelineTab("asr")}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden",
              activePipelineTab === "asr"
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              layoutMode === "small" && "min-w-[75px] snap-center flex-1 py-1.5 px-1",
              getPulseClass(isAsrCategoryMissing, hasAsrUpdate)
            )}
          >
            {renderOverlayIcon(isAsrCategoryMissing, hasAsrUpdate)}
            <Sparkles size={18} className={cn("transition-colors shrink-0", activePipelineTab === "asr" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">STT</span>
            <span className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isAsrVerified ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "bg-[rgb(var(--accent))]/30"
            )} />
          </button>

          {/* NODE 3: LLM */}
          <button
            onClick={() => setActivePipelineTab("llm")}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden",
              activePipelineTab === "llm"
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              layoutMode === "small" && "min-w-[75px] snap-center flex-1 py-1.5 px-1",
              getPulseClass(isLlmCategoryMissing, hasLlmUpdate)
            )}
          >
            {renderOverlayIcon(isLlmCategoryMissing, hasLlmUpdate)}
            <Brain size={18} className={cn("transition-colors shrink-0", activePipelineTab === "llm" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">LLM</span>
            <span className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isLlmDownloaded ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "bg-[rgb(var(--accent))]/30"
            )} />
          </button>

          {/* NODE 4: TTS */}
          <button
            onClick={() => setActivePipelineTab("tts")}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden",
              activePipelineTab === "tts"
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              layoutMode === "small" && "min-w-[75px] snap-center flex-1 py-1.5 px-1",
              getPulseClass(isTtsCategoryMissing, hasTtsUpdate)
            )}
          >
            {renderOverlayIcon(isTtsCategoryMissing, hasTtsUpdate)}
            <Volume2 size={18} className={cn("transition-colors shrink-0", activePipelineTab === "tts" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">TTS</span>
            <span className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              isTtsVerified ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]" : "bg-[rgb(var(--accent))]/30"
            )} />
          </button>

          {/* NODE 5: AUXILIARY */}
          <button
            onClick={() => setActivePipelineTab("auxiliary")}
            className={cn(
              "p-2 rounded-lg flex flex-col items-center justify-center gap-1.5 border text-center transition-all duration-300 relative group overflow-hidden",
              activePipelineTab === "auxiliary"
                ? "bg-[rgb(var(--accent))]/10 border-[rgb(var(--accent))] scale-[1.02]"
                : "bg-transparent border-transparent hover:bg-[rgb(var(--foreground))]/[0.03]",
              layoutMode === "small" && "min-w-[75px] snap-center flex-1 py-1.5 px-1"
            )}
          >
            <Layers size={18} className={cn("transition-colors shrink-0", activePipelineTab === "auxiliary" ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground-muted))]/80 group-hover:text-[rgb(var(--foreground))]")} />
            <span className="text-[11px] font-bold text-[rgb(var(--foreground))] uppercase tracking-wide">Auxiliary</span>
            <span className={cn(
              "w-1 h-1 rounded-full shrink-0 mt-0.5",
              (modelPresence["distilbert_query_classifier"] && modelPresence["minilm_l12_v2"] && modelPresence["deberta_v3_xsmall_nli"] && modelPresence["vox_translit_rnn"])
                ? "bg-[rgb(var(--accent))] shadow-[0_0_6px_rgba(var(--accent),0.8)]"
                : "bg-[rgb(var(--accent))]/30"
            )} />
          </button>

        </div>

        {/* Workspace Detail Panel */}
        <div className={cn(
          "h-auto w-full flex flex-col glass rounded-xl p-3 relative bg-[rgba(var(--foreground),0.02)]",
          layoutMode === "small" ? "max-h-none overflow-y-visible" : "max-h-[190px] overflow-y-auto custom-scrollbar"
        )}>
                   {/* TAB 1: SILENCE DETECTION (VAD) */}
          {activePipelineTab === "vad" && (
            <div className="space-y-3">
              {activeCategoryTab === "model" ? (
                <div className={cn("grid gap-3", layoutMode === "small" ? "grid-cols-1" : "grid-cols-2")}>
                  <SubModelCard
                    id="earshot"
                    name="Earshot (Built-in)"
                    description="Pure Rust voice detection. Embedded weights, runs instantly with zero CPU load."
                    parameters="Built-in"
                    ramUsage="0 MB"
                    isDownloaded={true}
                    isActive={activeVadBackend === "earshot"}
                    isRequired={true}
                    layoutMode={layoutMode}
                    onSelect={() => updateDraft("vad", "vad_backend", "earshot")}
                    confirmDeleteId={confirmDeleteId}
                    setConfirmDeleteId={setConfirmDeleteId}
                    startDownload={() => {}}
                    deleteModel={() => {}}
                  />
                  <SubModelCard
                    id="ten_vad"
                    name="TenVAD Engine"
                    description="ONNX-based voice detector. Requires downloading auxiliary neural files."
                    parameters="ONNX"
                    ramUsage="~2 MB"
                    isDownloaded={modelPresence["ten_vad"]}
                    isActive={activeVadBackend === "ten_vad"}
                    isRequired={false}
                    layoutMode={layoutMode}
                    onSelect={() => updateDraft("vad", "vad_backend", "ten_vad")}
                    confirmDeleteId={confirmDeleteId}
                    setConfirmDeleteId={setConfirmDeleteId}
                    downloadStatus={downloadStatuses["ten_vad"]}
                    startDownload={() => startDownload("ten_vad")}
                    deleteModel={() => deleteModel("ten_vad")}
                  />
                </div>
              ) : (
                /* VAD Settings */
                <div className="space-y-4 p-1">
                  <div className="space-y-2">
                    <span className="text-[12px] text-[rgb(var(--foreground))] font-bold block">Silence Threshold</span>
                    <div className="flex gap-1">
                      {[
                        { label: "Sensitive", value: 0.3 },
                        { label: "Balanced", value: 0.5 },
                        { label: "Conservative", value: 0.7 },
                        { label: "Aggressive", value: 0.9 },
                      ].map(({ label, value }) => (
                        <button key={value} onClick={() => updateDraft("vad", "threshold", value)}
                          className={cn(
                            "flex-1 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                            Math.abs(draftSettings.vad.threshold - value) < 0.01
                              ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                              : "glass text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
                          )}
                        >{label}</button>
                      ))}
                    </div>
                  </div>
                </div>
              )}
            </div>
          )}

          {/* TAB 2: VOICE RECOGNITION (ASR) — Local Embedded Only */}
          {activePipelineTab === "asr" && (
            <div className="space-y-3">
              {/* Embedded model cards */}
              <div className={cn("grid gap-2.5", layoutMode === "small" ? "grid-cols-1" : "grid-cols-2")}>
                {modelCatalog.asr.map((model) => {
                  const isSelected = draftSettings.asr.model === model.id;
                  const modelGroupId = model.id;
                  const isDownloaded = modelPresence[modelGroupId];
                  const status = downloadStatuses[modelGroupId];

                  return (
                    <SubModelCard
                      key={model.id}
                      id={modelGroupId}
                      name={model.name}
                      description={model.description}
                      parameters={model.parameters}
                      ramUsage={model.ram_usage}
                      tradeoffs={model.tradeoffs}
                      isDownloaded={isDownloaded}
                      isActive={isSelected}
                      isRequired={isGroupRequired(model.id)}
                      layoutMode={layoutMode}
                      onSelect={() => {
                        updateDraft("asr", "model", model.id);
                        updateDraft("asr", "provider", { kind: "embedded", model_type: model.id });
                      }}
                      confirmDeleteId={confirmDeleteId}
                      setConfirmDeleteId={setConfirmDeleteId}
                      downloadStatus={status}
                      startDownload={() => startDownload(modelGroupId)}
                      deleteModel={() => deleteModel(modelGroupId)}
                    />
                  );
                })}
              </div>
            </div>
          )}



          {/* TAB 4: AI REASONING (LLM) */}
          {activePipelineTab === "llm" && (
            <div className="space-y-4">
              {activeCategoryTab === "model" ? (
                isRemoteLlm ? (
                  /* Remote Models Picker Panel */
                  <div className="space-y-3 p-3 rounded-2xl bg-[rgba(var(--foreground),0.015)] border border-[rgba(var(--foreground),0.02)] hover:border-[rgba(var(--accent),0.1)] transition-all duration-300 w-full animate-fade-in">
                    <div className="flex items-center justify-between">
                      <div className="flex flex-col">
                        <span className="font-bold text-[rgb(var(--foreground))]/90 text-[12px] flex items-center gap-1.5">
                          <Network size={16} className="text-[rgb(var(--accent))]" /> Connected Server
                        </span>
                        <span className="text-[10px] text-[rgb(var(--foreground-muted))]/70 font-mono mt-0.5">
                          {provider?.base_url || "No server configured"}
                        </span>
                      </div>
                      {loadingRemoteModels ? (
                        <span className="text-[10px] font-bold text-[rgb(var(--accent))] flex items-center gap-1">
                          <RefreshCw size={14} className="animate-spin" /> Fetching...
                        </span>
                      ) : (
                        <span className="text-[10px] font-bold text-[rgb(var(--foreground-muted))]/60">
                          {remoteModels.length} models available
                        </span>
                      )}
                    </div>

                    {remoteModelsError && (
                      <div className="text-[11px] font-bold text-red-400/80 bg-red-400/5 border border-red-400/15 rounded-xl px-3 py-2 flex items-center gap-2">
                        <AlertCircle size={16} />
                        <span>{remoteModelsError}</span>
                      </div>
                    )}

                    <div className={cn(
                      "grid grid-cols-1 gap-2 pr-1",
                      layoutMode === "small" ? "max-h-none overflow-y-visible" : "max-h-[220px] overflow-y-auto"
                    )}>
                      {getFilteredModels().length === 0 ? (
                        <div className="text-center py-6 text-[11px] text-[rgb(var(--foreground-muted))]/70">
                          No remote models loaded. Ensure the server is online and configured in the Interaction Card.
                        </div>
                      ) : (
                        getFilteredModels().map((model) => {
                          const isSelected = provider?.model === model.id;
                          const probed = probingMap[model.id]?.capabilities || model.capabilities;
                          const isTesting = probingMap[model.id]?.status === 'testing';
                          const isGpu = probed?.is_gpu_accelerated;

                          return (
                            <button
                              key={model.id}
                              onClick={() => {
                                updateDraft("llm", "provider", {
                                  ...provider,
                                  model: model.id,
                                });
                                if (!probed && !isTesting) {
                                  handleProbeCapabilities(model.id);
                                }
                              }}
                              className={cn(
                                "w-full text-left p-3 rounded-xl border transition-all duration-300 flex items-center justify-between gap-3 relative overflow-hidden",
                                isGpu ? "border-purple-500/50 shadow-[0_0_12px_rgba(168,85,247,0.2)]" : "",
                                isSelected
                                  ? "bg-[rgba(var(--accent),0.05)] border-[rgb(var(--accent))]"
                                  : "bg-[rgba(var(--foreground),0.01)] border-[rgba(var(--foreground),0.04)] hover:border-[rgba(var(--accent),0.2)]"
                              )}
                            >
                              <div className="flex-1 space-y-1.5 min-w-0">
                                <div className="flex items-center gap-1.5 flex-wrap">
                                  <span className="font-bold text-[rgb(var(--foreground))]/90 text-[11px] truncate">
                                    {model.name}
                                  </span>
                                  {model.quantization && (
                                    <span className="text-[9px] font-bold font-mono px-1.5 py-0.5 rounded bg-[rgba(var(--foreground),0.05)] text-[rgb(var(--foreground))]/70 border border-[rgba(var(--foreground),0.04)] leading-none">
                                      {model.quantization}
                                    </span>
                                  )}
                                  {model.family && (
                                    <span className="text-[9px] font-bold px-1.5 py-0.5 rounded bg-[rgb(var(--accent))]/10 text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.08)] leading-none">
                                      {model.family}
                                    </span>
                                  )}
                                  {isGpu ? (
                                    <span title={probed?.gpu_status || "GPU Offloaded"} className="text-[9px] font-bold font-mono px-1.5 py-0.5 rounded bg-purple-500/15 text-purple-300 border border-purple-500/30 leading-none flex items-center gap-1">
                                      🚀 GPU {probed?.vram_bytes ? `(${(probed.vram_bytes / (1024 * 1024)).toFixed(0)}MB)` : ""}
                                    </span>
                                  ) : probed?.server_has_gpu ? (
                                    <span title="Server has GPU hardware, but model is running in CPU mode" className="text-[9px] font-bold font-mono px-1.5 py-0.5 rounded bg-amber-500/15 text-amber-300 border border-amber-500/30 leading-none flex items-center gap-1">
                                      ⚠️ GPU Server (CPU)
                                    </span>
                                  ) : null}
                                </div>

                                <div className="flex items-center gap-2 text-[10px] text-[rgb(var(--foreground-muted))]/70">
                                  <span className="font-mono truncate">{model.id}</span>
                                  {model.size_bytes !== null && model.size_bytes !== undefined && (
                                    <>
                                      <span>•</span>
                                      <span>{(model.size_bytes / (1024 * 1024 * 1024)).toFixed(2)} GB</span>
                                    </>
                                  )}
                                </div>

                                {/* Capability Badges & Readouts */}
                                <div className="flex items-center gap-1.5 flex-wrap pt-0.5">
                                  {isTesting ? (
                                    <span className="text-[9px] font-bold text-[rgb(var(--accent))] flex items-center gap-1">
                                      <Loader2 size={10} className="animate-spin" />
                                      Testing capabilities...
                                    </span>
                                  ) : probed ? (
                                    <>
                                      {probed.supports_tools && (
                                        <span title="Supports Tool Calling" className="text-[9px] font-bold px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-400 border border-blue-500/20 flex items-center gap-1">
                                          🛠️ Tools
                                        </span>
                                      )}
                                      {probed.supports_latin && (
                                        <span title="Latin Script (EN)" className="text-[9px] font-mono font-bold px-1.5 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
                                          EN
                                        </span>
                                      )}
                                      {probed.supports_devanagari && (
                                        <span title="Devanagari Script (Hindi/Hinglish)" className="text-[9px] font-mono font-bold px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400 border border-amber-500/20">
                                          DEV
                                        </span>
                                      )}
                                      {probed.context_window && (
                                        <span title="Context Window" className="text-[9px] font-mono px-1.5 py-0.5 rounded bg-zinc-800/60 text-zinc-300 border border-zinc-700/50">
                                          🧠 {probed.context_window >= 1000000 ? `${(probed.context_window / 1000000).toFixed(1)}M ctx` : `${Math.round(probed.context_window / 1024)}k ctx`}
                                        </span>
                                      )}
                                      {probed.tps && (
                                        <span title="Generation Speed" className="text-[9px] font-mono text-emerald-400 font-bold">
                                          ⚡ {probed.tps.toFixed(1)} tps
                                        </span>
                                      )}
                                    </>
                                  ) : (
                                    <button
                                      type="button"
                                      onClick={(e) => {
                                        e.stopPropagation();
                                        handleProbeCapabilities(model.id);
                                      }}
                                      className="text-[9px] font-bold text-[rgb(var(--accent))] hover:underline flex items-center gap-1"
                                    >
                                      <Sparkles size={10} /> Test Capabilities
                                    </button>
                                  )}
                                </div>
                              </div>

                              <div className="flex items-center gap-1.5 shrink-0 ml-auto">

                                {isSelected && (
                                  <div className="w-5 h-5 rounded-full bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] flex items-center justify-center">
                                    <Check size={16} strokeWidth={3} />
                                  </div>
                                )}
                              </div>
                            </button>
                          );
                        })
                      )}
                    </div>

                    {/* Custom Model ID field */}
                    <div className="mt-3 pt-3 border-t border-[rgba(var(--foreground),0.06)] space-y-2">
                      <span className="text-[10px] font-bold text-[rgb(var(--foreground-muted))]/80 uppercase tracking-wider block">
                        Use Custom Model ID
                      </span>
                      <div className="flex gap-2">
                        <div className="flex-1 border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                          <input
                            type="text"
                            value={customModelId}
                            onChange={(e) => {
                              setCustomModelId(e.target.value);
                              setCustomModelStatus('idle');
                            }}
                            placeholder="e.g. gemini-2.5-pro"
                            className="w-full bg-transparent border-none outline-none text-[11px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
                          />
                        </div>
                        <button
                          onClick={handleValidateCustomModel}
                          disabled={!customModelId.trim() || customModelStatus === 'checking'}
                          className={cn(
                            "px-3 py-1.5 rounded-lg text-[10px] font-bold uppercase tracking-wider transition-all border shrink-0",
                            customModelStatus === 'checking' && "bg-[rgba(var(--foreground),0.05)] border-[rgba(var(--border),0.1)] text-[rgb(var(--foreground-muted))]",
                            customModelStatus === 'valid' && "bg-emerald-500/10 border-emerald-500/20 text-emerald-400 hover:bg-emerald-500/20",
                            customModelStatus === 'invalid' && "bg-amber-500/10 border-amber-500/20 text-amber-400 hover:bg-amber-500/20",
                            customModelStatus === 'idle' && "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] border-[rgba(var(--accent),0.2)] hover:scale-[1.02] active:scale-95"
                          )}
                        >
                          {customModelStatus === 'checking' && "Checking..."}
                          {customModelStatus === 'valid' && "Valid ✓"}
                          {customModelStatus === 'invalid' && "Not Listed ⚠"}
                          {customModelStatus === 'idle' && "Validate & Use"}
                        </button>
                      </div>
                      {customModelStatus === 'invalid' && (
                        <div className="text-[9px] text-amber-400/80 leading-normal flex items-start gap-1">
                          <span>⚠</span>
                          <span>Model ID not in standard server list. Selected in draft anyway, but verify spelling.</span>
                        </div>
                      )}
                      {customModelStatus === 'valid' && (
                        <div className="text-[9px] text-emerald-400/80 leading-normal flex items-start gap-1">
                          <span>✓</span>
                          <span>Model verified successfully! Selected and ready to save.</span>
                        </div>
                      )}
                    </div>
                  </div>
                ) : (
                  /* Local GGUF Card Grid */
                  <div className={cn("grid gap-2.5", layoutMode === "small" ? "grid-cols-1" : "grid-cols-2")}>
                    {[...modelCatalog.llm].sort((a, b) => {
                      if (selectedLlmId === a.id) return -1;
                      if (selectedLlmId === b.id) return 1;
                      return 0;
                    }).map((model) => {
                      const isSelected = selectedLlmId === model.id;
                      const modelGroupId = model.id;
                      const isDownloaded = modelPresence[modelGroupId];
                      const status = downloadStatuses[modelGroupId];

                      return (
                        <SubModelCard
                          key={model.id}
                          id={modelGroupId}
                          name={model.name}
                          description={model.description}
                          parameters={model.parameters}
                          ramUsage={model.ram_usage}
                          tradeoffs={model.tradeoffs}
                          isDownloaded={isDownloaded}
                          isActive={isSelected}
                          isRequired={isGroupRequired(model.id)}
                          layoutMode={layoutMode}
                          onSelect={() => updateDraft("llm", "model", model.id)}
                          confirmDeleteId={confirmDeleteId}
                          setConfirmDeleteId={setConfirmDeleteId}
                          downloadStatus={status}
                          startDownload={() => startDownload(modelGroupId)}
                          deleteModel={() => deleteModel(modelGroupId)}
                          showTooltip={true}
                        />
                      );
                    })}
                  </div>
                )
              ) : (
                /* LLM Settings */
                <div className="space-y-4 p-1">
                  {/* Context Size */}
                  <div className="space-y-1.5">
                    <div className="flex items-center justify-between">
                      <span className="text-[12px] text-[rgb(var(--foreground))] font-bold">Memory Context Tokens</span>
                      <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.llm.ctx_size}</span>
                    </div>
                    <div className="flex gap-1">
                      {[512, 1024, 2048, 4096, 8192].map(val => (
                        <button key={val} onClick={() => updateDraft("llm", "ctx_size", val)}
                          className={cn(
                            "flex-1 py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                            draftSettings.llm.ctx_size === val
                              ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                              : "glass text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
                          )}
                        >{val < 1024 ? val : `${val / 1024}k`}</button>
                      ))}
                    </div>
                  </div>

                  {/* Threads */}
                  {(() => {
                    const totalCores = (typeof navigator !== 'undefined' ? navigator.hardwareConcurrency : undefined) || 4;
                    const maxSafe = Math.max(2, totalCores - 2);
                    const threadPresets = (() => {
                      const base = [2, 4];
                      if (maxSafe > 4 && maxSafe !== totalCores) return [...base, maxSafe, totalCores];
                      if (maxSafe > 4) return [...base, maxSafe];
                      return base;
                    })();
                    return (
                      <div className="space-y-1.5">
                        <div className="flex items-center justify-between">
                          <span className="text-[12px] text-[rgb(var(--foreground))] font-bold">Processor Threads</span>
                          <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.llm.threads}</span>
                        </div>
                        <div className="flex gap-1">
                          {threadPresets.map(val => (
                            <button key={val} onClick={() => updateDraft("llm", "threads", val)}
                              className={cn(
                                "flex-1 py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                                draftSettings.llm.threads === val
                                  ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                                  : "glass text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
                              )}
                            >{val}{val === maxSafe && val !== totalCores ? " (max)" : ""}{val === totalCores && val !== maxSafe ? " (all)" : ""}</button>
                          ))}
                        </div>
                      </div>
                    );
                  })()}
                </div>
              )}
            </div>
          )}

          {/* TAB 5: VOICE SYNTHESIS (TTS) */}
          {activePipelineTab === "tts" && (
            <div className="space-y-4">
              {activeCategoryTab === "model" ? (
                <div className="space-y-4">
                  {draftSettings.tts.provider?.kind === "chatterbox_remote" && isRemoteTtsHealthy !== true ? (
                    /* Show only setup panel + description banner */
                    <div className="space-y-4">
                      {/* Concise info box describing setup and model */}
                      <div className="border border-[rgba(var(--accent),0.15)] bg-[rgba(var(--accent),0.02)] rounded-xl p-4 space-y-2">
                        <div className="flex items-center gap-2 text-[rgb(var(--accent))]">
                          <Info size={16} />
                          <span className="font-bold text-[11px] uppercase tracking-[0.1em]">Chatterbox Remote Deployment</span>
                        </div>
                        <p className="text-[11px] text-[rgb(var(--foreground-muted))]/80 leading-relaxed font-medium">
                          Deploy Chatterbox on a remote CUDA-accelerated GPU host (e.g. RunPod, Vast.ai, or homelab) to offload memory-intensive flow-matching voice synthesis. Enter your SSH connection info below to automatically sync the codebase, download GGUF models, and run the server.
                        </p>
                      </div>

                      {/* Setup Panel */}
                      <div className="border border-[rgba(var(--accent),0.15)] bg-[rgba(var(--accent),0.02)] rounded-xl p-3 animate-fade-in space-y-3">
                        <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-1.5">
                          <span className="font-bold text-[11px] text-[rgb(var(--foreground))] flex items-center gap-1.5">
                            <Network size={14} className="text-[rgb(var(--accent))]" />
                            Setup Remote GPU Server (SSH Setup Required)
                          </span>
                          <span className="text-[9px] font-black uppercase bg-rose-500/10 text-rose-400 px-1.5 py-0.5 rounded border border-rose-500/20">
                            Offline / Unconfigured
                          </span>
                        </div>
                        
                        <div className="grid grid-cols-[2.5fr_1fr_2.5fr] gap-2.5">
                          <div className="space-y-1">
                            <label className="text-[9px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75">SSH Host / Profile</label>
                            <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                              <input
                                type="text"
                                value={sshConnectionString}
                                onChange={(e) => setSshConnectionString(e.target.value)}
                                placeholder="user@hostname"
                                className="w-full bg-transparent border-none outline-none text-[11px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
                              />
                            </div>
                          </div>
                          <div className="space-y-1">
                            <label className="text-[9px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75">SSH Port</label>
                            <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                              <input
                                type="text"
                                value={sshPort}
                                onChange={(e) => setSshPort(e.target.value)}
                                placeholder="22"
                                className="w-full bg-transparent border-none outline-none text-[11px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
                              />
                            </div>
                          </div>
                          <div className="space-y-1">
                            <label className="text-[9px] uppercase font-bold text-[rgb(var(--foreground-muted))]/75">Identity Key Path</label>
                            <div className="border-b border-[rgba(var(--border),0.12)] focus-within:border-b-2 focus-within:border-[rgb(var(--accent))] transition-all duration-300 pb-0.5">
                              <input
                                type="text"
                                value={sshIdentityKey}
                                onChange={(e) => setSshIdentityKey(e.target.value)}
                                placeholder="~/.ssh/id_rsa"
                                className="w-full bg-transparent border-none outline-none text-[11px] font-mono py-0.5 text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/25"
                              />
                            </div>
                          </div>
                        </div>

                        <div className="flex items-center justify-between gap-3 pt-1 border-t border-[rgba(var(--border),0.04)]">
                          <div className="flex-1 min-w-0">
                            {setupStatus ? (
                              <div className="space-y-1">
                                <div className="flex items-center justify-between text-[10px]">
                                  <span className={cn(
                                    "font-bold uppercase tracking-wider",
                                    setupStatus.step === "complete" ? "text-emerald-400" : setupStatus.step === "failed" ? "text-rose-400 animate-pulse" : "text-[rgb(var(--accent))]"
                                  )}>
                                    {setupStatus.step === "complete" ? "Ready" : setupStatus.step === "failed" ? "Setup Failed" : `Phase: ${setupStatus.step}`}
                                  </span>
                                  <span className="font-mono font-bold text-[rgb(var(--foreground-muted))]/70">{setupStatus.progress}%</span>
                                </div>
                                <div className="h-1 bg-[rgba(var(--foreground),0.04)] rounded-full overflow-hidden relative">
                                  <div 
                                    className="h-full bg-[rgb(var(--accent))] transition-all duration-300 rounded-full"
                                    style={{ width: `${setupStatus.progress}%` }}
                                  />
                                </div>
                                <p className="text-[9px] text-[rgb(var(--foreground-muted))]/60 font-semibold truncate leading-none mt-1">
                                  {setupStatus.log_line}
                                </p>
                              </div>
                            ) : (
                              <p className="text-[9px] text-[rgb(var(--foreground-muted))]/55 font-semibold leading-normal">
                                Pipes setup_server.sh to the host over native SSH. Key auth / SSH configs supported.
                              </p>
                            )}
                          </div>

                          <button
                            type="button"
                            onClick={triggerRemoteSetup}
                            disabled={!sshConnectionString || (setupStatus && setupStatus.progress > 0 && setupStatus.progress < 100 && setupStatus.step !== "failed")}
                            className={cn(
                              "px-3 py-1.5 rounded-lg text-[10px] font-black uppercase tracking-wider transition-all duration-300 flex items-center gap-1.5 shrink-0 select-none outline-none",
                              !sshConnectionString
                                ? "bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/30 border border-transparent cursor-not-allowed"
                                : (setupStatus && setupStatus.progress > 0 && setupStatus.progress < 100 && setupStatus.step !== "failed")
                                  ? "bg-[rgba(var(--accent),0.05)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.15)] cursor-wait"
                                  : "bg-[rgb(var(--accent))]/10 border border-[rgb(var(--accent))]/25 text-[rgb(var(--accent))] hover:bg-[rgb(var(--accent))]/20 hover:border-[rgb(var(--accent))]/40 active:scale-95"
                            )}
                          >
                            {setupStatus && setupStatus.progress > 0 && setupStatus.progress < 100 && setupStatus.step !== "failed" ? (
                              <>
                                <Loader2 size={11} className="animate-spin text-[rgb(var(--accent))]" />
                                Deploying
                              </>
                            ) : setupStatus?.step === "failed" ? (
                              "Retry Deploy"
                            ) : setupStatus?.step === "complete" ? (
                              <>
                                <Check size={11} className="text-emerald-400" />
                                Deployed
                              </>
                            ) : (
                              "Deploy Server"
                            )}
                          </button>
                        </div>
                      </div>
                    </div>
                  ) : (
                    /* Show only selected filter models list */
                    <div className={cn("grid gap-3", layoutMode === "small" ? "grid-cols-1" : "grid-cols-2")}>
                      {(modelCatalog?.tts || [])
                        .filter((model) => {
                          const isRemoteConfig = draftSettings.tts.provider?.kind === "chatterbox_remote";
                          if (isRemoteConfig) {
                            return model.id === "chatterbox_remote";
                          } else {
                            return model.id === "edge_tts" || model.id === "supertonic_tts" || model.id === "chatterbox_tts";
                          }
                        })
                        .map((model) => {
                          const isSelected = (model.id === "edge_tts" && draftSettings.tts.provider?.kind === "edge_tts") ||
                                            (model.id === "supertonic_tts" && draftSettings.tts.provider?.kind === "supertonic") ||
                                            (model.id === "chatterbox_tts" && draftSettings.tts.provider?.kind === "chatterbox") ||
                                            (model.id === "chatterbox_remote" && draftSettings.tts.provider?.kind === "chatterbox_remote");
                          const isDownloaded = (model.id === "edge_tts" || model.id === "chatterbox_remote") ? true : modelPresence[model.id];
                          const status = downloadStatuses[model.id];

                          return (
                            <div key={model.id} className="relative">
                              <SubModelCard
                                id={model.id}
                                name={model.name}
                                description={model.description}
                                parameters={model.parameters}
                                ramUsage={model.ram_usage}
                                tradeoffs={model.tradeoffs}
                                isDownloaded={isDownloaded}
                                isActive={isSelected}
                                isRequired={false}
                                layoutMode={layoutMode}
                                onSelect={() => {
                                  if (model.id === "edge_tts") {
                                    updateDraft("tts", "provider", { kind: "edge_tts", voice: "en-US-AriaNeural" });
                                  } else if (model.id === "supertonic_tts") {
                                    updateDraft("tts", "provider", { kind: "supertonic" });
                                  } else if (model.id === "chatterbox_tts") {
                                    updateDraft("tts", "provider", { kind: "chatterbox", language: "en", quality_steps: 8, speed: 1.0 });
                                  } else if (model.id === "chatterbox_remote") {
                                    updateDraft("tts", "provider", {
                                      kind: "chatterbox_remote",
                                      endpoint: draftSettings.tts.provider?.kind === "chatterbox_remote" ? (draftSettings.tts.provider.endpoint || "http://127.0.0.1:7860") : "http://127.0.0.1:7860",
                                      language: "en",
                                      quality_steps: 8,
                                      speed: 1.0,
                                      remote_path: draftSettings.tts.provider?.kind === "chatterbox_remote" ? (draftSettings.tts.provider.remote_path || "~/.vox") : "~/.vox"
                                    });
                                  }
                                }}
                                confirmDeleteId={confirmDeleteId}
                                setConfirmDeleteId={setConfirmDeleteId}
                                downloadStatus={status}
                                startDownload={() => startDownload(model.id)}
                                deleteModel={() => deleteModel(model.id)}
                              />
                              {model.id === "chatterbox_remote" && isSelected && (
                                <div className="absolute top-2.5 right-2.5 flex items-center gap-1.5 select-none pointer-events-none">
                                  {checkingTtsHealth ? (
                                    <Loader2 size={10} className="animate-spin text-[rgb(var(--accent))]" />
                                  ) : isRemoteTtsHealthy === true ? (
                                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 shadow-[0_0_6px_rgba(16,185,129,0.7)]" />
                                  ) : isRemoteTtsHealthy === false ? (
                                    <span className="w-1.5 h-1.5 rounded-full bg-rose-500 shadow-[0_0_6px_rgba(239,68,68,0.7)] animate-pulse" />
                                  ) : null}
                                </div>
                              )}
                            </div>
                          );
                        })}
                    </div>
                  )}
                </div>
              ) : (() => {
                const isEdgeTts = draftSettings.tts.provider?.kind === "edge_tts";
                const isChatterbox = draftSettings.tts.provider?.kind?.startsWith("chatterbox");

                const currentEdgeVoice = (draftSettings.tts.provider as any)?.voice || "en-US-AriaNeural";

                if (isEdgeTts) {
                  return (
                    <div className="w-full flex flex-col gap-3 p-3 rounded-lg border border-[rgba(var(--accent),0.15)] bg-[rgba(var(--surface-bg),0.4)] mt-2">
                      <div className="flex items-center justify-between border-b border-[rgba(var(--accent),0.08)] pb-2">
                        <span className="text-[12px] font-bold text-[rgb(var(--foreground))] flex items-center gap-1.5 uppercase tracking-wider">
                          <Globe size={14} className="text-[rgb(var(--accent))]" />
                          Edge TTS Voice Configuration
                        </span>
                        <span className="text-[10px] font-semibold text-[rgb(var(--foreground-muted))]">
                          {loadingEdgeVoices ? "Loading voices..." : edgeTtsVoices.length > 0 ? `${edgeTtsVoices.length} Neural Voices Available` : "Cloud Neural TTS"}
                        </span>
                      </div>

                      {edgeTtsError ? (
                        <div className="flex flex-col gap-2 p-3 rounded bg-rose-500/10 border border-rose-500/20 text-rose-300 text-[11px]">
                          <div className="flex items-center gap-2 font-bold">
                            <AlertTriangle size={14} className="text-rose-400 shrink-0" />
                            <span>Network Error Loading Edge Voices</span>
                          </div>
                          <p className="text-[10px] leading-relaxed text-rose-300/80">{edgeTtsError}</p>
                          <button
                            type="button"
                            onClick={loadEdgeVoices}
                            disabled={loadingEdgeVoices}
                            className="self-start px-2.5 py-1 text-[10px] font-bold rounded bg-rose-500/20 hover:bg-rose-500/30 text-rose-200 transition-all flex items-center gap-1 mt-1 outline-none"
                          >
                            {loadingEdgeVoices ? <Loader2 size={10} className="animate-spin" /> : <RefreshCw size={10} />}
                            <span>Retry Connection</span>
                          </button>
                        </div>
                      ) : (
                        <div className="flex flex-col gap-1.5">
                          <label className="text-[10px] font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
                            Select Edge Neural Voice
                          </label>
                          <div className="relative">
                            <select
                              value={currentEdgeVoice}
                              disabled={loadingEdgeVoices}
                              onChange={(e) => updateDraft("tts", "provider", { kind: "edge_tts", voice: e.target.value })}
                              className="w-full bg-[rgb(var(--surface-bg))] border border-[rgba(var(--accent),0.2)] rounded px-3 py-2 text-[12px] font-medium text-[rgb(var(--foreground))] outline-none focus:border-[rgb(var(--accent))] transition-all appearance-none cursor-pointer"
                            >
                              {edgeTtsVoices.length === 0 ? (
                                <option value="en-US-AriaNeural">en-US-AriaNeural (Default Aria Online Natural)</option>
                              ) : (
                                edgeTtsVoices.map((v) => (
                                  <option key={v.short_name} value={v.short_name}>
                                    {v.friendly_name} ({v.gender}) [{v.locale}]
                                  </option>
                                ))
                              )}
                            </select>
                            {loadingEdgeVoices && (
                              <div className="absolute right-3 top-2.5 pointer-events-none">
                                <Loader2 size={12} className="animate-spin text-[rgb(var(--accent))]" />
                              </div>
                            )}
                          </div>
                        </div>
                      )}
                    </div>
                  );
                }
                const simplifyVoiceName = (n: string) => {
                  if (n.includes("Pain")) return "Pain";
                  if (n.includes("Madara")) return "Madara";
                  if (n.includes("Shreya")) return "Shreya";
                  if (n.includes("Hayami")) return "Hayami";
                  if (n.includes("Ellen")) return "Ellen";
                  if (n.includes("Juniper")) return "Juniper";
                  if (n.includes("Mark")) return "Mark";
                  if (n.includes("Spuds")) return "Spuds";
                  return n;
                };

                const voices = isChatterbox 
                  ? [
                      { id: "default", name: "Default" },
                      ...customVoices.map(v => ({ id: v.id, name: simplifyVoiceName(v.name), isCustom: true }))
                    ]
                  : (modelCatalog?.voices || []).map(v => ({ id: String(v.id), name: simplifyVoiceName(v.name) }));

                const selectedVoiceId = isChatterbox 
                  ? (draftSettings.tts.provider?.kind === "chatterbox" ? (draftSettings.tts.provider as any).voice_id || "default" : "default")
                  : String(draftSettings.tts.voice);

                const handleVoiceChange = (id: string) => {
                  if (isChatterbox) {
                    const voiceIdVal = id === "default" ? null : id;
                    updateDraft("tts", "provider", {
                      ...draftSettings.tts.provider,
                      voice_id: voiceIdVal
                    });
                  } else {
                    updateDraft("tts", "voice", Number(id));
                  }
                };

                const canShowVoiceProfile = isTtsVerified || draftSettings.tts.provider?.kind === "chatterbox_remote";

                return (
                  /* TTS Settings */
                  <div className={cn(
                    "w-full items-stretch",
                    layoutMode === "small" ? "flex flex-col gap-3" : "flex flex-row gap-4"
                  )}>
                    {/* Left column: Voice Carousel (60% width) */}
                    <div className={cn(
                      "shrink-0 flex flex-col justify-center",
                      layoutMode === "small" ? "w-full" : "w-[60%] min-w-[200px]"
                    )}>
                      {canShowVoiceProfile ? (
                        <VoiceCarousel
                          voices={voices as any}
                          selected={selectedVoiceId as any}
                          onChange={handleVoiceChange as any}
                          disabled={false}
                          onVoicesChanged={loadCustomVoices}
                          isAdding={chatterboxIsAdding}
                          setIsAdding={setChatterboxIsAdding}
                        />
                      ) : (
                        <div className="flex items-center justify-center h-full min-h-[100px] border border-dashed border-[rgba(var(--accent),0.15)] rounded-lg text-[rgb(var(--foreground-muted))]/60 text-[11px] font-bold uppercase tracking-wider text-center p-2 leading-tight">
                          Deploy Remote Server first
                        </div>
                      )}
                    </div>

                    {/* Right column: Quality & Speed Sliders + Clone Button (40% width) */}
                    <div className="flex-1 flex flex-col justify-between gap-3.5 min-w-0">
                      <div className="flex flex-col gap-3.5">
                        {isChatterbox ? (
                          <>
                            {/* Chatterbox Quality Slider (cfm_steps) */}
                            <div className="space-y-1.5">
                              <div className="flex items-center justify-between">
                                <span className="text-[12px] text-[rgb(var(--foreground))] font-bold">Quality</span>
                                <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold">
                                  {((draftSettings.tts.provider as any).quality_steps || 8)} steps
                                </span>
                              </div>
                              <input 
                                type="range" 
                                min="2" max="12" step="1"
                                value={((draftSettings.tts.provider as any).quality_steps || 8)}
                                onChange={(e) => {
                                  updateDraft("tts", "provider", {
                                    ...draftSettings.tts.provider,
                                    quality_steps: Number(e.target.value)
                                  });
                                }}
                                className="w-full h-1 bg-[rgba(var(--border),0.1)] rounded-lg appearance-none cursor-pointer accent-[rgb(var(--accent))]"
                              />
                            </div>

                            {/* Chatterbox Speed Slider */}
                            <div className="space-y-1.5">
                              <div className="flex items-center justify-between">
                                <span className="text-[12px] text-[rgb(var(--foreground))] font-bold">Speed</span>
                                <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold">
                                  {((draftSettings.tts.provider as any).speed || 1.0).toFixed(2)}x
                                </span>
                              </div>
                              <input 
                                type="range" 
                                min="0.7" max="2.0" step="0.05"
                                value={((draftSettings.tts.provider as any).speed || 1.0)}
                                onChange={(e) => {
                                  updateDraft("tts", "provider", {
                                    ...draftSettings.tts.provider,
                                    speed: Number(e.target.value)
                                  });
                                }}
                                className="w-full h-1 bg-[rgba(var(--border),0.1)] rounded-lg appearance-none cursor-pointer accent-[rgb(var(--accent))]"
                              />
                            </div>
                          </>
                        ) : (
                          <>
                            {/* Supertonic Quality Steps */}
                            <div className="space-y-1.5">
                              <div className="flex items-center justify-between">
                                <span className="text-[12px] text-[rgb(var(--foreground))] font-bold">Quality</span>
                                <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold">
                                  {draftSettings.tts.quality_steps <= 4 ? "Speed" : draftSettings.tts.quality_steps <= 8 ? "Quality" : "Best"}
                                </span>
                              </div>
                              <div className="flex gap-1">
                                {[2, 4, 6, 8, 10, 12].map(step => (
                                  <button key={step} onClick={() => updateDraft("tts", "quality_steps", step)}
                                    className={cn(
                                      "flex-1 py-1 rounded-lg text-[11px] font-bold uppercase tracking-wider transition-all duration-300",
                                      draftSettings.tts.quality_steps === step
                                        ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                                        : "glass text-[rgb(var(--foreground-muted))]/80 border border-[rgba(var(--border),0.04)] hover:border-[rgb(var(--accent))]/20"
                                    )}
                                  >{step}</button>
                                ))}
                              </div>
                            </div>

                            {/* Supertonic Speed */}
                            <div className="space-y-1.5">
                              <div className="flex items-center justify-between">
                                <span className="text-[12px] text-[rgb(var(--foreground))] font-bold">Speed</span>
                                <span className="text-[12px] font-mono text-[rgb(var(--accent))] font-bold">{draftSettings.tts.speed.toFixed(2)}x</span>
                              </div>
                              <input 
                                type="range" 
                                min="0.7" max="2.0" step="0.05"
                                value={draftSettings.tts.speed}
                                onChange={(e) => updateDraft("tts", "speed", Number(e.target.value))}
                                className="w-full h-1 bg-[rgba(var(--border),0.1)] rounded-lg appearance-none cursor-pointer accent-[rgb(var(--accent))]"
                              />
                            </div>
                          </>
                        )}
                      </div>

                      {/* Clone Voice button (toggles isAdding on the left) */}
                      {isChatterbox && (
                        <button
                          type="button"
                          onClick={() => setChatterboxIsAdding(prev => !prev)}
                          className={cn(
                            "w-full py-2 rounded-xl text-[10px] font-black uppercase tracking-wider transition-all duration-300 flex items-center justify-center gap-1.5 shadow-[0_0_12px_rgba(var(--accent),0.1)]",
                            chatterboxIsAdding
                              ? "bg-rose-500/10 border border-rose-500/30 text-rose-400 hover:bg-rose-500/20"
                              : "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))] hover:scale-[1.01] active:scale-95 hover:shadow-[0_0_16px_rgba(var(--accent),0.25)]"
                          )}
                        >
                          {chatterboxIsAdding ? (
                            <>
                              <ArrowLeft size={11} />
                              Back to Presets
                            </>
                          ) : (
                            <>
                              <Sparkles size={11} />
                              Clone Voice Profile
                            </>
                          )}
                        </button>
                      )}
                    </div>
                  </div>
                );
              })()}
            </div>
          )}

          {/* TAB 5: AUXILIARY UTILITY MODELS */}
          {activePipelineTab === "auxiliary" && (
            <div className="space-y-4">
              <div className={cn("grid gap-2.5", layoutMode === "small" ? "grid-cols-1" : "grid-cols-2")}>
                {(() => {
                  const auxiliaryCategories = ["classifier", "embedding", "nli", "translit"];
                  const groups = (manifest?.model_groups || []).filter(g => auxiliaryCategories.includes(g.category));
                  
                  if (groups.length === 0) {
                    return (
                      <div className="col-span-2 text-center py-6 text-[11px] text-[rgb(var(--foreground-muted))]/70">
                        Loading auxiliary models manifest from backend...
                      </div>
                    );
                  }

                  const categoryDescriptions: Record<string, string> = {
                    classifier: "Intent router classifying user queries into Generic or Semantic memory paths.",
                    embedding: "Dense vector encoder for personal memory retrieval & semantic search.",
                    nli: "Intra-collection contradiction detector ensuring memory consistency.",
                    translit: "Converts Devanagari (Hindi) script to natural Hinglish phonetic spelling."
                  };

                  return groups.map((group) => {
                    const totalBytes = group.files.reduce((acc, f) => acc + f.size, 0);
                    const formattedSize = totalBytes > 0 ? `${(totalBytes / (1024 * 1024)).toFixed(1)} MB` : "ONNX";
                    const isDownloaded = modelPresence[group.id] ?? false;
                    const isRequired = group.files.some(f => f.required);
                    const status = downloadStatuses[group.id];

                    return (
                      <SubModelCard
                        key={group.id}
                        id={group.id}
                        name={group.name}
                        description={categoryDescriptions[group.category] || `${group.name} engine.`}
                        parameters={formattedSize}
                        ramUsage={`~${Math.round(totalBytes / (1024 * 1024))} MB`}
                        isDownloaded={isDownloaded}
                        isActive={true}
                        isRequired={isRequired}
                        layoutMode={layoutMode}
                        onSelect={() => {}}
                        confirmDeleteId={confirmDeleteId}
                        setConfirmDeleteId={setConfirmDeleteId}
                        downloadStatus={status}
                        startDownload={() => startDownload(group.id)}
                        deleteModel={() => deleteModel(group.id)}
                      />
                    );
                  });
                })()}
              </div>
            </div>
          )}

        </div>
      </div>
    </div>
  );
});

ModelsCard.displayName = "ModelsCard";
