import { memo, useRef, useCallback, useEffect, useState } from "react";
import { cn } from "@/shared/lib/utils";
import {
  hexToOklch,
  oklchToHex,
  ACCENT_HUE_CONIC,
  DARK_BG_CONIC,
  LIGHT_BG_CONIC,
} from "@/shared/theme/oklchColor";
import { Copy, Check } from "lucide-react";
import { copyToClipboard } from "@/shared/lib/clipboard";

interface OklchDonutPickerProps {
  mode: "accent" | "background";
  hex: string;
  onChange: (hex: string) => void;
  onCommit: () => void;
  isLight: boolean;
  size?: number;
  className?: string;
}

export const OklchDonutPicker = memo(
  ({
    mode,
    hex,
    onChange,
    onCommit,
    isLight,
    size = 126,
    className,
  }: OklchDonutPickerProps) => {
    const donutRef = useRef<HTMLDivElement>(null);
    const draggingTargetRef = useRef<"hue" | "lightness" | null>(null);

    // Parse incoming hex to OKLCH
    const parsed = hexToOklch(hex, mode === "accent" ? 200 : 280);
    const [hue, setHue] = useState(parsed.h);
    const [lightness, setLightness] = useState(parsed.l);
    const [chroma, setChroma] = useState(parsed.c);
    const [copied, setCopied] = useState(false);

    // Sync with external hex changes when not actively dragging
    useEffect(() => {
      if (!draggingTargetRef.current) {
        const next = hexToOklch(hex, hue);
        setHue(next.h);
        setLightness(next.l);
        if (mode === "accent" && next.c < 0.08) {
          setChroma(0.14);
        } else {
          setChroma(next.c);
        }
      }
    }, [hex, mode, hue]);

    // Bounds for lightness based on mode and theme
    const minL = mode === "accent" ? 0.25 : isLight ? 0.75 : 0.0;
    const maxL = mode === "accent" ? 0.92 : isLight ? 1.0 : 0.35;

    // Radius thresholds for radial hit testing
    const halfSize = size / 2;
    const outerRingRadius = halfSize - 6; // Centerline of outer ring (~57px)
    const innerRingRadius = halfSize - 22; // Centerline of inner ring (~41px)

    // Handle Hue pointer update
    const updateHueFromPointer = useCallback(
      (clientX: number, clientY: number) => {
        if (!donutRef.current) return;
        const rect = donutRef.current.getBoundingClientRect();
        const cx = rect.left + rect.width / 2;
        const cy = rect.top + rect.height / 2;
        const dx = clientX - cx;
        const dy = clientY - cy;

        // Angle 0deg at 12 o'clock
        let angle = (Math.atan2(dy, dx) * 180) / Math.PI + 90;
        if (angle < 0) angle += 360;

        const nextHue = Math.round(angle);
        setHue(nextHue);

        const effectiveChroma = mode === "accent" ? Math.max(0.12, chroma) : 0.025;
        const newHex = oklchToHex(lightness, effectiveChroma, nextHue);
        onChange(newHex);
      },
      [chroma, lightness, mode, onChange]
    );

    // Handle Lightness pointer update (vertical position on inner ring)
    const updateLightnessFromPointer = useCallback(
      (clientY: number) => {
        if (!donutRef.current) return;
        const rect = donutRef.current.getBoundingClientRect();
        const cy = rect.top + rect.height / 2;
        const dy = clientY - cy; // positive down, negative up

        // Clamped ratio: top (dy = -innerRingRadius) -> 1, bottom (dy = +innerRingRadius) -> 0
        const span = innerRingRadius * 1.6;
        const ratio = Math.max(0, Math.min(1, (innerRingRadius * 0.8 - dy) / span));
        const nextL = Number((minL + ratio * (maxL - minL)).toFixed(3));
        setLightness(nextL);

        const effectiveChroma = mode === "accent" ? Math.max(0.12, chroma) : 0.025;
        const newHex = oklchToHex(nextL, effectiveChroma, hue);
        onChange(newHex);
      },
      [chroma, hue, innerRingRadius, maxL, minL, mode, onChange]
    );

    // Pointer Events on Donut
    const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
      if (!donutRef.current) return;
      const rect = donutRef.current.getBoundingClientRect();
      const cx = rect.left + rect.width / 2;
      const cy = rect.top + rect.height / 2;
      const dx = e.clientX - cx;
      const dy = e.clientY - cy;
      const dist = Math.sqrt(dx * dx + dy * dy);

      if (dist >= halfSize - 14) {
        // Outer Ring (Hue)
        draggingTargetRef.current = "hue";
        e.currentTarget.setPointerCapture(e.pointerId);
        updateHueFromPointer(e.clientX, e.clientY);
      } else if (dist >= halfSize - 30) {
        // Inner Ring (Lightness)
        draggingTargetRef.current = "lightness";
        e.currentTarget.setPointerCapture(e.pointerId);
        updateLightnessFromPointer(e.clientY);
      } else {
        // Center Core: Copy Hex
        void handleCenterCopy();
      }
    };

    const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
      if (!draggingTargetRef.current) return;
      if (draggingTargetRef.current === "hue") {
        updateHueFromPointer(e.clientX, e.clientY);
      } else {
        updateLightnessFromPointer(e.clientY);
      }
    };

    const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
      if (draggingTargetRef.current) {
        draggingTargetRef.current = null;
        try {
          e.currentTarget.releasePointerCapture(e.pointerId);
        } catch {
          // ignore
        }
        onCommit();
      }
    };

    const handleCenterCopy = async () => {
      const ok = await copyToClipboard(hex);
      if (ok) {
        setCopied(true);
        setTimeout(() => setCopied(false), 1400);
      }
    };

    // Calculate Outer Ring Thumb (Hue)
    const hueAngleRad = ((hue - 90) * Math.PI) / 180;
    const outerThumbX = halfSize + outerRingRadius * Math.cos(hueAngleRad);
    const outerThumbY = halfSize + outerRingRadius * Math.sin(hueAngleRad);

    // Calculate Inner Ring Thumb (Lightness)
    const lRatio = Math.max(0, Math.min(1, (lightness - minL) / (maxL - minL)));
    // Sweep along the right-hand arc of the inner ring: angle from 180deg (bottom) to 0deg (top)
    const innerAngleRad = Math.PI - lRatio * Math.PI; // from PI down to 0
    const innerThumbX = halfSize + innerRingRadius * Math.sin(innerAngleRad);
    const innerThumbY = halfSize + innerRingRadius * Math.cos(innerAngleRad);

    // Gradients
    const outerConic =
      mode === "accent"
        ? ACCENT_HUE_CONIC
        : isLight
        ? LIGHT_BG_CONIC
        : DARK_BG_CONIC;

    const innerStartHex = oklchToHex(minL, mode === "accent" ? 0.14 : 0.02, hue);
    const innerEndHex = oklchToHex(maxL, mode === "accent" ? 0.14 : 0.02, hue);

    return (
      <div className={cn("flex flex-col items-center select-none", className)}>
        {/* Standard Two-Ring Donut Color Picker */}
        <div
          ref={donutRef}
          onPointerDown={handlePointerDown}
          onPointerMove={handlePointerMove}
          onPointerUp={handlePointerUp}
          onPointerCancel={handlePointerUp}
          className="relative rounded-full cursor-crosshair touch-none shadow-xs"
          style={{
            width: `${size}px`,
            height: `${size}px`,
          }}
          role="slider"
          aria-label={`${mode === "accent" ? "Accent" : "Background"} 2-ring color picker`}
          aria-valuenow={hue}
          tabIndex={0}
        >
          {/* Ring 1 (Outer Ring): 360° Hue */}
          <div
            className="absolute inset-0 rounded-full"
            style={{ background: outerConic }}
          />

          {/* Track Gap between Ring 1 and Ring 2 */}
          <div
            className="absolute rounded-full bg-[rgb(var(--card))]"
            style={{
              top: "12px",
              left: "12px",
              right: "12px",
              bottom: "12px",
            }}
          />

          {/* Ring 2 (Inner Ring): Lightness / Tone Sweep */}
          <div
            className="absolute rounded-full"
            style={{
              top: "15px",
              left: "15px",
              right: "15px",
              bottom: "15px",
              background: `linear-gradient(to top, ${innerStartHex}, ${innerEndHex})`,
            }}
          />

          {/* Track Gap between Ring 2 and Center Core */}
          <div
            className="absolute rounded-full bg-[rgb(var(--card))]"
            style={{
              top: "27px",
              left: "27px",
              right: "27px",
              bottom: "27px",
            }}
          />

          {/* Center Core: Live Color Swatch (Click to Copy) */}
          <button
            type="button"
            onClick={handleCenterCopy}
            className="absolute rounded-full border border-[rgba(var(--border),0.2)] shadow-inner flex items-center justify-center cursor-pointer transition-transform active:scale-95 group"
            style={{
              top: "30px",
              left: "30px",
              right: "30px",
              bottom: "30px",
              backgroundColor: hex,
            }}
            aria-label={`Current color ${hex}, click to copy`}
          >
            {copied ? (
              <Check size={13} className="text-white drop-shadow-md" />
            ) : (
              <Copy
                size={11}
                className="text-white/0 group-hover:text-white/90 drop-shadow-xs transition-colors"
              />
            )}
          </button>

          {/* Outer Ring Thumb Indicator (Hue) */}
          <div
            className="absolute w-3.5 h-3.5 rounded-full bg-white border-2 border-black/50 shadow-md pointer-events-none -translate-x-1/2 -translate-y-1/2"
            style={{
              left: `${outerThumbX}px`,
              top: `${outerThumbY}px`,
            }}
          />

          {/* Inner Ring Thumb Indicator (Lightness) */}
          <div
            className="absolute w-2.5 h-2.5 rounded-full bg-white border border-black/50 shadow-xs pointer-events-none -translate-x-1/2 -translate-y-1/2"
            style={{
              left: `${innerThumbX}px`,
              top: `${innerThumbY}px`,
            }}
          />
        </div>
      </div>
    );
  }
);

OklchDonutPicker.displayName = "OklchDonutPicker";
