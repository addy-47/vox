export type SettingsLayoutMode = "full-max" | "full-min" | "small";

export const SETTINGS_DOMAIN_CARD_HEIGHT_CLASS = "lg:h-[340px]";

const SMALL_SETTINGS_LIST_MAX_HEIGHT_CLASS = "max-h-[260px]";

export const getSettingsInlineListMaxHeightClass = (
  layoutMode?: SettingsLayoutMode
): string =>
  layoutMode === "small"
    ? SMALL_SETTINGS_LIST_MAX_HEIGHT_CLASS
    : "max-h-[480px]";
