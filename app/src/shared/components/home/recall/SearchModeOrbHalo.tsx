import { memo } from "react";

/**
 * Atmospheric celestial time rings and rising glow rendered behind the Orb
 * during Search / Recall Mode.
 */
export const SearchModeOrbHalo = memo(() => {
  return (
    <>
      {/* Atmospheric celestial rising glow behind the arc of cards */}
      <div className="absolute inset-[-14%] rounded-full bg-[radial-gradient(circle_at_center_bottom,rgba(var(--accent),0.25)_0%,rgba(168,85,247,0.12)_45%,transparent_70%)] blur-xl pointer-events-none animate-pulse" />
      {/* Rotating celestial chronos ring with time tick marks */}
      <div className="absolute inset-[-6%] rounded-full border border-dashed border-[rgba(var(--accent),0.28)] opacity-60 pointer-events-none animate-[spin_90s_linear_infinite]" />
      {/* Outer static framing orbit ring */}
      <div className="absolute inset-[-16%] rounded-full border border-[rgba(var(--accent),0.12)] opacity-40 pointer-events-none" />
    </>
  );
});

SearchModeOrbHalo.displayName = "SearchModeOrbHalo";
