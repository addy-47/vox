# Mobile Wizard Redesign Implementation Checklist

## Batch 1: Copy & Type Definitions
- [ ] `app/src/data/welcomeCopy.ts`
  - Add `skipModal` keys to `MOBILE_ONBOARDING_COPY`.
  - Add `modelPresets` keys to `MOBILE_ONBOARDING_COPY`.
  - Update `MOBILE_ONBOARDING_COPY.ready` for centered copy and status chips.

## Batch 2: Skip Modal & Header Layout
- Depends on: Batch 1
- [ ] `app/src/wizard/mobile/MobileSkipConfirmModal.tsx`
  - Implement centered glassmorphism confirmation modal with informative copy.
- [ ] `app/src/wizard/mobile/MobileOnboardingFlow.tsx`
  - Decouple Skip button from step counter in header.
  - Render dedicated step indicator in header.
  - Wire Skip click to show confirmation modal.

## Batch 3: Centered Model Download Step
- Depends on: Batch 1, Batch 2
- [ ] `app/src/wizard/mobile/MobileModelStep.tsx`
  - Add preset selection state: `'minimal' | 'full' | 'custom'`.
  - Implement centered preset selector chips and purpose summary cards.
  - Add badges for Core/Coupled vs Optional, Local vs Free Cloud.
  - Center header copy and progress bar while retaining truthful multi-file progress.

## Batch 4: Centered Ready Step
- Depends on: Batch 1
- [ ] `app/src/wizard/mobile/MobileReadyStep.tsx`
  - Add centered `VoxOrb` component with ambient radial glow.
  - Center title, description, and status capsules.
  - Replace vertical cards with centered summary and usage tips.

## Batch 5: Verification & Gate
- Depends on: Batches 1–4
- [ ] `app/`
  - Run `pnpm tsc --noEmit`.
  - Run `pnpm test`.
  - Run `app/src-tauri/scripts/verify-targets.sh`.
