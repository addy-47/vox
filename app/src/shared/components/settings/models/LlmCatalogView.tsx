import { memo } from "react";
import type { LlmModelInfo, ModelCapabilities, LlmProviderConfig } from "@/store/settingsStore";
import type { ModelStatus } from "@/shared/hooks/useModelDownloads";
import { LocalCatalogSection } from "./catalog/LocalCatalogSection";
import { RemoteCatalogSection } from "./catalog/RemoteCatalogSection";

// Re-export probe utilities for tests and external consumers
export {
  REMOTE_MODEL_PAGE_SIZE,
  provenanceLabel,
  probeAgeDays,
  failedProbeChecks,
  hasProbeWarning,
  probeWarningLabel,
} from "./catalog/catalogProbeUtils";

// Re-export subcomponents
export { RemoteModelCard, type RemoteModelCardProps } from "./catalog/RemoteModelCard";
export { RemoteModelsModalView, type RemoteModelsModalViewProps } from "./catalog/RemoteModelsModalView";
export { LocalCatalogSection, type LocalCatalogSectionProps } from "./catalog/LocalCatalogSection";
export { RemoteCatalogSection, type RemoteCatalogSectionProps } from "./catalog/RemoteCatalogSection";

export interface LlmCatalogViewProps {
  layoutMode?: "full-max" | "full-min" | "small";
  selectedLlmId: string;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, ModelStatus>;
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  startDownload: (id: string) => void;
  handleDeleteModelGroup: (id: string) => void;
  isGroupRequired: (id: string) => boolean;
  isRemoteLlm: boolean;
  provider?: LlmProviderConfig;
  remoteModels: LlmModelInfo[];
  loadingRemoteModels: boolean;
  remoteModelsError: string | null;
  probingMap: Record<string, { status: 'idle' | 'testing' | 'success' | 'error'; capabilities?: ModelCapabilities; error?: string }>;
  handleProbeCapabilities: (id?: string) => void;
}

export const LlmCatalogView = memo(({
  layoutMode,
  selectedLlmId,
  modelPresence,
  downloadStatuses,
  confirmDeleteId,
  setConfirmDeleteId,
  startDownload,
  handleDeleteModelGroup,
  isGroupRequired,
  isRemoteLlm,
  provider,
  remoteModels,
  loadingRemoteModels,
  remoteModelsError,
  probingMap,
  handleProbeCapabilities,
}: LlmCatalogViewProps) => {
  if (isRemoteLlm) {
    return (
      <RemoteCatalogSection
        layoutMode={layoutMode}
        provider={provider}
        remoteModels={remoteModels}
        loadingRemoteModels={loadingRemoteModels}
        remoteModelsError={remoteModelsError}
        probingMap={probingMap}
        handleProbeCapabilities={handleProbeCapabilities}
      />
    );
  }

  return (
    <LocalCatalogSection
      layoutMode={layoutMode}
      selectedLlmId={selectedLlmId}
      modelPresence={modelPresence}
      downloadStatuses={downloadStatuses}
      confirmDeleteId={confirmDeleteId}
      setConfirmDeleteId={setConfirmDeleteId}
      startDownload={startDownload}
      handleDeleteModelGroup={handleDeleteModelGroup}
      isGroupRequired={isGroupRequired}
    />
  );
});

LlmCatalogView.displayName = "LlmCatalogView";
