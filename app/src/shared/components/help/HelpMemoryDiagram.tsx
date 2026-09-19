import { memo } from "react";

export const HelpMemoryDiagram = memo(() => {
  return (
    <div className="flex flex-col gap-2 py-1">
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-mono font-bold uppercase tracking-wider text-[rgb(var(--foreground-muted))] flex items-center gap-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-[rgb(var(--accent))]" />
          How Vox Remembers
        </span>
        <span className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/60">
          Private on your device
        </span>
      </div>

      {/* SVG Diagram */}
      <div className="relative w-full h-32 flex items-center justify-center py-1">
        <svg viewBox="0 0 240 130" className="w-56 h-32">
          {/* Connecting lines */}
          <line x1="120" y1="65" x2="55" y2="35" stroke="rgba(56, 189, 248, 0.35)" strokeWidth="1.2" strokeDasharray="3 3" />
          <line x1="120" y1="65" x2="185" y2="35" stroke="rgba(167, 139, 250, 0.35)" strokeWidth="1.2" strokeDasharray="3 3" />
          <line x1="120" y1="65" x2="55" y2="95" stroke="rgba(52, 211, 153, 0.35)" strokeWidth="1.2" strokeDasharray="3 3" />
          <line x1="120" y1="65" x2="185" y2="95" stroke="rgba(251, 191, 36, 0.35)" strokeWidth="1.2" strokeDasharray="3 3" />

          {/* Central Crystal Core */}
          <g
            className="animate-pulse origin-center"
            style={{ transformOrigin: "120px 65px" }}
          >
            <polygon
              points="120,45 136,65 120,85 104,65"
              fill="rgba(var(--accent), 0.25)"
              stroke="rgb(var(--accent))"
              strokeWidth="2"
            />
            <text x="120" y="68" textAnchor="middle" fill="currentColor" className="text-[rgb(var(--foreground))]" fontSize="8" fontWeight="bold" fontFamily="monospace">
              YOU
            </text>
          </g>

          {/* Fact Node 1: Work */}
          <g transform="translate(55, 35)">
            <circle r="13" fill="rgba(56, 189, 248, 0.2)" stroke="#38bdf8" strokeWidth="1.5" />
            <text x="0" y="3" textAnchor="middle" fill="#38bdf8" fontSize="7.5" fontWeight="bold">FACTS</text>
          </g>

          {/* Fact Node 2: Goals */}
          <g transform="translate(185, 35)">
            <circle r="13" fill="rgba(167, 139, 250, 0.2)" stroke="#a78bfa" strokeWidth="1.5" />
            <text x="0" y="3" textAnchor="middle" fill="#a78bfa" fontSize="7.5" fontWeight="bold">GOALS</text>
          </g>

          {/* Fact Node 3: Style */}
          <g transform="translate(55, 95)">
            <circle r="13" fill="rgba(52, 211, 153, 0.2)" stroke="#34d399" strokeWidth="1.5" />
            <text x="0" y="3" textAnchor="middle" fill="#34d399" fontSize="7.5" fontWeight="bold">STYLE</text>
          </g>

          {/* Fact Node 4: Notes */}
          <g transform="translate(185, 95)">
            <circle r="13" fill="rgba(251, 191, 36, 0.2)" stroke="#facc15" strokeWidth="1.5" />
            <text x="0" y="3" textAnchor="middle" fill="#facc15" fontSize="7.5" fontWeight="bold">NOTES</text>
          </g>
        </svg>
      </div>
    </div>
  );
});

HelpMemoryDiagram.displayName = "HelpMemoryDiagram";
