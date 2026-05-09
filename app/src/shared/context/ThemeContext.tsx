import React, { createContext, useContext, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

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

  // 1. Initialize theme from backend settings
  useEffect(() => {
    const initTheme = async () => {
      try {
        const settings: any = await invoke("get_settings");
        // Structure is usually { appearance: { theme: 'dark' } } or { theme: 'dark' }
        const loadedTheme = (settings?.appearance?.theme || settings?.theme || "dark") as Theme;
        setThemeState(loadedTheme);
        document.documentElement.setAttribute("data-theme", loadedTheme);
      } catch (error) {
        console.error("[ThemeContext] Failed to initialize theme:", error);
      } finally {
        setIsLoading(false);
      }
    };
    initTheme();
  }, []);

  // 2. Listen for theme changes from other windows via backend broadcast
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    
    const setupListener = async () => {
      try {
        const appWindow = getCurrentWindow();
        unlisten = await appWindow.listen<string>("theme-changed", (event) => {
          const newTheme = event.payload as Theme;
          setThemeState(newTheme);
          document.documentElement.setAttribute("data-theme", newTheme);
        });
      } catch (error) {
        console.error("[ThemeContext] Failed to setup theme listener:", error);
      }
    };

    setupListener();
    return () => { if (unlisten) unlisten(); };
  }, []);

  const setTheme = async (newTheme: Theme) => {
    setThemeState(newTheme);
    document.documentElement.setAttribute("data-theme", newTheme);
    try {
      // This command should update settings.json and emit "theme-changed" globally
      await invoke("update_theme", { theme: newTheme });
    } catch (error) {
      console.error("[ThemeContext] Failed to update theme:", error);
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


