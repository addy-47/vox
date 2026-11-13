import { memo } from "react";

/**
 * Atmospheric celestial horizon corona and ambient body glow rendered behind the Orb
 * during Search / Recall Mode.
 *
 * Implements:
 * 1. Primary horizon rim focused at 70-80% height of the Orb, illuminating the card crest.
 * 2. Subtle secondary accent glow rising from bottom through the middle to ~40% height,
 *    fading out smoothly towards the top crown of the sphere.
 * 3. Downward alpha falloff preserving clean void-black contrast for bottom scrubber & dock.
 * 4. Soothing 7-second slow breathing pulse.
 */
export const SearchModeOrbHalo = memo(() => {
  return (
    <>
      {/* ── Secondary Celestial Atmosphere Glow (Rising from bottom through middle to ~38%, fading out to top) ── */}
      <div
        className="absolute inset-x-[-4%] top-[38%] h-[58%] pointer-events-none transform-gpu animate-[vox-halo-breathe_7s_ease-in-out_infinite]"
        style={{
          background:
            "radial-gradient(ellipse 70% 60% at 50% 50%, rgba(var(--accent-complementary),0.11) 0%, rgba(var(--accent),0.06) 55%, transparent 80%)",
          filter: "blur(28px)",
          WebkitMaskImage:
            "linear-gradient(to bottom, transparent 0%, rgba(0,0,0,0.4) 15%, black 40%, black 75%, transparent 100%)",
          maskImage:
            "linear-gradient(to bottom, transparent 0%, rgba(0,0,0,0.4) 15%, black 40%, black 75%, transparent 100%)",
        }}
      />

      {/* ── Primary Celestial Horizon Corona (Rim lighting focused at 70-80% of Orb height) ── */}
      <div
        className="absolute left-[-6%] right-[-6%] top-[68%] h-[30%] pointer-events-none transform-gpu animate-[vox-halo-breathe_7s_ease-in-out_infinite]"
        style={{
          background:
            "radial-gradient(ellipse 75% 55% at 50% 18%, rgba(var(--accent),0.32) 0%, rgba(var(--accent-complementary),0.12) 50%, transparent 78%)",
          filter: "blur(22px)",
          WebkitMaskImage:
            "linear-gradient(to bottom, transparent 0%, black 25%, black 65%, transparent 100%)",
          maskImage:
            "linear-gradient(to bottom, transparent 0%, black 25%, black 65%, transparent 100%)",
        }}
      />

      {/* ── Subtle Core Horizon Arc (Tightly hugging the lower contour of the sphere) ── */}
      <div
        className="absolute left-[4%] right-[4%] top-[72%] h-[16%] pointer-events-none transform-gpu"
        style={{
          background:
            "radial-gradient(ellipse 65% 45% at 50% 25%, rgba(var(--accent),0.20) 0%, transparent 75%)",
          filter: "blur(12px)",
        }}
      />

      {/* ── Static celestial chronos framing ring ── */}
      <div className="absolute inset-[-4%] rounded-full border border-dashed border-[rgba(var(--accent),0.20)] opacity-35 pointer-events-none" />

      {/* ── Outer static framing orbit ring ── */}
      <div className="absolute inset-[-14%] rounded-full border border-[rgba(var(--accent),0.08)] opacity-25 pointer-events-none" />
    </>
  );
});

SearchModeOrbHalo.displayName = "SearchModeOrbHalo";
