import { describe, it, expect, vi, beforeEach } from "vitest";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import {
  showMainWindow,
  hideTrayWindow,
  syncHudVisibility,
  setHudIgnoreCursor,
} from "../windowService";

describe("windowService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Window Visibility & Cursor Management IPC", () => {
    it("should show main window and hide tray window", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await showMainWindow();
      expect(mockInvoke).toHaveBeenCalledWith("show_main_window");

      await hideTrayWindow();
      expect(mockInvoke).toHaveBeenCalledWith("hide_tray_window");
    });

    it("should sync HUD visibility and set HUD ignore cursor", async () => {
      mockInvoke.mockResolvedValue(undefined);

      await syncHudVisibility(true);
      expect(mockInvoke).toHaveBeenCalledWith("sync_hud_visibility", { visible: true });

      await setHudIgnoreCursor(false);
      expect(mockInvoke).toHaveBeenCalledWith("set_hud_ignore_cursor", { ignore: false });
    });
  });
});
