import { describe, it, expect, afterEach, vi } from "vitest";
import {
  registerOverlay,
  closeTopmost,
  getStackSize,
  installOverlayStack,
} from "@/shared/lib/overlayStack";

/**
 * Global overlay stack regression suite (shared/lib/overlayStack.ts).
 *
 * Phase 1 — Production Path Trace
 * -------------------------------
 * SUT:
 *   The single dismissal authority: FILO Escape pops (profiler drawer below
 *   monitoring popover), topmost-only outside-click dismissal, and the
 *   idempotent global listener installation that App mount + HSR call repeatedly.
 *
 * Production Entry Seam:
 *   registerOverlay / closeTopmost / installOverlayStack — the real exports.
 *   Events are real DOM events on the jsdom window: the same KEYDOWN /
 *   POINTERDOWN capture listeners production installs run against them.
 *   No vi.mock: the SUT *is* the window-listening layer.
 *
 * Direction Check:
 *   Entry is the exact trigger a key press and a click are; a component
 *   render test would only reach the module through more indirection.
 *
 * Production Path:
 *   Esc key / outside pointerdown (real user input)
 *   → window capture listener (installOverlayStack)
 *   → closeTopmost / topmost outside-or-predicate logic
 *   → the overlay's onClose (drawer unmount, popover pop)
 *
 * Observable Exit: onClose call order among overlays + getStackSize + whether
 * the event's default was prevented (drawer hosts rely on Esc being swallowed
 * before per-surface handlers see it).
 *
 * Phase 2a — Testability 4/4: no stubs of the SUT; module-global state is
 * managed through the production unmount function every registerOverlay
 * returns (afterEach drains the stack). The jsdom window is the production
 * event carrier; dispatched Event targets are real DOM nodes in a real tree.
 *
 * Phase 2b — False-Green table (each row a one-line production mutation):
 *   | closeTopmost pops the BOTTOM overlay → FILO broken            | must fail (order) |
 *   | dismissOnOutside check dropped → non-opted overlays close on click | must fail |
 *   | shouldDismissOnPointerDown predicate ignored                  | must fail |
 *   | installed guard removed → listeners installed twice           | must fail (one Esc pops two) |
 *   | unregister no-ops → closed overlay still is the topmost       | must fail |
 *
 * Phase 4 — Governing sentence:
 *   If the FILO order silently became FIFO, this test would fail because the
 *   profiler drawer's onClose would fire before the popover's.
 */

const unmounters: Array<() => void> = [];
afterEach(() => {
  while (unmounters.length) {
    unmounters.pop()?.();
  }
  vi.restoreAllMocks();
});

/** Registers via the production API, wrapping onClose in a spy for assertions. */
function open(opts: Parameters<typeof registerOverlay>[0]): [onClose: ReturnType<typeof vi.fn>, unmount: () => void] {
  const closeSpy = vi.fn(opts.onClose);
  const unmount = registerOverlay({ ...opts, onClose: closeSpy });
  unmounters.push(unmount);
  return [closeSpy, unmount];
}

/** Real pointerdown dispatched ON the node so capture hits window with target=el. */
function pointerAt(el: HTMLElement): void {
  el.dispatchEvent(
    new MouseEvent("pointerdown", { bubbles: true, cancelable: true })
  );
}

describe("closeTopmost — FILO semantics", () => {
  it("pops the most recently registered overlay first, then the older one", () => {
    const [profilerClose] = open({ onClose: () => {} });
    const [popoverClose] = open({ onClose: () => {} });

    expect(getStackSize()).toBe(2);
    expect(closeTopmost()).toBe(true);
    expect(popoverClose).toHaveBeenCalledTimes(1);
    expect(profilerClose).not.toHaveBeenCalled();
    expect(getStackSize()).toBe(1);

    expect(closeTopmost()).toBe(true);
    expect(profilerClose).toHaveBeenCalledTimes(1);
    expect(getStackSize()).toBe(0);
  });

  it("NEGATIVE: empty stack reports a no-op without throwing", () => {
    expect(getStackSize()).toBe(0);
    expect(closeTopmost()).toBe(false);
  });

  it("unregistering a closed middle overlay keeps the topmost dismissal target intact", () => {
    const [, unregisterBottom] = open({ onClose: () => {} });
    const [topClose] = open({ onClose: () => {} });

    unregisterBottom();
    expect(getStackSize()).toBe(1);
    expect(closeTopmost()).toBe(true);
    expect(topClose).toHaveBeenCalledTimes(1);
  });
});

describe("installOverlayStack — real window capture listeners", () => {
  it("ONE Escape press pops exactly the topmost overlay and swallows the default", () => {
    installOverlayStack();
    const [bottomClose] = open({ onClose: () => {} });
    const [topClose] = open({ onClose: () => {} });

    // A second window-level capture listener, registered after the SUT's,
    // observes the same event after the SUT's handler has run.
    const witness = vi.fn();
    window.addEventListener("keydown", witness, true);
    try {
      window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true })
      );

      expect(topClose).toHaveBeenCalledTimes(1);
      expect(bottomClose).not.toHaveBeenCalled();
      expect(getStackSize()).toBe(1);
      expect(witness.mock.calls[0]?.[0].defaultPrevented).toBe(true);
    } finally {
      window.removeEventListener("keydown", witness, true);
    }
  });

  it("NEGATIVE: non-Escape keys are ignored entirely", () => {
    installOverlayStack();
    const [topClose] = open({ onClose: () => {} });

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", cancelable: true }));
    window.dispatchEvent(new KeyboardEvent("keydown", { key: "escape", cancelable: true })); // case-sensitive

    expect(topClose).not.toHaveBeenCalled();
    expect(getStackSize()).toBe(1);
  });

  it("installOverlayStack is idempotent against same-identity double installs", () => {
    installOverlayStack();
    installOverlayStack(); // App mount + a second module's install() call

    const [bottomClose] = open({ onClose: () => {} });
    const [topClose] = open({ onClose: () => {} });

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", cancelable: true }));

    // Exactly one pop: the popover. The DOM dedups identical (type, fn,
    // capture) listener triples, so a repeated addEventListener of the same
    // handler must never produce a second dismissal. NOTE: the `installed`
    // module flag additionally guards HMR re-instants, where a *new* listener
    // identity would otherwise be installed — that path is not observable
    // through a single module instance.
    expect(topClose).toHaveBeenCalledTimes(1);
    expect(bottomClose).toHaveBeenCalledTimes(0);
    expect(getStackSize()).toBe(1);
  });
});

describe("outside pointerdown — topmost-only, opt-in", () => {
  function fixture(): { inside: HTMLElement; outside: HTMLElement } {
    const inside = document.createElement("div");
    const outside = document.createElement("div");
    document.body.append(inside, outside);
    return { inside, outside };
  }

  it("NEGATIVE: without dismissOnOutside, an outside click never dismisses", () => {
    installOverlayStack();
    const { outside } = fixture();
    const [close] = open({ onClose: () => {}, getEl: () => outside, dismissOnOutside: false });

    pointerAt(document.body); // click outside the element the overlay owns
    expect(close).not.toHaveBeenCalled();
    expect(getStackSize()).toBe(1);
  });

  it("dismissOnOutside closes the topmost overlay when the click lands outside its element", () => {
    installOverlayStack();
    const { outside } = fixture();
    const [close] = open({ onClose: () => {}, getEl: () => outside, dismissOnOutside: true });

    pointerAt(document.body); // a click outside the `outside` sentinel element
    expect(close).toHaveBeenCalledTimes(1);
  });

  it("NEGATIVE: a click INSIDE the element does not dismiss", () => {
    installOverlayStack();
    const { inside } = fixture();
    const [close] = open({ onClose: () => {}, getEl: () => inside, dismissOnOutside: true });

    pointerAt(inside);
    expect(close).not.toHaveBeenCalled();
    expect(getStackSize()).toBe(1);
  });

  it("NEGATIVE: only the topmost overlay sees the outside click", () => {
    installOverlayStack();
    const { outside } = fixture();
    const [drawerClose] = open({ onClose: () => {}, getEl: () => outside, dismissOnOutside: true });
    const [popoverClose] = open({ onClose: () => {}, getEl: () => outside, dismissOnOutside: true });

    pointerAt(document.body);
    expect(popoverClose).toHaveBeenCalledTimes(1);
    expect(drawerClose).not.toHaveBeenCalled();
  });

  it("a lazily-resolving element must still be hit-tested at event time, not at registration", () => {
    installOverlayStack();
    const { outside } = fixture();
    let el: HTMLElement | null = null; // ref populated after mount in production
    const [close] = open({ onClose: () => {}, getEl: () => el, dismissOnOutside: true });

    pointerAt(document.body); // getEl() === null → contains() must not throw and must not dismiss
    expect(close).not.toHaveBeenCalled();

    el = outside;
    pointerAt(document.body); // element now exists, the click is outside it
    expect(close).toHaveBeenCalledTimes(1);
  });

  it("shouldDismissOnPointerDown wins over containment", () => {
    installOverlayStack();
    const { inside, outside } = fixture();
    const [close] = open({
      onClose: () => {},
      getEl: () => inside,
      dismissOnOutside: true,
      shouldDismissOnPointerDown: (target) => target === outside,
    });

    pointerAt(inside); // inside the element but the predicate vetoes dismissal
    expect(close).not.toHaveBeenCalled();

    pointerAt(outside);
    expect(close).toHaveBeenCalledTimes(1);
  });
});
