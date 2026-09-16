import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";

/**
 * Standard bottom-dock feather. Single linear bottom-up dissolve (opaque card
 * token at the dock edge → transparent above) applied as BOTH background and
 * mask, so scrolling content fades out before it can visually collide with any
 * floating element docked at the viewport bottom.
 *
 * Deliberately zero `backdrop-blur`: the dissolve alone reads identically at
 * these sizes for a fraction of the compositor cost.
 *
 * Contract: render as the FIRST child of the dock wrapper (sibling order keeps
 * it behind the controls), and size it via `className` so it extends above the
 * dock — ~40px for corner docks (`-top-10` overscan), ~110px full-width for
 * bottom bars. Always `pointer-events-none` (baked in).
 * The controls sibling MUST be positioned (`relative`): an `absolute` feather
 * otherwise paints above non-positioned siblings regardless of order.
 */
const FEATHER_STYLE: React.CSSProperties = {
  background:
    "linear-gradient(to top, rgb(var(--card)) 0%, rgba(var(--card), 0.8) 55%, transparent 100%)",
  maskImage: "linear-gradient(to top, black 0%, black 60%, transparent 100%)",
  WebkitMaskImage: "linear-gradient(to top, black 0%, black 60%, transparent 100%)",
};

interface BottomDockFeatherProps {
  /** Positioning + size from the caller (e.g. `fixed bottom-0 inset-x-0 h-[110px]`). */
  className?: string;
}

export const BottomDockFeather: React.FC<BottomDockFeatherProps> = memo(
  ({ className }) => (
    <div aria-hidden="true" style={FEATHER_STYLE} className={cn("pointer-events-none", className)} />
  )
);

BottomDockFeather.displayName = "BottomDockFeather";
