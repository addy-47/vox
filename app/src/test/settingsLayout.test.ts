import { describe, expect, it } from "vitest";
import { getSettingsInlineListMaxHeightClass } from "../shared/components/settings/shared/settingsLayout";

describe("settings inline list sizing", () => {
  it("uses the Language card's 260px cap for small layouts", () => {
    expect(getSettingsInlineListMaxHeightClass("small")).toBe("max-h-[260px]");
  });

  it("uses the standard card-list cap for desktop layouts", () => {
    expect(getSettingsInlineListMaxHeightClass("full-max")).toBe("max-h-[480px]");
    expect(getSettingsInlineListMaxHeightClass("full-min")).toBe("max-h-[480px]");
  });
});
