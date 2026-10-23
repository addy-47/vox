import { memo, useMemo, useState } from "react";
import { Cpu, Search } from "lucide-react";
import { EmptyState } from "@/shared/components/common";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { useModelDownloads } from "@/shared/hooks/useModelDownloads";
import { activeSelection } from "@/shared/lib/activeSelection";
import { CardHeader, type CardSortOrder } from "../shared/CardHeader";
import { LlmTuningControls } from "./LlmTuningControls";
import { SpeechModelCard } from "../speech/SpeechModelCard";
import { LLM_CATALOG_COPY } from "@/data/settingsCopy";

export interface LocalModelsViewProps {
  layoutMode?: "full-max" | "full-min" | "small";
  onBack: () => void;
}

/**
 * The on-device provider's models: every GGUF group that is present on disk,
 * plus the built-in groups that ship with the binary. Nothing downloadable
 * appears here — this face answers "what can run right now".
 */
export const LocalModelsView = memo(({ layoutMode, onBack }: LocalModelsViewProps) => {
  const modelCatalog = useCatalogStore((s) => s.modelCatalog);
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const committedSettings = useSettingsStore((s) => s.settings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const {
    downloadStatuses,
    modelPresence,
    confirmDeleteId,
    setConfirmDeleteId,
    startDownload,
    handleDeleteModelGroup,
    isGroupRequired,
  } = useModelDownloads();

  const [search, setSearch] = useState("");
  const [searchOpen, setSearchOpen] = useState(false);
  const [sort, setSort] = useState<CardSortOrder>("az");

  const committedLocalId = committedSettings?.llm.active.model ?? "";
  const selectedLocalId = draftSettings?.llm.active.model ?? "";

  const models = useMemo(() => {
    const query = search.trim().toLowerCase();
    const available = (modelCatalog?.llm ?? []).filter(
      (model) => model.is_built_in || modelPresence[model.id]
    );
    const matched = query
      ? available.filter(
          (model) =>
            model.name.toLowerCase().includes(query) ||
            model.id.toLowerCase().includes(query)
        )
      : available;

    const sorted = [...matched].sort((a, b) => {
      const cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
      return sort === "az" ? cmp : -cmp;
    });

    // The committed model stays first so the chosen row never hides on scroll.
    const index = sorted.findIndex((model) => model.id === committedLocalId);
    if (index > 0) {
      const [pinned] = sorted.splice(index, 1);
      sorted.unshift(pinned);
    }
    return sorted;
  }, [modelCatalog?.llm, modelPresence, search, sort, committedLocalId]);

  return (
    <div className="flex h-full min-h-0 w-full flex-col animate-fade-in">
      <CardHeader
        icon={<Cpu size={14} className="text-[rgb(var(--accent))]" />}
        title={LLM_CATALOG_COPY.onDeviceTitle}
        onBack={onBack}
        action={<LlmTuningControls />}
        sort={{
          order: sort,
          onToggle: () => setSort((prev) => (prev === "az" ? "za" : "az")),
        }}
        search={{
          open: searchOpen,
          value: search,
          placeholder: LLM_CATALOG_COPY.searchPlaceholder,
          onChange: setSearch,
          onToggle: () => {
            if (searchOpen && search) setSearch("");
            else setSearchOpen((prev) => !prev);
          },
          onClose: () => {
            setSearch("");
            setSearchOpen(false);
          },
        }}
      />

      <div className="mt-2 flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar pr-1">
        {models.length === 0 ? (
          <div className="py-2">
            <EmptyState
              icon={search ? Search : Cpu}
              title={search ? "No Models Matching" : LLM_CATALOG_COPY.noLocalModelsTitle}
              description={search ? `${LLM_CATALOG_COPY.noModelsMatch} "${search}"` : LLM_CATALOG_COPY.noLocalModelsHint}
              action={
                search ? (
                  <button
                    type="button"
                    onClick={() => setSearch("")}
                    className="px-3.5 py-1.5 rounded-xl text-[12px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.25)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
                  >
                    {LLM_CATALOG_COPY.clearSearch}
                  </button>
                ) : undefined
              }
              className="py-6 border-dashed border-[rgba(var(--accent),0.15)] bg-transparent min-h-[180px]"
            />
          </div>
        ) : (
          <div
            className={
              layoutMode === "small"
                ? "flex flex-col gap-2"
                : "grid grid-cols-1 gap-2 sm:grid-cols-2 auto-rows-max content-start"
            }
          >
            {models.map((model) => (
              <SpeechModelCard
                key={model.id}
                id={model.id}
                name={model.name}
                description={model.description || ""}
                parameters={model.parameters || ""}
                ramUsage={model.ram_usage}
                tradeoffs={model.tradeoffs}
                badge={
                  model.is_built_in ? LLM_CATALOG_COPY.badgeBuiltIn : LLM_CATALOG_COPY.badgeOnDevice
                }
                isDownloaded={Boolean(model.is_built_in || modelPresence[model.id])}
                isActive={selectedLocalId === model.id}
                isRequired={isGroupRequired(model.id)}
                layoutMode={layoutMode}
                onSelect={() => {
                  updateDraft("llm", "active", activeSelection("embedded", model.id));
                }}
                confirmDeleteId={confirmDeleteId}
                setConfirmDeleteId={setConfirmDeleteId}
                downloadStatus={downloadStatuses[model.id]}
                startDownload={() => startDownload(model.id)}
                deleteModel={() => handleDeleteModelGroup(model.id)}
                showTooltip={false}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
});

LocalModelsView.displayName = "LocalModelsView";
