import { memo, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { BrainCircuit } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { useSettingsStore, selectActiveLlmProvider } from "@/store/settingsStore";
import { useRemoteLlmProbing } from "@/shared/hooks/useRemoteLlmProbing";
import { useProviderSelection } from "@/shared/hooks/useProviderSelection";
import { ExpandableList } from "@/shared/ui/ExpandableList";
import { SettingsCommitControls } from "@/shared/components/settings/SettingsCommitControls";
import { CardTitleBar } from "../shared/CardTitleBar";
import { LANGUAGE_CARD_COPY } from "@/data/settingsCopy";
import { ProviderGrid } from "./ProviderGrid";
import { ProviderModal } from "./ProviderModal";
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

  const [view, setView] = useState<"providers" | "models">("providers");
  const selection = useProviderSelection({ category: "LLM" });

  return (
    <div
      className={cn(
        "w-full flex flex-col text-[14px] leading-relaxed text-[rgb(var(--foreground))]/85 select-none justify-between",
        layoutMode === "small"
          ? "bg-transparent p-0 h-auto"
          : cn("glass-card p-5", SETTINGS_DOMAIN_CARD_HEIGHT_CLASS)
      )}
    >
      {view === "models" ? (
        <div key="models-face" className="flex flex-col flex-1 min-h-0 animate-fade-in">
          {provider.kind === "embedded" ? (
            <LocalModelsView
              layoutMode={layoutMode}
              onBack={() => setView("providers")}
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
              onBack={() => setView("providers")}
            />
          )}
        </div>
      ) : (
        <div key="providers-face" className="flex flex-col flex-1 min-h-0 animate-fade-in">
          <CardTitleBar
            icon={<BrainCircuit size={17} />}
            title={LANGUAGE_CARD_COPY.title}
            actionLabel={LANGUAGE_CARD_COPY.selectModel}
            onAction={() => setView("models")}
          />

          <div className="flex-1 w-full flex flex-col min-h-0 relative">
            <ExpandableList
              className="flex-1 min-h-0"
              hideInlineTrigger
              inlineMaxHeightClass={getSettingsInlineListMaxHeightClass(layoutMode)}
              icon={<BrainCircuit size={16} className="text-[rgb(var(--accent))]" />}
              title={
                <span className="font-display text-[15px] font-bold tracking-tight text-[rgb(var(--foreground))]">
                  {LANGUAGE_CARD_COPY.providerModalTitle}
                </span>
              }
              subtitle={
                <span className="font-mono text-[11px] text-[rgb(var(--foreground-muted))]">
                  {isRemote ? provider.provider_name ?? provider.kind : "On-device"}
                </span>
              }
              modalSubtitle={LANGUAGE_CARD_COPY.providerModalSubtitle}
              expandLabel={LANGUAGE_CARD_COPY.expandProviders}
              ariaLabel={LANGUAGE_CARD_COPY.providersAria}
              headerActions={<SettingsCommitControls domainId="language" layoutMode={layoutMode} />}
              modalContent={({ close }) => (
                <ProviderModal
                  {...selection}
                  category="LLM"
                  onNavigateToModels={() => {
                    setView("models");
                    close();
                  }}
                />
              )}
            >
              <ProviderGrid onOpenModels={() => setView("models")} />
            </ExpandableList>
          </div>
        </div>
      )}
    </div>
  );
});

LanguageCard.displayName = "LanguageCard";
