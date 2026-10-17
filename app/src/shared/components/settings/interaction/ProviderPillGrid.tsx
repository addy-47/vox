import { memo } from "react";
import { Check, X, Pencil, Trash2, ChevronDown, ChevronUp } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import { ProviderLogo } from "@/shared/components/settings/providers/ProviderLogo";
import type { CloudProvider } from "@/data/providersCopy";
import type { LlmCustomServer, TtsCustomServer } from "@/store/settingsStore";
import { INTERACTION_CONFIG_DESK_COPY, PROVIDERS_HEADER_COPY } from "@/data/settingsCopy";

export interface ProviderPillGridProps {
  currentCategory: "STT" | "LLM" | "TTS";
  showLocalCard: boolean;
  isLocalActive: boolean;
  committedActiveCategory?: "embedded" | "server" | "cloud" | null;
  committedActiveProviderId?: string | null;
  localMeta: { name: string; tagline: string };
  onSelectLocal: () => void;

  showCustomCards: boolean;
  customServers: Array<LlmCustomServer | TtsCustomServer>;
  activeCustomServerId?: string | null;
  onSelectCustomServer: (server: LlmCustomServer | TtsCustomServer) => void;
  onEditCustomServer: (server: LlmCustomServer | TtsCustomServer) => void;
  onDeleteCustomServer: (serverId: string) => void;

  providers: CloudProvider[];
  activeCloudProviderId?: string | null;
  editingProviderId: string | null;
  editingKeyValue: string;
  setEditingKeyValue: (val: string) => void;
  getProviderKey: (providerId: string) => string | undefined;
  onSelectCloudProvider: (provider: CloudProvider) => void;
  onStartInlineEdit: (providerId: string, currentKey: string) => void;
  onSaveInlineKey: (providerId: string) => void;
  onCancelInlineKey: () => void;

  showAllInline?: boolean;
  onToggleShowAll?: () => void;
  totalProvidersCount?: number;
  containerRef?: React.Ref<HTMLDivElement>;
}

export const ProviderPillGrid = memo(function ProviderPillGrid({
  currentCategory,
  showLocalCard,
  isLocalActive,
  committedActiveCategory,
  committedActiveProviderId,
  localMeta,
  onSelectLocal,
  showCustomCards,
  customServers,
  activeCustomServerId,
  onSelectCustomServer,
  onEditCustomServer,
  onDeleteCustomServer,
  providers,
  activeCloudProviderId,
  editingProviderId,
  editingKeyValue,
  setEditingKeyValue,
  getProviderKey,
  onSelectCloudProvider,
  onStartInlineEdit,
  onSaveInlineKey,
  onCancelInlineKey,
  showAllInline = false,
  onToggleShowAll,
  totalProvidersCount = 0,
  containerRef,
}: ProviderPillGridProps) {
  const copy = INTERACTION_CONFIG_DESK_COPY;

  const localCardNode = showLocalCard ? (
    <div
      key="local-pill"
      onClick={onSelectLocal}
      className={cn(
        "group flex items-center gap-2 px-2.5 py-2 rounded-xl transition-all cursor-pointer select-none border min-h-[46px]",
        isLocalActive
          ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
          : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
      )}
    >
      <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0">
        <ProviderLogo providerId="on-device" logo={null} size={14} className="shrink-0" />
      </div>
      <div className="flex flex-col min-w-0 flex-1 justify-center">
        <div className="flex items-center gap-1.5 leading-tight">
          <span className="text-[11.5px] font-semibold truncate text-[rgb(var(--foreground))]">
            {localMeta.name}
          </span>
        </div>
        <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/70 truncate mt-0.5 leading-tight">
          {localMeta.tagline}
        </span>
      </div>
      {isLocalActive && (
        <div className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 ml-auto">
          <Check size={9} strokeWidth={2.5} />
        </div>
      )}
    </div>
  ) : null;

  const customServerNodes = showCustomCards
    ? customServers.map((server) => {
        const isSelected = activeCustomServerId === server.id;
        return (
          <div
            key={server.id || server.url}
            onClick={() => onSelectCustomServer(server)}
            className={cn(
              "group flex items-center gap-2 px-2.5 py-2 rounded-xl transition-all cursor-pointer select-none border min-h-[46px]",
              isSelected
                ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
                : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
            )}
          >
            <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0">
              <ProviderLogo providerId={server.id || "custom-server"} logo={null} size={14} className="shrink-0" />
            </div>
            <div className="flex flex-col min-w-0 flex-1 justify-center">
              <span className="text-[11.5px] font-semibold truncate leading-tight text-[rgb(var(--foreground))]">
                {server.name || "Custom Server"}
              </span>
              <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/70 truncate mt-0.5 leading-tight">
                {server.url}
              </span>
            </div>
            <div className="flex items-center justify-end w-12 shrink-0 ml-auto relative" onClick={(e) => e.stopPropagation()}>
              {isSelected && (
                <div className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 transition-opacity [@media(hover:hover)]:group-hover:opacity-0 pointer-events-none">
                  <Check size={9} strokeWidth={2.5} />
                </div>
              )}
              <div className="absolute right-0 flex items-center gap-1 opacity-0 [@media(hover:hover)]:group-hover:opacity-100 transition-opacity">
                <button
                  type="button"
                  onClick={() => onEditCustomServer(server)}
                  className="p-1 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
                  title="Edit Server"
                  aria-label="Edit Server"
                >
                  <Pencil size={11} />
                </button>
                <button
                  type="button"
                  onClick={() => onDeleteCustomServer(server.id)}
                  className="p-1 rounded text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--error))] transition-colors cursor-pointer"
                  title="Delete Server"
                  aria-label="Delete Server"
                >
                  <Trash2 size={11} />
                </button>
              </div>
            </div>
          </div>
        );
      })
    : null;

  const cloudNodes = providers.map((provider) => {
    const isSelected = activeCloudProviderId === provider.id;
    const isEditing = editingProviderId === provider.id;
    const savedKey = getProviderKey(provider.id) ?? "";
    const hasKey = Boolean(savedKey.trim());

    return (
      <div
        key={provider.id}
        onClick={() => {
          if (!isEditing) {
            onSelectCloudProvider(provider);
          }
        }}
        className={cn(
          "group flex items-center gap-2 px-2.5 py-2 rounded-xl transition-all cursor-pointer select-none border min-h-[46px]",
          isSelected
            ? "bg-[rgba(var(--foreground),0.025)] border-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.12)] text-[rgb(var(--foreground))]"
            : "bg-[rgba(var(--foreground),0.015)] border-[rgba(var(--accent),0.06)] hover:border-[rgba(var(--accent),0.25)] hover:bg-[rgba(var(--foreground),0.03)] text-[rgb(var(--foreground-muted))]/80 hover:text-[rgb(var(--foreground))]"
        )}
      >
        {isEditing ? (
          <div
            className="w-full flex items-center gap-1.5 border-b border-[rgb(var(--accent))] pb-0.5 animate-fade-in"
            onClick={(e) => e.stopPropagation()}
          >
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
              title={copy.llm.cloud.saveKey}
              aria-label={copy.llm.cloud.saveKey}
              className="text-[rgb(var(--accent))] hover:opacity-75 transition-opacity p-0.5 shrink-0 cursor-pointer"
            >
              <Check size={13} />
            </button>
            <button
              type="button"
              onClick={onCancelInlineKey}
              title={copy.llm.cloud.cancelKey}
              aria-label={copy.llm.cloud.cancelKey}
              className="text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--foreground))] transition-colors p-0.5 shrink-0 cursor-pointer"
            >
              <X size={13} />
            </button>
          </div>
        ) : (
          <>
            <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0">
              <ProviderLogo providerId={provider.id} logo={provider.logo ?? null} size={14} className="shrink-0" />
            </div>
            <div className="flex flex-col min-w-0 flex-1 justify-center">
              <span
                className={cn(
                  "text-[11.5px] truncate leading-tight",
                  isSelected ? "font-semibold text-[rgb(var(--foreground))]" : "font-medium text-[rgb(var(--foreground))]/90"
                )}
              >
                {provider.name}
              </span>
              <span className="text-[9.5px] text-[rgb(var(--foreground-muted))]/70 truncate mt-0.5 leading-tight">
                {provider.tagline || "Cloud API"}
              </span>
            </div>
            <div className="shrink-0 flex items-center justify-end w-12 ml-auto relative" onClick={(e) => e.stopPropagation()}>
              {isSelected && (
                <div className="w-4 h-4 rounded-full bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.3)] flex items-center justify-center text-[rgb(var(--accent))] shrink-0 transition-opacity [@media(hover:hover)]:group-hover:opacity-0 pointer-events-none">
                  <Check size={9} strokeWidth={2.5} />
                </div>
              )}
              <div className="absolute right-0 flex items-center gap-1 opacity-0 [@media(hover:hover)]:group-hover:opacity-100 transition-opacity">
                {hasKey ? (
                  <button
                    type="button"
                    onClick={() => onStartInlineEdit(provider.id, savedKey)}
                    className="p-1 rounded text-[rgb(var(--foreground-muted))]/60 hover:text-[rgb(var(--accent))] transition-colors cursor-pointer"
                    title={copy.llm.cloud.editKey}
                    aria-label={copy.llm.cloud.editKey}
                  >
                    <Pencil size={11} />
                  </button>
                ) : (
                  <button
                    type="button"
                    onClick={() => onStartInlineEdit(provider.id, "")}
                    className="w-4 h-4 rounded flex items-center justify-center text-[13px] font-bold text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.12)] transition-all leading-none cursor-pointer"
                    aria-label="Connect provider"
                  >
                    +
                  </button>
                )}
              </div>
            </div>
          </>
        )}
      </div>
    );
  });

  return (
    <div
      ref={containerRef}
      className="max-h-[160px] overflow-y-scroll overscroll-contain custom-scrollbar pr-1 pb-2 w-full [scrollbar-gutter:stable]"
    >
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 content-start w-full">
        {committedActiveCategory === "server" && committedActiveProviderId
          ? <>{customServerNodes}{localCardNode}{cloudNodes}</>
          : committedActiveCategory === "cloud" && committedActiveProviderId
          ? <>{cloudNodes}{localCardNode}{customServerNodes}</>
          : <>{localCardNode}{cloudNodes}{customServerNodes}</>}

      {/* ── 5. Cloud STT Placeholder ── */}
      {currentCategory === "STT" && (
        <div
          title={PROVIDERS_HEADER_COPY.comingSoon}
          className="flex items-center gap-2 px-2.5 py-2 rounded-xl border border-[rgba(var(--foreground),0.04)] bg-[rgba(var(--foreground),0.01)] opacity-50 min-h-[46px] select-none cursor-not-allowed"
        >
          <div className="w-6 h-6 rounded-md bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--foreground),0.06)] flex items-center justify-center shrink-0 opacity-60">
            <ProviderLogo logo={null} size={14} className="shrink-0" />
          </div>
          <div className="flex flex-col min-w-0 flex-1 justify-center">
            <span className="text-[11.5px] font-medium text-[rgb(var(--foreground))] truncate leading-tight">
              Cloud STT
            </span>
            <span className="text-[9.5px] text-[rgb(var(--foreground-muted))] truncate mt-0.5 leading-tight">
              Hosted transcription
            </span>
          </div>
          <div className="text-[rgb(var(--foreground-muted))]/60 font-medium text-[10px] uppercase tracking-wider shrink-0 ml-auto bg-[rgba(var(--foreground),0.04)] px-1.5 py-0.5 rounded">
            {PROVIDERS_HEADER_COPY.comingSoon}
          </div>
        </div>
      )}

      {/* ── 6. Show All / Show Less Toggle ── */}
      {currentCategory === "LLM" && totalProvidersCount > providers.length && onToggleShowAll && (
        <div className="col-span-1 sm:col-span-2 pt-2 pb-1 flex flex-col items-center justify-center">
          <button
            type="button"
            onClick={onToggleShowAll}
            className="flex flex-col items-center justify-center gap-0.5 text-[rgb(var(--accent))] hover:opacity-80 transition-opacity cursor-pointer select-none group"
          >
            <span className="text-[11.5px] font-semibold tracking-tight">
              {showAllInline ? "Show less" : `Show all ${totalProvidersCount} providers`}
            </span>
            {showAllInline ? (
              <ChevronUp size={13} strokeWidth={2.5} className="group-hover:-translate-y-0.5 transition-transform" />
            ) : (
              <ChevronDown size={13} strokeWidth={2.5} className="group-hover:translate-y-0.5 transition-transform" />
            )}
          </button>
        </div>
      )}
      </div>
    </div>
  );
});

ProviderPillGrid.displayName = "ProviderPillGrid";
