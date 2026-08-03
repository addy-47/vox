import { memo } from "react";

export const OpenAIIcon = memo(({ size = 18 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M12 2a10 10 0 1 0 10 10A10 10 0 0 0 12 2zm0 18a8 8 0 1 1 8-8 8 8 0 0 1-8 8z" />
    <path d="M12 6a6 6 0 1 0 6 6 6 6 0 0 0-6-6zm0 10a4 4 0 1 1 4-4 4 4 0 0 1-4 4z" />
  </svg>
));
OpenAIIcon.displayName = "OpenAIIcon";

export const GeminiIcon = memo(({ size = 18 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
    <path d="M12 0C12 6.627 6.627 12 0 12c6.627 0 12 5.373 12 12 0-6.627 5.373-12 12-12-6.627 0-12-5.373-12-12z" />
  </svg>
));
GeminiIcon.displayName = "GeminiIcon";

export const AnthropicIcon = memo(({ size = 18 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
    <path d="M17.472 17.472H21.6L13.8 3h-3.6L2.4 17.472h4.128l1.536-2.88h7.872l1.536 2.88zM10.2 9.12l1.8-3.36 1.8 3.36h-3.6z" />
  </svg>
));
AnthropicIcon.displayName = "AnthropicIcon";

export const DeepgramIcon = memo(({ size = 18 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
    <circle cx="12" cy="12" r="9" />
    <path d="M12 7v10M8 10v4M16 10v4" />
  </svg>
));
DeepgramIcon.displayName = "DeepgramIcon";

export const ElevenLabsIcon = memo(({ size = 18 }: { size?: number }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
    <path d="M7 4h3v16H7V4zm7 0h3v16h-3V4z" />
  </svg>
));
ElevenLabsIcon.displayName = "ElevenLabsIcon";
