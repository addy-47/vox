export interface ThemeVariantTokens {
  name: string;
  bg: string;
  card: string;
  field: string;
  sidebar: string;
  fg: string;
  fgMuted: string;
  accent: string;
  accentComp: string;
  border: string;
}

export interface ThemePresetDefinition {
  id: string;
  label: string;
  subtitle: string;
  dark: ThemeVariantTokens;
  light: ThemeVariantTokens;
}

export interface ThemeAppearance {
  theme: string;
  preset?: string;
  dark_accent?: string;
  light_accent?: string;
  dark_bg?: string;
  light_bg?: string;
}

export const PRESET_THEMES: readonly ThemePresetDefinition[] = [
  {
    id: "vox",
    label: "Vox Default",
    subtitle: "Graphite & Linen",
    dark: {
      name: "Cyber",
      bg: "#131315",
      card: "#1C1D22",
      field: "#16161A",
      sidebar: "#16161A",
      fg: "#ECEAE6",
      fgMuted: "#9C9994",
      accent: "#00DBE9",
      accentComp: "#A855F7",
      border: "255, 255, 255",
    },
    light: {
      name: "Linen",
      bg: "#F7F6F2",
      card: "#FFFFFF",
      field: "#EEEAE6",
      sidebar: "#EEEAE6",
      fg: "#1C1A18",
      fgMuted: "#66625C",
      accent: "#0E7490",
      accentComp: "#7C3AED",
      border: "0, 0, 0",
    },
  },
  {
    id: "tokyo_night",
    label: "Tokyo Night",
    subtitle: "Moon & Day",
    dark: {
      name: "Moon",
      bg: "#16161E",
      card: "#1F2335",
      field: "#1A1B26",
      sidebar: "#1A1B26",
      fg: "#C0CAF5",
      fgMuted: "#7982A9",
      accent: "#7AA2F7",
      accentComp: "#BB9AF7",
      border: "255, 255, 255",
    },
    light: {
      name: "Day",
      bg: "#F2F3F7",
      card: "#FFFFFF",
      field: "#E6E8EF",
      sidebar: "#E6E8EF",
      fg: "#1E202E",
      fgMuted: "#6C6E75",
      accent: "#2E7DE9",
      accentComp: "#7C3AED",
      border: "0, 0, 0",
    },
  },
  {
    id: "catppuccin",
    label: "Catppuccin",
    subtitle: "Mocha & Latte",
    dark: {
      name: "Mocha",
      bg: "#1E1E2E",
      card: "#25263A",
      field: "#181825",
      sidebar: "#181825",
      fg: "#CDD6F4",
      fgMuted: "#A6ADC8",
      accent: "#CBA6F7",
      accentComp: "#F5C2E7",
      border: "255, 255, 255",
    },
    light: {
      name: "Latte",
      bg: "#EFF1F5",
      card: "#FFFFFF",
      field: "#E6E9EF",
      sidebar: "#E6E9EF",
      fg: "#4C4F69",
      fgMuted: "#6C6F85",
      accent: "#8839EF",
      accentComp: "#EA76CB",
      border: "0, 0, 0",
    },
  },
  {
    id: "one_dark",
    label: "One Dark",
    subtitle: "Pro & Light",
    dark: {
      name: "Pro",
      bg: "#1E2227",
      card: "#282C34",
      field: "#181A1F",
      sidebar: "#181A1F",
      fg: "#ECEFF4",
      fgMuted: "#7F848E",
      accent: "#61AFEF",
      accentComp: "#C678DD",
      border: "255, 255, 255",
    },
    light: {
      name: "One Light",
      bg: "#F9F9FB",
      card: "#FFFFFF",
      field: "#EEF0F4",
      sidebar: "#EEF0F4",
      fg: "#282C34",
      fgMuted: "#6A737D",
      accent: "#4078F2",
      accentComp: "#A626A4",
      border: "0, 0, 0",
    },
  },
  {
    id: "dracula",
    label: "Dracula",
    subtitle: "Vampire & Alucard",
    dark: {
      name: "Vampire",
      bg: "#1E1F29",
      card: "#282A36",
      field: "#181920",
      sidebar: "#181920",
      fg: "#F8F8F2",
      fgMuted: "#959DCB",
      accent: "#BD93F9",
      accentComp: "#FF79C6",
      border: "255, 255, 255",
    },
    light: {
      name: "Dracula Alucard",
      bg: "#F8F8F2",
      card: "#FFFFFF",
      field: "#EBEBE5",
      sidebar: "#EBEBE5",
      fg: "#282A36",
      fgMuted: "#6272A4",
      accent: "#7B4BC7",
      accentComp: "#FF5555",
      border: "0, 0, 0",
    },
  },
  {
    id: "nord",
    label: "Nord",
    subtitle: "Polar & Snow",
    dark: {
      name: "Polar Night",
      bg: "#242933",
      card: "#2E3440",
      field: "#1E222A",
      sidebar: "#1E222A",
      fg: "#ECEFF4",
      fgMuted: "#8892B0",
      accent: "#88C0D0",
      accentComp: "#81A1C1",
      border: "255, 255, 255",
    },
    light: {
      name: "Snow Storm",
      bg: "#ECEFF4",
      card: "#FFFFFF",
      field: "#E5E9F0",
      sidebar: "#E5E9F0",
      fg: "#2E3440",
      fgMuted: "#616E85",
      accent: "#5E81AC",
      accentComp: "#88C0D0",
      border: "0, 0, 0",
    },
  },
  {
    id: "night_owl",
    label: "Night Owl",
    subtitle: "Night & Light",
    dark: {
      name: "Deep Night",
      bg: "#011627",
      card: "#0B2942",
      field: "#01111D",
      sidebar: "#01111D",
      fg: "#FFFFFF",
      fgMuted: "#7E97AC",
      accent: "#82AAFF",
      accentComp: "#7FDBCA",
      border: "255, 255, 255",
    },
    light: {
      name: "Light Owl",
      bg: "#F4F5F8",
      card: "#FFFFFF",
      field: "#E6E8EF",
      sidebar: "#E6E8EF",
      fg: "#1B2233",
      fgMuted: "#6A7B95",
      accent: "#0C83E2",
      accentComp: "#05A081",
      border: "0, 0, 0",
    },
  },
  {
    id: "rose_pine",
    label: "Rosé Pine",
    subtitle: "Moon & Dawn",
    dark: {
      name: "Moon",
      bg: "#191724",
      card: "#211F30",
      field: "#15131E",
      sidebar: "#15131E",
      fg: "#E0DEF4",
      fgMuted: "#908CAA",
      accent: "#EBBCBA",
      accentComp: "#C4A7E7",
      border: "255, 255, 255",
    },
    light: {
      name: "Dawn",
      bg: "#FAF4ED",
      card: "#FFFFFF",
      field: "#F2E9E1",
      sidebar: "#F2E9E1",
      fg: "#575279",
      fgMuted: "#9893A5",
      accent: "#D7827E",
      accentComp: "#907AA9",
      border: "0, 0, 0",
    },
  },
  {
    id: "solarized",
    label: "Solarized",
    subtitle: "Dark & Light",
    dark: {
      name: "Solar Dark",
      bg: "#002B36",
      card: "#073642",
      field: "#00212B",
      sidebar: "#00212B",
      fg: "#93A1A1",
      fgMuted: "#657B83",
      accent: "#268BD2",
      accentComp: "#2AA198",
      border: "255, 255, 255",
    },
    light: {
      name: "Solar Light",
      bg: "#FDF6E3",
      card: "#FFFFFF",
      field: "#EEE8D5",
      sidebar: "#EEE8D5",
      fg: "#586E75",
      fgMuted: "#839496",
      accent: "#2AA198",
      accentComp: "#268BD2",
      border: "0, 0, 0",
    },
  },
] as const;

export const DEFAULT_DARK_ACCENT = "#00DBE9";
export const DEFAULT_LIGHT_ACCENT = "#0E7490";
export const DEFAULT_DARK_BG = "#131315";
export const DEFAULT_LIGHT_BG = "#F7F6F2";

export const ACCENT_SWATCHES = [
  "#00DBE9", // Cyan (Vox)
  "#7AA2F7", // Tokyo Blue
  "#CBA6F7", // Catppuccin Mauve
  "#BD93F9", // Dracula Purple
  "#61AFEF", // One Dark Sky
  "#10B981", // Emerald
  "#F59E0B", // Amber
  "#EC4899", // Pink
  "#EBBCBA", // Rosé
  "#88C0D0", // Nord Frost
] as const;

export interface CuratedAccentPreset {
  id: string;
  name: string;
  darkHex: string;
  lightHex: string;
}

export const CURATED_ACCENT_PRESETS: readonly CuratedAccentPreset[] = [
  {
    id: "astral_mauve",
    name: "Astral Mauve",
    darkHex: "#CBA6F7",
    lightHex: "#3B1D62",
  },
  {
    id: "sakura_bloom",
    name: "Sakura Bloom",
    darkHex: "#F7A6D2",
    lightHex: "#611D47",
  },
  {
    id: "neo_mint",
    name: "Neo Mint",
    darkHex: "#A6F7D3",
    lightHex: "#22573C",
  },
  {
    id: "solar_amber",
    name: "Solar Amber",
    darkHex: "#F9BA5F",
    lightHex: "#5A2E05",
  },
] as const;

export const DARK_BG_PRESETS: readonly { hex: string; label: string; fg: string }[] = [
  { hex: "#000000", label: "OLED Black", fg: "#ECEAE6" },
  { hex: "#131315", label: "Graphite",   fg: "#ECEAE6" },
  { hex: "#16161E", label: "Midnight",   fg: "#C0CAF5" },
  { hex: "#181514", label: "Charcoal",   fg: "#ECEAE6" },
] as const;

export const LIGHT_BG_PRESETS: readonly { hex: string; label: string; fg: string }[] = [
  { hex: "#FFFFFF", label: "Pure White", fg: "#1C1A18" },
  { hex: "#F7F6F2", label: "Warm Linen", fg: "#1C1A18" },
  { hex: "#F1F5F9", label: "Cool Cloud", fg: "#343B58" },
  { hex: "#F4EFE6", label: "Warm Sand",  fg: "#1C1A18" },
] as const;

export function getActiveAccent(appearance: ThemeAppearance): string {
  const isLight = appearance.theme === "light";
  if (isLight) {
    return appearance.light_accent || DEFAULT_LIGHT_ACCENT;
  }
  return appearance.dark_accent || DEFAULT_DARK_ACCENT;
}

export function getActiveBg(appearance: ThemeAppearance): string {
  const isLight = appearance.theme === "light";
  if (isLight) {
    return appearance.light_bg || DEFAULT_LIGHT_BG;
  }
  return appearance.dark_bg || DEFAULT_DARK_BG;
}
