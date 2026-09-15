import { memo } from "react";
import { motion } from "framer-motion";
import { Orbit } from "lucide-react";

export const HelpHistoryDiagram = memo(() => {
  return (
    <div className="rounded-2xl border border-[rgba(var(--border),0.12)] bg-[rgba(var(--card),0.7)] p-4 flex flex-col gap-3 backdrop-blur-md">
      <div className="flex items-center justify-between">
        <span className="text-[12px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--accent))] flex items-center gap-2">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          Timeline Orbit Navigation
        </span>
        <span className="text-[11px] text-[rgb(var(--foreground-muted))]">
          Drag or press [ and ]
        </span>
      </div>

      {/* SVG Canvas */}
      <div className="relative w-full h-36 rounded-xl bg-[rgba(var(--foreground),0.03)] border border-[rgba(var(--border),0.1)] overflow-hidden flex items-center justify-center">
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
          <motion.g
            animate={{ rotate: 360 }}
            transition={{ duration: 24, repeat: Infinity, ease: "linear" }}
            style={{ transformOrigin: "100px 65px" }}
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
          </motion.g>
        </svg>

        {/* Orbit scrubbing pill */}
        <div className="absolute bottom-2 px-3 py-1 rounded-full bg-[rgba(var(--card),0.9)] border border-[rgba(var(--border),0.15)] shadow-xs backdrop-blur-md flex items-center gap-1.5 text-[11px] text-[rgb(var(--foreground))]">
          <Orbit size={12} className="text-[rgb(var(--accent))]" />
          <span>Click any circle to reopen that conversation</span>
        </div>
      </div>
    </div>
  );
});

HelpHistoryDiagram.displayName = "HelpHistoryDiagram";
