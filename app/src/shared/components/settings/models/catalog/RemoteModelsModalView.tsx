import {
  memo,
  useState,
  useMemo,
  useRef,
  useEffect,
  useCallback,
  useDeferredValue,
} from "react";
import type Lenis from "lenis";
import { createContainedLenis } from "@/shared/lib/lenisScroll";
import { useScrollTopOnChange } from "@/shared/hooks/useScrollTopOnChange";
import { Search, X } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import type { LlmModelInfo, ModelCapabilities } from "@/store/settingsStore";
import { REMOTE_MODEL_PAGE_SIZE } from "./catalogProbeUtils";
import { RemoteModelCard } from "./RemoteModelCard";

export interface RemoteModelsModalViewProps {
  remoteModels: LlmModelInfo[];
  selectedModelId?: string;
  /** Committed (saved) model id driving list order; selection highlight stays on the draft id. */
  pinnedModelId?: string;
  probingMap: Record<string, { status: 'idle' | 'testing' | 'success' | 'error'; capabilities?: ModelCapabilities; error?: string }>;
  capabilitiesCache?: Record<string, ModelCapabilities>;
  onSelectModel: (id: string) => void;
  onProbeCapabilities: (id?: string) => void;
}

export const RemoteModelsModalView = memo(({
  remoteModels,
  selectedModelId,
  pinnedModelId,
  probingMap,
  capabilitiesCache,
  onSelectModel,
  onProbeCapabilities,
}: RemoteModelsModalViewProps) => {
  const [modalSearch, setModalSearch] = useState("");
  const deferredSearch = useDeferredValue(modalSearch);
  const [filterCategory, setFilterCategory] = useState<"all" | "benchmarked" | "tools" | "fast">("all");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const scrollContainerRef = useRef<HTMLDivElement>(null);
  const lenisRef = useRef<Lenis | null>(null);

  // Smooth Lenis scrolling on the contained viewport
  useEffect(() => {
    const el = scrollContainerRef.current;
    if (!el) return undefined;

    const lenis = createContainedLenis(el, { duration: 0.75 });
    if (!lenis) return undefined;
    lenisRef.current = lenis;

    return () => {
      lenisRef.current = null;
      lenis.destroy();
    };
  }, []);

  useScrollTopOnChange(scrollContainerRef, pinnedModelId, lenisRef);

  const handleCopyId = useCallback((id: string) => {
    navigator.clipboard.writeText(id);
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 1800);
  }, []);

  // Compute metrics counts for filter badges
  const benchmarkedCount = useMemo(() => {
    return remoteModels.filter((m) => {
      const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
      return probed?.tps && probed.tps > 0;
    }).length;
  }, [remoteModels, probingMap, capabilitiesCache]);

  const toolsCount = useMemo(() => {
    return remoteModels.filter((m) => {
      const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
      return probed?.supports_tools === "supported";
    }).length;
  }, [remoteModels, probingMap, capabilitiesCache]);

  const fastCount = useMemo(() => {
    return remoteModels.filter((m) => {
      const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
      return probed?.tps && probed.tps >= 30;
    }).length;
  }, [remoteModels, probingMap, capabilitiesCache]);

  const modalFilteredModels = useMemo(() => {
    let list = remoteModels;
    const query = deferredSearch.trim().toLowerCase();
    if (query) {
      list = list.filter((m) =>
        m.id.toLowerCase().includes(query) ||
        (m.name && m.name.toLowerCase().includes(query)) ||
        (m.family && m.family.toLowerCase().includes(query)) ||
        (m.quantization && m.quantization.toLowerCase().includes(query))
      );
    }

    if (filterCategory === "benchmarked") {
      list = list.filter((m) => {
        const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
        return probed?.tps && probed.tps > 0;
      });
    } else if (filterCategory === "tools") {
      list = list.filter((m) => {
        const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
        return probed?.supports_tools === "supported";
      });
    } else if (filterCategory === "fast") {
      list = list.filter((m) => {
        const probed = probingMap[m.id]?.capabilities || m.capabilities || capabilitiesCache?.[`server:${m.id}`] || capabilitiesCache?.[`cloud:${m.id}`] || capabilitiesCache?.[`embedded:${m.id}`] || capabilitiesCache?.[m.id];
        return probed?.tps && probed.tps >= 30;
      });
    }

    if (pinnedModelId) {
      const idx = list.findIndex((m) => m.id === pinnedModelId);
      if (idx > 0) {
        const copy = [...list];
        const [selected] = copy.splice(idx, 1);
        copy.unshift(selected);
        return copy;
      }
    }

    return list;
  }, [remoteModels, deferredSearch, filterCategory, probingMap, capabilitiesCache, pinnedModelId]);

  // Mounted window over the filtered list. Slicing here rather than inside the
  // map keeps the `.length` readouts honest — they still report the true match
  // count, not the capped one.
  const [visibleModalModelCount, setVisibleModalModelCount] = useState(REMOTE_MODEL_PAGE_SIZE);
  useEffect(() => {
    setVisibleModalModelCount(REMOTE_MODEL_PAGE_SIZE);
  }, [modalSearch, deferredSearch, filterCategory, remoteModels]);

  const visibleModalModels = useMemo(
    () => modalFilteredModels.slice(0, visibleModalModelCount),
    [modalFilteredModels, visibleModalModelCount]
  );
  const modalHiddenCount = modalFilteredModels.length - visibleModalModels.length;

  return (
    <div className="flex flex-col h-full min-h-0 gap-3">
      {/* Modal Toolbar: Clean Search + Filter Chips Bar (Zero native dropdown) */}
      <div className="flex flex-col gap-2.5 shrink-0">
        <div className="flex items-center justify-between gap-3">
          {/* Search Input */}
          <div className="flex-1 flex items-center gap-2 px-3 py-1.5 rounded-lg bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] focus-within:border-[rgb(var(--accent))] focus-within:ring-1 focus-within:ring-[rgb(var(--accent))]/30 transition-all">
            <Search size={14} className="text-[rgb(var(--foreground-muted))] shrink-0" />
            <input
              type="text"
              value={modalSearch}
              onChange={(e) => setModalSearch(e.target.value)}
              placeholder="Search models..."
              className="w-full bg-transparent border-none outline-none text-[13px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
            />
            {modalSearch && (
              <button
                type="button"
                onClick={() => setModalSearch("")}
                className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] p-0.5 cursor-pointer"
              >
                <X size={13} />
              </button>
            )}
          </div>

          <div className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] px-2.5 py-1.5 rounded-lg bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] shrink-0">
            <span className="hidden sm:inline">Showing <span className="text-[rgb(var(--foreground))] font-bold">{modalFilteredModels.length}</span> of {remoteModels.length} models</span>
            <span className="inline sm:hidden"><span className="text-[rgb(var(--foreground))] font-bold">{modalFilteredModels.length}</span>/{remoteModels.length}</span>
          </div>
        </div>

        {/* Filter Chips Bar */}
        <div className="flex items-center gap-1.5 overflow-x-auto no-scrollbar py-0.5 shrink-0">
          {(
            [
              { id: "all", label: `All (${remoteModels.length})` },
              { id: "benchmarked", label: `⚡ Benchmarked (${benchmarkedCount})` },
              { id: "tools", label: `🛠️ Tools (${toolsCount})` },
              { id: "fast", label: `🚀 Fast >30 TPS (${fastCount})` },
            ] as const
          ).map((chip) => (
            <button
              key={chip.id}
              type="button"
              onClick={() => setFilterCategory(chip.id)}
              className={cn(
                "px-2.5 py-1 rounded-md text-[11px] font-mono font-medium transition-all cursor-pointer border whitespace-nowrap shrink-0",
                filterCategory === chip.id
                  ? "bg-[rgb(var(--accent))]/15 border-[rgb(var(--accent))]/60 text-[rgb(var(--accent))] shadow-sm"
                  : "bg-[rgba(var(--foreground),0.02)] border-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.2)]"
              )}
            >
              {chip.label}
            </button>
          ))}
        </div>
      </div>

      {/* Contained Cards Scroll Area with Constant Inset Padding */}
      <div className="flex-1 min-h-0 overflow-hidden flex flex-col relative">
        <div
          ref={scrollContainerRef}
          className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar p-1 sm:p-2"
        >
          {modalFilteredModels.length === 0 ? (
            <div className="h-64 flex flex-col items-center justify-center text-center space-y-2">
              <p className="text-[14px] font-bold text-[rgb(var(--foreground))]">No models found</p>
              <p className="text-[12px] text-[rgb(var(--foreground-muted))] max-w-sm">
                No available models match your current search & filter criteria.
              </p>
              <button
                type="button"
                onClick={() => {
                  setModalSearch("");
                  setFilterCategory("all");
                }}
                className="mt-2 px-3 py-1.5 rounded-lg text-[12px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.3)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
              >
                Reset Filters
              </button>
            </div>
          ) : (
            <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-3">
              {visibleModalModels.map((model) => {
                const isSelected = selectedModelId === model.id;
                const probed =
                  probingMap[model.id]?.capabilities ||
                  model.capabilities ||
                  capabilitiesCache?.[`server:${model.id}`] ||
                  capabilitiesCache?.[`cloud:${model.id}`] ||
                  capabilitiesCache?.[`embedded:${model.id}`] ||
                  capabilitiesCache?.[model.id];
                const isTesting = probingMap[model.id]?.status === "testing";
                const probeError = probingMap[model.id]?.status === "error" ? probingMap[model.id]?.error : undefined;

                return (
                  <RemoteModelCard
                    key={model.id}
                    model={model}
                    isSelected={isSelected}
                    probed={probed}
                    isTesting={isTesting}
                    probeError={probeError}
                    onSelect={onSelectModel}
                    onProbe={onProbeCapabilities}
                    onCopy={handleCopyId}
                    isCopied={copiedId === model.id}
                  />
                );
              })}
              {modalHiddenCount > 0 && (
                <div className="col-span-full flex items-center justify-center pt-1">
                  <button
                    type="button"
                    onClick={() =>
                      setVisibleModalModelCount((c) => c + REMOTE_MODEL_PAGE_SIZE)
                    }
                    className="px-3 py-1.5 rounded-lg text-[11px] font-bold uppercase tracking-wider text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.25)] hover:bg-[rgb(var(--accent))]/20 transition-colors cursor-pointer"
                  >
                    {`Show ${modalHiddenCount} more`}
                  </button>
                </div>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
});

RemoteModelsModalView.displayName = "RemoteModelsModalView";
