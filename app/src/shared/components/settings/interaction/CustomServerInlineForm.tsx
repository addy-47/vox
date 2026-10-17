import { memo } from "react";
import { AlertCircle } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { UnderlineInput } from "@/shared/ui";
import { INTERACTION_CONFIG_DESK_COPY } from "@/data/settingsCopy";

export interface CustomServerInlineFormProps {
  category: "STT" | "LLM" | "TTS";
  url: string;
  onChangeUrl: (val: string) => void;
  apiKey: string;
  onChangeApiKey: (val: string) => void;
  name: string;
  onChangeName: (val: string) => void;
  ttsPath?: string;
  onChangeTtsPath?: (val: string) => void;
  layoutMode?: "full-max" | "full-min" | "small";
  modelsError?: string | null;
  showHealthErrorConfirm?: boolean;
  onDiscard?: () => void;
  onSaveAnyway?: () => void;
}

export const CustomServerInlineForm = memo(function CustomServerInlineForm({
  category,
  url,
  onChangeUrl,
  apiKey,
  onChangeApiKey,
  name,
  onChangeName,
  ttsPath = "~/.vox",
  onChangeTtsPath,
  layoutMode,
  modelsError,
  showHealthErrorConfirm,
  onDiscard,
  onSaveAnyway,
}: CustomServerInlineFormProps) {
  const copy = INTERACTION_CONFIG_DESK_COPY;

  return (
    <div className="flex flex-col gap-3 flex-1 min-h-0 overflow-y-auto custom-scrollbar justify-start animate-fade-in pt-1">
      {category === "LLM" && (
        <div
          className={cn(
            "grid gap-3 items-end flex-1 pb-1",
            layoutMode === "small" ? "grid-cols-1" : "grid-cols-1 sm:grid-cols-2"
          )}
        >
          <UnderlineInput
            label={copy.llm.server.urlLabel}
            value={url}
            onChange={(e) => onChangeUrl(e.target.value)}
            placeholder={copy.llm.server.urlPlaceholder}
          />
          <UnderlineInput
            type="password"
            label={copy.llm.server.apiKeyLabel}
            value={apiKey}
            onChange={(e) => onChangeApiKey(e.target.value)}
            placeholder={copy.llm.server.apiKeyPlaceholder}
          />
          <div className="col-span-1 sm:col-span-2">
            <UnderlineInput
              label={copy.llm.server.nameLabel ?? "Server Name (optional)"}
              value={name}
              onChange={(e) => onChangeName(e.target.value)}
              placeholder={copy.llm.server.namePlaceholder ?? "Auto-detect (e.g. Ollama, vLLM, LM Studio)"}
            />
          </div>
        </div>
      )}

      {category === "TTS" && (
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pb-1 flex-1 items-end">
          <UnderlineInput
            label={copy.tts.server.urlLabel}
            value={url}
            onChange={(e) => onChangeUrl(e.target.value)}
            placeholder={copy.tts.server.urlPlaceholder}
          />
          <UnderlineInput
            label={copy.tts.server.pathLabel}
            value={ttsPath}
            onChange={(e) => onChangeTtsPath?.(e.target.value)}
            placeholder={copy.tts.server.pathPlaceholder}
          />
        </div>
      )}

      {showHealthErrorConfirm && modelsError && (
        <div className="p-2.5 rounded-lg border border-[rgba(var(--error),0.3)] bg-[rgba(var(--error),0.06)] flex flex-col gap-2 shrink-0 min-w-0 overflow-hidden animate-fade-in">
          <div className="flex items-start gap-1.5 text-[11px] text-[rgb(var(--error))] min-w-0">
            <AlertCircle size={13} className="shrink-0 mt-px" />
            <span className="min-w-0 flex-1">
              <span className="font-bold">Server unreachable. </span>
              <span className="font-mono [overflow-wrap:anywhere]">{modelsError}</span>
            </span>
          </div>
          <div className="flex items-center justify-end gap-2">
            {onDiscard && (
              <button
                type="button"
                onClick={onDiscard}
                className="px-2 py-0.5 rounded text-[10.5px] font-medium text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors cursor-pointer"
              >
                {copy.llm.server.discard ?? "Discard"}
              </button>
            )}
            {onSaveAnyway && (
              <button
                type="button"
                onClick={onSaveAnyway}
                className="px-2.5 py-0.5 rounded text-[10.5px] font-bold bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.25)] border border-[rgba(var(--accent),0.3)] transition-all cursor-pointer"
              >
                Save Anyway
              </button>
            )}
          </div>
        </div>
      )}

      {modelsError && !showHealthErrorConfirm && (
        <span className="text-[11px] text-[rgb(var(--error))]/80 flex items-start gap-1 ml-0.5 shrink-0 min-w-0">
          <AlertCircle size={13} className="shrink-0 mt-px" />
          <span className="min-w-0 flex-1 [overflow-wrap:anywhere]">{modelsError}</span>
        </span>
      )}
    </div>
  );
});

CustomServerInlineForm.displayName = "CustomServerInlineForm";
