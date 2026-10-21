import type { SettingsDomainId } from "@/data/settingsCopy";

export type SettingsLayoutMode = "full-max" | "full-min" | "small";

export const SETTINGS_DOMAIN_CARD_HEIGHT_CLASS = "lg:h-[340px]";

const SMALL_SETTINGS_LIST_MAX_HEIGHT_CLASS = "max-h-[260px]";

export const getSettingsInlineListMaxHeightClass = (
  layoutMode?: SettingsLayoutMode
): string =>
  layoutMode === "small"
    ? SMALL_SETTINGS_LIST_MAX_HEIGHT_CLASS
    : "max-h-[480px]";

/**
 * Card width by domain and layout mode. Single home for the width grammar:
 * the wrapper applies it to the card shell and the skeleton mirrors it, so a
 * loaded card and its placeholder can never disagree.
 */
export const getDomainCardWidth = (
  domainId: string | undefined,
  layoutMode: SettingsLayoutMode
): string => {
  if (layoutMode === "small") return "w-full";
  if (domainId === "persona") {
    return layoutMode === "full-min"
      ? "lg:w-[350px] xl:w-[370px] 2xl:w-[440px]"
      : "lg:w-[470px]";
  }
  if (domainId === "appearance") {
    return layoutMode === "full-min"
      ? "lg:w-[240px] xl:w-[260px] 2xl:w-[280px]"
      : "lg:w-[290px] xl:w-[310px]";
  }
  return layoutMode === "full-min" ? "lg:w-[360px] xl:w-[420px] 2xl:w-[520px]" : "lg:w-[520px]";
};

/** Hub connector geometry for a domain's node-to-card line. */
export type SettingsConnectorKind = "vertical" | "fork-left" | "fork-right";

export interface SettingsDomainSlot {
  /** Hub connector geometry. */
  connector: SettingsConnectorKind;
  /**
   * Carousel pairing in constrained viewports. `rank: "primary"` sits on top;
   * `"secondary"` is reached through the bottom chevron.
   */
  carousel?: { side: "left" | "right"; rank: "primary" | "secondary" };
  /** Order in the compact single-column list. Lower sorts first. */
  mobileOrder: number;
}

/**
 * SSOT for the hub's slot geometry: which connector a domain draws, which two
 * domains share a carousel card, and the compact-list order. `Settings.tsx`,
 * the side carousel, and the connectors overlay all read this map — adding or
 * renaming a domain touches here first.
 */
export const SETTINGS_DOMAIN_SLOTS: Record<SettingsDomainId, SettingsDomainSlot> = {
  interaction: {
    connector: "fork-right",
    carousel: { side: "left", rank: "primary" },
    mobileOrder: 0,
  },
  memory: {
    connector: "fork-left",
    carousel: { side: "right", rank: "primary" },
    mobileOrder: 1,
  },
  language: {
    connector: "fork-left",
    carousel: { side: "right", rank: "secondary" },
    mobileOrder: 2,
  },
  speech: {
    connector: "fork-right",
    carousel: { side: "left", rank: "secondary" },
    mobileOrder: 3,
  },
  persona: { connector: "vertical", mobileOrder: 4 },
  appearance: { connector: "vertical", mobileOrder: 5 },
};

/** The two domains sharing a carousel card, `[primary, secondary]`. */
export const getCarouselPair = (
  side: "left" | "right"
): [SettingsDomainId, SettingsDomainId] => {
  const forSide = (Object.entries(SETTINGS_DOMAIN_SLOTS) as [
    SettingsDomainId,
    SettingsDomainSlot,
  ][]).filter(([, slot]) => slot.carousel?.side === side);
  const primary = forSide.find(([, slot]) => slot.carousel?.rank === "primary");
  const secondary = forSide.find(([, slot]) => slot.carousel?.rank === "secondary");
  if (!primary || !secondary) {
    throw new Error(`Settings carousel side "${side}" needs a primary and a secondary domain`);
  }
  return [primary[0], secondary[0]];
};

