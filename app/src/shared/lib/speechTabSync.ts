/**
 * Typed bridge from provider-selection surfaces to the Speech card's category
 * strip. Replaces the raw `sync_pipeline_tab` window CustomEvent: only the
 * STT/TTS requests have a consumer, so the helper narrows the contract to
 * exactly those two.
 */
export type SpeechTabRequest = "stt" | "tts";

const SPEECH_TAB_EVENT = "vox:speech-tab-request";

/** Asks the Speech card to switch its category strip to `tab`. */
export function requestSpeechTab(tab: SpeechTabRequest): void {
  window.dispatchEvent(new CustomEvent<SpeechTabRequest>(SPEECH_TAB_EVENT, { detail: tab }));
}

/** Subscribes to speech-tab requests. Returns the unsubscribe function. */
export function onSpeechTabRequest(handler: (tab: SpeechTabRequest) => void): () => void {
  const listener = (e: Event) => handler((e as CustomEvent<SpeechTabRequest>).detail);
  window.addEventListener(SPEECH_TAB_EVENT, listener);
  return () => window.removeEventListener(SPEECH_TAB_EVENT, listener);
}
