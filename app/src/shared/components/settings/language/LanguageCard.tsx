import { memo, useState, useCallback } from "react";
import { useShallow } from "zustand/react/shallow";
import { BrainCircuit } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore, selectActiveLlmProvider } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { useRemoteLlmProbing } from "@/shared/hooks/useRemoteLlmProbing";
import { useProviderSelection } from "@/shared/hooks/useProviderSelection";
import { ExpandableList } from "@/shared/ui/ExpandableList";
import { SettingsCommitControls } from "@/shared/components/settings/SettingsCommitControls";
import { CardTitleBar } from "../shared/CardTitleBar";
import { LANGUAGE_CARD_COPY, LLM_CATALOG_COPY } from "@/data/settingsCopy";
import { ProviderGrid } from "./ProviderGrid";
import { ProviderModal } from "./ProviderModal";
import { ModelModal } from "./ModelModal";
import { RemoteCatalogSection } from "./RemoteCatalogSection";
import { LocalModelsView } from "./LocalModelsView";
import {
  getSettingsInlineListMaxHeightClass,
  SETTINGS_DOMAIN_CARD_HEIGHT_CLASS,
} from "../shared/settingsLayout";

export interface LanguageCardProps {
  layoutMode?: "full-max" | "full-min" | "small";
}

/**
 * Language model card (04:00). One job: decide what does the thinking.
 *
 * Face one lists only what works right now (on-device, keyed cloud providers,
 * custom servers) plus the add-provider card. Select model swaps the face for
 * the active provider's catalog, where search, sort, and facets live. The
 * expand trigger opens the full provider catalog.
 */
export const LanguageCard = memo(({ layoutMode = "full-max" }: LanguageCardProps) => {
  const provider = useSettingsStore(useShallow(selectActiveLlmProvider));
  const isRemote = provider.kind !== "embedded";
  const { remoteModels, loadingRemoteModels, remoteModelsError, probingMap, handleProbeCapabilities } =
    useRemoteLlmProbing(provider, "llm", isRemote);

  const [cardFace, setCardFace] = useState<"providers" | "models">("providers");
  const [modalLevel, setModalLevel] = useState<"providers" | "models">("providers");

  const selection = useProviderSelection({ category: "LLM" });

  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const committedRemoteModelId = useSettingsStore((s) => s.settings?.llm.active.model);
  const selectedModelId = useSettingsStore(
    (s) => s.draftSettings?.llm.active.model ?? committedRemoteModelId ?? ""
  );
  const capabilitiesCache = useCatalogStore((s) => s.capabilitiesCache);

  const handleSelectModel = useCallback(
    (modelId: string) => {
      const active = useSettingsStore.getState().draftSettings?.llm.active;
      if (active && (active.category === "server" || active.category === "cloud")) {
        updateDraft("llm", "active", { ...active, model: modelId });
      }
    },
    [updateDraft]
  );

  return (
    <div
      className={cn(
        "w-full flex flex-col text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none justify-between",
        layoutMode === "small"
          ? "bg-transparent p-0 h-auto"
          : cn("glass-card p-5", SETTINGS_DOMAIN_CARD_HEIGHT_CLASS)
      )}
    >
      <ExpandableList
        className="flex-1 min-h-0 flex flex-col"
        hideInlineTrigger
        inlineMaxHeightClass={getSettingsInlineListMaxHeightClass(layoutMode)}
        icon={<BrainCircuit size={16} className="text-[rgb(var(--accent))]" />}
        title={
          <span className="font-display text-[15px] font-bold tracking-tight text-[rgb(var(--foreground))]">
            {modalLevel === "models"
              ? (isRemote ? LLM_CATALOG_COPY.catalogTitle : LANGUAGE_CARD_COPY.title)
              : LANGUAGE_CARD_COPY.providerModalTitle}
          </span>
        }
        subtitle={
          <span className="font-mono text-[11px] text-[rgb(var(--foreground-muted))]">
            {modalLevel === "models"
              ? (isRemote
                  ? `${provider.provider_name ?? provider.kind} · ${remoteModels.length} ${LLM_CATALOG_COPY.modelCountLabel}`
                  : LANGUAGE_CARD_COPY.onDevice)
              : (isRemote ? provider.provider_name ?? provider.kind : LANGUAGE_CARD_COPY.onDevice)}
          </span>
        }
        modalSubtitle={
          modalLevel === "models"
            ? LLM_CATALOG_COPY.modelsModalSubtitle
            : LANGUAGE_CARD_COPY.providerModalSubtitle
        }
        expandLabel={LANGUAGE_CARD_COPY.expandProviders}
        ariaLabel={LANGUAGE_CARD_COPY.providersAria}
        headerActions={<SettingsCommitControls domainId="language" layoutMode={layoutMode} />}
        onBack={
          modalLevel === "models"
            ? () => setModalLevel("providers")
            : undefined
        }
        onClose={() => setModalLevel("providers")}
        modalContent={() =>
          modalLevel === "models" ? (
            isRemote ? (
              <ModelModal
                remoteModels={remoteModels}
                selectedModelId={selectedModelId}
                pinnedModelId={committedRemoteModelId}
                probingMap={probingMap}
                capabilitiesCache={capabilitiesCache}
                onSelectModel={handleSelectModel}
                onProbeCapabilities={handleProbeCapabilities}
              />
            ) : (
              <LocalModelsView
                layoutMode={layoutMode}
                onBack={() => setModalLevel("providers")}
              />
            )
          ) : (
            <ProviderModal
              {...selection}
              category="LLM"
              onNavigateToModels={() => {
                setModalLevel("models");
                setCardFace("models");
              }}
            />
          )
        }
      >
        {cardFace === "models" ? (
          <div key="models-face" className="flex flex-col flex-1 min-h-0 animate-fade-in">
            {provider.kind === "embedded" ? (
              <LocalModelsView
                layoutMode={layoutMode}
                onBack={() => setCardFace("providers")}
              />
            ) : (
              <RemoteCatalogSection
                layoutMode={layoutMode}
                provider={provider}
                remoteModels={remoteModels}
                loadingRemoteModels={loadingRemoteModels}
                remoteModelsError={remoteModelsError}
                probingMap={probingMap}
                handleProbeCapabilities={handleProbeCapabilities}
                onBack={() => setCardFace("providers")}
                onExpand={() => setModalLevel("models")}
              />
            )}
          </div>
        ) : (
          <div key="providers-face" className="flex flex-col flex-1 min-h-0 animate-fade-in">
            <CardTitleBar
              icon={<BrainCircuit size={17} />}
              title={LANGUAGE_CARD_COPY.title}
              actionLabel={LANGUAGE_CARD_COPY.selectModel}
              onAction={() => setCardFace("models")}
            />
            <div className="flex-1 w-full flex flex-col min-h-0 relative">
              <ProviderGrid
                onOpenModels={() => setCardFace("models")}
                onExpand={() => setModalLevel("providers")}
              />
            </div>
          </div>
        )}
      </ExpandableList>
    </div>
  );
});

LanguageCard.displayName = "LanguageCard";
