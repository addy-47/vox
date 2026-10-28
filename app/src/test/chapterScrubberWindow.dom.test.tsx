import { describe, it, expect, afterEach } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { ChapterScrubber, type Chapter } from "@/shared/ui/ChapterScrubber";

/**
 * Regression suite for the paged dialogue scrubber.
 *
 * 1. A long session must not drop old turns: only `maxVisible` rows render and
 *    the chevrons page through the rest.
 * 2. The tick colour must use the foreground token. `bg-white/80` made every
 *    non-current tick invisible in light mode.
 */

const CHAPTERS: Chapter[] = Array.from({ length: 100 }, (_, index) => ({
  id: `turn-${index}`,
  title: `Turn ${index + 1}`,
  description: `Assistant reply ${index + 1}`,
  meta: `Turn #${index + 1}`,
}));

function renderScrubber() {
  return render(
    <ChapterScrubber
      chapters={CHAPTERS}
      maxVisible={15}
      currentIndex={CHAPTERS.length - 1}
      label="Dialogue History Scrubber"
    />,
  );
}

function optionNames(): string[] {
  return screen
    .getAllByRole("option")
    .map((option) => option.getAttribute("aria-label") ?? "");
}

afterEach(cleanup);

describe("ChapterScrubber paging", () => {
  it("renders only the visible window and opens on the newest page", () => {
    renderScrubber();

    const names = optionNames();
    expect(names).toHaveLength(15);
    expect(names[0]).toContain("Turn 86");
    expect(names[14]).toContain("Turn 100");
    expect(screen.queryByRole("option", { name: /Turn 85/ })).toBeNull();
  });

  it("pages older turns and stops at the first turn", () => {
    renderScrubber();

    fireEvent.click(screen.getByRole("button", { name: "Show older turns" }));
    expect(optionNames()[0]).toContain("Turn 71");
    expect(optionNames()[14]).toContain("Turn 85");
    expect(screen.getByText("71–85")).toBeTruthy();

    for (let page = 0; page < 10; page += 1) {
      fireEvent.click(screen.getByRole("button", { name: "Show older turns" }));
    }

    const names = optionNames();
    expect(names[0]).toContain("Turn 1");
    expect(screen.getByText("1–15")).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: "Show older turns" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      (screen.getByRole("button", { name: "Show newer turns" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false);
  });

  it("pages back to the newest turn and disables the newer chevron there", () => {
    renderScrubber();

    fireEvent.click(screen.getByRole("button", { name: "Show older turns" }));
    fireEvent.click(screen.getByRole("button", { name: "Show newer turns" }));

    expect(optionNames()[14]).toContain("Turn 100");
    expect(
      (screen.getByRole("button", { name: "Show newer turns" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("snaps back to the newest page when a new turn arrives", () => {
    const { rerender } = render(
      <ChapterScrubber
        chapters={CHAPTERS}
        maxVisible={15}
        currentIndex={CHAPTERS.length - 1}
        label="Dialogue History Scrubber"
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Show older turns" }));
    expect(optionNames()[14]).toContain("Turn 85");

    const withNewTurn: Chapter[] = [
      ...CHAPTERS,
      {
        id: "turn-100",
        title: "Turn 101",
        description: "Fresh reply",
        meta: "Turn #101",
      },
    ];

    rerender(
      <ChapterScrubber
        chapters={withNewTurn}
        maxVisible={15}
        currentIndex={withNewTurn.length - 1}
        label="Dialogue History Scrubber"
      />,
    );

    expect(optionNames()[14]).toContain("Turn 101");
  });

  it("uses the foreground token for resting ticks so they survive light mode", () => {
    renderScrubber();

    const restingTick = screen
      .getAllByRole("option")[0]
      .querySelector("span");
    expect(restingTick?.className).toContain("var(--foreground)");
    expect(restingTick?.className).not.toContain("bg-white");
  });

  it("hides the chevrons when every turn fits", () => {
    render(
      <ChapterScrubber
        chapters={CHAPTERS.slice(0, 15)}
        maxVisible={15}
        label="Dialogue History Scrubber"
      />,
    );

    expect(optionNames()).toHaveLength(15);
    expect(
      screen.queryByRole("button", { name: "Show older turns" }),
    ).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Show newer turns" }),
    ).toBeNull();
  });
});
