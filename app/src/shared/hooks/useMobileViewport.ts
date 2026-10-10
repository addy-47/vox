import { useEffect, useState } from 'react';

/** Matches the `md` breakpoint Tailwind uses across the app. */
export const MOBILE_BREAKPOINT = 768;

/**
 * True when the viewport is phone-sized.
 *
 * This deliberately answers only "is this narrow?", NOT "is this a real
 * Android device?". Those are different questions and conflating them is what
 * put a desktop phone mockup inside a real phone — see `isRealMobileDevice`
 * below and `WizardRoot`.
 */
export function isMobileViewport(width?: number): boolean {
  if (typeof width === 'number') return width < MOBILE_BREAKPOINT;
  if (typeof window === 'undefined') return false;
  return window.innerWidth < MOBILE_BREAKPOINT;
}

/**
 * True only for a real handheld OS: Android or iOS.
 *
 * Use this to gate anything that would be wrong on a real device — chiefly the
 * developer chrome in `WizardRoot` (the "Android Flow" / "Phone Frame"
 * toggles), which exists to preview the mobile layout from a desktop browser and
 * must never appear on the device it is previewing.
 */
export function isRealMobileDevice(): boolean {
  if (typeof navigator === 'undefined') return false;
  return /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
}

export function useIsMobileViewport(): boolean {
  const [isMobile, setIsMobile] = useState(() => isMobileViewport());

  useEffect(() => {
    const onResize = () => setIsMobile(isMobileViewport());
    window.addEventListener('resize', onResize);
    window.addEventListener('orientationchange', onResize);
    return () => {
      window.removeEventListener('resize', onResize);
      window.removeEventListener('orientationchange', onResize);
    };
  }, []);

  return isMobile;
}