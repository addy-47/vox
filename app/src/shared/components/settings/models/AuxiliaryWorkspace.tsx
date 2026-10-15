import { memo, useState, useMemo } from "react";
import { SubModelCard } from "../SubModelCard";
import type { ModelStatus } from "@/shared/hooks/useModelDownloads";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore } from "@/store/settingsStore";
import { LifeBuoy } from "lucide-react";
import { CatalogHeader, type CatalogSortOrder } from "./catalog/CatalogHeader";

interface AuxiliaryWorkspaceProps {
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, ModelStatus>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
}

export const AuxiliaryWorkspace = memo(
  ({
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
  }: AuxiliaryWorkspaceProps) => {
    const auxiliaryModels = useSettingsStore((s) => s.modelCatalog?.auxiliary) ?? [];

    const [searchQuery, setSearchQuery] = useState("");
    const [isSearchOpen, setIsSearchOpen] = useState(false);
    const [sortOrder, setSortOrder] = useState<CatalogSortOrder>("az");

    const filteredModels = useMemo(() => {
      let list = auxiliaryModels;
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase();
        list = list.filter(
          (m) =>
            m.name.toLowerCase().includes(q) ||
            m.id.toLowerCase().includes(q) ||
            (m.description && m.description.toLowerCase().includes(q))
        );
      }
      return [...list].sort((a, b) => {
        const cmp = a.name.localeCompare(b.name);
        return sortOrder === "az" ? cmp : -cmp;
      });
    }, [auxiliaryModels, searchQuery, sortOrder]);

    return (
      <div className="flex-1 min-h-0 w-full flex flex-col gap-2">
        <CatalogHeader
          icon={<LifeBuoy size={14} className="text-[rgb(var(--accent))]" />}
          title=""
          sort={{
            order: sortOrder,
            onToggle: () => setSortOrder((o) => (o === "az" ? "za" : "az")),
          }}
          search={{
            open: isSearchOpen,
            value: searchQuery,
            placeholder: "Search support models...",
            onChange: setSearchQuery,
            onToggle: () => {
              setIsSearchOpen((o) => !o);
              if (isSearchOpen) setSearchQuery("");
            },
            onClose: () => {
              setIsSearchOpen(false);
              setSearchQuery("");
            },
          }}
        />
        <div className={cn("flex-1 min-h-0 w-full overflow-y-auto custom-scrollbar pr-1", layoutMode === "small" && "max-h-[220px]")}>
          <div
            className={cn(
              "grid gap-2.5 auto-rows-max content-start",
              layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2"
            )}
          >
            {filteredModels.map((model) => (
              <SubModelCard
                key={model.id}
                id={model.id}
                name={model.name}
                description={model.description || ""}
                parameters={model.parameters || "ONNX"}
                ramUsage={model.ram_usage || "N/A"}
                tradeoffs={model.tradeoffs || ""}
                isDownloaded={!!modelPresence[model.id]}
                isActive={!!modelPresence[model.id]}
                isRequired={false}
                layoutMode={layoutMode}
                onSelect={() => {}}
                confirmDeleteId={confirmDeleteId}
                setConfirmDeleteId={setConfirmDeleteId}
                downloadStatus={downloadStatuses[model.id]}
                startDownload={() => startDownload(model.id)}
                deleteModel={() => deleteModel(model.id)}
              />
            ))}
          </div>
        </div>
      </div>
    );
  }
);

AuxiliaryWorkspace.displayName = "AuxiliaryWorkspace";
