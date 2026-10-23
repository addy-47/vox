import { memo, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { Check, Plus, X } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
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
              <>
                <button
                  type="button"
                  aria-label={LANGUAGE_CARD_COPY.saveKey}
                  onClick={(e) => {
                    e.stopPropagation();
                    void saveKey();
                  }}
                  className="rounded-md p-1 text-[rgb(var(--accent))] transition-colors hover:bg-[rgba(var(--accent),0.14)] cursor-pointer"
                >
                  <Check size={13} strokeWidth={2.5} />
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
              </>
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
          body={
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
                className="min-w-0 flex-1 bg-transparent text-[12px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/60 focus:outline-none"
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
      ...servers.map((server) => ({
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
          />
        ),
      })),
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
                  <>
                    <button
                      type="button"
                      aria-label={LANGUAGE_CARD_COPY.saveKey}
                      onClick={(e) => {
                        e.stopPropagation();
                        void saveKey();
                      }}
                      className="rounded-md p-1 text-[rgb(var(--accent))] transition-colors hover:bg-[rgba(var(--accent),0.14)] cursor-pointer"
                    >
                      <Check size={13} strokeWidth={2.5} />
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
                  </>
                ) : undefined
              }
              body={
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
                    className="min-w-0 flex-1 bg-transparent text-[12px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/60 focus:outline-none"
                  />
                ) : undefined
              }
            />
          ),
        };
      }),
    ];

    return [...rows].sort(
      (a, b) => Number(b.key === savedKey) - Number(a.key === savedKey)
    );
  })();

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
            <>
              <button
                type="button"
                aria-label={LANGUAGE_CARD_COPY.saveKey}
                onClick={(e) => {
                  e.stopPropagation();
                  void saveKey();
                }}
                className="rounded-md p-1 text-[rgb(var(--accent))] transition-colors hover:bg-[rgba(var(--accent),0.14)] cursor-pointer"
              >
                <Check size={13} strokeWidth={2.5} />
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
            </>
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
        body={
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
              className="min-w-0 flex-1 bg-transparent text-[12px] text-[rgb(var(--foreground))] placeholder:text-[rgb(var(--foreground-muted))]/60 focus:outline-none"
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
      <div className="grid grid-cols-2 gap-2 auto-rows-max content-start">
        {readyRows.map((row) => (
          <div key={row.key} className="contents">
            {row.node}
          </div>
        ))}
        <AddCard
          title={LANGUAGE_CARD_COPY.addProvider}
          onClick={() => {
            onExpand?.();
            open();
          }}
        />
      </div>

      {restRows.length > 0 && (
        <>
          <div className="my-3 h-px w-full bg-[rgba(var(--foreground),0.1)]" />
          <div className="grid grid-cols-2 gap-2 auto-rows-max content-start">
            {restRows}
          </div>
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
