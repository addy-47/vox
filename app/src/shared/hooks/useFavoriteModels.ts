import { useCallback, useEffect, useState } from "react";
import { listFavoriteModels, toggleModelFavorite } from "@/services/catalogService";

let cachedFavorites: Set<string> | null = null;
const listeners = new Set<(next: Set<string>) => void>();

function notifyFavorites(next: Set<string>) {
  cachedFavorites = next;
  listeners.forEach((fn) => fn(next));
}

export function useFavoriteModels() {
  const [favorites, setFavorites] = useState<Set<string>>(
    () => cachedFavorites ?? new Set()
  );

  useEffect(() => {
    listeners.add(setFavorites);
    if (cachedFavorites === null) {
      listFavoriteModels()
        .then((ids) => {
          const next = new Set(ids);
          if (
            cachedFavorites &&
            cachedFavorites.size === next.size &&
            [...next].every((id) => cachedFavorites!.has(id))
          ) {
            return;
          }
          notifyFavorites(next);
        })
        .catch((err) => {
          console.error("[useFavoriteModels] Failed to load favorite models from DB:", err);
        });
    }
    return () => {
      listeners.delete(setFavorites);
    };
  }, []);

  const toggleFavorite = useCallback(async (modelId: string) => {
    const wasFav = favorites.has(modelId);
    // optimistic local update
    setFavorites((prev) => {
      const next = new Set(prev);
      if (wasFav) next.delete(modelId);
      else next.add(modelId);
      return next;
    });
    try {
      const nowFav = await toggleModelFavorite(modelId);
      setFavorites((prev) => {
        const next = new Set(prev);
        if (nowFav) next.add(modelId);
        else next.delete(modelId);
        notifyFavorites(next);
        return next;
      });
    } catch (err) {
      console.error("[useFavoriteModels] Failed to toggle favorite:", err);
      // revert
      setFavorites((prev) => {
        const next = new Set(prev);
        if (wasFav) next.add(modelId);
        else next.delete(modelId);
        return next;
      });
    }
  }, [favorites]);

  const isFavorite = useCallback((modelId: string) => favorites.has(modelId), [favorites]);

  return { favoriteModelIds: favorites, toggleFavorite, isFavorite };
}