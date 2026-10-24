import { memo, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { AlertCircle, Check, Loader2, Plus, X } from "lucide-react";
import { useSettingsStore, type LlmCustomServer } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import {
  activeSelection,
  getEffectiveCloudApiKey,
  getEffectiveCloudKeyMap,
  getEffectiveLlmServers,
  resolveProviderModel,
} from "@/shared/lib/activeSelection";
import { useExpandableList } from "@/shared/ui/ExpandableList";
import { VoxLogoLoader } from "@/shared/components/common/VoxLogoLoader";
import { LANGUAGE_CARD_COPY } from "@/data/settingsCopy";
import { Tooltip } from "@/shared/ui/Tooltip";
import { probeProviderTarget } from "@/services/catalogService";
import { ipcErrorMessage } from "@/shared/lib/ipcError";
import { cn } from "@/shared/lib/utils";
import { AddCard } from "../shared/AddCard";
import { CatalogReveal } from "../shared/CatalogReveal";
import { ProviderPill } from "./ProviderPill";

export interface ProviderGridProps {
  /** Swaps the card face to the active provider's model catalog. */
  onOpenModels: () => void;
  /** Ensures parent modal level is primed to 'providers' before opening. */
  onExpand?: () => void;
}

/**
 * Renders items in two decoupled vertical columns (alternating left/right).
 * Vertical expansion in one column shifts only that column downward.
 */
function DecoupledColumnGrid({
  items,
  className,
}: {
  items: React.ReactNode[];
  className?: string;
}) {
  const left = items.filter((_, i) => i % 2 === 0);
  const right = items.filter((_, i) => i % 2 === 1);

  return (
    <div className={cn("flex gap-2 items-start w-full", className)}>
      <div className="flex-1 min-w-0 flex flex-col gap-2">{left}</div>
      <div className="flex-1 min-w-0 flex flex-col gap-2">{right}</div>
    </div>
  );
}

interface CustomServerInPlaceCardProps {
  name: string;
  url: string;
  apiKey: string;
  isSaving: boolean;
  unhealthyReason: string | null;
  onNameChange: (val: string) => void;
  onUrlChange: (val: string) => void;
  onKeyChange: (val: string) => void;
  onSave: () => void;
  onCancel: () => void;
}

const CustomServerInPlaceCard = memo(({
  name,
  url,
  apiKey,
  isSaving,
  unhealthyReason,
  onNameChange,
  onUrlChange,
  onKeyChange,
  onSave,
  onCancel,
}: CustomServerInPlaceCardProps) => {
  return (
    <div
      onClick={(e) => e.stopPropagation()}
      className="group relative flex w-full flex-col select-none rounded-xl border border-[rgba(var(--accent),0.35)] bg-[rgba(var(--foreground),0.035)] p-2.5 transition-colors"
    >
      {/* Top Row: Name Input + Action Buttons (No header text) */}
      <div className="flex items-center justify-between gap-2 w-full">
        <input
          autoFocus
          type="text"
          value={name}
          onChange={(e) => onNameChange(e.target.value)}
          placeholder="Server name (e.g. Ollama, vLLM)"
          aria-label="Server name"
          className="min-w-0 flex-1 text-[13px] font-semibold pb-0.5 bg-transparent border-b border-[rgba(var(--foreground),0.15)] focus:border-[rgb(var(--accent))] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 transition-colors"
        />
        <div className="flex items-center gap-1 shrink-0">
          <Tooltip label="Save Server" side="top" asChild>
            <button
              type="button"
              disabled={isSaving || !url.trim()}
              onClick={(e) => {
                e.stopPropagation();
                onSave();
              }}
              className="p-1 rounded-md text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.14)] transition-colors cursor-pointer disabled:opacity-40"
              aria-label="Save Server"
            >
              {isSaving ? <Loader2 size={13} className="animate-spin" /> : <Check size={13} strokeWidth={2.5} />}
            </button>
          </Tooltip>
          <Tooltip label={LANGUAGE_CARD_COPY.cancel} side="top" asChild>
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onCancel();
              }}
              className="p-1 rounded-md text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.08)] transition-colors cursor-pointer"
              aria-label="Cancel"
            >
              <X size={13} strokeWidth={2.25} />
            </button>
          </Tooltip>
        </div>
      </div>

      {/* URL & Key Inputs: Minimal border-b inputs matching design spec */}
      <div className="mt-2 flex flex-col gap-1.5 w-full">
        <input
          type="text"
          value={url}
          onChange={(e) => onUrlChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") onSave();
            if (e.key === "Escape") onCancel();
          }}
          placeholder="http://localhost:11434"
          aria-label="Server URL"
          className="w-full text-[11.5px] font-mono pb-0.5 bg-transparent border-b border-[rgba(var(--foreground),0.12)] focus:border-[rgb(var(--accent))] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 transition-colors"
        />
        <input
          type="password"
          value={apiKey}
          onChange={(e) => onKeyChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") onSave();
            if (e.key === "Escape") onCancel();
          }}
          placeholder="API Key (optional)"
          aria-label="API Key"
          className="w-full text-[11.5px] font-mono pb-0.5 bg-transparent border-b border-[rgba(var(--foreground),0.12)] focus:border-[rgb(var(--accent))] outline-none text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 transition-colors"
        />
      </div>

      {unhealthyReason && (
        <p className="mt-1.5 text-[11px] text-[rgb(var(--danger))] flex items-center gap-1">
          <AlertCircle size={12} className="shrink-0" />
          <span className="truncate">{unhealthyReason}</span>
        </p>
      )}
    </div>
  );
});

CustomServerInPlaceCard.displayName = "CustomServerInPlaceCard";

/**
 * The language card's first face.
 *
 * One scroller. Ready providers sit at the top in full strength, the saved one
 * first: the grid follows the saved configuration, so picking a provider in
 * the draft never reshuffles the list. A hairline separates them from the
 * unconnected providers, which stay dimmed and carry a plus that opens an
 * inline key field inside their own row.
 */
export const ProviderGrid = memo(({ onOpenModels, onExpand }: ProviderGridProps) => {
  const draftSettings = useSettingsStore(useShallow((s) => s.draftSettings));
  const committedSettings = useSettingsStore(useShallow((s) => s.settings));
  const modelCatalog = useCatalogStore((s) => s.modelCatalog);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const cloudProviders = useCatalogStore((s) => s.cloudProviders);
  const { open } = useExpandableList();

  const [editingKeyId, setEditingKeyId] = useState<string | null>(null);
  const [keyValue, setKeyValue] = useState("");
  const [savingKey, setSavingKey] = useState(false);

  // Custom Server In-Place Form State
  const [isCreatingServer, setIsCreatingServer] = useState(false);
  const [editingServerId, setEditingServerId] = useState<string | null>(null);
  const [serverName, setServerName] = useState("");
  const [serverUrl, setServerUrl] = useState("");
  const [serverKey, setServerKey] = useState("");
  const [isProbing, setIsProbing] = useState(false);
  const [unhealthyReason, setUnhealthyReason] = useState<string | null>(null);

  const startAddServer = () => {
    setEditingKeyId(null);
    setEditingServerId(null);
    setServerName("");
    setServerUrl("");
    setServerKey("");
    setUnhealthyReason(null);
    setIsCreatingServer(true);
  };

  const startEditServer = (server: { id: string; name?: string; url?: string; api_key?: string | null }) => {
    setEditingKeyId(null);
    setIsCreatingServer(false);
    setServerName(server.name || "");
    setServerUrl(server.url || "");
    setServerKey(server.api_key || "");
    setUnhealthyReason(null);
    setEditingServerId(server.id);
  };

  const cancelCustomServer = () => {
    setIsCreatingServer(false);
    setEditingServerId(null);
    setServerName("");
    setServerUrl("");
    setServerKey("");
    setUnhealthyReason(null);
  };

  const saveCustomServer = async () => {
    const trimmedUrl = serverUrl.trim();
    if (!trimmedUrl || isProbing) return;

    setIsProbing(true);
    setUnhealthyReason(null);

    try {
      const res = await probeProviderTarget({
        kind: "server",
        baseUrl: trimmedUrl,
        apiKey: serverKey.trim() || undefined,
        providerId: serverName.trim() || undefined,
      });

      if (res.healthy) {
        const defaultDialectName = res.dialect
          ? res.dialect.charAt(0).toUpperCase() + res.dialect.slice(1)
          : "Remote Server";
        const resolvedName = serverName.trim() || defaultDialectName;
        const newServer: LlmCustomServer = {
          id: editingServerId ?? `llm_custom_${Date.now()}_${Math.random().toString(36).slice(2, 8)}`,
          name: resolvedName,
          url: trimmedUrl,
          model: "",
          protocol: "openai",
          api_key: serverKey.trim() || null,
        };
        const currentList = draftSettings?.llm.servers ?? draftSettings?.providers?.llm.servers ?? [];
        const nextList = currentList.some((s) => s.id === newServer.id)
          ? currentList.map((s) => (s.id === newServer.id ? newServer : s))
          : [...currentList, newServer];

        updateDraft("llm", "servers", nextList);
        cancelCustomServer();
        await useSettingsStore.getState().commitChanges({ domain: "llm", key: "servers" });
      } else {
        setUnhealthyReason(res.reason || "Connection failed");
      }
    } catch (err) {
      setUnhealthyReason(ipcErrorMessage(err));
    } finally {
      setIsProbing(false);
    }
  };

  const deleteCustomServer = async (serverId: string) => {
    if (editingServerId === serverId) {
      cancelCustomServer();
    }
    const currentList = draftSettings?.llm.servers ?? draftSettings?.providers?.llm.servers ?? [];
    updateDraft("llm", "servers", currentList.filter((s) => s.id !== serverId));
    const isActive = activeCategory === "server" && active?.provider === serverId;
    if (isActive) {
      const fallback = draftSettings?.llm.last_selected.embedded ?? "";
      updateDraft("llm", "active", activeSelection("embedded", fallback));
    }
    await useSettingsStore.getState().commitChanges({ domain: "llm", key: "servers" });
    if (isActive) {
      await useSettingsStore.getState().commitChanges({ domain: "llm", key: "active" });
    }
  };

  const isRealtime = draftSettings?.interaction.pipeline_mode === "realtime";
  const active = draftSettings?.llm.active;
  const activeCategory = active?.category ?? "embedded";
  const localModelId =
    draftSettings?.llm.last_selected.embedded ??
    (activeCategory === "embedded" ? active?.model : undefined) ??
    "";

  const localModelName = useMemo(() => {
    const match = modelCatalog?.llm.find((m) => m.id === localModelId);
    return match?.name ?? "";
  }, [modelCatalog?.llm, localModelId]);

  const servers = useMemo(
    () => getEffectiveLlmServers(draftSettings, committedSettings),
    [draftSettings, committedSettings]
  );

  /** Split cloud providers by key state. Ready keeps the contract order. */
  const { readyClouds, restClouds } = useMemo(() => {
    const ready: typeof cloudProviders = [];
    const rest: typeof cloudProviders = [];
    for (const provider of cloudProviders) {
      const hasKey = Boolean(
        getEffectiveCloudApiKey(draftSettings, committedSettings, provider.id)?.trim()
      );
      (provider.isConfigured || hasKey ? ready : rest).push(provider);
    }
    return { readyClouds: ready, restClouds: rest };
  }, [cloudProviders, draftSettings, committedSettings]);

  /**
   * The saved provider leads the list. The draft selection is a separate
   * thing: it decides what the model catalog shows, never the order here.
   */
  const savedKey = useMemo(() => {
    const saved = committedSettings?.llm.active;
    if (!saved) return "local";
    if (saved.category === "cloud") return `cloud:${saved.provider ?? ""}`;
    if (saved.category === "server") return `server:${saved.provider ?? ""}`;
    return "local";
  }, [committedSettings?.llm.active]);

  const committedActive = committedSettings?.llm.active;

  /**
   * Switching provider writes a model that belongs to the target provider.
   * Carrying the previous provider's model id is what kept the domain dirty
   * after the user returned to the saved provider.
   *
   * Neither of these navigates: the row body selects, and the row's arrow is a
   * separate hitbox that browses that provider's models.
   */
  const selectLocal = () => {
    useCatalogStore.getState().clearRemoteModelsError();
    updateDraft(
      "llm",
      "active",
      activeSelection(
        "embedded",
        resolveProviderModel({ category: "embedded", provider: null }, committedActive, localModelId)
      )
    );
  };

  const selectCloud = (providerId: string) => {
    useCatalogStore.getState().clearRemoteModelsError();
    // A cloud provider has no model until its catalog supplies one. Reverting
    // to the committed provider restores the committed model instead.
    updateDraft(
      "llm",
      "active",
      activeSelection(
        "cloud",
        resolveProviderModel({ category: "cloud", provider: providerId }, committedActive),
        providerId
      )
    );
  };

  const selectServer = (serverId: string, model: string) => {
    useCatalogStore.getState().clearRemoteModelsError();
    updateDraft(
      "llm",
      "active",
      activeSelection(
        "server",
        resolveProviderModel({ category: "server", provider: serverId }, committedActive, model),
        serverId
      )
    );
  };

  /**
   * The arrow hitbox: browse this provider's models, selecting it first only
   * when it is not already the draft target. Re-selecting an already selected
   * provider would restore its committed model and silently drop a pending
   * model choice.
   */
  const browseModels = (select: () => void, alreadyActive: boolean) => {
    if (!alreadyActive) select();
    onOpenModels();
  };

  const saveKey = async () => {
    const providerId = editingKeyId;
    const trimmed = keyValue.trim();
    if (!providerId || !trimmed || savingKey) return;

    setSavingKey(true);
    try {
      if (isRealtime) {
        const keys = { ...(draftSettings?.realtime?.api_keys ?? {}) };
        keys[providerId] = trimmed;
        updateDraft("realtime", "api_keys", keys);
        if (providerId === "gemini_live") {
          const cloudKeys = getEffectiveCloudKeyMap(draftSettings, committedSettings);
          cloudKeys.gemini = trimmed;
          updateDraft("llm", "cloud_keys", cloudKeys);
        }
        await useSettingsStore.getState().commitChanges({ domain: "realtime", key: "api_keys" });
        setEditingKeyId(null);
        setKeyValue("");
        return;
      }
      const keys = getEffectiveCloudKeyMap(draftSettings, committedSettings);
      keys[providerId] = trimmed;
      updateDraft("llm", "cloud_keys", keys);
      await useSettingsStore.getState().commitChanges({ domain: "llm", key: "cloud_keys" });
      // A rejected write keeps the editor open: the footer banner names the
      // failed key and the typed value stays editable instead of vanishing.
      const failed = useSettingsStore.getState().failedSaveDomains.language ?? [];
      if (failed.length > 0) return;
      setEditingKeyId(null);
      setKeyValue("");
    } finally {
      setSavingKey(false);
    }
  };

  const deleteCloudKey = async (providerId: string) => {
    if (editingKeyId === providerId) {
      setEditingKeyId(null);
      setKeyValue("");
    }
    if (isRealtime) {
      const keys = { ...(draftSettings?.realtime?.api_keys ?? {}) };
      delete keys[providerId];
      updateDraft("realtime", "api_keys", keys);
      await useSettingsStore.getState().commitChanges({ domain: "realtime", key: "api_keys" });
      return;
    }
    const keys = getEffectiveCloudKeyMap(draftSettings, committedSettings);
    delete keys[providerId];
    updateDraft("llm", "cloud_keys", keys);

    const isActive = activeCategory === "cloud" && active?.provider === providerId;
    if (isActive) {
      updateDraft(
        "llm",
        "active",
        activeSelection(
          "embedded",
          resolveProviderModel({ category: "embedded", provider: null }, committedActive, localModelId)
        )
      );
    }

    await useSettingsStore.getState().commitChanges({ domain: "llm", key: "cloud_keys" });
    if (isActive) {
      await useSettingsStore.getState().commitChanges({ domain: "llm", key: "active" });
    }
  };

  if (isRealtime) {
    const realtimeProviders = [
      {
        id: "gemini_live",
        name: "Gemini Live",
        providerName: "gemini",
        defaultModel: "gemini-2.5-flash",
      },
      {
        id: "deepgram_voice_agent",
        name: "Deepgram Agent",
        providerName: "deepgram",
        defaultModel: "gpt-5.2",
      },
    ];

    const currentPid = draftSettings?.realtime.active.provider ?? "gemini_live";
    const currentModel = draftSettings?.realtime.active.model ?? "";

    const rows = realtimeProviders.map((prov) => {
      const isActive = currentPid === prov.id;
      const key =
        draftSettings?.realtime?.api_keys?.[prov.id] ||
        (prov.id === "gemini_live"
          ? getEffectiveCloudApiKey(draftSettings, committedSettings, "gemini")
          : undefined) ||
        "";
      const hasKey = key.trim().length > 0;
      const isEditing = editingKeyId === prov.id;

      return (
        <ProviderPill
          key={prov.id}
          id={prov.id}
          name={prov.name}
          providerName={prov.providerName}
          meta={
            hasKey
              ? isActive && currentModel
                ? `${LANGUAGE_CARD_COPY.metaCloud} · ${currentModel}`
                : LANGUAGE_CARD_COPY.keySaved
              : LANGUAGE_CARD_COPY.needsKey
          }
          active={isActive}
          muted={!hasKey && !isEditing}
          onSelect={() => {
            updateDraft("realtime", "active", {
              category: "cloud",
              provider: prov.id,
              model: isActive && currentModel ? currentModel : prov.defaultModel,
            });
          }}
          onBrowse={
            isEditing
              ? undefined
              : () => {
                  if (!isActive) {
                    updateDraft("realtime", "active", {
                      category: "cloud",
                      provider: prov.id,
                      model: prov.defaultModel,
                    });
                  }
                  onOpenModels();
                }
          }
          onEdit={
            isEditing
              ? undefined
              : () => {
                  setEditingKeyId(prov.id);
                  setKeyValue(key);
                }
          }
          onDelete={
            isEditing || !hasKey
              ? undefined
              : () => {
                  void deleteCloudKey(prov.id);
                }
          }
          trailing={
            isEditing ? (
              <div className="flex items-center gap-1 shrink-0">
                <button
                  type="button"
                  aria-label={LANGUAGE_CARD_COPY.saveKey}
                  onClick={(e) => {
                    e.stopPropagation();
                    void saveKey();
                  }}
                  disabled={savingKey || !keyValue.trim()}
                  className="rounded-md p-1 text-[rgb(var(--accent))] transition-colors hover:bg-[rgba(var(--accent),0.14)] cursor-pointer disabled:opacity-40"
                >
                  {savingKey ? <Loader2 size={12} className="animate-spin" /> : <Check size={13} strokeWidth={2.5} />}
                </button>
                <button
                  type="button"
                  aria-label={LANGUAGE_CARD_COPY.cancel}
                  onClick={(e) => {
                    e.stopPropagation();
                    setEditingKeyId(null);
                  }}
                  className="rounded-md p-1 text-[rgb(var(--foreground-muted))] transition-colors hover:bg-[rgba(var(--foreground),0.08)] hover:text-[rgb(var(--foreground))] cursor-pointer"
                >
                  <X size={13} strokeWidth={2.25} />
                </button>
              </div>
            ) : !hasKey ? (
              <button
                type="button"
                aria-label={LANGUAGE_CARD_COPY.addProvider}
                onClick={(e) => {
                  e.stopPropagation();
                  setEditingKeyId(prov.id);
                  setKeyValue("");
                }}
                className="rounded-md p-1 text-[rgb(var(--foreground-muted))]/60 transition-colors hover:bg-[rgba(var(--foreground),0.06)] hover:text-[rgb(var(--foreground))] cursor-pointer"
              >
                <Plus size={14} />
              </button>
            ) : undefined
          }
          expandedContent={
            isEditing ? (
              <input
                autoFocus
                type="password"
                value={keyValue}
                onClick={(e) => e.stopPropagation()}
                onChange={(e) => setKeyValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") {
                    e.stopPropagation();
                    setEditingKeyId(null);
                  }
                  if (e.key === "Enter") {
                    e.stopPropagation();
                    void saveKey();
                  }
                }}
                placeholder={`${prov.name} ${LANGUAGE_CARD_COPY.keyPlaceholder}`}
                aria-label={`${prov.name} ${LANGUAGE_CARD_COPY.keyPlaceholder}`}
                className="w-full bg-transparent border-b border-[rgba(var(--foreground),0.15)] focus:border-[rgb(var(--accent))] py-1 text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 focus:outline-none transition-colors"
              />
            ) : undefined
          }
        />
      );
    });

    return (
      <div className="flex-1 w-full flex flex-col min-h-0 relative">
        <CatalogReveal
          className="flex-1 min-h-0"
          featherHeight={48}
          scrollerClassName="flex-1 min-h-0 overflow-y-auto overscroll-contain pr-1 custom-scrollbar [scrollbar-gutter:stable]"
        >
          <div className="space-y-1.5 pb-2">{rows}</div>
        </CatalogReveal>
      </div>
    );
  }

  /**
   * Ready rows in display order: the saved provider leads, then on-device,
   * servers, and keyed cloud providers. A stable sort keeps the rest in order.
   */
  const readyRows = (() => {
    const rows: Array<{ key: string; node: React.ReactNode }> = [
      {
        key: "local",
        node: (
          <ProviderPill
            id="embedded"
            name={LANGUAGE_CARD_COPY.onDevice}
            leading={<VoxLogoLoader size={28} animated={false} decorative />}
            meta={
              localModelName
                ? `${LANGUAGE_CARD_COPY.metaLocal} · ${localModelName}`
                : LANGUAGE_CARD_COPY.localPrivate
            }
            active={activeCategory === "embedded"}
            onSelect={selectLocal}
            onBrowse={() => browseModels(selectLocal, activeCategory === "embedded")}
          />
        ),
      },
      ...servers.map((server) => {
        if (editingServerId === server.id) {
          return {
            key: `server:${server.id}`,
            node: (
              <CustomServerInPlaceCard
                key={`server-edit:${server.id}`}
                name={serverName}
                url={serverUrl}
                apiKey={serverKey}
                isSaving={isProbing}
                unhealthyReason={unhealthyReason}
                onNameChange={setServerName}
                onUrlChange={setServerUrl}
                onKeyChange={setServerKey}
                onSave={saveCustomServer}
                onCancel={cancelCustomServer}
              />
            ),
          };
        }

        return {
          key: `server:${server.id}`,
          node: (
            <ProviderPill
              id={server.id}
              name={server.name || LANGUAGE_CARD_COPY.customServerFallback}
              providerName={server.name}
              meta={
                activeCategory === "server" && active?.provider === server.id && active.model
                  ? `${LANGUAGE_CARD_COPY.metaSelfHosted} · ${active.model}`
                  : `${LANGUAGE_CARD_COPY.metaSelfHosted} · ${server.model || replaceProtocol(server.url)}`
              }
              active={activeCategory === "server" && active?.provider === server.id}
              onSelect={() => selectServer(server.id, server.model || "")}
              onBrowse={() =>
                browseModels(
                  () => selectServer(server.id, server.model || ""),
                  activeCategory === "server" && active?.provider === server.id
                )
              }
              onEdit={() => startEditServer(server)}
              onDelete={() => void deleteCustomServer(server.id)}
            />
          ),
        };
      }),
      ...readyClouds.map((provider) => {
        const isEditing = editingKeyId === provider.id;
        const currentKey = getEffectiveCloudApiKey(draftSettings, committedSettings, provider.id) ?? "";
        return {
          key: `cloud:${provider.id}`,
          node: (
            <ProviderPill
              id={provider.id}
              name={provider.name}
              logo={provider.logo ?? null}
              meta={
                activeCategory === "cloud" && active?.provider === provider.id && active.model
                  ? `${LANGUAGE_CARD_COPY.metaCloud} · ${active.model}`
                  : LANGUAGE_CARD_COPY.keySaved
              }
              active={activeCategory === "cloud" && active?.provider === provider.id}
              onSelect={() => {
                if (isEditing) return;
                selectCloud(provider.id);
              }}
              onBrowse={
                isEditing
                  ? undefined
                  : () =>
                      browseModels(
                        () => selectCloud(provider.id),
                        activeCategory === "cloud" && active?.provider === provider.id
                      )
              }
              onEdit={
                isEditing
                  ? undefined
                  : () => {
                      setEditingKeyId(provider.id);
                      setKeyValue(currentKey);
                    }
              }
              onDelete={
                isEditing
                  ? undefined
                  : () => {
                      void deleteCloudKey(provider.id);
                    }
              }
              trailing={
                isEditing ? (
                  <div className="flex items-center gap-1 shrink-0">
                    <Tooltip label={LANGUAGE_CARD_COPY.saveKey} side="top" asChild>
                      <button
                        type="button"
                        aria-label={LANGUAGE_CARD_COPY.saveKey}
                        onClick={(e) => {
                          e.stopPropagation();
                          void saveKey();
                        }}
                        disabled={savingKey || !keyValue.trim()}
                        className="rounded-md p-1 text-[rgb(var(--accent))] transition-colors hover:bg-[rgba(var(--accent),0.14)] cursor-pointer disabled:opacity-40"
                      >
                        {savingKey ? <Loader2 size={12} className="animate-spin" /> : <Check size={13} strokeWidth={2.5} />}
                      </button>
                    </Tooltip>
                    <Tooltip label={LANGUAGE_CARD_COPY.cancel} side="top" asChild>
                      <button
                        type="button"
                        aria-label={LANGUAGE_CARD_COPY.cancel}
                        onClick={(e) => {
                          e.stopPropagation();
                          setEditingKeyId(null);
                        }}
                        className="rounded-md p-1 text-[rgb(var(--foreground-muted))] transition-colors hover:bg-[rgba(var(--foreground),0.08)] hover:text-[rgb(var(--foreground))] cursor-pointer"
                      >
                        <X size={13} strokeWidth={2.25} />
                      </button>
                    </Tooltip>
                  </div>
                ) : undefined
              }
              expandedContent={
                isEditing ? (
                  <input
                    autoFocus
                    type="password"
                    value={keyValue}
                    onClick={(e) => e.stopPropagation()}
                    onChange={(e) => setKeyValue(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Escape") {
                        e.stopPropagation();
                        setEditingKeyId(null);
                      }
                      if (e.key === "Enter") {
                        e.stopPropagation();
                        void saveKey();
                      }
                    }}
                    placeholder={`${provider.name} ${LANGUAGE_CARD_COPY.keyPlaceholder}`}
                    aria-label={`${provider.name} ${LANGUAGE_CARD_COPY.keyPlaceholder}`}
                    className="w-full bg-transparent border-b border-[rgba(var(--foreground),0.15)] focus:border-[rgb(var(--accent))] py-1 text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 focus:outline-none transition-colors"
                  />
                ) : undefined
              }
            />
          ),
        };
      }),
    ];

    const allCandidates = rows;
    const savedRow = allCandidates.find((r) => r.key === savedKey);

    const orderedConnectedClouds = readyClouds
      .map((p) => allCandidates.find((r) => r.key === `cloud:${p.id}`))
      .filter((r): r is { key: string; node: React.ReactNode } => Boolean(r && r.key !== savedKey));

    const orderedServers = servers
      .map((s) => allCandidates.find((r) => r.key === `server:${s.id}`))
      .filter((r): r is { key: string; node: React.ReactNode } => Boolean(r && r.key !== savedKey));

    const localRow = allCandidates.find((r) => r.key === "local");
    const orderedLocal = localRow && localRow.key !== savedKey ? [localRow] : [];

    const finalRows: Array<{ key: string; node: React.ReactNode }> = [];
    if (savedRow) {
      finalRows.push(savedRow);
    }
    finalRows.push(...orderedConnectedClouds);
    finalRows.push(...orderedServers);
    finalRows.push(...orderedLocal);

    return finalRows;
  })();

  const addProviderNode = isCreatingServer ? (
    <CustomServerInPlaceCard
      key="add-custom-server-form"
      name={serverName}
      url={serverUrl}
      apiKey={serverKey}
      isSaving={isProbing}
      unhealthyReason={unhealthyReason}
      onNameChange={setServerName}
      onUrlChange={setServerUrl}
      onKeyChange={setServerKey}
      onSave={saveCustomServer}
      onCancel={cancelCustomServer}
    />
  ) : (
    <AddCard
      key="add-provider-card"
      title={LANGUAGE_CARD_COPY.addProvider}
      onClick={() => {
        startAddServer();
      }}
    />
  );

  const readyItems = readyRows.map((r) => r.node).concat(addProviderNode);

  const restRows = restClouds.map((provider) => {
    const isEditing = editingKeyId === provider.id;

    return (
      <ProviderPill
        key={`cloud:${provider.id}`}
        id={provider.id}
        name={provider.name}
        logo={provider.logo ?? null}
        meta={LANGUAGE_CARD_COPY.needsKey}
        active={false}
        muted={!isEditing}
        trailing={
          isEditing ? (
            <div className="flex items-center gap-1 shrink-0">
              <Tooltip label={LANGUAGE_CARD_COPY.saveKey} side="top" asChild>
                <button
                  type="button"
                  aria-label={LANGUAGE_CARD_COPY.saveKey}
                  onClick={(e) => {
                    e.stopPropagation();
                    void saveKey();
                  }}
                  disabled={savingKey || !keyValue.trim()}
                  className="rounded-md p-1 text-[rgb(var(--accent))] transition-colors hover:bg-[rgba(var(--accent),0.14)] cursor-pointer disabled:opacity-40"
                >
                  {savingKey ? <Loader2 size={12} className="animate-spin" /> : <Check size={13} strokeWidth={2.5} />}
                </button>
              </Tooltip>
              <Tooltip label={LANGUAGE_CARD_COPY.cancel} side="top" asChild>
                <button
                  type="button"
                  aria-label={LANGUAGE_CARD_COPY.cancel}
                  onClick={(e) => {
                    e.stopPropagation();
                    setEditingKeyId(null);
                  }}
                  className="rounded-md p-1 text-[rgb(var(--foreground-muted))] transition-colors hover:bg-[rgba(var(--foreground),0.08)] hover:text-[rgb(var(--foreground))] cursor-pointer"
                >
                  <X size={13} strokeWidth={2.25} />
                </button>
              </Tooltip>
            </div>
          ) : (
            <Plus
              size={14}
              strokeWidth={2.25}
              className="text-[rgb(var(--foreground-muted))] transition-colors group-hover:text-[rgb(var(--accent))]"
            />
          )
        }
        onSelect={() => {
          if (isEditing) {
            setEditingKeyId(null);
            return;
          }
          setEditingKeyId(provider.id);
          setKeyValue("");
        }}
        expandedContent={
          isEditing ? (
            <input
              autoFocus
              type="password"
              value={keyValue}
              onClick={(e) => e.stopPropagation()}
              onChange={(e) => setKeyValue(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  e.stopPropagation();
                  setEditingKeyId(null);
                }
                if (e.key === "Enter") {
                  e.stopPropagation();
                  void saveKey();
                }
              }}
              placeholder={`${provider.name} ${LANGUAGE_CARD_COPY.keyPlaceholder}`}
              aria-label={`${provider.name} ${LANGUAGE_CARD_COPY.keyPlaceholder}`}
              className="w-full bg-transparent border-b border-[rgba(var(--foreground),0.15)] focus:border-[rgb(var(--accent))] py-1 text-[12px] font-mono text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/40 focus:outline-none transition-colors"
            />
          ) : undefined
        }
      />
    );
  });

  return (
    <CatalogReveal
      className="flex-1 min-h-0"
      onReveal={() => {
        onExpand?.();
        open();
      }}
      scrollerClassName="min-h-0 flex-1 overflow-y-auto overscroll-contain custom-scrollbar pr-1 [scrollbar-gutter:stable] [contain:paint]"
    >
      <DecoupledColumnGrid items={readyItems} />

      {restRows.length > 0 && (
        <>
          <div className="my-3 h-px w-full bg-[rgba(var(--foreground),0.1)]" />
          <DecoupledColumnGrid items={restRows} />
        </>
      )}
    </CatalogReveal>
  );
});

ProviderGrid.displayName = "ProviderGrid";

/** Strips the scheme so the endpoint fits one line. */
function replaceProtocol(url: string): string {
  return url.replace(/^https?:\/\//, "");
}
