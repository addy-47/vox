import { memo, useState, useMemo, useCallback, useDeferredValue, useRef, useEffect } from "react";
import { useLenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";
import { useScrollTopOnChange } from "@/shared/hooks/useScrollTopOnChange";
import { CLOUD_PROVIDERS, type CloudProvider } from "@/data/providersCopy";
import { INTERACTION_CONFIG_DESK_COPY, SETTINGS_COPY } from "@/data/settingsCopy";
import { useSettingsStore, type LlmCustomServer, type TtsCustomServer } from "@/store/settingsStore";
import { checkLlmProviderHealth } from "@/services/settingsService";
import { ipcErrorMessage } from "@/shared/lib/ipcError";
import { activeSelection } from "@/shared/lib/activeSelection";
import { SettingsFooterBanner } from "@/shared/components/settings/SettingsCommitControls";
import { Tooltip } from "@/shared/ui/Tooltip";
import {
  AlertCircle,
  ArrowDownAZ,
  ArrowLeft,
  ArrowUpZA,
  ArrowRight,
  Check,
  Cloud,
  Filter,
  Globe,
  HardDrive,
  HelpCircle,
  Key,
  KeyRound,
  Pencil,
  Plus,
  Search,
  Trash2,
  X,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { VoxLogoLoader } from "@/shared/components/common";
import { ProviderGuideView } from "../models/catalog/ProviderGuideView";
import { ProviderLogo } from "./ProviderLogo";
import { TTS_MODAL_PROVIDERS, type TtsProviderEntry } from "@/data/providersCopy";

export interface CloudProviderCardProps {
  provider: CloudProvider;
  isSelected: boolean;
  savedKey: string | undefined;
  isEditing: boolean;
  editingKeyValue: string;
  setEditingKeyValue: (val: string) => void;
  onSelect: (provider: CloudProvider) => void;
  onStartInlineEdit: (id: string, currentKey: string) => void;
  onCancelInlineKey: () => void;
  onSaveInlineKey: (id: string) => void;
  onNavigateToModels?: (provider: CloudProvider) => void;
}

export const CloudProviderCard = memo(({
  provider,
  isSelected,
  savedKey,
  isEditing,
  editingKeyValue,
  setEditingKeyValue,
  onSelect,
  onStartInlineEdit,
  onCancelInlineKey,
  onSaveInlineKey,
  onNavigateToModels,
}: CloudProviderCardProps) => {
  const hasKey = Boolean(savedKey && savedKey.trim().length > 0);

  return (
    <div
      onClick={() => {
        if (!isEditing) {
          onSelect(provider);
        }
      }}
      className={cn(
        "relative flex flex-col justify-between p-4.5 rounded-xl border transition-all duration-200 select-none group cursor-pointer transform-gpu h-[188px]",
        isSelected
          ? "bg-[rgba(var(--card),0.7)] border-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.18)] ring-1 ring-[rgb(var(--accent))]/30"
          : "bg-[rgba(var(--card),0.4)] border-[rgba(var(--foreground),0.07)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.65)]"
      )}
    >
      <div className="absolute top-3 right-3 flex items-center gap-1.5 z-10">
        {onNavigateToModels && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onSelect(provider);
              onNavigateToModels(provider);
            }}
            className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-colors cursor-pointer"
            title="Browse models"
            aria-label="Browse models"
          >
            <ArrowRight size={13} strokeWidth={2} />
          </button>
        )}
      </div>

      <div className="flex flex-col items-center text-center my-auto px-1 pt-1">
        <div className={cn(
          "w-12 h-12 rounded-2xl flex items-center justify-center shrink-0 border transition-all",
          isSelected
            ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] shadow-[0_0_12px_rgba(var(--accent),0.15)]"
            : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--foreground),0.07)] group-hover:border-[rgba(var(--accent),0.25)]"
        )}>
          <ProviderLogo providerId={provider.id} size={24} />
        </div>
        <h4 className="font-bold text-[14.5px] text-[rgb(var(--foreground))] tracking-tight truncate w-full text-center mt-2.5 leading-tight group-hover:text-[rgb(var(--accent))] transition-colors">
          {provider.name}
        </h4>
        <p className="text-[12px] text-[rgb(var(--foreground-muted))] mt-1 line-clamp-2 leading-relaxed text-center px-1 h-[36px]">
          {provider.tagline || "OpenAI-compatible inference gateway."}
        </p>
      </div>

      <div className="mt-auto pt-2.5 border-t border-[rgba(var(--foreground),0.06)] h-[28px] flex items-center justify-between" onClick={(e) => e.stopPropagation()}>
        {isEditing ? (
          <div className="flex items-center gap-1.5 border-b border-[rgb(var(--accent))] pb-0.5 w-full bg-transparent animate-fade-in">
            <input
              type="password"
              autoFocus
              value={editingKeyValue}
              onChange={(e) => setEditingKeyValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") onSaveInlineKey(provider.id);
                if (e.key === "Escape") onCancelInlineKey();
              }}
              placeholder={provider.keyPlaceholder}
              className="w-full bg-transparent text-[11px] font-mono outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/30"
            />
            <button
              type="button"
              onClick={() => onSaveInlineKey(provider.id)}
              title={INTERACTION_CONFIG_DESK_COPY.llm.cloud.saveKey}
              aria-label={INTERACTION_CONFIG_DESK_COPY.llm.cloud.saveKey}
              className="text-[rgb(var(--accent))] hover:opacity-75 transition-opacity p-0.5 shrink-0 bg-transparent cursor-pointer"
            >
              <Check size={13} strokeWidth={2.2} />
            </button>
            <button
              type="button"
              onClick={onCancelInlineKey}
              title={INTERACTION_CONFIG_DESK_COPY.llm.cloud.cancelKey}
              aria-label={INTERACTION_CONFIG_DESK_COPY.llm.cloud.cancelKey}
              className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0 bg-transparent cursor-pointer"
            >
              <X size={13} />
            </button>
          </div>
        ) : (
          <div className="flex items-center justify-between text-[11px] font-mono w-full">
            <span className="text-[rgb(var(--foreground-muted))]/70">Cloud Gateway</span>
            {hasKey ? (
              <div className="flex items-center gap-1.5">
                <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))] shrink-0" />
                <span className="text-[rgb(var(--foreground-muted))]">
                  ••••••••{savedKey?.slice(-4)}
                </span>
                <button
                  type="button"
                  onClick={() => onStartInlineEdit(provider.id, savedKey ?? "")}
                  className="p-1 rounded hover:bg-white/[0.08] hover:text-[rgb(var(--accent))] transition-colors text-[rgb(var(--foreground-muted))]/60 cursor-pointer shrink-0"
                  title="Edit API Key"
                >
                  <Pencil size={11} />
                </button>
              </div>
            ) : (
              <button
                type="button"
                onClick={() => onStartInlineEdit(provider.id, "")}
                className="text-[rgb(var(--accent))] hover:underline cursor-pointer font-medium flex items-center gap-1"
              >
                + Connect Key
              </button>
            )}
          </div>
        )}
      </div>
    </div>
  );
});
CloudProviderCard.displayName = "CloudProviderCard";

export interface LocalProviderCardProps {
  category: "STT" | "LLM" | "TTS";
  isSelected: boolean;
  onSelect: () => void;
}

export const LocalProviderCard = memo(({
  category,
  isSelected,
  onSelect,
}: LocalProviderCardProps) => {
  const meta = {
    STT: {
      name: "On-Device Speech Recognition",
      tagline: "High-accuracy local Whisper & Nemotron transcription. 100% offline.",
    },
    LLM: {
      name: "On-Device Model",
      tagline: "Local GGUF neural reasoning on CPU or GPU. Zero telemetry, private.",
    },
    TTS: {
      name: "On-Device Neural Voices",
      tagline: "Local Kokoro neural synthesis with peak normalization and zero latency.",
    },
  }[category];

  return (
    <div
      onClick={onSelect}
      className={cn(
        "relative flex flex-col justify-between p-4.5 rounded-xl border transition-all duration-200 select-none group cursor-pointer transform-gpu h-[188px]",
        isSelected
          ? "bg-[rgba(var(--card),0.7)] border-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.18)] ring-1 ring-[rgb(var(--accent))]/30"
          : "bg-[rgba(var(--card),0.4)] border-[rgba(var(--foreground),0.07)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.65)]"
      )}
    >
      <div className="flex flex-col items-center text-center my-auto px-1 pt-1">
        <div className={cn(
          "w-12 h-12 rounded-2xl flex items-center justify-center shrink-0 border transition-all",
          isSelected
            ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] shadow-[0_0_12px_rgba(var(--accent),0.15)] text-[rgb(var(--accent))]"
            : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--foreground),0.07)] text-[rgb(var(--accent))] group-hover:border-[rgba(var(--accent),0.25)]"
        )}>
          <VoxLogoLoader size={24} animated={false} />
        </div>
        <h4 className="font-bold text-[14.5px] text-[rgb(var(--foreground))] tracking-tight truncate w-full text-center mt-2.5 leading-tight group-hover:text-[rgb(var(--accent))] transition-colors">
          {meta.name}
        </h4>
        <p className="text-[12px] text-[rgb(var(--foreground-muted))] mt-1 line-clamp-2 leading-relaxed text-center px-1 h-[36px]">
          {meta.tagline}
        </p>
      </div>

      <div className="mt-auto pt-2.5 flex items-center justify-between text-[11px] font-mono border-t border-[rgba(var(--foreground),0.06)] h-[28px]">
        <span className="text-[rgb(var(--foreground-muted))]/70">Local Hardware</span>
        <span className="text-[rgb(var(--accent))] font-bold flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Offline Ready
        </span>
      </div>
    </div>
  );
});
LocalProviderCard.displayName = "LocalProviderCard";

export interface CustomProviderCreationCardProps {
  category: "STT" | "LLM" | "TTS";
  onClick?: () => void;
}

export const CustomProviderCreationCard = memo(({
  category,
  onClick,
}: CustomProviderCreationCardProps) => {
  const isTts = category === "TTS";
  return (
    <div
      onClick={onClick}
      className="relative flex flex-col justify-between p-4.5 rounded-xl border border-dashed border-[rgba(var(--foreground),0.2)] hover:border-[rgb(var(--accent))] bg-transparent hover:bg-[rgba(var(--accent),0.03)] transition-all duration-200 select-none group cursor-pointer transform-gpu h-[188px]"
    >
      <div className="flex flex-col items-center text-center my-auto px-1 pt-1">
        <div className="w-12 h-12 rounded-2xl border border-dashed border-[rgba(var(--foreground),0.25)] group-hover:border-[rgb(var(--accent))] flex items-center justify-center text-[rgb(var(--foreground-muted))] group-hover:text-[rgb(var(--accent))] group-hover:scale-105 transition-all bg-[rgba(var(--foreground),0.02)]">
          <Plus size={20} strokeWidth={2.25} />
        </div>
        <h4 className="font-bold text-[14.5px] text-[rgb(var(--foreground))] tracking-tight truncate w-full text-center mt-2.5 leading-tight group-hover:text-[rgb(var(--accent))] transition-colors">
          {isTts ? "Add Voice Server" : "Add Custom Server"}
        </h4>
        <p className="text-[12px] text-[rgb(var(--foreground-muted))] mt-1 line-clamp-2 leading-relaxed text-center px-1 h-[36px]">
          {isTts
            ? "Connect Chatterbox GPU node or remote synthesis host."
            : "Connect self-hosted Ollama, vLLM, or LM Studio endpoint."}
        </p>
      </div>
      <div className="mt-auto pt-2.5 flex items-center justify-between text-[11px] font-mono border-t border-dashed border-[rgba(var(--foreground),0.12)] h-[28px]">
        <span className="text-[rgb(var(--foreground-muted))]/60">Self-Hosted</span>
        <span className="text-[rgb(var(--accent))] font-medium flex items-center gap-1 group-hover:translate-x-0.5 transition-transform">
          + Configure <ArrowRight size={11} />
        </span>
      </div>
    </div>
  );
});
CustomProviderCreationCard.displayName = "CustomProviderCreationCard";

export interface SavedCustomServerCardProps {
  server: LlmCustomServer | TtsCustomServer;
  isSelected: boolean;
  onSelect: () => void;
  onDelete: () => void;
  onEdit: () => void;
}

/** Reveal-on-hover corner actions. Touch has no hover, so the actions stay
 * visible there and only hide on hover-capable pointers. */
export const HOVER_REVEAL =
  "[@media(hover:hover)]:opacity-0 [@media(hover:hover)]:group-hover:opacity-100 [@media(hover:hover)]:focus-within:opacity-100 transition-opacity";

export const SavedCustomServerCard = memo(({
  server,
  isSelected,
  onSelect,
  onDelete,
  onEdit,
}: SavedCustomServerCardProps) => {
  const isOllama =
    (server.name && server.name.toLowerCase().includes("ollama")) ||
    (server.id && server.id.toLowerCase().includes("ollama")) ||
    (server.url && (server.url.includes("11434") || server.url.toLowerCase().includes("ollama")));
  const serverLogoId = isOllama ? "ollama" : server.id || server.name || "server";

  return (
    <div
      onClick={onSelect}
      className={cn(
        "relative flex flex-col justify-between p-4.5 rounded-xl border transition-all duration-200 select-none group cursor-pointer transform-gpu h-[188px]",
        isSelected
          ? "bg-[rgba(var(--card),0.7)] border-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.18)] ring-1 ring-[rgb(var(--accent))]/30"
          : "bg-[rgba(var(--card),0.4)] border-[rgba(var(--foreground),0.07)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.65)]"
      )}
    >
      <div className="absolute top-3 right-3 flex items-center gap-1 z-10" onClick={(e) => e.stopPropagation()}>
        <div className="flex items-center gap-1 [@media(hover:hover)]:hidden [@media(hover:hover)]:group-hover:flex">
          <Tooltip label="Edit Server" side="top">
            <button
              type="button"
              onClick={onEdit}
              className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.1)] transition-all cursor-pointer"
              aria-label="Edit Server"
            >
              <Pencil size={13} />
            </button>
          </Tooltip>
          <Tooltip label="Delete Server" side="top">
            <button
              type="button"
              onClick={onDelete}
              className="p-1 rounded-md text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--error))] hover:bg-[rgba(var(--error),0.1)] transition-all cursor-pointer"
              aria-label="Delete Server"
            >
              <Trash2 size={13} />
            </button>
          </Tooltip>
        </div>
      </div>

      <div className="flex flex-col items-center text-center my-auto px-1 pt-1">
        <div
          className={cn(
            "w-12 h-12 rounded-2xl flex items-center justify-center shrink-0 border transition-all",
            isSelected
              ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] shadow-[0_0_12px_rgba(var(--accent),0.15)]"
              : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--foreground),0.07)] group-hover:border-[rgba(var(--accent),0.25)]"
          )}
        >
          <ProviderLogo providerId={serverLogoId} size={24} />
        </div>
        <h4 className="font-bold text-[14.5px] text-[rgb(var(--foreground))] tracking-tight truncate w-full text-center mt-2.5 leading-tight group-hover:text-[rgb(var(--accent))] transition-colors">
          {server.name || "Custom Server"}
        </h4>
        <p className="text-[12px] font-mono text-[rgb(var(--foreground-muted))] mt-1 truncate w-full text-center px-1">
          {server.url}
        </p>
      </div>

      <div className="mt-auto pt-2.5 border-t border-[rgba(var(--foreground),0.06)] h-[28px] flex items-center justify-between text-[11px] font-mono">
        <span className="text-[rgb(var(--foreground-muted))]/70">Self-Hosted</span>
        <span className="text-[rgb(var(--accent))] font-bold flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Connected
        </span>
      </div>
    </div>
  );
});
SavedCustomServerCard.displayName = "SavedCustomServerCard";

/** Inserts a server or replaces the entry with the same id (edit mode). */
export function upsertServer<T extends { id: string }>(list: T[], server: T): T[] {
  return list.some((s) => s.id === server.id)
    ? list.map((s) => (s.id === server.id ? server : s))
    : [...list, server];
}

export interface CustomServerFormCardProps {
  category: "STT" | "LLM" | "TTS";
  onClose: () => void;
  onServerSaved: (server: LlmCustomServer | TtsCustomServer) => void;
  /** Present when editing a saved server: form prefills and the id is kept. */
  initial?: LlmCustomServer | TtsCustomServer | null;
}

export const CustomServerFormCard = memo(({
  category,
  onClose,
  onServerSaved,
  initial = null,
}: CustomServerFormCardProps) => {
  const isTts = category === "TTS";
  const initialUrl = initial?.url ?? (isTts ? "http://127.0.0.1:7860" : "http://127.0.0.1:11434");
  const [url, setUrl] = useState(initialUrl);
  const [apiKey, setApiKey] = useState(initial?.api_key ?? "");
  const [name, setName] = useState(initial?.name ?? "");
  const [isProbing, setIsProbing] = useState(false);
  const [unhealthyReason, setUnhealthyReason] = useState<string | null>(null);

  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const commitChanges = useSettingsStore((s) => s.commitChanges);

  const handleSave = async () => {
    const trimmedUrl = url.trim();
    if (!trimmedUrl) return;

    setIsProbing(true);
    setUnhealthyReason(null);

    try {
      if (isTts) {
        let healthy = false;
        let reason = "Endpoint unreachable";
        try {
          const resp = await fetch(`${trimmedUrl.replace(/\/$/, "")}/health`, {
            signal: AbortSignal.timeout(2000),
          });
          healthy = resp.ok;
          if (!healthy) reason = `HTTP ${resp.status}`;
        } catch (e) {
          reason = ipcErrorMessage(e);
        }

        if (healthy) {
          const resolvedName = name.trim() || "Chatterbox Remote";
          const newServer: TtsCustomServer = {
            id: initial?.id ?? `tts_custom_${Date.now()}`,
            name: resolvedName,
            url: trimmedUrl,
            remote_path: (initial as TtsCustomServer | null)?.remote_path ?? "~/.vox",
            api_key: apiKey.trim() || null,
          };
          const currentList = draftSettings?.tts?.servers ?? draftSettings?.providers?.tts?.servers ?? [];
          console.info(
            `[Providers] saveCustomServer url=${trimmedUrl} prevCount=${currentList.length} id=${newServer.id}`
          );
          updateDraft("tts", "servers", upsertServer(currentList, newServer));
          onClose();
          onServerSaved(newServer);
          await commitChanges({ domain: "tts", key: "servers" });
        } else {
          setUnhealthyReason(reason);
        }
      } else {
        const res = await checkLlmProviderHealth({
          kind: "server",
          base_url: trimmedUrl,
          model: "",
          api_key: apiKey.trim() || undefined,
          provider_name: name.trim() || undefined,
        });

        if (res.healthy) {
          const resolvedName = name.trim() || res.dialect || "Ollama";
          const newServer: LlmCustomServer = {
            id: initial?.id ?? `llm_custom_${Date.now()}`,
            name: resolvedName,
            url: trimmedUrl,
            model: (initial as LlmCustomServer | null)?.model ?? "",
            protocol: (initial as LlmCustomServer | null)?.protocol ?? "openai",
            api_key: apiKey.trim() || null,
          };
          const currentList = draftSettings?.llm?.servers ?? draftSettings?.providers?.llm?.servers ?? [];
          console.info(
            `[Providers] saveCustomServer url=${trimmedUrl} prevCount=${currentList.length} id=${newServer.id}`
          );
          updateDraft("llm", "servers", upsertServer(currentList, newServer));
          onClose();
          onServerSaved(newServer);
          await commitChanges({ domain: "llm", key: "servers" });
        } else {
          setUnhealthyReason(res.reason || "Connection failed");
        }
      }
    } catch (err) {
      const reason = ipcErrorMessage(err);
      setUnhealthyReason(reason);
    } finally {
      setIsProbing(false);
    }
  };

  const handleSaveAnyway = async () => {
    const trimmedUrl = url.trim() || (isTts ? "http://127.0.0.1:7860" : "http://127.0.0.1:11434");
    const fallbackName = name.trim() || "Custom Server";

    if (isTts) {
      const newServer: TtsCustomServer = {
        id: initial?.id ?? `tts_custom_${Date.now()}`,
        name: fallbackName,
        url: trimmedUrl,
        remote_path: (initial as TtsCustomServer | null)?.remote_path ?? "~/.vox",
        api_key: apiKey.trim() || null,
      };
      const currentList = draftSettings?.tts?.servers ?? draftSettings?.providers?.tts?.servers ?? [];
      console.info(
        `[Providers] saveCustomServerAnyway url=${trimmedUrl} prevCount=${currentList.length} id=${newServer.id}`
      );
      updateDraft("tts", "servers", upsertServer(currentList, newServer));
      onClose();
      onServerSaved(newServer);
      await commitChanges({ domain: "tts", key: "servers" });
    } else {
      const newServer: LlmCustomServer = {
        id: initial?.id ?? `llm_custom_${Date.now()}`,
        name: fallbackName,
        url: trimmedUrl,
        model: (initial as LlmCustomServer | null)?.model ?? "",
        protocol: (initial as LlmCustomServer | null)?.protocol ?? "openai",
        api_key: apiKey.trim() || null,
      };
      const currentList = draftSettings?.llm?.servers ?? draftSettings?.providers?.llm?.servers ?? [];
      console.info(
        `[Providers] saveCustomServerAnyway url=${trimmedUrl} prevCount=${currentList.length} id=${newServer.id}`
      );
      updateDraft("llm", "servers", upsertServer(currentList, newServer));
      onClose();
      onServerSaved(newServer);
      await commitChanges({ domain: "llm", key: "servers" });
    }
  };

  const handleDiscard = () => {
    onClose();
  };

  return (
    <div className={cn(
      "relative flex flex-col justify-between p-4.5 rounded-xl border border-dashed transition-all select-none h-[188px]",
      unhealthyReason ? "border-[rgba(var(--error),0.4)]" : "border-[rgba(var(--accent))]"
    )}>
      <div className="flex items-center justify-between pb-1.5 border-b border-[rgba(var(--foreground),0.06)] shrink-0">
        <span className="text-[12.5px] font-bold text-[rgb(var(--foreground))] truncate flex items-center gap-1.5">
          <span className={cn("w-1.5 h-1.5 rounded-full", unhealthyReason ? "bg-[rgb(var(--error))]" : "bg-[rgb(var(--accent))]")} />
          {isTts ? "New Voice Server" : "New Custom Server"}
        </span>
        <button
          type="button"
          onClick={handleDiscard}
          className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-1 cursor-pointer bg-transparent rounded"
          title="Cancel"
          aria-label="Cancel"
        >
          <X size={14} />
        </button>
      </div>

      {unhealthyReason ? (
        <div className="flex flex-col justify-between flex-1 py-1 gap-1.5">
          <div className="flex flex-col justify-center flex-1 px-1 my-auto min-w-0">
            <span className="text-[12px] font-mono font-bold text-[rgb(var(--error))] flex items-center gap-1.5">
              <AlertCircle size={14} className="shrink-0" />
              Unhealthy Endpoint
            </span>
            <p className="text-[11px] font-mono text-[rgb(var(--error))]/90 mt-1 line-clamp-2 leading-relaxed [overflow-wrap:anywhere]">
              {unhealthyReason}
            </p>
            <p className="text-[10.5px] text-[rgb(var(--foreground-muted))] mt-1">
              Adjust URL, save anyway offline, or discard.
            </p>
          </div>
          <div className="flex items-center justify-between pt-1 border-t border-[rgba(var(--foreground),0.06)] mt-auto h-[28px]">
            <button
              type="button"
              onClick={() => setUnhealthyReason(null)}
              className="text-[11px] font-medium text-[rgb(var(--accent))] hover:underline transition-all cursor-pointer bg-transparent flex items-center gap-1 p-0"
              title="Return to editing inputs"
            >
              <ArrowLeft size={12} />
              Edit
            </button>
            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={handleDiscard}
                className="px-2 py-0.5 text-[11px] font-medium text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer bg-transparent"
              >
                Discard
              </button>
              <button
                type="button"
                onClick={handleSaveAnyway}
                className="px-2 py-0.5 text-[11px] font-bold text-[rgb(var(--error))] hover:opacity-80 transition-opacity cursor-pointer bg-transparent flex items-center gap-1"
              >
                Save Anyway
              </button>
            </div>
          </div>
        </div>
      ) : (
        <div className="flex flex-col gap-2 py-2 flex-1 justify-center">
          <input
            type="text"
            value={url}
            autoFocus
            onChange={(e) => setUrl(e.target.value)}
            placeholder={isTts ? "http://127.0.0.1:7860" : "http://127.0.0.1:11434"}
            className="w-full text-[11.5px] font-mono pb-1 bg-transparent border-b border-[rgba(var(--foreground),0.12)] focus:border-[rgb(var(--accent))] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 transition-colors"
          />
          <div className="flex gap-2">
            <input
              type="password"
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="API Key (Optional)"
              className="flex-1 text-[11.5px] font-mono pb-1 bg-transparent border-b border-[rgba(var(--foreground),0.12)] focus:border-[rgb(var(--accent))] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 transition-colors"
            />
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Name (Auto)"
              className="w-24 text-[11.5px] pb-1 bg-transparent border-b border-[rgba(var(--foreground),0.12)] focus:border-[rgb(var(--accent))] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 transition-colors"
            />
          </div>
        </div>
      )}

      {!unhealthyReason && (
        <div className="flex items-center justify-between pt-2 border-t border-[rgba(var(--foreground),0.06)] mt-auto h-[28px]">
          <span className="text-[10.5px] font-mono text-[rgb(var(--foreground-muted))]/70 truncate">
            {isProbing ? "Probing endpoint..." : "2s auto-discovery"}
          </span>
          <div className="flex items-center gap-1.5">
            <button
              type="button"
              disabled={isProbing || !url.trim()}
              onClick={handleSave}
              className="flex items-center gap-1 p-1 rounded text-[rgb(var(--accent))] hover:opacity-75 transition-opacity font-bold text-[11.5px] cursor-pointer disabled:opacity-40 bg-transparent"
              title="Save Server"
              aria-label="Save Server"
            >
              {isProbing ? (
                <span className="w-3.5 h-3.5 border-2 border-[rgb(var(--accent))] border-t-transparent rounded-full animate-spin" />
              ) : (
                <Check size={14} strokeWidth={2.5} />
              )}
              <span>Save</span>
            </button>
          </div>
        </div>
      )}
    </div>
  );
});
CustomServerFormCard.displayName = "CustomServerFormCard";

export interface TtsModalCardProps {
  entry: TtsProviderEntry;
  isSelected: boolean;
  onSelect: () => void;
}

export const TtsModalCard = memo(({
  entry,
  isSelected,
  onSelect,
}: TtsModalCardProps) => {
  return (
    <div
      onClick={entry.isComingSoon ? undefined : onSelect}
      className={cn(
        "relative flex flex-col justify-between p-4.5 rounded-xl border transition-all duration-200 select-none group transform-gpu h-[188px]",
        entry.isComingSoon
          ? "opacity-45 cursor-not-allowed bg-[rgba(var(--card),0.2)] border-[rgba(var(--foreground),0.04)]"
          : isSelected
          ? "bg-[rgba(var(--card),0.7)] border-[rgb(var(--accent))] shadow-[0_0_20px_rgba(var(--accent),0.18)] ring-1 ring-[rgb(var(--accent))]/30 cursor-pointer"
          : "bg-[rgba(var(--card),0.4)] border-[rgba(var(--foreground),0.07)] hover:border-[rgba(var(--accent),0.35)] hover:bg-[rgba(var(--card),0.65)] cursor-pointer"
      )}
    >
      <div className="absolute top-3 right-3 flex items-center gap-1.5 z-10">
        {entry.isFree ? (
          <span className="text-[10px] font-mono font-bold px-1.5 py-0.5 rounded-md bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] border border-[rgba(var(--accent),0.2)]">
            Free
          </span>
        ) : entry.isComingSoon ? (
          <span className="text-[10px] font-mono font-medium px-1.5 py-0.5 rounded-md bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]">
            Soon
          </span>
        ) : null}
      </div>

      <div className="flex flex-col items-center text-center my-auto px-1 pt-1">
        <div className={cn(
          "w-12 h-12 rounded-2xl flex items-center justify-center shrink-0 border transition-all",
          isSelected
            ? "bg-[rgba(var(--accent),0.12)] border-[rgba(var(--accent),0.3)] shadow-[0_0_12px_rgba(var(--accent),0.15)]"
            : "bg-[rgba(var(--foreground),0.04)] border-[rgba(var(--foreground),0.07)] group-hover:border-[rgba(var(--accent),0.25)]"
        )}>
          <ProviderLogo
            providerId={entry.id.includes("edge") ? "microsoft" : entry.id.includes("google") ? "google" : "azure"}
            size={24}
          />
        </div>
        <h4 className="font-bold text-[14.5px] text-[rgb(var(--foreground))] tracking-tight truncate w-full text-center mt-2.5 leading-tight group-hover:text-[rgb(var(--accent))] transition-colors">
          {entry.name}
        </h4>
        <p className="text-[12px] text-[rgb(var(--foreground-muted))] mt-1 line-clamp-2 leading-relaxed text-center px-1 h-[36px]">
          {entry.tagline}
        </p>
      </div>

      <div className="mt-auto pt-2.5 flex items-center justify-between text-[11px] font-mono border-t border-[rgba(var(--foreground),0.06)] h-[28px]">
        <span className="text-[rgb(var(--foreground-muted))]/70">
          {entry.isFree ? "Keyless Cloud" : "Cloud Voice"}
        </span>
        <span className={cn(
          "font-bold flex items-center gap-1.5",
          entry.isComingSoon
            ? "text-[rgb(var(--foreground-muted))]/60 font-normal"
            : isSelected
            ? "text-[rgb(var(--accent))]"
            : "text-[rgb(var(--accent))]"
        )}>
          {!entry.isComingSoon && <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />}
          {entry.isComingSoon ? "In Development" : isSelected ? "Active" : "Ready"}
        </span>
      </div>
    </div>
  );
});
TtsModalCard.displayName = "TtsModalCard";

export interface CloudProvidersModalViewProps {
  category?: "STT" | "LLM" | "TTS";
  isLocalActive?: boolean;
  onSelectLocal?: () => void;
  activeCloudProviderId: string | null;
  getProviderKey: (id: string) => string | undefined;
  /** Committed (saved) active provider + key lookup driving list order; selection highlight stays on draft ids. */
  pinnedProviderId: string | undefined;
  getCommittedProviderKey: (id: string) => string | undefined;
  onSelectProvider: (provider: CloudProvider) => void;
  editingProviderId: string | null;
  editingKeyValue: string;
  setEditingKeyValue: (val: string) => void;
  onStartInlineEdit: (id: string, currentKey: string) => void;
  onCancelInlineKey: () => void;
  onSaveInlineKey: (id: string) => void;
  onNavigateToModels?: (provider: CloudProvider) => void;
  onAddCustomServer?: () => void;
}

export const CloudProvidersModalView = memo(({
  category = "LLM",
  onSelectLocal,
  activeCloudProviderId,
  getProviderKey,
  pinnedProviderId,
  getCommittedProviderKey,
  onSelectProvider,
  editingProviderId,
  editingKeyValue,
  setEditingKeyValue,
  onStartInlineEdit,
  onCancelInlineKey,
  onSaveInlineKey,
  onNavigateToModels,
}: CloudProvidersModalViewProps) => {
  const [modalSearch, setModalSearch] = useState("");
  const deferredSearch = useDeferredValue(modalSearch);
  const [filterCategory, setFilterCategory] = useState<
    "all" | "local" | "cloud" | "configured" | "unconfigured" | "guide"
  >("configured");
  const [sort, setSort] = useState<"az" | "za">("az");
  const [isFilterDropdownOpen, setIsFilterDropdownOpen] = useState(false);
  const [isCreatingCustomServer, setIsCreatingCustomServer] = useState(false);
  const [editingServer, setEditingServer] = useState<LlmCustomServer | TtsCustomServer | null>(null);
  const filterDropdownRef = useRef<HTMLDivElement>(null);

  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const committedSettings = useSettingsStore((s) => s.settings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const isTts = category === "TTS";

  const customServers = useMemo(() => {
    if (isTts) return draftSettings?.tts?.servers ?? draftSettings?.providers?.tts?.servers ?? [];
    return draftSettings?.llm?.servers ?? draftSettings?.providers?.llm?.servers ?? [];
  }, [isTts, draftSettings?.tts?.servers, draftSettings?.providers?.tts?.servers, draftSettings?.llm?.servers, draftSettings?.providers?.llm?.servers]);

  const activeCategoryType = useMemo(() => {
    if (category === "STT") return draftSettings?.stt?.active?.category ?? "embedded";
    if (category === "TTS") return draftSettings?.tts?.active?.category ?? "embedded";
    return draftSettings?.llm?.active?.category ?? "embedded";
  }, [category, draftSettings?.stt?.active?.category, draftSettings?.tts?.active?.category, draftSettings?.llm?.active?.category]);

  const isLocalSelected = activeCategoryType === "embedded";
  const activeCloudId = activeCategoryType === "cloud" ? activeCloudProviderId : null;
  const activeServerId =
    activeCategoryType === "server"
      ? (isTts ? draftSettings?.tts?.active?.provider : draftSettings?.llm?.active?.provider)
      : null;

  const handleSelectServer = useCallback(
    (server: LlmCustomServer | TtsCustomServer) => {
      useSettingsStore.getState().clearRemoteModelsError();
      if (isTts) {
        updateDraft("tts", "active", activeSelection("server", "", server.id));
        window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "tts" }));
      } else {
        const llmServ = server as LlmCustomServer;
        updateDraft("llm", "active", activeSelection("server", llmServ.model || "", llmServ.id));
        window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "llm" }));
      }
    },
    [isTts, updateDraft]
  );

  const handleDeleteServer = useCallback(
    async (serverId: string) => {
      const commitChanges = useSettingsStore.getState().commitChanges;
      if (isTts) {
        const currentList = draftSettings?.tts?.servers ?? draftSettings?.providers?.tts?.servers ?? [];
        console.info(
          `[Providers] deleteCustomServer id=${serverId} prevCount=${currentList.length}`
        );
        updateDraft("tts", "servers", currentList.filter((s) => s.id !== serverId));
        if (draftSettings?.tts?.active?.provider === serverId) {
          updateDraft("tts", "active", activeSelection("embedded", draftSettings?.tts?.last_selected?.embedded ?? ""));
          await commitChanges({ domain: "tts", key: "servers" });
          await commitChanges({ domain: "tts", key: "active" });
        } else {
          await commitChanges({ domain: "tts", key: "servers" });
        }
      } else {
        const currentList = draftSettings?.llm?.servers ?? draftSettings?.providers?.llm?.servers ?? [];
        console.info(
          `[Providers] deleteCustomServer id=${serverId} prevCount=${currentList.length}`
        );
        updateDraft("llm", "servers", currentList.filter((s) => s.id !== serverId));
        if (draftSettings?.llm?.active?.provider === serverId) {
          updateDraft("llm", "active", activeSelection("embedded", draftSettings?.llm?.last_selected?.embedded ?? ""));
          await commitChanges({ domain: "llm", key: "servers" });
          await commitChanges({ domain: "llm", key: "active" });
        } else {
          await commitChanges({ domain: "llm", key: "servers" });
        }
      }
    },
    [isTts, draftSettings?.tts?.servers, draftSettings?.providers?.tts?.servers, draftSettings?.llm?.servers, draftSettings?.providers?.llm?.servers, draftSettings?.tts?.active?.provider, draftSettings?.tts?.last_selected?.embedded, draftSettings?.llm?.active?.provider, draftSettings?.llm?.last_selected?.embedded, updateDraft]
  );

  const handleSelectLocalProvider = useCallback(() => {
    useSettingsStore.getState().clearRemoteModelsError();
    if (onSelectLocal) {
      onSelectLocal();
    } else {
      if (category === "STT") {
        updateDraft("stt", "active", activeSelection("embedded", draftSettings?.stt?.last_selected?.embedded ?? ""));
        window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "stt" }));
      } else if (category === "TTS") {
        updateDraft("tts", "active", activeSelection("embedded", draftSettings?.tts?.last_selected?.embedded ?? ""));
        window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "tts" }));
      } else {
        updateDraft("llm", "active", activeSelection("embedded", draftSettings?.llm?.last_selected?.embedded ?? ""));
        window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "llm" }));
      }
    }
  }, [category, draftSettings?.llm?.last_selected?.embedded, draftSettings?.stt?.last_selected?.embedded, draftSettings?.tts?.last_selected?.embedded, onSelectLocal, updateDraft]);

  // Close compact filter dropdown on outside click or Escape
  useEffect(() => {
    if (!isFilterDropdownOpen) return;
    const handleDown = (e: MouseEvent | TouchEvent) => {
      if (filterDropdownRef.current && !filterDropdownRef.current.contains(e.target as Node)) {
        setIsFilterDropdownOpen(false);
      }
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setIsFilterDropdownOpen(false);
    };
    window.addEventListener("mousedown", handleDown);
    window.addEventListener("touchstart", handleDown);
    window.addEventListener("keydown", handleKey);
    return () => {
      window.removeEventListener("mousedown", handleDown);
      window.removeEventListener("touchstart", handleDown);
      window.removeEventListener("keydown", handleKey);
    };
  }, [isFilterDropdownOpen]);

  const { containerRef: scrollContainerRef, lenisRef } =
    useLenisScrollContainer<HTMLDivElement>();

  // Save-gated reorder glide: fires when a save changes the committed order.
  const orderFingerprint = useMemo(
    () =>
      `${pinnedProviderId ?? ""}|${CLOUD_PROVIDERS.map((p) =>
        getCommittedProviderKey(p.id)?.trim() ? "1" : "0"
      ).join("")}`,
    [pinnedProviderId, getCommittedProviderKey]
  );
  useScrollTopOnChange(scrollContainerRef, orderFingerprint, lenisRef);

  const configuredCount = useMemo(() => {
    return CLOUD_PROVIDERS.filter((p) => Boolean(getProviderKey(p.id)?.trim())).length;
  }, [getProviderKey]);

  const unconfiguredCount = useMemo(() => {
    return CLOUD_PROVIDERS.filter((p) => !getProviderKey(p.id)?.trim()).length;
  }, [getProviderKey]);

  const ttsFilteredProviders = useMemo(() => {
    if (!isTts) return [];
    let list = TTS_MODAL_PROVIDERS;
    const q = deferredSearch.trim().toLowerCase();
    if (q) {
      list = list.filter(
        (p) =>
          p.name.toLowerCase().includes(q) ||
          p.id.toLowerCase().includes(q) ||
          p.tagline.toLowerCase().includes(q)
      );
    }
    if (filterCategory === "local") return [];
    if (filterCategory === "configured") {
      list = list.filter((p) => p.isFree || Boolean(getProviderKey(p.id)?.trim()));
    }
    return list;
  }, [deferredSearch, filterCategory, getProviderKey, isTts]);

  const ttsAvailableCount = useMemo(() => {
    return TTS_MODAL_PROVIDERS.filter((p) => p.isFree || Boolean(getProviderKey(p.id)?.trim())).length;
  }, [getProviderKey]);

  const providerFilters = useMemo(
    () =>
      isTts
        ? [
            { id: "configured" as const, label: "Available", count: 1 + ttsAvailableCount + customServers.length, Icon: Key },
            { id: "all" as const, label: "All Providers", count: 1 + 5 + customServers.length, Icon: Globe },
            { id: "local" as const, label: "On-Device", count: 1, Icon: HardDrive },
            { id: "cloud" as const, label: "Cloud Voices", count: 3, Icon: Cloud },
          ]
        : [
            { id: "configured" as const, label: "Available", count: 1 + configuredCount + customServers.length, Icon: Key },
            { id: "all" as const, label: "All Providers", count: 1 + CLOUD_PROVIDERS.length + customServers.length, Icon: Globe },
            { id: "local" as const, label: "On-Device", count: 1, Icon: HardDrive },
            { id: "cloud" as const, label: "Cloud APIs", count: CLOUD_PROVIDERS.length, Icon: Cloud },
            { id: "unconfigured" as const, label: "Needs Key", count: unconfiguredCount, Icon: KeyRound },
          ],
    [configuredCount, isTts, unconfiguredCount, customServers.length, ttsAvailableCount]
  );

  const modalFilteredProviders = useMemo(() => {
    let list = CLOUD_PROVIDERS;
    const q = deferredSearch.trim().toLowerCase();
    if (q) {
      list = list.filter(
        (p) =>
          p.name.toLowerCase().includes(q) ||
          p.id.toLowerCase().includes(q) ||
          p.tagline?.toLowerCase().includes(q) ||
          p.url.toLowerCase().includes(q)
      );
    }

    if (filterCategory === "configured") {
      list = list.filter((p) => Boolean((p as { isFree?: boolean }).isFree) || Boolean(getProviderKey(p.id)?.trim()));
    } else if (filterCategory === "unconfigured") {
      list = list.filter((p) => !getProviderKey(p.id)?.trim());
    } else if (filterCategory === "local") {
      list = [];
    }

    return [...list].sort((a, b) => {
      const aKey = Boolean(getCommittedProviderKey(a.id)?.trim());
      const bKey = Boolean(getCommittedProviderKey(b.id)?.trim());
      if (aKey && !bKey) return -1;
      if (!aKey && bKey) return 1;
      return sort === "az"
        ? a.name.localeCompare(b.name, undefined, { sensitivity: "base" })
        : b.name.localeCompare(a.name, undefined, { sensitivity: "base" });
    });
  }, [deferredSearch, filterCategory, getProviderKey, getCommittedProviderKey, sort]);

  const matchesLocal = useMemo(() => {
    const q = deferredSearch.trim().toLowerCase();
    if (!q) return true;
    return "built-in on-device local neural whisper gguf kokoro vox offline".includes(q);
  }, [deferredSearch]);

  const showLocalCard =
    (filterCategory === "local" ||
      filterCategory === "all" ||
      filterCategory === "configured") &&
    matchesLocal;

  const showCustomSection = filterCategory === "all" || filterCategory === "configured";

  const showCustomCreationCard =
    showCustomSection &&
    (!deferredSearch.trim() || "custom server self-hosted endpoint".includes(deferredSearch.trim().toLowerCase()));

  const filteredCustomServers = useMemo(() => {
    if (!showCustomSection) return [];
    const q = deferredSearch.trim().toLowerCase();
    const list = !q
      ? [...customServers]
      : customServers.filter(
          (s) =>
            (s.name && s.name.toLowerCase().includes(q)) ||
            (s.url && s.url.toLowerCase().includes(q)) ||
            (s.id && s.id.toLowerCase().includes(q))
        );
    // Save-gated pinning: the COMMITTED-active server moves first, so cards
    // only reshuffle after Apply & Restart — never on draft selection.
    const committedId = isTts
      ? committedSettings?.tts?.active?.provider
      : committedSettings?.llm?.active?.provider;
    const committedCategory = isTts
      ? committedSettings?.tts?.active?.category
      : committedSettings?.llm?.active?.category;
    if (committedCategory === "server" && committedId) {
      const idx = list.findIndex((s) => s.id === committedId);
      if (idx > 0) {
        const [pinned] = list.splice(idx, 1);
        list.unshift(pinned);
      }
    }
    return list;
  }, [showCustomSection, deferredSearch, customServers, isTts, committedSettings?.tts?.active, committedSettings?.llm?.active]);

  const totalVisibleCount =
    (showLocalCard ? 1 : 0) +
    (isTts ? ttsFilteredProviders.length : modalFilteredProviders.length) +
    filteredCustomServers.length +
    (showCustomSection && isCreatingCustomServer
      ? 1
      : showCustomCreationCard && !editingServer
      ? 1
      : 0);

  const totalAllProvidersCount = isTts
    ? 1 + 5 + customServers.length
    : 1 + CLOUD_PROVIDERS.length + customServers.length;

  return (
    <div className="flex flex-col h-full min-h-0 gap-3.5">
      {/* Top Toolbar: Search + Filter Dropdown (compact) + Sort Icon + Count */}
      <div className="flex items-center justify-between gap-2.5 shrink-0 flex-wrap">
        {/* Mobile/Compact Filter Dropdown Trigger */}
        <div ref={filterDropdownRef} className="relative md:hidden shrink-0">
          <button
            type="button"
            onClick={() => setIsFilterDropdownOpen((v) => !v)}
            className={cn(
              "p-2 rounded-xl border transition-all cursor-pointer flex items-center gap-1.5",
              isFilterDropdownOpen || filterCategory !== "all"
                ? "bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] border-[rgba(var(--accent),0.35)] shadow-xs"
                : "bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground-muted))] border-[rgba(var(--foreground),0.08)] hover:text-[rgb(var(--foreground))]"
            )}
            aria-label="Toggle provider filters"
            aria-expanded={isFilterDropdownOpen}
          >
            <Filter size={15} />
            {filterCategory !== "all" && (
              <span className="w-2 h-2 rounded-full bg-[rgb(var(--accent))]" />
            )}
          </button>

          {/* Compact Filter Menu Popover */}
          {isFilterDropdownOpen && (
            <div className="absolute left-0 top-full mt-2 z-50 w-64 rounded-xl bg-[rgba(var(--card),0.96)] border border-[rgba(var(--foreground),0.12)] shadow-2xl p-3 flex flex-col gap-1.5 animate-fade-in">
              <div className="flex items-center justify-between pb-1.5 border-b border-[rgba(var(--foreground),0.06)] px-1">
                <span className="text-[12px] font-bold text-[rgb(var(--foreground))]">Filters</span>
                {filterCategory !== "all" && (
                  <button
                    type="button"
                    onClick={() => {
                      setFilterCategory("all");
                      setIsFilterDropdownOpen(false);
                    }}
                    className="text-[11px] font-medium text-[rgb(var(--accent))] hover:underline cursor-pointer"
                  >
                    Reset
                  </button>
                )}
              </div>
              {providerFilters.map((cat) => (
                <button
                  key={cat.id}
                  type="button"
                  onClick={() => {
                    setFilterCategory(cat.id);
                    setIsFilterDropdownOpen(false);
                  }}
                  className={cn(
                    "w-full flex items-center justify-between px-2.5 py-1.5 rounded-xl text-[12px] transition-all cursor-pointer font-medium",
                    filterCategory === cat.id
                      ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold shadow-xs"
                      : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                  )}
                >
                  <div className="flex items-center gap-2 truncate">
                    <cat.Icon size={13} className={cn("shrink-0", filterCategory === cat.id ? "text-[rgb(var(--accent))]" : "opacity-60")} />
                    <span className="truncate">{cat.label}</span>
                  </div>
                  <span className={cn(
                    "text-[10px] font-mono px-1.5 py-0.5 rounded-full font-bold shrink-0",
                    filterCategory === cat.id
                      ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                      : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]"
                  )}>
                    {cat.count}
                  </span>
                </button>
              ))}

              <div className="pt-1.5 border-t border-[rgba(var(--foreground),0.06)] mt-1">
                <button
                  type="button"
                  onClick={() => {
                    setFilterCategory("guide");
                    setIsFilterDropdownOpen(false);
                  }}
                  className={cn(
                    "w-full flex items-center gap-2 px-2.5 py-1.5 rounded-xl text-[12px] transition-all cursor-pointer font-medium",
                    filterCategory === "guide"
                      ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold shadow-xs"
                      : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                  )}
                >
                  <HelpCircle size={13} className={cn("shrink-0", filterCategory === "guide" ? "text-[rgb(var(--accent))]" : "opacity-60")} />
                  <span>Help & Guide</span>
                </button>
              </div>
            </div>
          )}
        </div>

        {/* Search Input */}
        <div className="flex-1 min-w-[140px] sm:min-w-[180px] flex items-center gap-2 px-3.5 py-2 rounded-xl bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] focus-within:border-[rgb(var(--accent))] focus-within:ring-1 focus-within:ring-[rgb(var(--accent))]/30 transition-all">
          <Search size={14} className="text-[rgb(var(--foreground-muted))] shrink-0" />
          <input
            type="text"
            value={modalSearch}
            onChange={(e) => setModalSearch(e.target.value)}
            placeholder="Search providers..."
            aria-label="Search providers"
            className="w-full bg-transparent border-none outline-none text-[13px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40"
          />
          {modalSearch && (
            <button
              type="button"
              onClick={() => setModalSearch("")}
              className="text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] p-0.5 cursor-pointer"
            >
              <X size={13} />
            </button>
          )}
        </div>

        {/* Sort Button */}
        <button
          type="button"
          onClick={() => setSort((s) => (s === "az" ? "za" : "az"))}
          className="p-2 rounded-xl text-[12px] font-medium bg-[rgba(var(--foreground),0.04)] border border-[rgba(var(--foreground),0.08)] text-[rgb(var(--foreground))] hover:border-[rgb(var(--accent))]/40 hover:text-[rgb(var(--accent))] flex items-center justify-center cursor-pointer shrink-0 transition-all"
          title={sort === "az" ? "Sort: A–Z" : "Sort: Z–A"}
          aria-label="Toggle alphabetical sort order"
        >
          {sort === "az" ? (
            <ArrowDownAZ size={15} className="text-[rgb(var(--accent))]" />
          ) : (
            <ArrowUpZA size={15} className="text-[rgb(var(--accent))]" />
          )}
        </button>

        {/* Count Pill */}
        <div className="text-[11px] font-mono text-[rgb(var(--foreground-muted))] px-2.5 py-1.5 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] shrink-0 select-none">
          <span className="hidden sm:inline">
            Showing <span className="text-[rgb(var(--foreground))] font-bold">{totalVisibleCount}</span> of {totalAllProvidersCount} providers
          </span>
          <span className="inline sm:hidden">
            <span className="text-[rgb(var(--foreground))] font-bold">{totalVisibleCount}</span>/{totalAllProvidersCount}
          </span>
        </div>
      </div>

      {/* Main Content Area */}
      <div className="flex-1 min-h-0 flex gap-4 overflow-hidden">
        {/* Facet Sidebar on Desktop (Aligned with VoiceCatalogModalView grammar) */}
        <div className="w-[200px] shrink-0 hidden md:flex flex-col gap-3.5 overflow-y-auto overscroll-contain custom-scrollbar pr-2 py-0.5">
          {/* Group 1: Source */}
          <div className="flex flex-col gap-1">
            <div className="px-2 pb-1 text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold">
              Source
            </div>
            {providerFilters.slice(0, 3).map((cat) => (
              <button
                key={cat.id}
                type="button"
                onClick={() => setFilterCategory(cat.id)}
                className={cn(
                  "w-full flex items-center justify-between px-3 py-2 rounded-xl text-[13px] transition-all cursor-pointer font-medium",
                  filterCategory === cat.id
                    ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold shadow-xs"
                    : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                )}
              >
                <div className="flex items-center gap-2.5 truncate">
                  <cat.Icon size={14} className={cn("shrink-0", filterCategory === cat.id ? "text-[rgb(var(--accent))]" : "opacity-60")} />
                  <span className="truncate">{cat.label}</span>
                </div>
                <span className={cn(
                  "text-[10px] font-mono px-1.5 py-0.5 rounded-full font-bold shrink-0",
                  filterCategory === cat.id
                    ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                    : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]"
                )}>
                  {cat.count}
                </span>
              </button>
            ))}
          </div>

          {/* Group 2: Status (for LLM) */}
          {!isTts && providerFilters.length > 3 && (
            <div className="pt-2 border-t border-[rgba(var(--foreground),0.06)] flex flex-col gap-1">
              <div className="px-2 pb-1 text-[11px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]/60 font-semibold">
                Status
              </div>
              {providerFilters.slice(3).map((cat) => (
                <button
                  key={cat.id}
                  type="button"
                  onClick={() => setFilterCategory(cat.id)}
                  className={cn(
                    "w-full flex items-center justify-between px-3 py-2 rounded-xl text-[13px] transition-all cursor-pointer font-medium",
                    filterCategory === cat.id
                      ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold shadow-xs"
                      : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
                  )}
                >
                  <div className="flex items-center gap-2.5 truncate">
                    <cat.Icon size={14} className={cn("shrink-0", filterCategory === cat.id ? "text-[rgb(var(--accent))]" : "opacity-60")} />
                    <span className="truncate">{cat.label}</span>
                  </div>
                  <span className={cn(
                    "text-[10px] font-mono px-1.5 py-0.5 rounded-full font-bold shrink-0",
                    filterCategory === cat.id
                      ? "bg-[rgb(var(--accent))] text-[rgb(var(--accent-foreground))]"
                      : "bg-[rgba(var(--foreground),0.06)] text-[rgb(var(--foreground-muted))]"
                  )}>
                    {cat.count}
                  </span>
                </button>
              ))}
            </div>
          )}

          {/* Group 3: Help & Guide Option at the bottom */}
          <div className="pt-2 border-t border-[rgba(var(--foreground),0.06)] mt-auto flex flex-col gap-1">
            <button
              type="button"
              onClick={() => setFilterCategory("guide")}
              className={cn(
                "w-full flex items-center justify-between px-3 py-2 rounded-xl text-[13px] transition-all cursor-pointer font-medium",
                filterCategory === "guide"
                  ? "bg-[rgba(var(--accent),0.12)] text-[rgb(var(--accent))] font-bold shadow-xs"
                  : "text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.03)]"
              )}
            >
              <div className="flex items-center gap-2.5 truncate">
                <HelpCircle size={14} className={cn("shrink-0", filterCategory === "guide" ? "text-[rgb(var(--accent))]" : "opacity-60")} />
                <span className="truncate">Setup Guide</span>
              </div>
            </button>
          </div>
        </div>

        {filterCategory === "guide" ? (
          <div className="flex-1 min-h-0 h-full overflow-hidden">
            <ProviderGuideView onBackToProviders={() => setFilterCategory("all")} category={category} />
          </div>
        ) : (
          <div
            ref={scrollContainerRef}
            className="flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar p-1 sm:p-2"
          >
            {totalVisibleCount === 0 ? (
              <div className="h-64 flex flex-col items-center justify-center text-center space-y-2">
                <p className="text-[14px] font-bold text-[rgb(var(--foreground))]">No providers found</p>
                <p className="text-[12px] text-[rgb(var(--foreground-muted))] max-w-sm">
                  No providers match your search query or filter.
                </p>
                <button
                  type="button"
                  onClick={() => {
                    setModalSearch("");
                    setFilterCategory("all");
                  }}
                  className="mt-2 px-3 py-1.5 rounded-lg text-[12px] font-bold text-[rgb(var(--accent))] bg-[rgb(var(--accent))]/10 border border-[rgba(var(--accent),0.3)] hover:bg-[rgb(var(--accent))]/20 transition-all cursor-pointer"
                >
                  Reset Filters
                </button>
              </div>
            ) : (
              <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-3">
                {showLocalCard && (
                  <LocalProviderCard
                    category={category}
                    isSelected={isLocalSelected}
                    onSelect={handleSelectLocalProvider}
                  />
                )}
                {isTts
                  ? ttsFilteredProviders.map((entry) => {
                      const isSelected = activeCloudId === entry.id;
                      return (
                        <TtsModalCard
                          key={entry.id}
                          entry={entry}
                          isSelected={isSelected}
                          onSelect={() => {
                            if (!entry.isComingSoon) {
                              onSelectProvider({
                                id: entry.id,
                                name: entry.name,
                                url: entry.url || "",
                                popular: true,
                                keyPlaceholder: "API Key (Optional)...",
                              });
                            }
                          }}
                        />
                      );
                    })
                  : modalFilteredProviders.map((provider) => {
                      const isSelected = activeCloudId === provider.id;
                      const savedKey = getProviderKey(provider.id);
                      const isEditing = editingProviderId === provider.id;

                      return (
                        <CloudProviderCard
                          key={provider.id}
                          provider={provider}
                          isSelected={isSelected}
                          savedKey={savedKey}
                          isEditing={isEditing}
                          editingKeyValue={editingKeyValue}
                          setEditingKeyValue={setEditingKeyValue}
                          onSelect={onSelectProvider}
                          onStartInlineEdit={onStartInlineEdit}
                          onCancelInlineKey={onCancelInlineKey}
                          onSaveInlineKey={onSaveInlineKey}
                          onNavigateToModels={onNavigateToModels}
                        />
                      );
                    })}

                {filteredCustomServers.map((server) => {
                  if (editingServer?.id === server.id) {
                    return (
                      <CustomServerFormCard
                        key={server.id}
                        category={category}
                        initial={server}
                        onClose={() => setEditingServer(null)}
                        onServerSaved={() => {}}
                      />
                    );
                  }
                  const isSelected =
                    activeCategoryType === "server" && activeServerId === server.id;
                  return (
                    <SavedCustomServerCard
                      key={server.id || server.url}
                      server={server}
                      isSelected={isSelected}
                      onSelect={() => handleSelectServer(server)}
                      onDelete={() => handleDeleteServer(server.id)}
                      onEdit={() => {
                        setEditingServer(server);
                        setIsCreatingCustomServer(false);
                      }}
                    />
                  );
                })}

                {showCustomSection && isCreatingCustomServer && (
                  <CustomServerFormCard
                    key="new"
                    category={category}
                    initial={null}
                    onClose={() => setIsCreatingCustomServer(false)}
                    onServerSaved={() => {}}
                  />
                )}

                {showCustomCreationCard && !isCreatingCustomServer && !editingServer && (
                  <CustomProviderCreationCard
                    category={category}
                    onClick={() => setIsCreatingCustomServer(true)}
                  />
                )}
              </div>
            )}
          </div>
        )}
      </div>

      <ModalProviderFooter category={category} />
    </div>
  );
});
CloudProvidersModalView.displayName = "CloudProvidersModalView";

/** Failure + transient-error footer for the providers modal, which has no
 * card footer of its own. Same grammar as every other settings banner. */
const ModalProviderFooter = memo(({ category }: { category: "STT" | "LLM" | "TTS" }) => {
  const failedKeys = useSettingsStore((s) => s.failedSaveDomains["models"]);
  const remoteModelsError = useSettingsStore((s) => s.remoteModelsError);
  const llmCategory = useSettingsStore((s) => s.draftSettings?.llm?.active?.category);
  const llmActiveProvider = useSettingsStore((s) => s.draftSettings?.llm?.active?.provider);
  const llmCloudKeys = useSettingsStore((s) => s.draftSettings?.llm?.cloud_keys);
  const llmProvidersKeys = useSettingsStore((s) => s.draftSettings?.providers?.llm?.cloud_keys);
  const cloudApiKey = llmActiveProvider
    ? (llmCloudKeys?.[llmActiveProvider] ?? llmProvidersKeys?.[llmActiveProvider])
    : undefined;
  const isCloudLlmMissingKey = category === "LLM" && llmCategory === "cloud" && !cloudApiKey?.trim();

  if (category === "STT") return null;
  if (!failedKeys && !remoteModelsError && !isCloudLlmMissingKey) return null;
  return (
    <div className="flex flex-col gap-2 shrink-0 w-full max-w-full">
      {isCloudLlmMissingKey && (
        <SettingsFooterBanner
          title={SETTINGS_COPY.apiKeyRequired}
          detail="Enter and save an API key for the selected provider to enable cloud inference."
          className="rounded-xl w-full max-w-full"
        />
      )}
      {failedKeys && failedKeys.length > 0 && (
        <SettingsFooterBanner
          title={SETTINGS_COPY.saveFailedTitle}
          detail={`${SETTINGS_COPY.saveFailedHint} ${failedKeys.join(", ")}`}
          className="rounded-xl w-full max-w-full"
        />
      )}
      {remoteModelsError && (
        <SettingsFooterBanner
          title={INTERACTION_CONFIG_DESK_COPY.status.connectionFailed}
          detail={remoteModelsError}
          className="rounded-xl w-full max-w-full"
        />
      )}
    </div>
  );
});
ModalProviderFooter.displayName = "ModalProviderFooter";

export function useCloudProvidersModalState(options?: { onNavigateToModels?: (provider: CloudProvider) => void }) {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const committedSettings = useSettingsStore((s) => s.settings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const [editingProviderId, setEditingProviderId] = useState<string | null>(null);
  const [editingKeyValue, setEditingKeyValue] = useState<string>("");

  const activeCategory = draftSettings?.llm?.active?.category ?? "embedded";

  const activeCloudProviderId = useMemo(() => {
    if (activeCategory !== "cloud") return null;
    return draftSettings?.llm?.active?.provider ?? null;
  }, [activeCategory, draftSettings?.llm?.active?.provider]);

  const getProviderKey = useCallback(
    (providerId: string) => {
      const keys = draftSettings?.llm?.cloud_keys ?? draftSettings?.providers?.llm?.cloud_keys;
      return keys?.[providerId];
    },
    [draftSettings?.llm?.cloud_keys, draftSettings?.providers?.llm?.cloud_keys]
  );

  // Save-gated ordering: the sort follows committed keys so selecting only
  // re-highlights; the reorder (with smooth scroll) lands on save.
  const pinnedProviderId = useMemo(() => {
    if (committedSettings?.llm?.active?.category !== "cloud") return undefined;
    return committedSettings?.llm?.active?.provider ?? undefined;
  }, [committedSettings?.llm?.active?.category, committedSettings?.llm?.active?.provider]);

  const getCommittedProviderKey = useCallback(
    (providerId: string) => {
      return committedSettings?.providers?.llm?.cloud_keys?.[providerId];
    },
    [committedSettings?.providers?.llm?.cloud_keys]
  );

  useEffect(() => {
    return () => {
      useSettingsStore.getState().clearRemoteModelsError();
    };
  }, []);

  const handleSelectProvider = useCallback(
    (provider: CloudProvider) => {
      useSettingsStore.getState().clearRemoteModelsError();
      updateDraft(
        "llm",
        "active",
        activeSelection("cloud", draftSettings?.llm?.active?.model || "", provider.id)
      );
      window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "llm" }));
    },
    [draftSettings?.llm?.active?.model, updateDraft]
  );

  const isLocalActive = activeCategory === "embedded";
  const handleSelectLocal = useCallback(() => {
    useSettingsStore.getState().clearRemoteModelsError();
    updateDraft(
      "llm",
      "active",
      activeSelection(
        "embedded",
        draftSettings?.llm?.last_selected?.embedded ?? draftSettings?.llm?.active?.model ?? ""
      )
    );
  }, [draftSettings?.llm?.last_selected?.embedded, draftSettings?.llm?.active?.model, updateDraft]);

  const handleStartInlineEdit = useCallback((id: string, currentKey: string) => {
    setEditingProviderId(id);
    setEditingKeyValue(currentKey);
  }, []);

  const handleCancelInlineKey = useCallback(() => {
    setEditingProviderId(null);
    setEditingKeyValue("");
  }, []);

  const handleSaveInlineKey = useCallback(
    async (providerId: string) => {
      const trimmed = editingKeyValue.trim();
      const currentKeys = { ...(draftSettings?.llm?.cloud_keys ?? draftSettings?.providers?.llm?.cloud_keys ?? {}) };
      if (trimmed) {
        currentKeys[providerId] = trimmed;
      } else {
        delete currentKeys[providerId];
      }

      updateDraft("llm", "cloud_keys", currentKeys);
      await useSettingsStore.getState().commitChanges({ domain: "llm", key: "cloud_keys" });

      setEditingProviderId(null);
      setEditingKeyValue("");
    },
    [editingKeyValue, draftSettings?.llm?.cloud_keys, draftSettings?.providers?.llm?.cloud_keys, updateDraft]
  );

  return {
    activeCloudProviderId,
    isLocalActive,
    onSelectLocal: handleSelectLocal,
    getProviderKey,
    pinnedProviderId,
    getCommittedProviderKey,
    onSelectProvider: handleSelectProvider,
    editingProviderId,
    editingKeyValue,
    setEditingKeyValue,
    onStartInlineEdit: handleStartInlineEdit,
    onCancelInlineKey: handleCancelInlineKey,
    onSaveInlineKey: handleSaveInlineKey,
    onNavigateToModels: options?.onNavigateToModels,
  };
}
