import { memo } from "react";

/**
 * Atmospheric celestial time rings and rising glow rendered behind the Orb
 * during Search / Recall Mode.
 *
 * Focuses feathered ambient glow around the center-to-lower hemisphere where the Orb
 * meets the card horizon, leaving the top region clear. Features a soothing slow breathe pulse.
 */
export const SearchModeOrbHalo = memo(() => {
  return (
    <>
      {/* Feathered atmospheric celestial glow focused around the center and lower horizon with slow breathe */}
      <div
        className="absolute inset-[-12%] rounded-full blur-xl pointer-events-none transform-gpu animate-[vox-halo-breathe_6s_ease-in-out_infinite]"
        style={{
          background:
            "radial-gradient(ellipse 85% 55% at 50% 75%, rgba(var(--accent),0.38) 0%, rgba(var(--accent-complementary),0.22) 45%, transparent 72%)",
          transform: "translateZ(0)",
        }}
      />
      {/* Static celestial chronos framing rings */}
      <div className="absolute inset-[-4%] rounded-full border border-dashed border-[rgba(var(--accent),0.24)] opacity-40 pointer-events-none" />
      {/* Outer static framing orbit ring */}
      <div className="absolute inset-[-14%] rounded-full border border-[rgba(var(--accent),0.10)] opacity-30 pointer-events-none" />
    </>
  );
});

SearchModeOrbHalo.displayName = "SearchModeOrbHalo";
