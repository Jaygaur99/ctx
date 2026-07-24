import { useCallback, useEffect, useState } from "react";
import type { ThemePreference } from "./types";

const SIMPLE_MODE_KEY = "ctx.simple-mode";
const THEME_KEY = "ctx.theme";

function initialSimpleMode() {
  try {
    return window.localStorage.getItem(SIMPLE_MODE_KEY) !== "detailed";
  } catch {
    return true;
  }
}

function initialTheme(): ThemePreference {
  try {
    const stored = window.localStorage.getItem(THEME_KEY);
    return stored === "light" || stored === "dark" ? stored : "system";
  } catch {
    return "system";
  }
}

export function useAppearancePreferences() {
  const [simpleMode, setSimpleModeState] = useState(initialSimpleMode);
  const [theme, setThemeState] = useState<ThemePreference>(initialTheme);

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.dataset.theme = theme;

    return () => {
      if (theme === "system" || root.dataset.theme === theme) {
        root.removeAttribute("data-theme");
      }
    };
  }, [theme]);

  const setSimpleMode = useCallback((enabled: boolean) => {
    setSimpleModeState(enabled);
    try {
      window.localStorage.setItem(SIMPLE_MODE_KEY, enabled ? "simple" : "detailed");
    } catch {
      // The view still changes for this session if storage is unavailable.
    }
  }, []);

  const setTheme = useCallback((preference: ThemePreference) => {
    setThemeState(preference);
    try {
      if (preference === "system") window.localStorage.removeItem(THEME_KEY);
      else window.localStorage.setItem(THEME_KEY, preference);
    } catch {
      // The theme still changes for this session if storage is unavailable.
    }
  }, []);

  return {
    simpleMode,
    theme,
    setSimpleMode,
    setTheme,
  };
}
