import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  showMainWindow,
  hideTrayWindow,
  setWindowClickThrough,
} from "../windowService";

describe("windowService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Window Visibility & Click-Through Management IPC", () => {
    it("should show main window and hide tray window", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await showMainWindow();
      expect(mockInvoke).toHaveBeenCalledWith("show_main_window");

      await hideTrayWindow();
      expect(mockInvoke).toHaveBeenCalledWith("hide_tray_window");
    });

    it("should set window click through for tray and toast", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await setWindowClickThrough("tray", true);
      expect(mockInvoke).toHaveBeenCalledWith("set_window_click_through", { window: "tray", enabled: true });

      await setWindowClickThrough("toast", false);
      expect(mockInvoke).toHaveBeenCalledWith("set_window_click_through", { window: "toast", enabled: false });
    });
  });
});
