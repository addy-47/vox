import { useSettingsStore } from "@/store/settingsStore";
import type { HelpTier } from "@/data/helpCopy";

/**
 * Derive a coarse hardware tier from the current settings. The real
 * backend may compute a more accurate tier; until that is exposed via
 * IPC we approximate from `pipeline_mode` and provider selections so
 * the help drawer can still surface tier-aware tips.
 *
 * - Tier 3: realtime pipeline
 * - Tier 2B: cloud LLM
 * - Tier 2A: server LLM
 * - Tier 1B: embedded LLM with at least one embedded model ready
 * - Tier 1A: anything else
 */
export function deriveTier(): HelpTier {
  const settings = useSettingsStore.getState().settings;
  if (!settings) return "1A";

  if (settings.interaction.pipeline_mode === "realtime") return "3";
  if (settings.llm.active === "cloud") return "2B";
  if (settings.llm.active === "server") return "2A";
  if (settings.llm.active === "embedded") return "1B";
  return "1A";
}
