import { memo, useId, useMemo, type ReactNode } from "react";
import { cn } from "@/shared/lib/utils";

export interface KeyboardHotkeySkeletonProps {
  hotkey: string;
  isRecording?: boolean;
  recordingPlaceholder?: string;
  onEdit?: () => void;
  className?: string;
  bottomSlot?: ReactNode;
}

interface KeycapSpec {
  id: string;
  w: number;
}

const ROWS: KeycapSpec[][] = [
  // Row 0: Esc + Numbers + Backspace (Total width ~ 172)
  [
    { id: "escape", w: 10 },
    { id: "1", w: 10.5 },
    { id: "2", w: 10.5 },
    { id: "3", w: 10.5 },
    { id: "4", w: 10.5 },
    { id: "5", w: 10.5 },
    { id: "6", w: 10.5 },
    { id: "7", w: 10.5 },
    { id: "8", w: 10.5 },
    { id: "9", w: 10.5 },
    { id: "0", w: 10.5 },
    { id: "-", w: 10.5 },
    { id: "=", w: 10.5 },
    { id: "backspace", w: 18 },
  ],
  // Row 1: Tab + QWERTY + Backslash
  [
    { id: "tab", w: 14.5 },
    { id: "q", w: 10.5 },
    { id: "w", w: 10.5 },
    { id: "e", w: 10.5 },
    { id: "r", w: 10.5 },
    { id: "t", w: 10.5 },
    { id: "y", w: 10.5 },
    { id: "u", w: 10.5 },
    { id: "i", w: 10.5 },
    { id: "o", w: 10.5 },
    { id: "p", w: 10.5 },
    { id: "[", w: 10.5 },
    { id: "]", w: 10.5 },
    { id: "\\", w: 13.5 },
  ],
  // Row 2: Caps + ASDF + Enter
  [
    { id: "capslock", w: 17.5 },
    { id: "a", w: 10.5 },
    { id: "s", w: 10.5 },
    { id: "d", w: 10.5 },
    { id: "f", w: 10.5 },
    { id: "g", w: 10.5 },
    { id: "h", w: 10.5 },
    { id: "j", w: 10.5 },
    { id: "k", w: 10.5 },
    { id: "l", w: 10.5 },
    { id: ";", w: 10.5 },
    { id: "'", w: 10.5 },
    { id: "enter", w: 21 },
  ],
  // Row 3: Shift + ZXCV + Shift
  [
    { id: "shift_l", w: 22 },
    { id: "z", w: 10.5 },
    { id: "x", w: 10.5 },
    { id: "c", w: 10.5 },
    { id: "v", w: 10.5 },
    { id: "b", w: 10.5 },
    { id: "n", w: 10.5 },
    { id: "m", w: 10.5 },
    { id: ",", w: 10.5 },
    { id: ".", w: 10.5 },
    { id: "/", w: 10.5 },
    { id: "shift_r", w: 27 },
  ],
  // Row 4: Modifiers + Spacebar
  [
    { id: "ctrl_l", w: 14 },
    { id: "super", w: 13 },
    { id: "alt_l", w: 13 },
    { id: "space", w: 68 },
    { id: "alt_r", w: 13 },
    { id: "fn", w: 13 },
    { id: "ctrl_r", w: 14 },
  ],
];

/**
 * Normalizes a hotkey string (e.g. "Alt+V", "Ctrl+Shift+D") into active key IDs.
 */
function parseActiveKeyIds(hotkeyStr: string): Set<string> {
  const active = new Set<string>();
  if (!hotkeyStr) return active;

  const clean = hotkeyStr.replace(/\.\.\.$/, "").trim();
  const parts = clean.split("+").map((s) => s.trim().toLowerCase()).filter(Boolean);

  for (const p of parts) {
    if (p === "ctrl" || p === "control") {
      active.add("ctrl_l");
    } else if (p === "alt" || p === "opt" || p === "option") {
      active.add("alt_l");
    } else if (p === "shift") {
      active.add("shift_l");
    } else if (p === "super" || p === "cmd" || p === "command" || p === "win" || p === "meta") {
      active.add("super");
    } else if (p === "space") {
      active.add("space");
    } else if (p === "enter" || p === "return") {
      active.add("enter");
    } else if (p === "tab") {
      active.add("tab");
    } else if (p === "esc" || p === "escape") {
      active.add("escape");
    } else if (p === "backspace") {
      active.add("backspace");
    } else {
      active.add(p);
    }
  }

  return active;
}

export const KeyboardHotkeySkeleton = memo(({
  hotkey,
  isRecording = false,
  onEdit,
  className,
  bottomSlot,
}: KeyboardHotkeySkeletonProps) => {
  const filterId = useId();
  const activeKeyIds = useMemo(() => parseActiveKeyIds(hotkey), [hotkey]);

  const KEY_H = 8.2;
  const ROW_GAP = 1.3;
  const START_X = 4;
  const START_Y = 4.5;

  return (
    <div className={cn("flex flex-col items-center justify-center select-none", className)}>
      <svg
        viewBox="0 0 180 58"
        className={cn(
          "w-[180px] h-[58px] overflow-visible transition-transform duration-200",
          onEdit && "cursor-pointer hover:scale-[1.02]"
        )}
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        onClick={onEdit}
        role={onEdit ? "button" : undefined}
        tabIndex={onEdit ? 0 : -1}
        onKeyDown={(e) => {
          if (onEdit && (e.key === "Enter" || e.key === " ")) {
            e.preventDefault();
            onEdit();
          }
        }}
        aria-label={`Keyboard hotkey ${hotkey}`}
      >
        <defs>
          <filter id={`${filterId}-key-glow`} x="-30%" y="-30%" width="160%" height="160%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="1.8" result="blur" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {/* Outer Keyboard Chassis - Pure Hollow Outline */}
        <rect
          x="1"
          y="1"
          width="178"
          height="56"
          rx="4"
          fill="none"
          stroke={isRecording ? "rgb(var(--accent))" : "rgba(var(--foreground), 0.14)"}
          strokeWidth={isRecording ? "1.25" : "0.75"}
          strokeDasharray={isRecording ? "4 3" : undefined}
          className={cn("transition-all duration-300", isRecording && "animate-pulse")}
        />

        {/* Keyboard Keycap Rows - Pure Hollow Skeleton */}
        {ROWS.map((row, rowIdx) => {
          const y = START_Y + rowIdx * (KEY_H + ROW_GAP);
          let currentX = START_X;

          // Compute exact gaps between keys for this row
          const totalKeyWidth = row.reduce((sum, k) => sum + k.w, 0);
          const totalRowSpace = 172 - totalKeyWidth;
          const keyGap = row.length > 1 ? totalRowSpace / (row.length - 1) : 0;

          return (
            <g key={`row-${rowIdx}`}>
              {row.map((k) => {
                const x = currentX;
                currentX += k.w + keyGap;
                const isActive = activeKeyIds.has(k.id);

                return (
                  <rect
                    key={k.id}
                    x={x.toFixed(2)}
                    y={y.toFixed(2)}
                    width={k.w.toFixed(2)}
                    height={KEY_H.toFixed(2)}
                    rx="1.5"
                    fill="none"
                    stroke={isActive ? "rgb(var(--accent))" : "rgba(var(--foreground), 0.12)"}
                    strokeWidth={isActive ? "1.4" : "0.65"}
                    className={cn(
                      "transition-all duration-200",
                      isActive && "animate-pulse"
                    )}
                    filter={isActive ? `url(#${filterId}-key-glow)` : undefined}
                  />
                );
              })}
            </g>
          );
        })}
      </svg>

      {/* Optional bottom slot (e.g. combination text + pencil icon) */}
      {bottomSlot}
    </div>
  );
});

KeyboardHotkeySkeleton.displayName = "KeyboardHotkeySkeleton";
