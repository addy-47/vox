import { memo, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";

/**
 * Two-column full-bleed settings pane with strict 65-35 column split.
 * Left side (65%): Title and concise description text.
 * Right side (35%): Compact 2x2 grid of controls, custom widget, or clean vector SVG.
 */
export interface SettingsTabPaneProps {
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

export const SettingsTabPane = memo(
  ({
    icon: Icon,
    title,
    description,
    rightSlot,
    controls,
    emptyGraphic,
    aside,
    layoutMode,
  }: SettingsTabPaneProps) => {
    const isSmall = layoutMode === "small";
    const resolvedRightContent = controls ? (
      <div className="grid grid-cols-2 gap-1.5 w-full max-w-[184px]">
        {controls}
      </div>
    ) : rightSlot ? (
      <div className="w-full flex items-center justify-center">
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
          "w-full flex select-none animate-fade-in",
          isSmall
            ? "flex-col gap-3 p-2.5 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)]"
            : "flex-row items-center justify-between gap-4 h-full p-2.5 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)]"
        )}
      >
        {/* Left Column (65%): Title & Concise Description */}
        <div
          className={cn(
            "min-w-0 flex flex-col justify-center gap-1",
            isSmall ? "w-full" : "w-[65%]"
          )}
        >
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
          <div
            className={cn(
              "shrink-0 flex items-center justify-center",
              isSmall ? "w-full py-1" : "w-[35%]"
            )}
          >
            {resolvedRightContent}
          </div>
        )}
      </div>
    );
  }
);

SettingsTabPane.displayName = "SettingsTabPane";

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
  placeholder: string;
  selected: boolean;
  inputMode?: "numeric" | "decimal" | "text";
  ariaLabel?: string;
}

export const PresetInput = memo(
  ({
    value,
    onChange,
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

export const RemoteComputeGraphic = memo(
  ({ label = "Remote Server", subLabel = "Zero Local RAM" }: GraphicProps) => (
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
          <circle cx="26" cy="14" r="1.75" fill="#34d399" />
          <circle cx="26" cy="14" r="3.5" stroke="#34d399" strokeWidth="0.6" strokeOpacity="0.4" />
        </svg>
      </div>
      <div className="flex flex-col items-center leading-none gap-0.5 mt-2">
        <span className="text-[8.5px] font-mono font-bold tracking-[0.14em] uppercase text-[rgb(var(--accent))]">
          {label}
        </span>
        <span className="text-[7.5px] font-mono font-medium tracking-[0.06em] uppercase text-[rgb(var(--foreground-muted))]/55">
          {subLabel}
        </span>
      </div>
    </div>
  )
);

RemoteComputeGraphic.displayName = "RemoteComputeGraphic";

export const ManagedContextGraphic = memo(
  ({ label = "Remote Managed", subLabel = "Dynamic Alloc" }: GraphicProps) => (
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
      <div className="flex flex-col items-center leading-none gap-0.5 mt-2">
        <span className="text-[8.5px] font-mono font-bold tracking-[0.14em] uppercase text-[rgb(var(--accent))]">
          {label}
        </span>
        <span className="text-[7.5px] font-mono font-medium tracking-[0.06em] uppercase text-[rgb(var(--foreground-muted))]/55">
          {subLabel}
        </span>
      </div>
    </div>
  )
);

ManagedContextGraphic.displayName = "ManagedContextGraphic";
