import { useCallback, useEffect, useState } from "react";

const STORAGE_KEY = "vox_favorite_models";

function loadFavorites(): Set<string> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return new Set();
    const parsed = JSON.parse(raw);
    return new Set(Array.isArray(parsed) ? parsed : []);
  } catch {
    return new Set();
  }
}

function saveFavorites(favorites: Set<string>): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(Array.from(favorites)));
  } catch {
    // localStorage unavailable / quota exceeded
  }
}

export function useFavoriteModels() {
  const [favorites, setFavorites] = useState<Set<string>>(() => loadFavorites());

  // Listen for storage events across tabs / windows
  useEffect(() => {
    const handleStorage = (e: StorageEvent) => {
      if (e.key === STORAGE_KEY) {
        setFavorites(loadFavorites());
      }
    };
    window.addEventListener("storage", handleStorage);
    return () => window.removeEventListener("storage", handleStorage);
  }, []);

  const toggleFavorite = useCallback((modelId: string) => {
    setFavorites((prev) => {
      const next = new Set(prev);
      if (next.has(modelId)) {
        next.delete(modelId);
      } else {
        next.add(modelId);
      }
      saveFavorites(next);
      return next;
    });
  }, []);

  const isFavorite = useCallback(
    (modelId: string) => favorites.has(modelId),
    [favorites]
  );

  return {
    favoriteModelIds: favorites,
    toggleFavorite,
    isFavorite,
  };
}
