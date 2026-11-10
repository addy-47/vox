import { memo } from "react";

/**
 * Atmospheric celestial time rings and rising glow rendered behind the Orb
 * during Search / Recall Mode.
 *
 * Performance notes:
 * - Keeps the soft feathered `blur-xl` aesthetic with `--accent` and `--accent-complementary`.
 * - Promoted to an isolated GPU compositing layer (`transform-gpu`) without infinite opacity
 *   or spin keyframes, eliminating GPU re-rasterization during coverflow scroll gestures.
 */
export const SearchModeOrbHalo = memo(() => {
  return (
    <>
      {/* Soft feathered atmospheric glow behind the cards and orb — subtle, static GPU layer */}
      <div
        className="absolute inset-[-14%] rounded-full blur-xl pointer-events-none transform-gpu"
        style={{
          background:
            "radial-gradient(circle at center bottom, rgba(var(--accent),0.25) 0%, rgba(var(--accent-complementary),0.14) 45%, transparent 70%)",
          opacity: 0.9,
          transform: "translateZ(0)",
        }}
      />
      {/* Static celestial chronos framing rings */}
      <div className="absolute inset-[-6%] rounded-full border border-dashed border-[rgba(var(--accent),0.25)] opacity-50 pointer-events-none" />
      {/* Outer static framing orbit ring */}
      <div className="absolute inset-[-16%] rounded-full border border-[rgba(var(--accent),0.12)] opacity-40 pointer-events-none" />
    </>
  );
});

SearchModeOrbHalo.displayName = "SearchModeOrbHalo";
