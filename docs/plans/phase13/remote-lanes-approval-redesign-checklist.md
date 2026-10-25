# Checklist: Remote Control 3-Lane Architecture & UI Overhaul

## Batch 1: Shared UI Primitives (`ConfirmModal` & Decoupled `AuthorizeDeviceModal`)
- [ ] `app/src/shared/ui/ConfirmModal.tsx`: Create shared minimal confirmation modal per Image 2 reference.
- [ ] `app/src/shared/components/remote/AuthorizeDeviceModal.tsx`: Create pixel-perfect Host Authorization modal per Image 3 reference.
- [ ] `app/src/shared/components/remote/PairingModal.tsx`: Extract authorization logic to `AuthorizeDeviceModal.tsx` and eliminate initial loading flicker.
- [ ] `app/src/features/settings/components/sections/RemoteDeskPanel.tsx`: Mount `AuthorizeDeviceModal` independently when `pendingPairingPrompt` is active.

## Batch 2: Backend 3-Lane Decoupling (Pairing vs Connection vs Revocation)
- [ ] `app/src-tauri/src/services/remote/protocol/messages.rs`: Add `LinkLossReason::Revoked` variant.
- [ ] `app/src-tauri/src/core/events.rs`: Add `IpcEvent::RemotePairedDevicesChanged`.
- [ ] `app/src-tauri/src/services/remote/session/listener.rs`: Update `complete_pairing` to send `PairApproved`, write `paired_devices`, emit IPC event, and close link without calling `attach_link`.
- [ ] `app/src-tauri/src/services/remote/session/manager.rs`: In `dial_and_connect`, complete pairing path without calling `attach_link`.
- [ ] `app/src-tauri/src/services/remote/session/manager.rs`: In `revoke_device`, send `ControlMessage::LinkLoss(LinkLoss { reason: LinkLossReason::Revoked })` before closing link if currently connected.

## Batch 3: Controller on Receiving Revocation & Frontend Store Sync
- [ ] `app/src-tauri/src/services/remote/session/manager.rs`: Handle inbound `LinkLoss(Revoked)` on controller to delete stored host from `paired_devices`.
- [ ] `app/src/store/remoteStore.ts`: Listen to `remote_paired_devices_changed` and re-fetch `listPairedDevices()`.
- [ ] `app/src/store/remoteStore.ts`: Clear `pairedDevices` if connection failure kind is revoked or rejected by host.

## Batch 4: Host & Controller Settings Panels Redesign
- [ ] `app/src/shared/components/settings/interaction/RemoteDeskPanel.tsx`: Remove inline gap and integrate `<ConfirmModal>` for revoking controller.
- [ ] `app/src/shared/components/settings/interaction/ControllerRemoteDeskPanel.tsx`: Base paired view on `pairedDevices.length > 0`, display Host name/IP on left, SVG trigger + `Connected` + subtle red `Click to revoke` triggering `<ConfirmModal>` on right.
- [ ] `app/src/shared/components/remote/RocketButton.tsx`: Guard connection strictly to Lane 2 live streaming.

## Batch 5: Verification & Review
- [ ] `pnpm tsc --noEmit`: Type safety gate.
- [ ] `pnpm test:unit` & `pnpm test:dom`: Unit and DOM test suites.
- [ ] `pnpm lint:arch`: Architecture linter.
- [ ] Update `docs/plans/phase13/work_log.md` with completed milestones.
