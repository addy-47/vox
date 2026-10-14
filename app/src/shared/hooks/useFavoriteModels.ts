import { useCallback, useEffect, useState } from "react";
import { listFavoriteModels, toggleModelFavorite } from "@/services/settingsService";

export function useFavoriteModels() {
  const [favorites, setFavorites] = useState<Set<string>>(new Set());

  // Load persistent favorites on mount
  useEffect(() => {
    let alive = true;
    listFavoriteModels()
      .then((ids) => {
        if (alive) {
          setFavorites(new Set(ids));
        }
      })
      .catch((err) => {
        console.error("[useFavoriteModels] Failed to load favorite models from DB:", err);
      });
    return () => {
      alive = false;
    };
  }, []);

  const toggleFavorite = useCallback((modelId: string) => {
    const trimmed = modelId.trim();
    if (!trimmed) return;

    // Optimistic UI toggle
    setFavorites((prev) => {
      const next = new Set(prev);
      if (next.has(trimmed)) {
        next.delete(trimmed);
      } else {
        next.add(trimmed);
      }
      return next;
    });

    // Persistent Turso DB update
    toggleModelFavorite(trimmed)
      .then((isFav) => {
        setFavorites((prev) => {
          const next = new Set(prev);
          if (isFav) {
            next.add(trimmed);
          } else {
            next.delete(trimmed);
          }
          return next;
        });
      })
      .catch((err) => {
        console.error("[useFavoriteModels] Failed to toggle model favorite:", err);
        // Rollback on failure
        setFavorites((prev) => {
          const next = new Set(prev);
          if (next.has(trimmed)) {
            next.delete(trimmed);
          } else {
            next.add(trimmed);
          }
          return next;
        });
      });
  }, []);

  const isFavorite = useCallback(
    (modelId: string) => favorites.has(modelId.trim()),
    [favorites]
  );

  return {
    favoriteModelIds: favorites,
    toggleFavorite,
    isFavorite,
  };
}
