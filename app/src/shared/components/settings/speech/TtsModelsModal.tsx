import { memo, useState } from "react";
import { Cloud, Cpu, Layers } from "lucide-react";
import { SPEECH_CARD_COPY } from "@/data/settingsCopy";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { useVoiceCatalog, useVoiceScope } from "@/shared/hooks/useVoiceCatalog";
import { CatalogFacetRail, type CatalogFacetGroup } from "../shared/CatalogFacetRail";
import { EngineFlowBanner } from "./EngineFlowBanner";
import { VoiceModal } from "./voices/VoiceModal";
import { TtsEngineGrid, type TtsEngineGridProps } from "./TtsEngineGrid";

type Facet = "all" | "local" | "cloud";

export interface TtsModelsModalProps extends Omit<TtsEngineGridProps, "facet" | "columns" | "className" | "onPickVoice"> {
  /** Engine whose voices are open. Null shows the engine grid. */
  engine: string | null;
  onPickEngine: (engineId: string) => void;
  /** Returns to the engine grid from the voice level. */
  onDone: () => void;
}

/** Voices for one engine, mounted only while that level is open (hooks are per-engine). */
const EngineVoices = memo(({ engine, onDone }: { engine: string; onDone: () => void }) => {
  const { scope, ready } = useVoiceScope(engine);
  const catalog = useVoiceCatalog({
    provider: ready ? scope.voice_provider || undefined : undefined,
    suspended: !ready,
    engine,
    cloudSync: ready ? scope.cloud_sync : false,
  });

  return (
    <VoiceModal
      catalog={catalog}
      voiceScope={(scope.voice_provider || engine || "").toLowerCase()}
      canClone={ready && scope.can_clone}
      cloudSync={ready && scope.cloud_sync}
      onCommit={() => onDone()}
      onCancel={() => onDone()}
    />
  );
});

EngineVoices.displayName = "EngineVoices";

/**
 * Expanded TTS surface: the engine grid with a left facet rail. Each engine
 * card carries a waveform glyph that opens that engine's voices in place,
 * with the modal header back arrow returning to the grid.
 */
export const TtsModelsModal = memo(
  ({ engine, onPickEngine, onDone, ...gridProps }: TtsModelsModalProps) => {
    const [facet, setFacet] = useState<Facet>("all");
    const modelCatalog = useCatalogStore((s) => s.modelCatalog);
    const activeTtsId = useSettingsStore((s) => s.draftSettings?.tts.active.model ?? "");

    if (engine) {
      return <EngineVoices engine={engine} onDone={onDone} />;
    }

    const engineName =
      modelCatalog?.tts?.find((model) => model.id === activeTtsId)?.name ?? activeTtsId;

    const groups: CatalogFacetGroup[] = [
      {
        title: SPEECH_CARD_COPY.sourceFacet,
        items: [
          { id: "all", label: SPEECH_CARD_COPY.facetAll, Icon: Layers },
          { id: "local", label: SPEECH_CARD_COPY.facetLocal, Icon: Cpu },
          { id: "cloud", label: SPEECH_CARD_COPY.facetCloud, Icon: Cloud },
        ],
      },
    ];

    return (
      <div className="flex flex-col flex-1 min-h-0 gap-4">
        <div className="flex flex-1 min-h-0 gap-4">
          <div className="w-36 sm:w-44 shrink-0">
            <CatalogFacetRail
              groups={groups}
              selectedId={facet}
              onSelect={(id) => setFacet(id as Facet)}
            />
          </div>
          <TtsEngineGrid
            {...gridProps}
            facet={facet}
            variant="cards"
            className="flex-1 min-h-0"
            onPickVoice={onPickEngine}
          />
        </div>
        {/* Same placement as the voice preview banner: pinned under the list,
            never above it, so the catalog keeps the top of the viewport. */}
        <EngineFlowBanner engineName={engineName} />
      </div>
    );
  }
);

TtsModelsModal.displayName = "TtsModelsModal";
