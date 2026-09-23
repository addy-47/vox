import React from "react";

export interface CloudProvider {
  id: string;
  name: string;
  url: string;
  keyPlaceholder: string;
  popular?: boolean;
  tagline?: string;
}

export const CLOUD_PROVIDERS: CloudProvider[] = [
  // Popular / Recommended
  { id: "opencode_zen", name: "OpenCode Zen", url: "https://api.opencode.ai/v1", keyPlaceholder: "zen_...", popular: true, tagline: "Reliable optimized models" },
  { id: "opencode_go", name: "OpenCode Go", url: "https://api.opencode.ai/v1", keyPlaceholder: "go_...", popular: true, tagline: "Low cost subscription for everyone" },
  { id: "openai", name: "OpenAI", url: "https://api.openai.com/v1", keyPlaceholder: "sk-proj-...", popular: true, tagline: "GPT-4o, o1, o3-mini" },
  { id: "anthropic", name: "Anthropic", url: "https://api.anthropic.com/v1", keyPlaceholder: "sk-ant-...", popular: true, tagline: "Claude 3.7 Sonnet, Claude 3.5 Haiku" },
  { id: "gemini", name: "Google Gemini", url: "https://generativelanguage.googleapis.com/v1beta/openai", keyPlaceholder: "AIzaSy...", popular: true, tagline: "Gemini 2.5 Flash & Pro" },
  { id: "openrouter", name: "OpenRouter", url: "https://openrouter.ai/api/v1", keyPlaceholder: "sk-or-v1-...", popular: true, tagline: "Unified multi-model routing gateway" },
  { id: "vercel", name: "Vercel AI Gateway", url: "https://gateway.ai.vercel.dev/v1", keyPlaceholder: "vcl_...", popular: true, tagline: "Edge AI gateway" },
  { id: "groq", name: "Groq", url: "https://api.groq.com/openai/v1", keyPlaceholder: "gsk_...", popular: true, tagline: "Ultra-fast LPU inference" },
  { id: "deepseek", name: "DeepSeek", url: "https://api.deepseek.com/v1", keyPlaceholder: "sk-...", popular: true, tagline: "DeepSeek-V3 & DeepSeek-R1" },
  { id: "together", name: "Together AI", url: "https://api.together.xyz/v1", keyPlaceholder: "tog_...", popular: true, tagline: "High-performance open source models" },
  { id: "mistral", name: "Mistral AI", url: "https://api.mistral.ai/v1", keyPlaceholder: "mis_...", popular: true, tagline: "Mistral Large, Codestral, Pixtral" },
  { id: "nvidia", name: "NVIDIA NIM", url: "https://integrate.api.nvidia.com/v1", keyPlaceholder: "nvapi-...", popular: true, tagline: "Enterprise NIM microservices" },
  { id: "cerebras", name: "Cerebras", url: "https://api.cerebras.ai/v1", keyPlaceholder: "csk-...", popular: true, tagline: "Wafer-scale high-throughput inference" },
  { id: "fireworks", name: "Fireworks AI", url: "https://api.fireworks.ai/inference/v1", keyPlaceholder: "fw_...", popular: true, tagline: "Fast production inference platform" },
  { id: "perplexity", name: "Perplexity", url: "https://api.perplexity.ai", keyPlaceholder: "pplx-...", popular: true, tagline: "Sonar search-grounded LLMs" },
  { id: "xai", name: "xAI (Grok)", url: "https://api.x.ai/v1", keyPlaceholder: "xai-...", popular: true, tagline: "Grok-2 & Grok-beta" },
  { id: "cohere", name: "Cohere", url: "https://api.cohere.com/v2", keyPlaceholder: "coh_...", popular: true, tagline: "Command R+ enterprise intelligence" },

  // Specialized / Gateways / Cloud Providers
  { id: "siliconflow", name: "SiliconFlow", url: "https://api.siliconflow.cn/v1", keyPlaceholder: "sk-...", tagline: "Fast cloud inference across top OSS models" },
  { id: "moonshot", name: "Moonshot AI (Kimi)", url: "https://api.moonshot.cn/v1", keyPlaceholder: "sk-...", tagline: "Kimi long-context models" },
  { id: "minimax", name: "MiniMax", url: "https://api.minimax.chat/v1", keyPlaceholder: "mm_...", tagline: "MiniMax-01 reasoning and dialogue" },
  { id: "alibaba", name: "Alibaba DashScope", url: "https://dashscope.aliyuncs.com/compatible-mode/v1", keyPlaceholder: "sk-...", tagline: "Qwen 2.5 series" },
  { id: "zhipu", name: "Zhipu AI (GLM)", url: "https://open.bigmodel.cn/api/paas/v4", keyPlaceholder: "glm_...", tagline: "GLM-4 foundation models" },
  { id: "deepinfra", name: "Deep Infra", url: "https://api.deepinfra.com/v1/openai", keyPlaceholder: "di_...", tagline: "Serverless model inference" },
  { id: "novita", name: "Novita AI", url: "https://api.novita.ai/v3/openai", keyPlaceholder: "nov_...", tagline: "Low latency serverless GPU inference" },
  { id: "chutes", name: "Chutes", url: "https://chutes.ai/v1", keyPlaceholder: "ch_...", tagline: "Decentralized GPU inference" },
  { id: "baseten", name: "Baseten", url: "https://bridge.baseten.co/v1", keyPlaceholder: "base_...", tagline: "Dedicated model deployments" },
  { id: "databricks", name: "Databricks", url: "https://<workspace>.databricks.com/serving-endpoints", keyPlaceholder: "dapi...", tagline: "Enterprise lakehouse model serving" },
  { id: "cloudflare", name: "Cloudflare AI Gateway", url: "https://gateway.ai.cloudflare.com/v1", keyPlaceholder: "cf_...", tagline: "Universal proxy with caching & guardrails" },
  { id: "friendli", name: "Friendli", url: "https://inference.friendli.ai/v1", keyPlaceholder: "flp_...", tagline: "Optimized throughput engine" },
  { id: "modal", name: "Modal", url: "https://<app>.modal.run/v1", keyPlaceholder: "mod_...", tagline: "Serverless custom cloud functions" },
  { id: "stepfun", name: "StepFun", url: "https://api.stepfun.com/v1", keyPlaceholder: "step_...", tagline: "Step-1 & Step-2 multimodal models" },
  { id: "ai21", name: "AI21 Labs", url: "https://api.ai21.com/studio/v1", keyPlaceholder: "ai21_...", tagline: "Jamba hybrid architecture models" },
  { id: "huggingface", name: "Hugging Face", url: "https://api-inference.huggingface.co/v1", keyPlaceholder: "hf_...", tagline: "Serverless inference API" },
  { id: "ollama_cloud", name: "Ollama Cloud", url: "https://ollama.ai/api/v1", keyPlaceholder: "ollama_...", tagline: "Managed Ollama endpoints" },

  // Custom
  { id: "custom", name: "Custom OpenAI-compatible", url: "https://api.example.com/v1", keyPlaceholder: "API Key / Token", tagline: "Any standard /v1/chat/completions endpoint" },
];

export const GeminiLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg viewBox="0 0 24 24" fill="none" {...props}>
    <path
      d="M12 3c0 4.5 3.5 8 8 8-4.5 0-8 3.5-8 8 0-4.5-3.5-8-8-8 4.5 0 8-3.5 8-8z"
      fill="currentColor"
      opacity={active ? 0.9 : 0.45}
    />
  </svg>
);

export const OpenAiLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg viewBox="0 0 24 24" fill="currentColor" {...props}>
    <path
      d="M21.3 11.1c0-.7-.2-1.4-.6-2-.4-.6-1-.9-1.7-1.1-.1-.7-.4-1.3-.9-1.8s-1.1-.9-1.8-1c-.5-.5-1.1-.9-1.8-1-.7-.2-1.4-.2-2.1 0-.6.2-1.2.5-1.7 1-.5-.5-1.1-.8-1.7-1-.7-.2-1.4-.2-2.1 0-.7.2-1.3.5-1.8 1-.5.5-.8 1.1-.9 1.8-.7.1-1.3.4-1.8.9C3 8.4 2.7 9 2.6 9.7c-.5.5-.9 1.1-1 1.8-.2.7-.2 1.4 0 2.1.2.6.5 1.2 1 1.7-.5.5-.8 1.1-1 1.7-.2.7-.2 1.4 0 2.1.2.7.5 1.3 1 1.8.5.5 1.1.8 1.8.9.1.7.4 1.3.9 1.8.5.5 1.1.9 1.8 1 .5.5 1.1.9 1.8 1 .7.2 1.4.2 2.1 0 .6-.2 1.2-.5 1.7-1 .5.5 1.1.8 1.7 1 .7.2 1.4.2 2.1 0 .7-.2 1.3-.5 1.8-1 .5-.5.8-1.1.9-1.8.7-.1 1.3-.4 1.8-.9.5-.5.8-1.1.9-1.8.5-.5.9-1.1 1-1.8.2-.7.2-1.4 0-2.1-.2-.6-.5-1.2-1-1.7.5-.5.8-1.1 1-1.7.2-.6.2-1.3 0-2zm-8.8 7.3l-2.9-1.7c-.2-.1-.3-.3-.3-.6V12.7l1.4.8c.2.1.3.3.3.6v2.1l1.5.9v-4.2l-1.4-.8c-.2-.1-.3-.3-.3-.6V9.3l2.9 1.7c.2.1.3.3.3.6v3.4l-1.4-.8c-.2-.1-.3-.3-.3-.6v-2.1l-1.5-.9v4.2l1.4.8c.2.1.3.3.3.6v1.9z"
      fill="currentColor"
      opacity={active ? 0.9 : 0.45}
    />
  </svg>
);

export const DeepgramLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2.5"
    strokeLinecap="round"
    strokeLinejoin="round"
    {...props}
  >
    <polygon
      points="12 2 22 8.5 22 15.5 12 22 2 15.5 2 8.5"
      opacity={active ? 0.95 : 0.5}
    />
    <path d="M12 22V12" opacity={active ? 0.95 : 0.5} />
    <path d="M12 12L22 8.5" opacity={active ? 0.95 : 0.5} />
    <path d="M12 12L2 8.5" opacity={active ? 0.95 : 0.5} />
  </svg>
);

export const ElevenLabsLogo = ({
  active,
  ...props
}: { active?: boolean } & React.SVGProps<SVGSVGElement>) => (
  <svg viewBox="0 0 24 24" fill="currentColor" {...props}>
    <rect
      x="5"
      y="4"
      width="5.5"
      height="16"
      rx="2.5"
      fill="currentColor"
      opacity={active ? 0.95 : 0.45}
    />
    <rect
      x="13.5"
      y="4"
      width="5.5"
      height="16"
      rx="2.5"
      fill="currentColor"
      opacity={active ? 0.75 : 0.3}
    />
  </svg>
);

export const CLOUD_PROVIDER_HOSTS = CLOUD_PROVIDERS.map((p) => {
  try {
    return new URL(p.url).hostname;
  } catch {
    return p.id;
  }
});

export const REALTIME_PROVIDERS = [
  {
    id: "gemini_live",
    name: "Gemini Live",
    subkey: "gemini_live",
    icon: GeminiLogo,
    desc: "Live Speech",
    url: "https://aistudio.google.com/apikey",
    tagline: "Google's direct live speech model with search grounding",
    keyPlaceholder: "AIzaSy...",
  },
  {
    id: "deepgram_voice_agent",
    name: "Deepgram Agent",
    subkey: "deepgram_voice_agent",
    icon: DeepgramLogo,
    desc: "Voice Assistant",
    url: "https://console.deepgram.com/",
    tagline: "Deepgram's real-time conversational voice agent",
    keyPlaceholder: "Token...",
  },
] as const;

