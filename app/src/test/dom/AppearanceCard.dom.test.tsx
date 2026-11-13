import { describe, it, expect, beforeEach, afterEach } from "vitest";
import { cleanup, render, screen, fireEvent } from "@testing-library/react";
import { AppearanceCard } from "@/shared/components/settings/appearance/AppearanceCard";
import { useSettingsStore } from "@/store/settingsStore";
import { resetThemeStateForTests } from "@/shared/theme";
import { minimalSettings } from "../harness/settingsFixtures";

describe("AppearanceCard DOM tests", () => {
  beforeEach(() => {
    resetThemeStateForTests();
    const settings = minimalSettings();
    useSettingsStore.setState({
      settings,
      draftSettings: structuredClone(settings),
      hasChanges: false,
    });
  });

  afterEach(() => {
    cleanup();
    resetThemeStateForTests();
  });

  it("renders card title and breadcrumb Customise action without in-card toggle or tabs", () => {
    render(<AppearanceCard />);

    expect(screen.getByText("Appearance")).toBeTruthy();
    expect(screen.getByText("Customise")).toBeTruthy();
    // In-card theme toggle button and old tabs should NOT exist
    expect(screen.queryByRole("button", { name: /switch to (light|dark) mode/i })).toBeNull();
    expect(screen.queryByRole("button", { name: "Studio Themes" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Custom Tuning" })).toBeNull();
  });

  it("renders predetermined themes and updates draft settings on selection", () => {
    render(<AppearanceCard />);

    // Predetermined themes should be listed in the Themes grid
    expect(screen.getByText("Tokyo Night")).toBeTruthy();
    expect(screen.getByText("Catppuccin")).toBeTruthy();
    expect(screen.getByText("Night Owl")).toBeTruthy();
    expect(screen.getByText("One Dark")).toBeTruthy();
    expect(screen.getByText("Dracula")).toBeTruthy();
    expect(screen.getByText("Nord")).toBeTruthy();
    expect(screen.getByText("Vox Default")).toBeTruthy();
    expect(screen.getByText("Rosé Pine")).toBeTruthy();
    expect(screen.getByText("Solarized")).toBeTruthy();
    expect(screen.getByText("Custom")).toBeTruthy();

    // Select Tokyo Night
    const tokyoNightBtn = screen.getByText("Tokyo Night").closest("button");
    expect(tokyoNightBtn).toBeTruthy();
    fireEvent.click(tokyoNightBtn!);

    const draft = useSettingsStore.getState().draftSettings?.appearance;
    expect(draft?.preset).toBe("tokyo_night");
    expect(draft?.dark_accent).toBe("#7AA2F7");
    expect(draft?.dark_bg).toBe("#16161E");
    expect(draft?.light_accent).toBe("#2E7DE9");
    expect(draft?.light_bg).toBe("#F2F3F7");
  });

  it("navigates to Custom Palette view via Customise breadcrumb and applies accent swatch", () => {
    render(<AppearanceCard />);

    // Click Customise action button in CardTitleBar
    fireEvent.click(screen.getByText("Customise"));

    // Expect breadcrumb title "Custom Palette" and segmented buttons
    expect(screen.getByText("Custom Palette")).toBeTruthy();
    expect(screen.getByText("Accent Color")).toBeTruthy();
    expect(screen.getByText("Background Tone")).toBeTruthy();

    // Theme toggle button should now be present in header in custom view
    expect(screen.getByRole("button", { name: /switch to (light|dark) mode/i })).toBeTruthy();

    // Click accent swatch (#CBA6F7 - Astral Mauve)
    const mauveSwatch = screen.getByRole("button", { name: /select accent #cba6f7/i });
    expect(mauveSwatch).toBeTruthy();
    fireEvent.click(mauveSwatch);

    const draft = useSettingsStore.getState().draftSettings?.appearance;
    expect(draft?.preset).toBe("custom");
    expect(draft?.dark_accent).toBe("#CBA6F7");
  });

  it("switches to Background Tone segmented tab and applies background preset", () => {
    render(<AppearanceCard />);

    // Navigate to Custom Palette
    fireEvent.click(screen.getByText("Customise"));

    // Click Background Tone segmented button
    fireEvent.click(screen.getByText("Background Tone"));

    // Click OLED Black background preset
    const oledBlackBtn = screen.getByRole("button", { name: /oled black/i });
    expect(oledBlackBtn).toBeTruthy();
    fireEvent.click(oledBlackBtn);

    const draft = useSettingsStore.getState().draftSettings?.appearance;
    expect(draft?.preset).toBe("custom");
    expect(draft?.dark_bg).toBe("#000000");
  });

  it("navigates back to themes view via breadcrumb back button", () => {
    render(<AppearanceCard />);

    // Navigate to Custom Palette
    fireEvent.click(screen.getByText("Customise"));
    expect(screen.getByText("Custom Palette")).toBeTruthy();

    // Click Back arrow button in CardTitleBar
    const backBtn = screen.getByRole("button", { name: /^back$/i });
    expect(backBtn).toBeTruthy();
    fireEvent.click(backBtn);

    // Should be back on Themes view
    expect(screen.getByText("Appearance")).toBeTruthy();
    expect(screen.getByText("Tokyo Night")).toBeTruthy();
    expect(screen.getByText("Catppuccin")).toBeTruthy();
  });

  it("adapts layout smoothly for small layoutMode", () => {
    const { container } = render(<AppearanceCard layoutMode="small" />);
    expect(container.firstChild).toBeTruthy();
    expect(screen.getByText("Tokyo Night")).toBeTruthy();
  });
});
