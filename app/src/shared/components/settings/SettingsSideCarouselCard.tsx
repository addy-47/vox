import { memo, useState, useCallback, useMemo, useEffect } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import { AnimatePresence, motion } from "framer-motion";
import { SETTINGS_DOMAINS as DOMAINS, type SettingsDomainId as DomainId } from "@/data/settingsCopy";
import { SettingsCardWrapper } from "./SettingsCardWrapper";

interface SettingsSideCarouselCardProps {
  side: "left" | "right";
  activeDomains: DomainId[];
  layoutMode: "full-max" | "full-min" | "small";
  onSwitchDomain: (id: DomainId) => void;
  renderDomainContent: (domain: DomainId, layoutMode?: "full-max" | "full-min" | "small") => React.ReactNode;
}

const slideVariants = {
  enter: (dir: "up" | "down") => ({
    opacity: 0,
    y: dir === "up" ? -32 : 32,
  }),
  center: {
    opacity: 1,
    y: 0,
  },
  exit: (dir: "up" | "down") => ({
    opacity: 0,
    y: dir === "up" ? 32 : -32,
  }),
};

export const SettingsSideCarouselCard = memo(({
  side,
  activeDomains,
  layoutMode,
  onSwitchDomain,
  renderDomainContent,
}: SettingsSideCarouselCardProps) => {
  const domainPair: [DomainId, DomainId] = useMemo(() => {
    return side === "left"
      ? ["interaction", "personal_memory"]
      : ["models", "working_memory"];
  }, [side]);

  const [primaryId, secondaryId] = domainPair;
  const primaryDomain = DOMAINS.find((d) => d.id === primaryId)!;
  const secondaryDomain = DOMAINS.find((d) => d.id === secondaryId)!;

  // Active domain is tracked from activeDomains, defaulting to primary
  const [currentId, setCurrentId] = useState<DomainId>(() => {
    if (activeDomains.includes(secondaryId) && !activeDomains.includes(primaryId)) {
      return secondaryId;
    }
    return primaryId;
  });

  const [direction, setDirection] = useState<"up" | "down">("down");

  // Keep in sync when user clicks a specific radial node in the hub
  useEffect(() => {
    const hasSec = activeDomains.includes(secondaryId);
    const hasPri = activeDomains.includes(primaryId);
    if (hasSec && !hasPri) {
      setCurrentId(secondaryId);
    } else if (hasPri && !hasSec) {
      setCurrentId(primaryId);
    }
  }, [activeDomains, primaryId, secondaryId]);

  const isCardActive = activeDomains.includes(primaryId) || activeDomains.includes(secondaryId);
  const currentDomain = currentId === primaryId ? primaryDomain : secondaryDomain;

  // 1-click immediate switch to bottom card (direction: down)
  const handleSwitchToBottom = useCallback(() => {
    setDirection("down");
    setCurrentId(secondaryId);
    onSwitchDomain(secondaryId);
  }, [secondaryId, onSwitchDomain]);

  // 1-click immediate switch to top card (direction: up)
  const handleSwitchToTop = useCallback(() => {
    setDirection("up");
    setCurrentId(primaryId);
    onSwitchDomain(primaryId);
  }, [primaryId, onSwitchDomain]);

  if (!isCardActive) {
    return null;
  }

  const isTopActive = currentId === primaryId;

  return (
    <div className="relative flex items-center justify-center w-full max-w-full">
      {/* ── Absolute top affordance: only visible when on bottom card ─────────── */}
      {/* Chevron ABOVE label so reading order is: ^ then INTERACTION              */}
      {!isTopActive && (
        <button
          type="button"
          onClick={handleSwitchToTop}
          className="absolute -top-10 left-1/2 -translate-x-1/2 flex flex-col items-center justify-center gap-0.5 group cursor-pointer select-none transition-transform hover:scale-105 z-10"
          aria-label={`Switch to ${primaryDomain.label}`}
        >
          <ChevronUp size={14} strokeWidth={2.5} className="text-[rgb(var(--accent))] transition-transform group-hover:-translate-y-0.5" />
          <span className="text-[10px] font-bold uppercase tracking-[0.16em] text-[rgb(var(--accent))] whitespace-nowrap">
            {primaryDomain.label}
          </span>
        </button>
      )}

      {/* ── Card: anchored in-place, never shifts ───────────────────────────── */}
      <AnimatePresence mode="wait" custom={direction}>
        <motion.div
          key={currentId}
          custom={direction}
          variants={slideVariants}
          initial="enter"
          animate="center"
          exit="exit"
          transition={{ duration: 0.22, ease: [0.16, 1, 0.3, 1] }}
          className="w-full flex items-center justify-center"
        >
          <SettingsCardWrapper
            domain={currentDomain}
            isActive={true}
            layoutMode={layoutMode}
          >
            {renderDomainContent(currentDomain.id, layoutMode)}
          </SettingsCardWrapper>
        </motion.div>
      </AnimatePresence>

      {/* ── Absolute bottom affordance: only visible when on top card ─────────── */}
      {/* Label ABOVE chevron so reading order is: WORKING then v                  */}
      {isTopActive && (
        <button
          type="button"
          onClick={handleSwitchToBottom}
          className="absolute -bottom-10 left-1/2 -translate-x-1/2 flex flex-col items-center justify-center gap-0.5 group cursor-pointer select-none transition-transform hover:scale-105 z-10"
          aria-label={`Switch to ${secondaryDomain.label}`}
        >
          <span className="text-[10px] font-bold uppercase tracking-[0.16em] text-[rgb(var(--accent))] whitespace-nowrap">
            {secondaryDomain.label}
          </span>
          <ChevronDown size={14} strokeWidth={2.5} className="text-[rgb(var(--accent))] transition-transform group-hover:translate-y-0.5" />
        </button>
      )}
    </div>
  );
});

SettingsSideCarouselCard.displayName = "SettingsSideCarouselCard";
