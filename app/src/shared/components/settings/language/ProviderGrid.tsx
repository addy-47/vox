import { memo, useMemo, useState } from "react";
import { useShallow } from "zustand/react/shallow";
import { Check, Plus, X } from "lucide-react";
import { useSettingsStore } from "@/store/settingsStore";
import { useCatalogStore } from "@/store/catalogStore";
import { activeSelection, getEffectiveCloudApiKey, getEffectiveLlmServers } from "@/shared/lib/activeSelection";
import { useExpandableList } from "@/shared/ui/ExpandableList";
import { VoxLogoLoader } from "@/shared/components/common/VoxLogoLoader";
import { LANGUAGE_CARD_COPY } from "@/data/settingsCopy";
import { AddCard } from "../shared/AddCard";
import { CatalogReveal } from "../shared/CatalogReveal";
import { ProviderPill } from "./ProviderPill";

export interface ProviderGridProps {
  /** Swaps the card face to the active provider's model catalog. */
  onOpenModels: () => void;
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
export const ProviderGrid = memo(({ onOpenModels }: ProviderGridProps) => {
  const draftSettings = useSettingsStore(useShallow((s) => s.draftSettings));
  const committedSettings = useSettingsStore(useShallow((s) => s.settings));
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);
  const updateDraft = useSettingsStore((s) => s.updateDraft);
  const cloudProviders = useCatalogStore((s) => s.cloudProviders);
  const { open } = useExpandableList();

  const [editingKeyId, setEditingKeyId] = useState<string | null>(null);
  const [keyValue, setKeyValue] = useState("");
  const [savingKey, setSavingKey] = useState(false);

  const active = draftSettings?.llm?.active;
  const activeCategory = active?.category ?? "embedded";
  const localModelId =
    draftSettings?.llm?.last_selected?.embedded ??
    (activeCategory === "embedded" ? active?.model : undefined) ??
    "";

  const localModelName = useMemo(() => {
    const match = modelCatalog?.llm?.find((m) => m.id === localModelId);
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
    const saved = committedSettings?.llm?.active;
    if (!saved) return "local";
    if (saved.category === "cloud") return `cloud:${saved.provider ?? ""}`;
    if (saved.category === "server") return `server:${saved.provider ?? ""}`;
    return "local";
  }, [committedSettings?.llm?.active]);

  const selectLocal = () => {
    useSettingsStore.getState().clearRemoteModelsError();
    updateDraft("llm", "active", activeSelection("embedded", localModelId));
    window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "llm" }));
    onOpenModels();
  };

  const selectCloud = (providerId: string) => {
    useSettingsStore.getState().clearRemoteModelsError();
    // Same contract as the provider modal: keep the current model id until the
    // catalog selection replaces it.
    updateDraft("llm", "active", activeSelection("cloud", active?.model ?? "", providerId));
    window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "llm" }));
    onOpenModels();
  };

  const selectServer = (serverId: string, model: string) => {
    useSettingsStore.getState().clearRemoteModelsError();
    updateDraft("llm", "active", activeSelection("server", model, serverId));
    window.dispatchEvent(new CustomEvent("sync_pipeline_tab", { detail: "llm" }));
    onOpenModels();
  };

  const saveKey = async () => {
    const providerId = editingKeyId;
    const trimmed = keyValue.trim();
    if (!providerId || !trimmed || savingKey) return;

    setSavingKey(true);
    try {
      const keys = {
        ...(draftSettings?.llm?.cloud_keys ?? {}),
        [providerId]: trimmed,
      };
      updateDraft("llm", "cloud_keys", keys);
      await useSettingsStore.getState().commitChanges({ domain: "llm", key: "cloud_keys" });
      setEditingKeyId(null);
      setKeyValue("");
    } finally {
      setSavingKey(false);
    }
  };

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
            meta={localModelName ? `Local · ${localModelName}` : LANGUAGE_CARD_COPY.localPrivate}
            active={activeCategory === "embedded"}
            onSelect={selectLocal}
          />
        ),
      },
      ...servers.map((server) => ({
        key: `server:${server.id}`,
        node: (
          <ProviderPill
            id={server.id}
            name={server.name || "Custom Server"}
            providerName={server.name}
            meta={
              activeCategory === "server" && active?.provider === server.id && active?.model
                ? `Self-hosted · ${active.model}`
                : `Self-hosted · ${server.model || replaceProtocol(server.url)}`
            }
            active={activeCategory === "server" && active?.provider === server.id}
            onSelect={() => selectServer(server.id, server.model || "")}
          />
        ),
      })),
      ...readyClouds.map((provider) => ({
        key: `cloud:${provider.id}`,
        node: (
          <ProviderPill
            id={provider.id}
            name={provider.name}
            logo={provider.logo ?? null}
            meta={
              activeCategory === "cloud" && active?.provider === provider.id && active?.model
                ? `Cloud · ${active.model}`
                : LANGUAGE_CARD_COPY.keySaved
            }
            active={activeCategory === "cloud" && active?.provider === provider.id}
            onSelect={() => selectCloud(provider.id)}
          />
        ),
      })),
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
              placeholder={`${provider.name} API key`}
              aria-label={`${provider.name} API key`}
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
      scrollerClassName="min-h-0 flex-1 overflow-y-auto overscroll-contain custom-scrollbar pr-1 [scrollbar-gutter:stable] [contain:paint]"
    >
      <div className="grid grid-cols-2 gap-2 auto-rows-max content-start">
        {readyRows.map((row) => (
          <div key={row.key} className="contents">
            {row.node}
          </div>
        ))}
        <AddCard title={LANGUAGE_CARD_COPY.addProvider} onClick={open} />
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
