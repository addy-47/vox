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
  enter: { opacity: 0 },
  center: { opacity: 1 },
  exit: { opacity: 0 },
};

export const SettingsSideCarouselCard = memo(({
  side,
  activeDomains,
  layoutMode,
  onSwitchDomain,
  renderDomainContent,
}: SettingsSideCarouselCardProps) => {
  /**
   * A side holds one or two cards. The right side holds Memory alone, so the
   * secondary entry is optional: no secondary means no carousel affordances.
   */
  const domainIds: DomainId[] = useMemo(
    () => (side === "left" ? ["interaction", "speech"] : ["memory", "language"]),
    [side]
  );

  const primaryId = domainIds[0];
  const secondaryId = domainIds[1];
  const primaryDomain = DOMAINS.find((d) => d.id === primaryId)!;
  const secondaryDomain = secondaryId
    ? DOMAINS.find((d) => d.id === secondaryId)
    : undefined;

  // Active domain is tracked from activeDomains, defaulting to primary
  const [currentId, setCurrentId] = useState<DomainId>(() => {
    if (secondaryId && activeDomains.includes(secondaryId) && !activeDomains.includes(primaryId)) {
      return secondaryId;
    }
    return primaryId;
  });


  // Keep in sync when user clicks a specific radial node in the hub
  useEffect(() => {
    if (!secondaryId) return;
    const hasSec = activeDomains.includes(secondaryId);
    const hasPri = activeDomains.includes(primaryId);
    if (hasSec && !hasPri) {
      setCurrentId(secondaryId);
    } else if (hasPri && !hasSec) {
      setCurrentId(primaryId);
    }
  }, [activeDomains, primaryId, secondaryId]);

  const isCardActive =
    activeDomains.includes(primaryId) ||
    (secondaryId ? activeDomains.includes(secondaryId) : false);
  const currentDomain =
    currentId === primaryId || !secondaryDomain ? primaryDomain : secondaryDomain;

  // 1-click immediate switch to bottom card (direction: down)
  const handleSwitchToBottom = useCallback(() => {
    if (!secondaryId) return;
    setCurrentId(secondaryId);
    onSwitchDomain(secondaryId);
  }, [secondaryId, onSwitchDomain]);

  // 1-click immediate switch to top card (direction: up)
  const handleSwitchToTop = useCallback(() => {
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
      {!isTopActive && secondaryDomain && (
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
      <AnimatePresence mode="wait">
        <motion.div
          key={currentId}
          variants={slideVariants}
          initial="enter"
          animate="center"
          exit="exit"
          transition={{ duration: 0.12, ease: "easeOut" }}
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
      {isTopActive && secondaryDomain && (
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
