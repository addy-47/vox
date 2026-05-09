import React, { createContext, useContext, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Theme = "dark" | "light";

interface ThemeContextType {
  theme: Theme;
  setTheme: (theme: Theme) => void;
  toggleTheme: () => void;
  isLoading: boolean;
}

const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

export const ThemeProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [theme, setThemeState] = useState<Theme>("dark");
  const [isLoading, setIsLoading] = useState(true);

  // Initialize theme from backend
  useEffect(() => {
    const initTheme = async () => {
      try {
        const settings: any = await invoke("get_settings");
        if (settings?.theme) {
          const loadedTheme = settings.theme as Theme;
          setThemeState(loadedTheme);
          document.documentElement.setAttribute("data-theme", loadedTheme);
        }
      } catch (error) {
        console.error("Failed to initialize theme:", error);
      } finally {
        setIsLoading(false);
      }
    };
    initTheme();
  }, []);

  const setTheme = async (newTheme: Theme) => {
    setThemeState(newTheme);
    document.documentElement.setAttribute("data-theme", newTheme);
    try {
      await invoke("update_theme", { theme: newTheme });
    } catch (error) {
      console.error("Failed to update theme in backend:", error);
    }
  };

  const toggleTheme = () => {
    setTheme(theme === "dark" ? "light" : "dark");
  };

  return (
    <ThemeContext.Provider value={{ theme, setTheme, toggleTheme, isLoading }}>
      {children}
    </ThemeContext.Provider>
  );
};

export const useTheme = () => {
  const context = useContext(ThemeContext);
  if (context === undefined) {
    throw new Error("useTheme must be used within a ThemeProvider");
  }
  return context;
};

