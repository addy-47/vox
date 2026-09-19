import { memo } from "react";

export const HelpHistoryDiagram = memo(() => {
  return (
    <div className="flex flex-col gap-2.5 py-1">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Timeline Orbit Navigation
        </span>
        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
          Drag or press [ and ]
        </span>
      </div>

      {/* SVG Canvas */}
      <div className="relative w-full h-32 rounded-xl overflow-hidden flex items-center justify-center">
        <svg viewBox="0 0 200 130" className="w-44 h-32">
          {/* Orbital timeline track */}
          <ellipse
            cx="100"
            cy="65"
            rx="75"
            ry="42"
            fill="none"
            stroke="rgba(var(--accent), 0.25)"
            strokeWidth="1.5"
            strokeDasharray="4 6"
          />

          {/* Central Clock Hub */}
          <circle cx="100" cy="65" r="18" fill="rgba(var(--card), 0.95)" stroke="rgb(var(--accent))" strokeWidth="1.5" />
          <text x="100" y="69" textAnchor="middle" fill="rgb(var(--accent))" fontSize="9" fontWeight="bold" fontFamily="monospace">
            TODAY
          </text>

          {/* Rotating Sessions on Orbit */}
          <g
            className="animate-spin origin-center"
            style={{ transformOrigin: "100px 65px", animationDuration: "24s" }}
          >
            {/* Session Node 1 */}
            <g transform="translate(165, 54)">
              <circle r="9" fill="rgba(var(--accent), 0.25)" stroke="rgb(var(--accent))" strokeWidth="1.5" />
              <circle r="4" fill="rgb(var(--accent))" />
            </g>
            {/* Session Node 2 */}
            <g transform="translate(35, 76)">
              <circle r="12" fill="rgba(56, 189, 248, 0.25)" stroke="#38bdf8" strokeWidth="1.5" />
              <circle r="5" fill="#38bdf8" />
            </g>
            {/* Session Node 3 */}
            <g transform="translate(100, 23)">
              <circle r="8" fill="rgba(167, 139, 250, 0.25)" stroke="#a78bfa" strokeWidth="1.5" />
              <circle r="3.5" fill="#a78bfa" />
            </g>
          </g>
        </svg>
      </div>
    </div>
  );
});

HelpHistoryDiagram.displayName = "HelpHistoryDiagram";
