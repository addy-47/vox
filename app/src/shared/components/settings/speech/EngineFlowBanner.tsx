import { memo } from "react";
import { SpeechFlowBanner, type SpeechFlowBannerProps } from "./SpeechFlowBanner";

export type EngineFlowBannerProps = Omit<SpeechFlowBannerProps, "kind">;

/**
 * Backward-compatible wrapper for TTS flow banner.
 * Delegates to the unified SpeechFlowBanner.
 */
export const EngineFlowBanner = memo((props: EngineFlowBannerProps) => {
  return <SpeechFlowBanner kind="tts" {...props} />;
});

EngineFlowBanner.displayName = "EngineFlowBanner";
export { SpeechFlowBanner };
