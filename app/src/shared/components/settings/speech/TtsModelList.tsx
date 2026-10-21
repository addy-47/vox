import { memo, useState } from "react";
import { AudioLines } from "lucide-react";
import { ExpandableList } from "@/shared/ui/ExpandableList";
import { SettingsCommitControls } from "@/shared/components/settings/SettingsCommitControls";
import { SPEECH_CARD_COPY, VOICE_CATALOG_COPY } from "@/data/settingsCopy";
import { TtsEngineGrid, type TtsEngineGridProps } from "./TtsEngineGrid";
import { TtsModelsModal } from "./TtsModelsModal";
import { getSettingsInlineListMaxHeightClass } from "../shared/settingsLayout";

export type TtsModelListProps = Omit<TtsEngineGridProps, "facet" | "columns" | "className" | "onPickVoice">;

/**
 * Text-to-speech engines inside the speech card. The corner trigger opens the
 * expanded surface; picking an engine's waveform there opens its voices in the
 * same modal.
 */
export const TtsModelList = memo((props: TtsModelListProps) => {
  const [engine, setEngine] = useState<string | null>(null);

  return (
    <ExpandableList
      className="flex-1 min-h-0"
      inlineMaxHeightClass={getSettingsInlineListMaxHeightClass(props.layoutMode)}
      triggerPlacement="floating"
      icon={<AudioLines size={16} className="text-[rgb(var(--accent))]" />}
      title={engine ? VOICE_CATALOG_COPY.title : SPEECH_CARD_COPY.ttsModelsTitle}
      modalSubtitle={engine ? VOICE_CATALOG_COPY.subtitle : SPEECH_CARD_COPY.ttsModelsSubtitle}
      expandLabel={SPEECH_CARD_COPY.expandTts}
      ariaLabel={SPEECH_CARD_COPY.ttsModelsAria}
      headerActions={<SettingsCommitControls domainId="speech" layoutMode={props.layoutMode} />}
      onBack={engine ? () => setEngine(null) : undefined}
      modalContent={() => (
        <TtsModelsModal
          {...props}
          engine={engine}
          onPickEngine={setEngine}
          onDone={() => setEngine(null)}
        />
      )}
    >
      <TtsEngineGrid {...props} className="h-full" />
    </ExpandableList>
  );
});

TtsModelList.displayName = "TtsModelList";
