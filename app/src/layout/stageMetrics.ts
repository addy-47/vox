/**
 * Mobile stage vertical anchors.
 *
 * The Home orb is optically centred on the handheld shell by anchoring at
 * `calc(50% - 22.4px)` (`MOBILE_STAGE_OFFSET_Y`). The mobile app header (~58px)
 * and the Home bottom-dock padding (~60px + safe area) cancel out, leaving this
 * tuned offset to place the orb on the visual centre line.
 *
 * Stage surfaces reuse this anchor so the loader, the empty states, and the orb
 * all share the same mobile centre line:
 * - full-viewport stage boxes (`viewport`): boot loader, route-change overlay;
 * - page boxes (`page`): elements rendered below the mobile AppHeader (orb,
 *   page loaders, empty states).
 *
 * `desktopY` is the pre-existing desktop anchor for the surface being positioned
 * (`0` for plain full-viewport centres, `36` for the earlier orb-aligned offsets).
 */
export const MOBILE_STAGE_OFFSET_Y = 22.4;

/**
 * Vertical `top` for a stage item anchored to its container's centre line via
 * `transform: translate(-50%, -50%)`. Mobile anchors at the orb optical offset;
 * desktop keeps the surface's pre-existing offset.
 */
export function stageTop(mobile: boolean, desktopY: number): string {
  const y = mobile ? MOBILE_STAGE_OFFSET_Y : desktopY;
  return `calc(50% - ${y}px)`;
}
