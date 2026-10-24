import { describe, it, expect, beforeEach, vi } from "vitest";
import { renderHook } from "@testing-library/react";
import { useRemoteGuard } from "@/shared/hooks/useRemoteGuard";
import { useRemoteStore } from "@/store/remoteStore";
import * as breakpoints from "@/layout/breakpoints";

describe("useRemoteGuard unit suite", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    useRemoteStore.setState({
      linkState: "Disconnected",
      failureReason: null,
      connectedPeer: null,
    });
  });

  describe("when running on Desktop Host", () => {
    beforeEach(() => {
      vi.spyOn(breakpoints, "isRealMobileDevice").mockReturnValue(false);
    });

    it("blocks mutating controls when linkState is Connected", () => {
      useRemoteStore.setState({ linkState: "Connected" });

      const { result } = renderHook(() => useRemoteGuard());

      expect(result.current.isBlocked).toBe(true);
      expect(result.current.tooltip).toBe("Remote session active");
      expect(result.current.guardProps.disabled).toBe(true);
      expect(result.current.guardProps["aria-disabled"]).toBe(true);
      expect(result.current.guardProps.title).toBe("Remote session active");
      expect(result.current.guardProps.className).toContain("opacity-40");
      expect(result.current.guardProps.className).toContain("pointer-events-none");
    });

    it("allows mutating controls when linkState is Disconnected", () => {
      useRemoteStore.setState({ linkState: "Disconnected" });

      const { result } = renderHook(() => useRemoteGuard());

      expect(result.current.isBlocked).toBe(false);
      expect(result.current.tooltip).toBeUndefined();
      expect(result.current.guardProps).toEqual({});
    });

    it("allows mutating controls during Connecting, Reconnecting, and Failed states", () => {
      for (const state of ["Connecting", "Reconnecting", "Failed"] as const) {
        useRemoteStore.setState({ linkState: state });
        const { result } = renderHook(() => useRemoteGuard());

        expect(result.current.isBlocked).toBe(false);
        expect(result.current.tooltip).toBeUndefined();
        expect(result.current.guardProps).toEqual({});
      }
    });
  });

  describe("when running on Mobile Controller", () => {
    beforeEach(() => {
      vi.spyOn(breakpoints, "isRealMobileDevice").mockReturnValue(true);
    });

    it("does not block controls even when linkState is Connected", () => {
      useRemoteStore.setState({ linkState: "Connected" });

      const { result } = renderHook(() => useRemoteGuard());

      expect(result.current.isBlocked).toBe(false);
      expect(result.current.tooltip).toBeUndefined();
      expect(result.current.guardProps).toEqual({});
    });
  });
});
