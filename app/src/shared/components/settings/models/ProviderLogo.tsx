import { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface ProviderLogoProps {
  providerId: string;
  size?: number;
  className?: string;
}

export const ProviderLogo = memo(function ProviderLogo({
  providerId,
  size = 16,
  className,
}: ProviderLogoProps) {
  const norm = (providerId || "").toLowerCase().trim();

  // 1. OpenAI
  if (norm.includes("openai") || norm.includes("whisper") || norm.includes("chatgpt")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="currentColor"
        className={cn("shrink-0 text-[rgb(var(--foreground))]", className)}
      >
        <path d="M22.28 10.36a5.75 5.75 0 0 0-.48-4.54 5.92 5.92 0 0 0-4.04-2.8 5.86 5.86 0 0 0-4.9 1.13A5.78 5.78 0 0 0 8.52 3.5a5.92 5.92 0 0 0-4.63 2.5 5.82 5.82 0 0 0-.82 4.98 5.78 5.78 0 0 0-1.8 4.2 5.9 5.9 0 0 0 2.2 4.6 5.86 5.86 0 0 0 4.25 1.48 5.8 5.8 0 0 0 4.34 1.94 5.92 5.92 0 0 0 4.63-2.5 5.82 5.82 0 0 0 .82-4.98 5.78 5.78 0 0 0 1.8-4.2 5.9 5.9 0 0 0-2.03-5.16zm-7.66 11.23a4.26 4.26 0 0 1-2.9-1.15l.13-.07 4.8-2.77a.82.82 0 0 0 .42-.72v-6.77l2.03 1.17a.08.08 0 0 1 .05.06v5.82a4.3 4.3 0 0 1-4.53 4.43zm-9.35-4.1a4.23 4.23 0 0 1-.5-3.07l.13.08 4.8 2.77a.8.8 0 0 0 .82 0l5.86-3.39v2.34a.09.09 0 0 1-.04.07l-5.05 2.92a4.3 4.3 0 0 1-6.02-1.72zm-1.8-8.9a4.25 4.25 0 0 1 2.39-1.92v5.6a.82.82 0 0 0 .4.72l5.87 3.39-2.03 1.17a.08.08 0 0 1-.07 0l-5.04-2.9a4.3 4.3 0 0 1-1.52-6.06zm13.1 2.65l-5.86-3.38 2.03-1.17a.08.08 0 0 1 .07 0l5.05 2.91a4.3 4.3 0 0 1 1.52 6.07 4.25 4.25 0 0 1-2.4 1.92v-5.62a.82.82 0 0 0-.41-.73zm2.5-3.23a4.26 4.26 0 0 1 .5 3.08l-.13-.08-4.8-2.77a.8.8 0 0 0-.82 0l-5.86 3.38v-2.34a.09.09 0 0 1 .04-.07l5.05-2.92a4.3 4.3 0 0 1 6.02 1.72zm-8.8 5.08l-2.35-1.36 2.35-1.35 2.36 1.35-2.36 1.36z" />
      </svg>
    );
  }

  // 2. Anthropic / Claude
  if (norm.includes("anthropic") || norm.includes("claude")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="currentColor"
        className={cn("shrink-0 text-[#d97757]", className)}
      >
        <path d="M17.47 3.38L12.5 13.92H11.5L6.53 3.38H4L10.9 18.06H13.1L20 3.38h-2.53zM12 18.78a1.84 1.84 0 1 0 0 3.68 1.84 1.84 0 0 0 0-3.68z" />
      </svg>
    );
  }

  // 3. Google Gemini
  if (norm.includes("gemini") || norm.includes("google")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        className={cn("shrink-0", className)}
      >
        <path
          d="M12 2C12 7.52 7.52 12 2 12C7.52 12 12 16.48 12 22C12 16.48 16.48 12 22 12C16.48 12 12 7.52 12 2Z"
          fill="url(#gemini-grad)"
        />
        <defs>
          <linearGradient id="gemini-grad" x1="2" y1="2" x2="22" y2="22" gradientUnits="userSpaceOnUse">
            <stop stopColor="#4E82EE" />
            <stop offset="0.5" stopColor="#9B72CB" />
            <stop offset="1" stopColor="#D96570" />
          </linearGradient>
        </defs>
      </svg>
    );
  }

  // 4. OpenRouter
  if (norm.includes("openrouter")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className={cn("shrink-0 text-[rgb(var(--accent))]", className)}
      >
        <circle cx="12" cy="12" r="9" />
        <path d="M12 7v5l3 3" />
        <path d="M16 12h2" />
        <path d="M6 12h2" />
      </svg>
    );
  }

  // 5. DeepSeek
  if (norm.includes("deepseek")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="#0066FF"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className={cn("shrink-0", className)}
      >
        <path d="M3 13.5C3 13.5 6 9 12 9C18 9 21 13.5 21 13.5" />
        <path d="M12 9V3" />
        <path d="M7 16.5C8.5 18 10.2 19 12 19C13.8 19 15.5 18 17 16.5" />
      </svg>
    );
  }

  // 6. Groq
  if (norm.includes("groq")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="#F55036"
        className={cn("shrink-0", className)}
      >
        <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 14.5h-2v-5h2v5zm0-7h-2V7h2v2.5z" />
      </svg>
    );
  }

  // 7. Together AI
  if (norm.includes("together")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="#0F6FFF"
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        className={cn("shrink-0", className)}
      >
        <circle cx="8" cy="12" r="5" />
        <circle cx="16" cy="12" r="5" />
      </svg>
    );
  }

  // 8. Mistral AI
  if (norm.includes("mistral")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="#FF7000"
        className={cn("shrink-0", className)}
      >
        <rect x="3" y="4" width="4" height="4" />
        <rect x="17" y="4" width="4" height="4" />
        <rect x="3" y="10" width="4" height="4" />
        <rect x="10" y="10" width="4" height="4" />
        <rect x="17" y="10" width="4" height="4" />
        <rect x="3" y="16" width="4" height="4" />
        <rect x="17" y="16" width="4" height="4" />
      </svg>
    );
  }

  // 9. Cerebras
  if (norm.includes("cerebras")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="currentColor"
        className={cn("shrink-0 text-[#FF5A00]", className)}
      >
        <polygon points="12,2 22,8.5 22,15.5 12,22 2,15.5 2,8.5" />
      </svg>
    );
  }

  // 10. Fireworks AI
  if (norm.includes("fireworks")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="#F43F5E"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className={cn("shrink-0", className)}
      >
        <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83" />
      </svg>
    );
  }

  // 11. Perplexity
  if (norm.includes("perplexity")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="#20B8CD"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className={cn("shrink-0", className)}
      >
        <path d="M12 2v20M2 12h20M5 5l14 14M19 5L5 19" />
      </svg>
    );
  }

  // 12. xAI (Grok)
  if (norm.includes("xai") || norm.includes("grok")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="currentColor"
        className={cn("shrink-0 text-[rgb(var(--foreground))]", className)}
      >
        <path d="M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z" />
      </svg>
    );
  }

  // 13. Cohere
  if (norm.includes("cohere")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="#39594D"
        className={cn("shrink-0", className)}
      >
        <circle cx="8" cy="8" r="4" fill="#D45D5D" />
        <circle cx="16" cy="10" r="5" fill="#39594D" />
        <circle cx="11" cy="17" r="4" fill="#3B5998" />
      </svg>
    );
  }

  // 14. Ollama
  if (norm.includes("ollama")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="currentColor"
        className={cn("shrink-0 text-[rgb(var(--foreground))]", className)}
      >
        <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeWidth="2" />
        <path d="M12 7c-2 0-3 1.5-3 3.5 0 2.5 1.5 4 3 6 1.5-2 3-3.5 3-6 0-2-1-3.5-3-3.5z" />
      </svg>
    );
  }

  // 15. LM Studio
  if (norm.includes("lmstudio") || norm.includes("lm-studio")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="#8B5CF6"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className={cn("shrink-0", className)}
      >
        <rect x="4" y="4" width="16" height="16" rx="3" />
        <path d="M9 9h6v6H9z" />
      </svg>
    );
  }

  // 16. vLLM
  if (norm.includes("vllm")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="#06B6D4"
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeLinejoin="round"
        className={cn("shrink-0", className)}
      >
        <path d="M4 6l8 12 8-12" />
      </svg>
    );
  }

  // 17. Vercel
  if (norm.includes("vercel")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="currentColor"
        className={cn("shrink-0 text-[rgb(var(--foreground))]", className)}
      >
        <polygon points="12,3 22,21 2,21" />
      </svg>
    );
  }

  // 18. OpenCode Zen / Go
  if (norm.includes("opencode") || norm.includes("zen") || norm.includes("go")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2.2"
        strokeLinecap="round"
        strokeLinejoin="round"
        className={cn("shrink-0 text-[rgb(var(--accent))]", className)}
      >
        <polyline points="16 18 22 12 16 6" />
        <polyline points="8 6 2 12 8 18" />
      </svg>
    );
  }

  // 19. Microsoft / Edge TTS
  if (norm.includes("microsoft") || norm.includes("edge")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        className={cn("shrink-0", className)}
      >
        <rect x="2" y="2" width="9.5" height="9.5" fill="#F25022" />
        <rect x="12.5" y="2" width="9.5" height="9.5" fill="#7FBA00" />
        <rect x="2" y="12.5" width="9.5" height="9.5" fill="#00A4EF" />
        <rect x="12.5" y="12.5" width="9.5" height="9.5" fill="#FFB900" />
      </svg>
    );
  }

  // 20. Azure
  if (norm.includes("azure")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="#0078D4"
        className={cn("shrink-0", className)}
      >
        <path d="M13.05 2.12l-7.39 13.9a.78.78 0 0 0 .69 1.15h4.63L7.33 21.88h9.34l5.3-19.76h-8.92z" />
      </svg>
    );
  }

  // 21. NVIDIA
  if (norm.includes("nvidia") || norm.includes("nemotron")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="#76B900"
        className={cn("shrink-0", className)}
      >
        <path d="M10.87 5.25c-3.1 0-5.63 2.53-5.63 5.63 0 2.68 1.88 4.92 4.41 5.47v-1.78c-1.57-.48-2.73-1.94-2.73-3.69 0-2.12 1.72-3.84 3.84-3.84 1.48 0 2.77.83 3.42 2.05h1.9c-.77-2.26-2.92-3.84-5.21-3.84zm0-3C5.97 2.25 2 6.22 2 11.12c0 4.14 2.84 7.63 6.7 8.62v-2.1c-2.75-.9-4.7-3.48-4.7-6.52 0-3.8 3.08-6.88 6.87-6.88 2.87 0 5.34 1.76 6.38 4.28h2.24c-1.18-3.74-4.68-6.37-8.62-6.37z" />
      </svg>
    );
  }

  // 22. Meta / Llama
  if (norm.includes("meta") || norm.includes("llama")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="#0081FB"
        className={cn("shrink-0", className)}
      >
        <path d="M12 7.2c-2.4 0-4.3 1.9-5.4 3.8-1.1-1.9-3-3.8-5.4-3.8C.5 7.2 0 8.7 0 10.6c0 3.7 3.3 6.6 6.6 6.6 2.4 0 4.3-1.9 5.4-3.8 1.1 1.9 3 3.8 5.4 3.8 3.3 0 6.6-2.9 6.6-6.6 0-1.9-.5-3.4-1.2-3.4-2.4 0-4.3 1.9-5.4 3.8-1.1-1.9-3-3.8-5.4-3.8z" />
      </svg>
    );
  }

  // 23. Hugging Face
  if (norm.includes("huggingface") || norm.includes("hf")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="#FFD21E"
        className={cn("shrink-0", className)}
      >
        <circle cx="12" cy="12" r="10" />
        <circle cx="8.5" cy="10" r="1.5" fill="#000" />
        <circle cx="15.5" cy="10" r="1.5" fill="#000" />
        <path d="M8 15s1.5 2 4 2 4-2 4-2" stroke="#000" strokeWidth="1.5" strokeLinecap="round" fill="none" />
      </svg>
    );
  }

  // 24. Cloudflare
  if (norm.includes("cloudflare")) {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="#F38020"
        className={cn("shrink-0", className)}
      >
        <path d="M19.35 10.04C18.67 6.59 15.64 4 12 4 9.11 4 6.6 5.64 5.35 8.04 2.34 8.36 0 10.91 0 14c0 3.31 2.69 6 6 6h13c2.76 0 5-2.24 5-5 0-2.64-2.05-4.78-4.65-4.96z" />
      </svg>
    );
  }

  // 25. SiliconFlow / Moonshot / MiniMax / Alibaba / Zhipu
  if (norm.includes("moonshot") || norm.includes("kimi")) {
    return (
      <svg width={size} height={size} viewBox="0 0 24 24" fill="#6366F1" className={cn("shrink-0", className)}>
        <path d="M12 2a10 10 0 1 0 10 10A10 10 0 0 1 12 2z" />
      </svg>
    );
  }

  if (norm.includes("siliconflow") || norm.includes("minimax") || norm.includes("alibaba") || norm.includes("qwen") || norm.includes("zhipu")) {
    return (
      <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" className={cn("shrink-0 text-[rgb(var(--accent))]", className)}>
        <polygon points="12 2 2 7 12 12 22 7 12 2" />
        <polyline points="2 17 12 22 22 17" />
        <polyline points="2 12 12 17 22 12" />
      </svg>
    );
  }

  // 26. On-Device / Embedded / Local
  if (norm.includes("embedded") || norm.includes("local") || norm.includes("on-device") || norm.includes("on_device")) {
    return (
      <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={cn("shrink-0 text-[rgb(var(--accent))]", className)}>
        <rect x="2" y="2" width="20" height="8" rx="2" />
        <rect x="2" y="14" width="20" height="8" rx="2" />
        <line x1="6" y1="6" x2="6.01" y2="6" strokeWidth="2.5" />
        <line x1="6" y1="18" x2="6.01" y2="18" strokeWidth="2.5" />
      </svg>
    );
  }

  // 27. Remote GPU / Custom Server
  if (norm.includes("server") || norm.includes("custom") || norm.includes("remote")) {
    return (
      <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={cn("shrink-0 text-[rgb(var(--accent))]", className)}>
        <rect x="2" y="4" width="20" height="16" rx="2" />
        <line x1="6" y1="12" x2="6.01" y2="12" strokeWidth="2.5" />
        <line x1="10" y1="12" x2="18" y2="12" />
      </svg>
    );
  }

  // Generic Clean Gateway Glyph (for unknown custom endpoints)
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className={cn("shrink-0 text-[rgb(var(--foreground-muted))]", className)}>
      <circle cx="12" cy="12" r="10" />
      <line x1="2" y1="12" x2="22" y2="12" />
      <path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" />
    </svg>
  );
});

ProviderLogo.displayName = "ProviderLogo";
