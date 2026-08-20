/**
 * Global overlay stack — the single authority for FILO (last-in, first-out)
 * overlay dismissal.
 *
 * Every overlay (drawer, popover, panel, card) registers itself on open and
 * unregisters on close. A single global `keydown` handler pops the TOPMOST
 * registered overlay on Escape, and a global `pointerdown` handler dismisses
 * the topmost overlay when the click lands outside its element. This gives
 * consistent FILO semantics across the app without each surface hand-rolling
 * its own listener:
 *
 *   profiler drawer open → monitoring popover opens on top
 *   → first Escape closes the popover → second Escape closes the profiler.
 */

interface OverlayEntry {
  /** Monotonic sequence — registration order, later = higher on the stack. */
  id: number;
  /** Called when this overlay is dismissed by the stack (Escape / outside-click). */
  onClose: () => void;
  /** Lazily read at event time so refs populated after mount still resolve. */
  getEl: () => HTMLElement | null;
  /** If true, a pointerdown outside `getEl()` dismisses this overlay. */
  dismissOnOutside: boolean;
}

const stack: OverlayEntry[] = [];
let seq = 0;
let installed = false;

/**
 * Register an overlay on the stack. Returns an unregister function.
 * An overlay is considered open from the moment it registers.
 */
export function registerOverlay(opts: {
  onClose: () => void;
  getEl?: () => HTMLElement | null;
  dismissOnOutside?: boolean;
}): () => void {
  const entry: OverlayEntry = {
    id: ++seq,
    onClose: opts.onClose,
    getEl: opts.getEl ?? (() => null),
    dismissOnOutside: opts.dismissOnOutside ?? false,
  };
  stack.push(entry);

  return () => {
    const idx = stack.indexOf(entry);
    if (idx !== -1) stack.splice(idx, 1);
  };
}

/**
 * Close the topmost overlay. Returns true if an overlay was closed.
 * The entry is popped immediately so a subsequent Escape targets the next
 * overlay even while the closed overlay is still animating out.
 */
export function closeTopmost(): boolean {
  const top = stack.pop();
  if (!top) return false;
  top.onClose();
  return true;
}

/** Number of currently-open overlays (for tests / debugging). */
export function getStackSize(): number {
  return stack.length;
}

/** Active overlay ids in registration order (for tests / debugging). */
export function getStackIds(): number[] {
  return stack.map((e) => e.id);
}

function onKeyDown(e: KeyboardEvent) {
  if (e.key !== "Escape" || stack.length === 0) return;
  e.preventDefault();
  e.stopPropagation();
  closeTopmost();
}

function onPointerDown(e: PointerEvent) {
  if (stack.length === 0) return;
  const top = stack[stack.length - 1];
  if (!top.dismissOnOutside) return;
  const el = top.getEl();
  if (el && e.target instanceof Node && !el.contains(e.target)) {
    top.onClose();
  }
}

/**
 * Install the global listeners. Idempotent — safe to call from multiple
 * modules (e.g. App mount + HMR). Listeners use capture so they run before
 * per-surface handlers and can prevent default reliably.
 */
export function installOverlayStack(): void {
  if (installed) return;
  installed = true;
  window.addEventListener("keydown", onKeyDown, true);
  window.addEventListener("pointerdown", onPointerDown, true);
}