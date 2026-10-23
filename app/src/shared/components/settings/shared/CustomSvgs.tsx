import { memo, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";

/**
 * Two-column full-bleed settings pane with strict 65-35 column split.
 * Left side (65%): Title and concise description text.
 * Right side (35%): Compact 2x2 grid of controls, custom widget, or clean vector SVG.
 */
export interface CustomSvgsProps {
  icon?: LucideIcon;
  title: string;
  description?: ReactNode;
  rightSlot?: ReactNode;
  controls?: ReactNode;
  emptyGraphic?: ReactNode;
  layoutMode?: "full-max" | "full-min" | "small";
  /** @deprecated Keep optional for backwards compatibility, not rendered */
  value?: ReactNode;
  /** @deprecated Keep optional for backwards compatibility, not rendered in header */
  aside?: ReactNode;
}

export const CustomSvgs = memo(
  ({
    icon: Icon,
    title,
    description,
    rightSlot,
    controls,
    emptyGraphic,
    aside,
    layoutMode,
  }: CustomSvgsProps) => {
    const isSmall = layoutMode === "small";
    const resolvedRightContent = controls ? (
      <div className="grid grid-cols-2 gap-1.5 w-full max-w-[184px]">
        {controls}
      </div>
    ) : rightSlot ? (
      <div className="w-full min-w-0 flex items-center justify-center">
        {rightSlot}
      </div>
    ) : emptyGraphic ? (
      <div className="w-full flex items-center justify-center">
        {emptyGraphic}
      </div>
    ) : aside ? (
      <div className="w-full flex items-center justify-center">
        {aside}
      </div>
    ) : null;

    return (
      <div
        className={cn(
          "w-full flex flex-row items-center justify-between gap-4 select-none animate-fade-in p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)]",
          isSmall ? "h-auto" : "h-full"
        )}
      >
        {/* Left Column (65%): Title & Concise Description */}
        <div className="min-w-0 flex flex-col justify-center gap-1 w-[65%]">
          <div className="flex items-center gap-2 min-w-0">
            {Icon && <Icon size={14} className="text-[rgb(var(--accent))] shrink-0" />}
            <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))] truncate">
              {title}
            </span>
          </div>
          {description && (
            <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
              {description}
            </p>
          )}
        </div>

        {/* Right Column (35%): Vertically Centered Controls or SVG Wireframe */}
        {resolvedRightContent && (
          <div className="shrink-0 flex items-center justify-center w-[35%]">
            {resolvedRightContent}
          </div>
        )}
      </div>
    );
  }
);

CustomSvgs.displayName = "custom-svgs";

/* ── Preset control primitives (Compact 32px height, non-stretching) ── */

export interface PresetButtonProps {
  selected: boolean;
  onClick?: () => void;
  mono?: boolean;
  children: ReactNode;
  className?: string;
}

export const PresetButton = memo(
  ({ selected, onClick, mono = true, children, className }: PresetButtonProps) => (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        "w-full h-[32px] py-1 px-1.5 rounded-lg border text-[10.5px] sm:text-[11px] font-bold transition-all duration-200 cursor-pointer flex items-center justify-center gap-1 shrink-0",
        mono && "font-mono",
        selected
          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] text-[rgb(var(--accent))] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[rgb(var(--foreground-muted))]/80 hover:border-[rgba(var(--accent),0.2)] hover:text-[rgb(var(--foreground))]",
        className
      )}
    >
      {children}
    </button>
  )
);

PresetButton.displayName = "PresetButton";

export interface PresetCellProps {
  children: ReactNode;
  className?: string;
}

export const PresetCell = memo(({ children, className }: PresetCellProps) => (
  <div
    className={cn(
      "w-full h-[32px] py-1 px-1.5 rounded-lg border border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] text-[10.5px] sm:text-[11px] font-mono font-bold text-[rgb(var(--foreground-muted))]/70 flex items-center justify-center shrink-0",
      className
    )}
  >
    {children}
  </div>
));

PresetCell.displayName = "PresetCell";

export interface PresetInputProps {
  value: string;
  onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
  onBlur?: (e: React.FocusEvent<HTMLInputElement>) => void;
  onKeyDown?: (e: React.KeyboardEvent<HTMLInputElement>) => void;
  placeholder: string;
  selected: boolean;
  inputMode?: "numeric" | "decimal" | "text";
  ariaLabel?: string;
}

export const PresetInput = memo(
  ({
    value,
    onChange,
    onBlur,
    onKeyDown,
    placeholder,
    selected,
    inputMode = "numeric",
    ariaLabel,
  }: PresetInputProps) => (
    <div
      className={cn(
        "w-full h-[32px] rounded-lg border flex items-center justify-center transition-all overflow-hidden shrink-0",
        selected
          ? "border-[rgb(var(--accent))] bg-[rgba(var(--accent),0.15)] shadow-[0_0_12px_rgba(var(--accent),0.25)]"
          : "border-[rgba(var(--accent),0.08)] bg-[rgba(var(--foreground),0.02)] focus-within:border-[rgba(var(--accent),0.35)]"
      )}
    >
      <input
        type="text"
        inputMode={inputMode}
        value={value}
        onChange={onChange}
        onBlur={onBlur}
        onKeyDown={onKeyDown}
        placeholder={placeholder}
        aria-label={ariaLabel}
        className={cn(
          "w-full text-center text-[10.5px] sm:text-[11px] font-mono font-bold bg-transparent outline-none py-1 appearance-none [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none",
          selected ? "text-[rgb(var(--accent))]" : "text-[rgb(var(--foreground))]",
          "placeholder:text-[rgb(var(--foreground-muted))]/40 placeholder:font-sans placeholder:font-normal"
        )}
      />
    </div>
  )
);

PresetInput.displayName = "PresetInput";

/* ── Clean Vector Wireframe Graphics (Zero pill shapes, pure architectural linework) ── */

export interface GraphicProps {
  label?: string;
  subLabel?: string;
}

export const RemoteComputeGraphic = memo(() => (
    <div className="flex flex-col items-center justify-center select-none text-[rgb(var(--accent))]">
      <div className="relative w-[48px] h-[40px] flex items-center justify-center">
        <svg
          viewBox="0 0 52 44"
          className="w-full h-full overflow-visible"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          {/* Isometric Blade Chassis Top Polygon */}
          <polygon
            points="26,4 46,14 26,24 6,14"
            stroke="currentColor"
            strokeWidth="1.15"
            strokeOpacity="0.85"
          />
          {/* Left Isometric Face */}
          <polygon
            points="6,14 26,24 26,38 6,28"
            stroke="currentColor"
            strokeWidth="1.1"
            strokeOpacity="0.75"
          />
          {/* Right Isometric Face */}
          <polygon
            points="26,24 46,14 46,28 26,38"
            stroke="currentColor"
            strokeWidth="1.1"
            strokeOpacity="0.75"
          />
          {/* Internal Blade Slots & Bus Traces */}
          <line
            x1="11"
            y1="20"
            x2="21"
            y2="25"
            stroke="currentColor"
            strokeWidth="0.8"
            strokeOpacity="0.4"
          />
          <line
            x1="11"
            y1="24"
            x2="21"
            y2="29"
            stroke="currentColor"
            strokeWidth="0.8"
            strokeOpacity="0.4"
          />
          <line
            x1="31"
            y1="25"
            x2="41"
            y2="20"
            stroke="currentColor"
            strokeWidth="0.8"
            strokeOpacity="0.4"
          />
          <line
            x1="31"
            y1="29"
            x2="41"
            y2="24"
            stroke="currentColor"
            strokeWidth="0.8"
            strokeOpacity="0.4"
          />
          {/* Active Node Telemetry LED */}
          <circle cx="26" cy="14" r="1.75" fill="rgb(var(--success))" />
          <circle cx="26" cy="14" r="3.5" stroke="rgb(var(--success))" strokeWidth="0.6" strokeOpacity="0.4" />
        </svg>
      </div>
    </div>
  )
);

RemoteComputeGraphic.displayName = "RemoteComputeGraphic";

export const ManagedContextGraphic = memo(() => (
  <div className="flex flex-col items-center justify-center select-none text-[rgb(var(--accent))]">
    <div className="relative w-[48px] h-[40px] flex items-center justify-center">
      <svg
        viewBox="0 0 52 44"
        className="w-full h-full overflow-visible"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        {/* Stacked Memory Context Layer 1 (Top) */}
        <polygon
          points="26,4 44,13 26,22 8,13"
          stroke="currentColor"
          strokeWidth="1.15"
          strokeOpacity="0.85"
        />
        {/* Stacked Memory Context Layer 2 (Middle) */}
        <polygon
          points="26,13 44,22 26,31 8,22"
          stroke="currentColor"
          strokeWidth="0.9"
          strokeDasharray="3 2"
          strokeOpacity="0.55"
        />
        {/* Stacked Memory Context Layer 3 (Bottom) */}
        <polygon
          points="26,22 44,31 26,40 8,31"
          stroke="currentColor"
          strokeWidth="0.8"
          strokeDasharray="3 2"
          strokeOpacity="0.35"
        />
        {/* Attention Vector Beams Linking Layers */}
        <line
          x1="26"
          y1="4"
          x2="26"
          y2="22"
          stroke="currentColor"
          strokeWidth="0.8"
          strokeDasharray="2 2"
          strokeOpacity="0.5"
        />
        <line
          x1="8"
          y1="13"
          x2="8"
          y2="31"
          stroke="currentColor"
          strokeWidth="0.8"
          strokeDasharray="2 2"
          strokeOpacity="0.3"
        />
        <line
          x1="44"
          y1="13"
          x2="44"
          y2="31"
          stroke="currentColor"
          strokeWidth="0.8"
          strokeDasharray="2 2"
          strokeOpacity="0.3"
        />
        {/* Active Attention Focal Node */}
        <circle cx="26" cy="13" r="2" fill="#38bdf8" />
        <circle cx="26" cy="13" r="3.75" stroke="#38bdf8" strokeWidth="0.6" strokeOpacity="0.4" />
      </svg>
    </div>
  </div>
));

ManagedContextGraphic.displayName = "ManagedContextGraphic";

export const ComingSoonGraphic = memo(
  ({ label = "Coming Soon" }: { label?: string }) => (
  <div className="flex flex-col items-center justify-center select-none text-[rgb(var(--accent))]">
    <div className="relative w-[48px] h-[40px] flex items-center justify-center">
      <svg
        viewBox="0 0 52 44"
        className="w-full h-full overflow-visible"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        {/* Isometric Blueprint Construction Module under Assembly */}
        <polygon
          points="26,6 42,15 26,24 10,15"
          stroke="currentColor"
          strokeWidth="1.15"
          strokeOpacity="0.85"
        />
        <polygon
          points="10,15 26,24 26,36 10,27"
          stroke="currentColor"
          strokeWidth="1.0"
          strokeOpacity="0.7"
        />
        <polygon
          points="26,24 42,15 42,27 26,36"
          stroke="currentColor"
          strokeWidth="1.0"
          strokeOpacity="0.7"
        />
        {/* Projection dashed lines to future expansion bounds */}
        <line
          x1="26"
          y1="2"
          x2="26"
          y2="6"
          stroke="currentColor"
          strokeWidth="0.8"
          strokeDasharray="2 2"
          strokeOpacity="0.4"
        />
        <line
          x1="6"
          y1="13"
          x2="10"
          y2="15"
          stroke="currentColor"
          strokeWidth="0.8"
          strokeDasharray="2 2"
          strokeOpacity="0.4"
        />
        <line
          x1="46"
          y1="13"
          x2="42"
          y2="15"
          stroke="currentColor"
          strokeWidth="0.8"
          strokeDasharray="2 2"
          strokeOpacity="0.4"
        />
        {/* Internal alignment crosshair */}
        <line
          x1="26"
          y1="12"
          x2="26"
          y2="18"
          stroke="currentColor"
          strokeWidth="0.75"
          strokeOpacity="0.6"
        />
        <line
          x1="21"
          y1="15"
          x2="31"
          y2="15"
          stroke="currentColor"
          strokeWidth="0.75"
          strokeOpacity="0.6"
        />
        {/* Central telemetry focal node */}
        <circle cx="26" cy="15" r="1.5" fill="rgb(var(--accent))" />
        <circle cx="26" cy="15" r="3.2" stroke="currentColor" strokeWidth="0.6" strokeDasharray="2 1" strokeOpacity="0.4" />
      </svg>
    </div>
    <div className="flex flex-col items-center leading-none mt-2">
        <span className="text-[8.5px] font-mono font-bold tracking-[0.14em] uppercase text-[rgb(var(--accent))]">
          {label}
        </span>
      </div>
  </div>
));

ComingSoonGraphic.displayName = "ComingSoonGraphic";

export const EmbeddedComputeGraphic = memo(
  ({ label = "On Device AI" }: { label?: string }) => (
    <div className="flex flex-col items-center justify-center select-none text-[rgb(var(--accent))]">
      <div className="relative w-[48px] h-[40px] flex items-center justify-center">
        <svg
          viewBox="0 0 52 44"
          className="w-full h-full overflow-visible"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          {/* Central Neural Chip Die */}
          <rect
            x="16"
            y="12"
            width="20"
            height="20"
            rx="2.5"
            stroke="currentColor"
            strokeWidth="1.2"
            strokeOpacity="0.85"
          />
          {/* Inner Core Matrix */}
          <rect
            x="20"
            y="16"
            width="12"
            height="12"
            rx="1.5"
            stroke="currentColor"
            strokeWidth="0.85"
            strokeDasharray="2 1"
            strokeOpacity="0.5"
          />
          {/* Die Center Pulse Core */}
          <circle cx="26" cy="22" r="2" fill="rgb(var(--accent))" />
          {/* Top Bus Traces */}
          <line x1="21" y1="5" x2="21" y2="12" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.6" />
          <line x1="26" y1="4" x2="26" y2="12" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.75" />
          <line x1="31" y1="5" x2="31" y2="12" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.6" />
          {/* Bottom Bus Traces */}
          <line x1="21" y1="32" x2="21" y2="39" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.6" />
          <line x1="26" y1="32" x2="26" y2="40" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.75" />
          <line x1="31" y1="32" x2="31" y2="39" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.6" />
          {/* Left Bus Traces */}
          <line x1="9" y1="17" x2="16" y2="17" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.6" />
          <line x1="8" y1="22" x2="16" y2="22" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.75" />
          <line x1="9" y1="27" x2="16" y2="27" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.6" />
          {/* Right Bus Traces */}
          <line x1="36" y1="17" x2="43" y2="17" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.6" />
          <line x1="36" y1="22" x2="44" y2="22" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.75" />
          <line x1="36" y1="27" x2="43" y2="27" stroke="currentColor" strokeWidth="0.9" strokeOpacity="0.6" />
        </svg>
      </div>
      <div className="flex flex-col items-center leading-none mt-2">
        <span className="text-[8.5px] font-mono font-bold tracking-[0.14em] uppercase text-[rgb(var(--accent))]">
          {label}
        </span>
      </div>
    </div>
  )
);

EmbeddedComputeGraphic.displayName = "EmbeddedComputeGraphic";

export const BuiltinVoiceGraphic = memo(() => (
  <div className="flex flex-col items-center justify-center select-none text-[rgb(var(--accent))]">
    <div className="relative w-[48px] h-[40px] flex items-center justify-center">
      <svg
        viewBox="0 0 52 44"
        className="w-full h-full overflow-visible"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
      >
        {/* Concentric Acoustic Emission Arcs */}
        <path
          d="M 26 8 A 14 14 0 0 1 26 36"
          stroke="currentColor"
          strokeWidth="1.15"
          strokeOpacity="0.8"
          strokeLinecap="round"
        />
        <path
          d="M 26 13 A 9 9 0 0 1 26 31"
          stroke="currentColor"
          strokeWidth="1.1"
          strokeOpacity="0.6"
          strokeLinecap="round"
        />
        <path
          d="M 26 18 A 4 4 0 0 1 26 26"
          stroke="currentColor"
          strokeWidth="1.1"
          strokeOpacity="0.45"
          strokeLinecap="round"
        />
        {/* Left Symmetrical Mirror Acoustic Arcs */}
        <path
          d="M 26 8 A 14 14 0 0 0 26 36"
          stroke="currentColor"
          strokeWidth="1.15"
          strokeOpacity="0.8"
          strokeLinecap="round"
        />
        <path
          d="M 26 13 A 9 9 0 0 0 26 31"
          stroke="currentColor"
          strokeWidth="1.1"
          strokeOpacity="0.6"
          strokeLinecap="round"
        />
        <path
          d="M 26 18 A 4 4 0 0 0 26 26"
          stroke="currentColor"
          strokeWidth="1.1"
          strokeOpacity="0.45"
          strokeLinecap="round"
        />
        {/* Center Voice Node */}
        <circle cx="26" cy="22" r="2.25" fill="rgb(var(--accent))" />
        <circle cx="26" cy="22" r="4.25" stroke="currentColor" strokeWidth="0.6" strokeOpacity="0.4" />
      </svg>
    </div>
  </div>
));

BuiltinVoiceGraphic.displayName = "BuiltinVoiceGraphic";

