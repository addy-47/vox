import { memo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import {
  AlertCircle,
  ArrowLeft,
  Check,
  X,
} from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { Tooltip } from "@/shared/ui/Tooltip";
import { useSettingsStore, type LlmCustomServer, type TtsCustomServer } from "@/store/settingsStore";
import { useShallow } from "zustand/react/shallow";
import { probeProviderTarget } from "@/services/catalogService";
import { ipcErrorMessage } from "@/shared/lib/ipcError";
import { PROVIDERS_CONFIG_DESK_COPY } from "@/data/settingsCopy";

/** Inserts a server or replaces the entry with the same id (edit mode). */
export function upsertServer<T extends { id: string }>(list: T[], server: T): T[] {
  return list.some((s) => s.id === server.id)
    ? list.map((s) => (s.id === server.id ? server : s))
    : [...list, server];
}

/**
 * backend-gap: backend should mint canonical server ids on probe/save
 * (`setup_remote_server` response). Temp id is collision-safe only.
 */
export function mintCustomServerId(prefix: "llm_custom" | "tts_custom"): string {
  return `${prefix}_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`;
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
  const defaultUrl = isTts
    ? PROVIDERS_CONFIG_DESK_COPY.tts.server.urlPlaceholder
    : PROVIDERS_CONFIG_DESK_COPY.llm.server.urlPlaceholder;
  const initialUrl = initial?.url ?? "";
  const [url, setUrl] = useState(initialUrl);
  const [apiKey, setApiKey] = useState(initial?.api_key ?? "");
  const [name, setName] = useState(initial?.name ?? "");
  const [isProbing, setIsProbing] = useState(false);
  const [unhealthyReason, setUnhealthyReason] = useState<string | null>(null);

  const draftSettings = useSettingsStore(useShallow((s) => s.draftSettings));
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const commitChanges = useSettingsStore((s) => s.commitChanges);

  const handleSave = async () => {
    const trimmedUrl = url.trim();
    if (!trimmedUrl) return;

    setIsProbing(true);
    setUnhealthyReason(null);

    try {
      if (isTts) {
        const res = await probeProviderTarget({
          kind: "tts",
          baseUrl: trimmedUrl,
          apiKey: apiKey.trim() || undefined,
          providerId: initial?.id,
        });

        if (res.healthy) {
          const resolvedName = name.trim() || "Remote";
          const newServer: TtsCustomServer = {
            id: initial?.id ?? mintCustomServerId("tts_custom"),
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
          setUnhealthyReason(res.reason || "Connection failed");
        }
      } else {
        const res = await probeProviderTarget({
          kind: "server",
          baseUrl: trimmedUrl,
          apiKey: apiKey.trim() || undefined,
          providerId: name.trim() || undefined,
        });

        if (res.healthy) {
          const defaultDialectName = res.dialect
            ? res.dialect.charAt(0).toUpperCase() + res.dialect.slice(1)
            : "Remote Server";
          const resolvedName = name.trim() || defaultDialectName;
          const newServer: LlmCustomServer = {
            id: initial?.id ?? mintCustomServerId("llm_custom"),
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
    const trimmedUrl = url.trim() || defaultUrl;
    const fallbackName = name.trim() || "Custom Server";

    if (isTts) {
      const newServer: TtsCustomServer = {
        id: initial?.id ?? mintCustomServerId("tts_custom"),
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
        id: initial?.id ?? mintCustomServerId("llm_custom"),
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

  const urlRef = useRef<HTMLInputElement | null>(null);
  const apiKeyRef = useRef<HTMLInputElement | null>(null);
  const nameRef = useRef<HTMLInputElement | null>(null);

  const handleFormKeyDown = (e: ReactKeyboardEvent) => {
    if (e.key === "Escape") {
      // Cancel only this form card — the parent modal must stay open.
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key !== "Enter") return;
    if (e.target === urlRef.current) {
      // URL field: advance to the API-key input.
      e.stopPropagation();
      apiKeyRef.current?.focus();
      return;
    }
    if (e.target === apiKeyRef.current) {
      // API-key field: advance to the name input.
      e.stopPropagation();
      nameRef.current?.focus();
      return;
    }
    if (e.target === nameRef.current) {
      // Name is the final field: submit the save probe.
      e.stopPropagation();
      if (!isProbing && url.trim()) void handleSave();
    }
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
        <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.llm.cloud.cancelKey} side="top" asChild>
          <button
            type="button"
            onClick={handleDiscard}
            className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-1 cursor-pointer bg-transparent rounded"
            aria-label={PROVIDERS_CONFIG_DESK_COPY.llm.cloud.cancelKey}
          >
            <X size={14} />
          </button>
        </Tooltip>
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
            <Tooltip label="Return to editing inputs" side="top" asChild>
              <button
                type="button"
                onClick={() => setUnhealthyReason(null)}
                className="text-[11px] font-medium text-[rgb(var(--accent))] hover:underline transition-all cursor-pointer bg-transparent flex items-center gap-1 p-0"
              >
                <ArrowLeft size={12} />
                Edit
              </button>
            </Tooltip>
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
        <div className="flex flex-col gap-2 py-2 flex-1 justify-center" onKeyDown={handleFormKeyDown}>
          <input
            type="text"
            ref={urlRef}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder={defaultUrl}
            className="w-full text-[11.5px] font-mono pb-1 bg-transparent border-b border-[rgba(var(--foreground),0.12)] focus:border-[rgb(var(--accent))] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 transition-colors"
          />
          <div className="flex gap-2">
            <input
              type="password"
              ref={apiKeyRef}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder={PROVIDERS_CONFIG_DESK_COPY.llm.server.apiKeyLabel}
              className="flex-1 text-[11.5px] font-mono pb-1 bg-transparent border-b border-[rgba(var(--foreground),0.12)] focus:border-[rgb(var(--accent))] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 transition-colors"
            />
            <input
              type="text"
              ref={nameRef}
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
            <Tooltip label={PROVIDERS_CONFIG_DESK_COPY.llm.server.saveServer} side="top" asChild>
              <button
                type="button"
                disabled={isProbing || !url.trim()}
                onClick={handleSave}
                className="flex items-center gap-1 p-1 rounded text-[rgb(var(--accent))] hover:opacity-75 transition-opacity font-bold text-[11.5px] cursor-pointer disabled:opacity-40 bg-transparent"
                aria-label={PROVIDERS_CONFIG_DESK_COPY.llm.server.saveServer}
              >
              {isProbing ? (
                <span className="w-3.5 h-3.5 border-2 border-[rgb(var(--accent))] border-t-transparent rounded-full animate-spin" />
              ) : (
                <Check size={14} strokeWidth={2.5} />
              )}
              <span>Save</span>
            </button>
          </Tooltip>
          </div>
        </div>
      )}
    </div>
  );
});
CustomServerFormCard.displayName = "CustomServerFormCard";
