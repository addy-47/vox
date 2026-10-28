import { describe, expect, it } from "vitest";
import {
  resolveRemoteActivityDisplay,
  resolveActivityDisplay,
} from "@/shared/lib/activityRegistry";

/**
 * Regression suite for the host remote-link status badge.
 *
 * The single status badge carries the remote link state streamed from
 * `remote_state_changed`; the redundant "REMOTE" kicker above it was removed.
 */

describe("resolveRemoteActivityDisplay", () => {
  it("maps Connected to the settled Remote Session badge", () => {
    expect(resolveRemoteActivityDisplay("Connected")).toEqual({
      label: "Remote Session",
      orbState: "breathing",
      motionPreset: "silk",
      shimmer: false,
    });
  });

  it("marks transient link phases with shimmer", () => {
    for (const state of ["Connecting", "Reconnecting"] as const) {
      const config = resolveRemoteActivityDisplay(state);
      expect(config).not.toBeNull();
      expect(config?.shimmer).toBe(true);
    }
    expect(resolveRemoteActivityDisplay("Connecting")?.label).toBe("Linking");
    expect(resolveRemoteActivityDisplay("Reconnecting")?.label).toBe(
      "Reconnecting",
    );
  });

  it("labels Failed without shimmer so controls can re-enable", () => {
    expect(resolveRemoteActivityDisplay("Failed")).toEqual({
      label: "Remote Failed",
      orbState: "breathing",
      motionPreset: "silk",
      shimmer: false,
    });
  });

  it("falls through to the local display for local-life link states", () => {
    for (const state of ["Disabled", "Unpaired", "Paired"] as const) {
      expect(resolveRemoteActivityDisplay(state)).toBeNull();
    }
  });

  it("uses the same breathing-orb visual as Sleeping, never a working state", () => {
    for (const state of [
      "Connecting",
      "Connected",
      "Reconnecting",
      "Failed",
    ] as const) {
      expect(resolveRemoteActivityDisplay(state)?.orbState).toBe("breathing");
    }
  });
});

describe("remote badge precedence against local states", () => {
  it("still renders the local Sleeping badge when no remote link is live", () => {
    expect(
      resolveActivityDisplay("Idle", null, false, true, false).label,
    ).toBe("Sleeping");
  });
});
