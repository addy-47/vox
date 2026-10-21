import { memo, useCallback } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import { cn } from "@/shared/lib/utils";
import { TRANSLITERATION_COPY } from "@/data/settingsCopy";

interface TransliterationToggleProps {
  enabled: boolean;
  onToggle: () => void;
  label: string;
  ariaLabel: string;
}

/**
 * Script transliteration switch. Lifted verbatim from the models card
 * (`speech/SttModelList.tsx`) when the control moved to the interaction desk.
 * The wireframe drawing is unchanged — only the panel around it is new.
 */
const TransliterationToggle = memo(
  ({ enabled, onToggle, label, ariaLabel }: TransliterationToggleProps) => (
    <button
      type="button"
      role="switch"
      aria-checked={enabled}
      aria-label={ariaLabel}
      onClick={onToggle}
      className={cn(
        "group flex flex-col items-center justify-center shrink-0 select-none cursor-pointer rounded-2xl px-3 py-2 transition-all duration-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-[rgb(var(--accent))]",
        enabled
          ? "text-[rgb(var(--accent))]"
          : "text-[rgb(var(--foreground-muted))]/40 hover:text-[rgb(var(--foreground-muted))]/70"
      )}
    >
      <div
        className="relative w-[76px] h-[46px] transition-transform duration-300 group-hover:scale-105"
        style={{
          animation: enabled ? "wm-globe-float 3.6s ease-in-out infinite" : "none",
        }}
      >
        <svg
          viewBox="0 0 76 46"
          className="w-full h-full overflow-visible select-none"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          {enabled ? (
            <g className="transition-all duration-300 animate-fade-in">
              <rect
                x="3"
                y="3"
                width="70"
                height="40"
                rx="10"
                stroke="currentColor"
                strokeWidth="1.1"
                className="opacity-70"
              />
              <text
                x="20"
                y="24"
                textAnchor="middle"
                dominantBaseline="central"
                fontSize="17"
                fontWeight="700"
                fill="currentColor"
                className="select-none"
                style={{ fontFamily: "'Noto Sans Devanagari', 'Mukta', sans-serif" }}
              >
                अ
              </text>
              <g className="opacity-80">
                <line
                  x1="32"
                  y1="23"
                  x2="44"
                  y2="23"
                  stroke="currentColor"
                  strokeWidth="1.2"
                  strokeDasharray="2 1.5"
                />
                <path
                  d="M 41 20 L 45 23 L 41 26"
                  stroke="currentColor"
                  strokeWidth="1.25"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                />
              </g>
              <text
                x="56"
                y="23"
                textAnchor="middle"
                dominantBaseline="central"
                fontSize="17"
                fontWeight="800"
                fill="currentColor"
                className="select-none"
                style={{ fontFamily: "monospace, system-ui, sans-serif" }}
              >
                a
              </text>
              <circle cx="20" cy="38" r="1.2" fill="currentColor" opacity="0.6" />
              <circle cx="56" cy="38" r="1.2" fill="currentColor" opacity="0.6" />
            </g>
          ) : (
            <g className="transition-all duration-300">
              <rect
                x="14"
                y="3"
                width="48"
                height="40"
                rx="10"
                stroke="currentColor"
                strokeWidth="1.1"
                strokeDasharray="3 2"
                className="opacity-30"
              />
              <text
                x="38"
                y="23"
                textAnchor="middle"
                dominantBaseline="central"
                fontSize="19"
                fontWeight="700"
                fill="currentColor"
                className="opacity-55 select-none"
                style={{ fontFamily: "'Noto Sans Devanagari', 'Mukta', sans-serif" }}
              >
                अ
              </text>
            </g>
          )}
        </svg>
      </div>
      <span
        className={cn(
          "mt-2 px-2.5 py-0.5 rounded-full text-[9px] font-mono font-bold uppercase tracking-[0.14em] leading-none transition-all duration-300",
          enabled
            ? "text-[rgb(var(--accent))]"
            : "text-[rgb(var(--foreground-muted))]/50"
        )}
      >
        {label}
      </span>
    </button>
  )
);

TransliterationToggle.displayName = "TransliterationToggle";

/**
 * The interaction desk panel for script transliteration. It owns the
 * `stt.transliterate_enabled` key, so the interaction card reports the dirty
 * state and the models card does not.
 */
export const TransliterationPanel = memo(() => {
  const draftSettings = useSettingsStore((s) => s.draftSettings);
  const updateDraft = useSettingsStore((s) => s.updateDraft);

  const enabled = draftSettings?.stt.transliterate_enabled ?? true;

  const handleToggle = useCallback(() => {
    // The `stt` scope maps to the models card by default (model selection);
    // this control lives in the interaction card, so the domain is explicit.
    updateDraft("stt", "transliterate_enabled", !enabled, "interaction");
  }, [enabled, updateDraft]);

  return (
    <div className="flex flex-row items-center justify-between gap-4 h-full p-2 sm:p-3 rounded-xl bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)] animate-fade-in">
      <div className="flex flex-col gap-1 min-w-0 flex-1">
        <span className="text-[12px] font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
          {TRANSLITERATION_COPY.title}
        </span>
        <p className="text-[11px] sm:text-[11.5px] text-[rgb(var(--foreground-muted))]/75 leading-relaxed font-medium">
          {TRANSLITERATION_COPY.description}
        </p>
      </div>
      <div className="shrink-0 w-[116px] flex flex-col items-center justify-center">
        <TransliterationToggle
          enabled={enabled}
          onToggle={handleToggle}
          label={enabled ? TRANSLITERATION_COPY.enabled : TRANSLITERATION_COPY.disabled}
          ariaLabel={TRANSLITERATION_COPY.toggleAria}
        />
      </div>
    </div>
  );
});

TransliterationPanel.displayName = "TransliterationPanel";
