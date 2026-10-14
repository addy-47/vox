import { memo, type CSSProperties, type ReactNode, type Ref } from "react";
import { cn } from "@/shared/lib/utils";
import { ErrorBoundary } from "@/shared/components/common/ErrorBoundary";
import { useLenisScrollContainer, type LenisScrollContainer } from "@/shared/hooks/useLenisScrollContainer";
import { SMOOTH_SCROLL_CONTAINER_CLASS, SMOOTH_SCROLL_DURATION } from "@/shared/lib/lenisScroll";

export interface SmoothScrollAreaProps {
  children: ReactNode;
  className?: string;
  /** Classes applied to the inner content wrapper (the Lenis content element). */
  contentClassName?: string;
  style?: CSSProperties;
  /** Lenis wheel-glide duration in seconds. Defaults to the canonical standard. */
  duration?: number;
  /** Exposes container/lenis refs for scroll-to-top helpers. */
  containerRefOut?: Ref<LenisScrollContainer<HTMLDivElement>>;
  id?: string;
  /** ErrorBoundary label for this region. */
  name?: string;
}

/**
 * Automatic smooth-scroll container (SSOT wrapper).
 *
 * Declarative replacement for hand-rolled `createContainedLenis` effects.
 * Attaches exactly one standardized Lenis instance (inertial wheel glide,
 * native touch momentum) and applies the canonical container classes.
 * Use this for every scrollable list/panel instead of ad-hoc effects or
 * bare `overflow-y-auto` divs — it prevents drift by construction.
 */
export const SmoothScrollArea = memo(({
  children,
  className,
  contentClassName,
  style,
  duration = SMOOTH_SCROLL_DURATION,
  containerRefOut,
  id,
  name = "ScrollArea",
}: SmoothScrollAreaProps) => {
  const { containerRef, lenisRef, scrollToTop } = useLenisScrollContainer<HTMLDivElement>(duration);

  if (containerRefOut) {
    if (typeof containerRefOut === "function") {
      containerRefOut({ containerRef, lenisRef, scrollToTop });
    } else {
      (containerRefOut as { current: LenisScrollContainer<HTMLDivElement> | null }).current = {
        containerRef,
        lenisRef,
        scrollToTop,
      };
    }
  }

  return (
    <div ref={containerRef} id={id} style={style} className={cn(SMOOTH_SCROLL_CONTAINER_CLASS, className)}>
      {/* Scoped so a render failure in one list cannot blank the whole view. */}
      <ErrorBoundary name={name}>
        <div className={cn("min-h-full w-full", contentClassName)}>{children}</div>
      </ErrorBoundary>
    </div>
  );
});

SmoothScrollArea.displayName = "SmoothScrollArea";
