import React, {
  useState,
  useEffect,
  useCallback,
  useRef,
  useMemo,
} from "react";
import { Target, Plus, Minus, Edit3, Download, Upload, Zap } from "lucide-react";
import ReactMarkdown from "react-markdown";
import {
  getPersonalMemory,
  savePersonalMemory,
  consolidatePersonalMemory,
  exportPersonalMemory,
  importPersonalMemory,
  getActiveFacts,
  type PersonalMemoryRecord,
  type FactRecord,
} from "@/services/memoryService";
import { AmbientBackground } from "@/shared/components/common";
import { Drawer } from "@/shared/ui/Drawer";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { cn } from "@/shared/lib/utils";

// ─── Fact type colour palette ─────────────────────────────────────────────────
const FACT_COLORS: Record<string, { hsl: string; glow: string; label: string }> = {
  personal:  { hsl: "hsl(185 80% 55%)", glow: "hsla(185,80%,55%,0.45)", label: "Identity" },
  objective: { hsl: "hsl(260 75% 65%)", glow: "hsla(260,75%,65%,0.45)", label: "Objective" },
  workdone:  { hsl: "hsl(140 60% 55%)", glow: "hsla(140,60%,55%,0.45)", label: "Work done" },
  blocker:   { hsl: "hsl(0   70% 60%)", glow: "hsla(0,70%,60%,0.45)",   label: "Blocker" },
  next_step: { hsl: "hsl(25  80% 58%)", glow: "hsla(25,80%,58%,0.45)",  label: "Next step" },
  pitfall:   { hsl: "hsl(45  85% 55%)", glow: "hsla(45,85%,55%,0.45)",  label: "Pitfall" },
};

const SPHERE_COLOR = { hsl: "hsl(42 90% 58%)", glow: "hsla(42,90%,58%,0.55)", label: "Personal Memory" };

// ─── Layout constants ─────────────────────────────────────────────────────────
const RING1_RADIUS = 170;
const RING2_RADIUS = 330;
const FACT_NODE_R  = 11;
const CLUSTER_FACT_R = 8;

// ─── Types ────────────────────────────────────────────────────────────────────
interface NodePosition { x: number; y: number }
interface TooltipState { fact: FactRecord; pos: NodePosition }

// ─── Helpers ──────────────────────────────────────────────────────────────────
function polarToXY(cx: number, cy: number, r: number, angleDeg: number): NodePosition {
  const rad = (angleDeg - 90) * (Math.PI / 180);
  return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) };
}

function groupBySession(facts: FactRecord[]): Map<string, FactRecord[]> {
  const map = new Map<string, FactRecord[]>();
  for (const f of facts) {
    const key = f.session_id !== null ? String(f.session_id) : "__unsessioned__";
    const arr = map.get(key) ?? [];
    arr.push(f);
    map.set(key, arr);
  }
  return map;
}

// ─── Sub-components ───────────────────────────────────────────────────────────

interface FactNodeProps {
  fact: FactRecord;
  x: number;
  y: number;
  r: number;
  onHover: (fact: FactRecord, pos: NodePosition) => void;
  onLeave: () => void;
}

const FactNode = React.memo(({ fact, x, y, r, onHover, onLeave }: FactNodeProps) => {
  const col = FACT_COLORS[fact.fact_type] ?? FACT_COLORS.personal;
  return (
    <g
      className="cursor-pointer"
      onMouseEnter={() => onHover(fact, { x, y })}
      onMouseLeave={onLeave}
    >
      <circle cx={x} cy={y} r={r + 5} fill={col.glow} />
      <circle cx={x} cy={y} r={r} fill={col.hsl} />
    </g>
  );
});
FactNode.displayName = "FactNode";

interface LegendCardProps { className?: string }
const LegendCard = React.memo(({ className }: LegendCardProps) => (
  <div
    className={cn(
      "rounded-xl px-3 py-2.5 flex flex-col gap-1.5 text-[11px] font-mono select-none",
      "bg-[rgba(10,10,18,0.72)] border border-[rgba(255,255,255,0.06)] backdrop-blur-md",
      className
    )}
  >
    {/* Central sphere entry */}
    <div className="flex items-center gap-2">
      <span className="w-2.5 h-2.5 rounded-full shrink-0" style={{ background: SPHERE_COLOR.hsl, boxShadow: `0 0 6px ${SPHERE_COLOR.glow}` }} />
      <span className="text-[rgb(var(--foreground-muted))]">{SPHERE_COLOR.label}</span>
    </div>
    {Object.entries(FACT_COLORS).map(([key, col]) => (
      <div key={key} className="flex items-center gap-2">
        <span className="w-2 h-2 rounded-full shrink-0" style={{ background: col.hsl, boxShadow: `0 0 4px ${col.glow}` }} />
        <span className="text-[rgb(var(--foreground-muted))]">{col.label}</span>
      </div>
    ))}
  </div>
));
LegendCard.displayName = "LegendCard";

// ─── Main page ────────────────────────────────────────────────────────────────
export const Memory: React.FC = () => {
  const svgRef = useRef<SVGSVGElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  const [dims, setDims] = useState({ w: 0, h: 0 });
  const [zoom, setZoom] = useState(1);
  const [pan, setPan] = useState<NodePosition>({ x: 0, y: 0 });
  const isPanning = useRef(false);
  const lastPan = useRef<NodePosition>({ x: 0, y: 0 });

  const [personalMemory, setPersonalMemory] = useState<PersonalMemoryRecord | null>(null);
  const [facts, setFacts] = useState<FactRecord[]>([]);
  const [loading, setLoading] = useState(true);

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [editing, setEditing] = useState(false);
  const [draftContent, setDraftContent] = useState("");
  const [saving, setSaving] = useState(false);

  const [tooltip, setTooltip] = useState<TooltipState | null>(null);

  // ── Measure container ──────────────────────────────────────────────────────
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const obs = new ResizeObserver((entries) => {
      const { width, height } = entries[0].contentRect;
      setDims({ w: width, h: height });
    });
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

  // ── Load data ──────────────────────────────────────────────────────────────
  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const [mem, allFacts] = await Promise.all([getPersonalMemory(), getActiveFacts()]);
      setPersonalMemory(mem);
      setFacts(allFacts);
    } catch (e) {
      console.error("[Memory] Failed to load:", e);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  // ── Recenter ───────────────────────────────────────────────────────────────
  const handleRecenter = useCallback(() => {
    setZoom(1);
    setPan({ x: 0, y: 0 });
  }, []);

  // ── Zoom ───────────────────────────────────────────────────────────────────
  const handleZoomIn  = useCallback(() => setZoom((z) => Math.min(z + 0.2, 3)), []);
  const handleZoomOut = useCallback(() => setZoom((z) => Math.max(z - 0.2, 0.3)), []);

  // ── Pan (drag on SVG background) ──────────────────────────────────────────
  const onPointerDown = useCallback((e: React.PointerEvent<SVGSVGElement>) => {
    if ((e.target as SVGElement).closest("g[data-node]")) return;
    isPanning.current = true;
    lastPan.current = { x: e.clientX, y: e.clientY };
    (e.currentTarget as SVGSVGElement).setPointerCapture(e.pointerId);
  }, []);

  const onPointerMove = useCallback((e: React.PointerEvent<SVGSVGElement>) => {
    if (!isPanning.current) return;
    const dx = e.clientX - lastPan.current.x;
    const dy = e.clientY - lastPan.current.y;
    lastPan.current = { x: e.clientX, y: e.clientY };
    setPan((p) => ({ x: p.x + dx, y: p.y + dy }));
  }, []);

  const onPointerUp = useCallback(() => { isPanning.current = false; }, []);

  // ── Graph layout ───────────────────────────────────────────────────────────
  const cx = dims.w / 2;
  const cy = dims.h / 2;

  const identityFacts  = useMemo(() => facts.filter((f) => f.fact_type === "personal"), [facts]);
  const sessionedFacts = useMemo(() => facts.filter((f) => f.fact_type !== "personal"), [facts]);
  const sessionClusters = useMemo(() => groupBySession(sessionedFacts), [sessionedFacts]);

  // Ring 1 — identity facts evenly spaced
  const ring1Nodes = useMemo(() => {
    if (identityFacts.length === 0) return [];
    return identityFacts.map((f, i) => {
      const angle = (360 / identityFacts.length) * i;
      return { fact: f, ...polarToXY(cx, cy, RING1_RADIUS, angle) };
    });
  }, [identityFacts, cx, cy]);

  // Ring 2 — cluster anchors evenly spaced; facts within each cluster orbit a local center
  const ring2Clusters = useMemo(() => {
    const clusterKeys = Array.from(sessionClusters.keys());
    return clusterKeys.map((key, ci) => {
      const angle = (360 / clusterKeys.length) * ci;
      const anchor = polarToXY(cx, cy, RING2_RADIUS, angle);
      const clusterFacts = sessionClusters.get(key) ?? [];
      const nodes = clusterFacts.map((f, fi) => {
        const spread = Math.min(360 / clusterFacts.length, 72);
        const localAngle = spread * fi;
        const localR = clusterFacts.length > 1 ? 32 : 0;
        const pos = localR > 0
          ? polarToXY(anchor.x, anchor.y, localR, localAngle)
          : anchor;
        return { fact: f, x: pos.x, y: pos.y };
      });
      return { key, anchor, nodes };
    });
  }, [sessionClusters, cx, cy]);

  // ── Drawer / editing ───────────────────────────────────────────────────────
  const openDrawer = useCallback(() => {
    setEditing(false);
    setDrawerOpen(true);
  }, []);

  const handleEdit = useCallback(() => {
    setDraftContent(personalMemory?.content ?? "");
    setEditing(true);
  }, [personalMemory]);

  const handleSave = useCallback(async () => {
    if (!personalMemory) return;
    setSaving(true);
    try {
      const updated = await savePersonalMemory(draftContent, personalMemory.version);
      setPersonalMemory(updated);
      setEditing(false);
    } catch (e) {
      console.error("[Memory] Save failed:", e);
    } finally {
      setSaving(false);
    }
  }, [personalMemory, draftContent]);

  const handleConsolidate = useCallback(async () => {
    try {
      const updated = await consolidatePersonalMemory();
      setPersonalMemory(updated);
      await refresh();
    } catch (e) {
      console.error("[Memory] Consolidate failed:", e);
    }
  }, [refresh]);

  const handleExport = useCallback(async () => {
    try {
      await exportPersonalMemory("~/personal_memory.md");
    } catch (e) {
      console.error("[Memory] Export failed:", e);
    }
  }, []);

  const handleImport = useCallback(async () => {
    try {
      const updated = await importPersonalMemory("~/personal_memory.md");
      setPersonalMemory(updated);
    } catch (e) {
      console.error("[Memory] Import failed:", e);
    }
  }, []);

  // ── Tooltip ────────────────────────────────────────────────────────────────
  const handleNodeHover = useCallback((fact: FactRecord, pos: NodePosition) => {
    setTooltip({ fact, pos });
  }, []);

  const handleNodeLeave = useCallback(() => {
    setTooltip(null);
  }, []);

  // ── Render ─────────────────────────────────────────────────────────────────
  return (
    <div
      ref={containerRef}
      className="relative flex-1 flex flex-col h-full w-full overflow-hidden bg-transparent select-none"
    >
      <AmbientBackground originX="50%" originY="50%" rippleSpeedMultiplier={1.2} />

      {/* ── Legend — top left ── */}
      <LegendCard className="absolute top-4 left-4 z-20" />

      {/* ── Header — top center ── */}
      <div className="absolute top-4 left-1/2 -translate-x-1/2 z-20 flex items-center gap-3">
        <button
          onClick={handleRecenter}
          title={MEMORY_COPY.recenterView}
          className="p-2 rounded-xl bg-[rgba(10,10,18,0.72)] border border-[rgba(255,255,255,0.07)] backdrop-blur-md text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.4)] transition-colors"
        >
          <Target size={15} />
        </button>
        <span className="text-[12px] font-display font-black tracking-[0.2em] uppercase text-[rgb(var(--accent))]">
          MEMORY
        </span>
      </div>

      {/* ── Zoom pill — top right ── */}
      <div className="absolute top-4 right-4 z-20 flex flex-col rounded-xl overflow-hidden border border-[rgba(255,255,255,0.07)] bg-[rgba(10,10,18,0.72)] backdrop-blur-md">
        <button
          onClick={handleZoomIn}
          title="Zoom in"
          className="p-2.5 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-colors border-b border-[rgba(255,255,255,0.06)]"
        >
          <Plus size={14} />
        </button>
        <button
          onClick={handleZoomOut}
          title="Zoom out"
          className="p-2.5 text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.08)] transition-colors"
        >
          <Minus size={14} />
        </button>
      </div>

      {/* ── SVG Canvas ── */}
      {dims.w > 0 && (
        <svg
          ref={svgRef}
          width={dims.w}
          height={dims.h}
          className="absolute inset-0 cursor-grab active:cursor-grabbing"
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
        >
          <g transform={`translate(${pan.x},${pan.y}) scale(${zoom})`} style={{ transformOrigin: `${cx}px ${cy}px` }}>
            {/* Orbit rings */}
            <circle cx={cx} cy={cy} r={RING1_RADIUS} fill="none" stroke="rgba(255,255,255,0.05)" strokeWidth={1} strokeDasharray="4 6" />
            <circle cx={cx} cy={cy} r={RING2_RADIUS} fill="none" stroke="rgba(255,255,255,0.04)" strokeWidth={1} strokeDasharray="4 8" />

            {/* Ring 1 — connection lines */}
            {ring1Nodes.map(({ fact, x, y }) => (
              <line key={`l1-${fact.id}`} x1={cx} y1={cy} x2={x} y2={y} stroke="rgba(100,220,220,0.12)" strokeWidth={1} />
            ))}

            {/* Ring 2 — cluster lines from ring-2 anchor to facts */}
            {ring2Clusters.map(({ anchor, nodes }) =>
              nodes.map(({ fact, x, y }) => (
                <line key={`l2-${fact.id}`} x1={anchor.x} y1={anchor.y} x2={x} y2={y} stroke="rgba(160,130,255,0.12)" strokeWidth={0.8} />
              ))
            )}

            {/* Ring 2 anchor → center lines */}
            {ring2Clusters.map(({ key, anchor }) => (
              <line key={`la-${key}`} x1={cx} y1={cy} x2={anchor.x} y2={anchor.y} stroke="rgba(255,255,255,0.04)" strokeWidth={0.8} strokeDasharray="3 5" />
            ))}

            {/* Ring 1 — identity fact nodes */}
            {ring1Nodes.map(({ fact, x, y }) => (
              <FactNode
                key={fact.id}
                fact={fact}
                x={x}
                y={y}
                r={FACT_NODE_R}
                onHover={handleNodeHover}
                onLeave={handleNodeLeave}
              />
            ))}

            {/* Ring 2 — session cluster nodes */}
            {ring2Clusters.map(({ nodes }) =>
              nodes.map(({ fact, x, y }) => (
                <FactNode
                  key={fact.id}
                  fact={fact}
                  x={x}
                  y={y}
                  r={CLUSTER_FACT_R}
                  onHover={handleNodeHover}
                  onLeave={handleNodeLeave}
                />
              ))
            )}

            {/* Central sphere */}
            <g
              className="cursor-pointer"
              onClick={openDrawer}
            >
              {/* Outer glow halo */}
              <circle cx={cx} cy={cy} r={68} fill="hsla(42,90%,58%,0.08)" />
              <circle cx={cx} cy={cy} r={56} fill="hsla(42,90%,58%,0.14)" />
              {/* Core sphere */}
              <radialGradient id="sphere-grad" cx="40%" cy="35%" r="65%">
                <stop offset="0%"   stopColor="hsl(50,95%,78%)" />
                <stop offset="55%"  stopColor="hsl(42,90%,58%)" />
                <stop offset="100%" stopColor="hsl(32,75%,32%)" />
              </radialGradient>
              <circle cx={cx} cy={cy} r={44} fill="url(#sphere-grad)" />
              {/* Specular highlight */}
              <ellipse cx={cx - 12} cy={cy - 14} rx={10} ry={6} fill="rgba(255,255,255,0.25)" />
            </g>
          </g>
        </svg>
      )}

      {/* ── Fact tooltip ── */}
      {tooltip && (
        <div
          className="absolute z-30 pointer-events-none"
          style={{
            left: tooltip.pos.x * zoom + pan.x + 16,
            top:  tooltip.pos.y * zoom + pan.y - 8,
          }}
        >
          <div className="max-w-[220px] rounded-xl bg-[rgba(10,10,18,0.88)] border border-[rgba(255,255,255,0.1)] backdrop-blur-lg px-3 py-2.5 shadow-2xl">
            <div className="flex items-center gap-1.5 mb-1">
              <span
                className="w-2 h-2 rounded-full shrink-0"
                style={{
                  background: FACT_COLORS[tooltip.fact.fact_type]?.hsl ?? "hsl(185 80% 55%)",
                  boxShadow: `0 0 5px ${FACT_COLORS[tooltip.fact.fact_type]?.glow ?? ""}`,
                }}
              />
              <span className="text-[10px] font-mono uppercase tracking-wider text-[rgb(var(--foreground-muted))]">
                {FACT_COLORS[tooltip.fact.fact_type]?.label ?? tooltip.fact.fact_type}
              </span>
            </div>
            <p className="text-[12px] text-[rgb(var(--foreground))] leading-relaxed m-0">
              {tooltip.fact.text}
            </p>
            {tooltip.fact.session_id !== null && (
              <p className="text-[10px] font-mono text-[rgb(var(--foreground-muted))] mt-1">
                Session #{tooltip.fact.session_id}
              </p>
            )}
          </div>
        </div>
      )}

      {/* ── Empty / loading indicator ── */}
      {!loading && facts.length === 0 && (
        <div className="absolute inset-0 flex flex-col items-center justify-center pointer-events-none">
          <p className="text-[13px] font-mono text-[rgb(var(--foreground-muted))] mt-40">
            "No memory facts yet — start a conversation to build your memory graph."
          </p>
        </div>
      )}

      {/* ── Personal Memory Drawer ── */}
      <Drawer
        open={drawerOpen}
        onClose={() => { setDrawerOpen(false); setEditing(false); }}
        position="global"
        ariaLabel="Personal Memory"
        bodyClassName="px-6 py-4 max-h-[55vh] overflow-y-auto"
        title={
          <span className="text-[13px] font-display font-black tracking-[0.16em] uppercase text-[rgb(var(--accent))]">
            Personal Memory
          </span>
        }
      >
        {/* Action row */}
        <div className="flex items-center gap-2 mb-4 flex-wrap">
          {!editing ? (
            <button
              onClick={handleEdit}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-mono bg-[rgba(var(--accent),0.1)] border border-[rgba(var(--accent),0.25)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.18)] transition-colors"
            >
              <Edit3 size={11} /> Edit
            </button>
          ) : (
            <>
              <button
                onClick={handleSave}
                disabled={saving}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-mono bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.25)] transition-colors disabled:opacity-50"
              >
                {saving ? "Saving…" : "Save"}
              </button>
              <button
                onClick={() => setEditing(false)}
                className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-mono bg-[rgba(255,255,255,0.05)] border border-[rgba(255,255,255,0.08)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
              >
                Cancel
              </button>
            </>
          )}
          <button
            onClick={handleConsolidate}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-mono bg-[rgba(255,255,255,0.05)] border border-[rgba(255,255,255,0.08)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
          >
            <Zap size={11} /> Consolidate
          </button>
          <button
            onClick={handleExport}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-mono bg-[rgba(255,255,255,0.05)] border border-[rgba(255,255,255,0.08)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
          >
            <Download size={11} /> Export
          </button>
          <button
            onClick={handleImport}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-[11px] font-mono bg-[rgba(255,255,255,0.05)] border border-[rgba(255,255,255,0.08)] text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors"
          >
            <Upload size={11} /> Import
          </button>
        </div>

        {/* Content — edit or read */}
        {editing ? (
          <textarea
            value={draftContent}
            onChange={(e) => setDraftContent(e.target.value)}
            className="w-full h-[200px] bg-[rgba(255,255,255,0.04)] border border-[rgba(255,255,255,0.08)] rounded-xl px-3 py-2.5 text-[13px] font-mono text-[rgb(var(--foreground))] resize-none focus:outline-none focus:border-[rgba(var(--accent),0.4)]"
            spellCheck={false}
          />
        ) : (
          <div className="prose prose-invert prose-sm max-w-none text-[rgb(var(--foreground))]">
            {personalMemory?.content ? (
              <ReactMarkdown>{personalMemory.content}</ReactMarkdown>
            ) : (
              <p className="text-[rgb(var(--foreground-muted))] text-[13px] font-mono">
                "No personal memory yet. Start a session to build your memory graph."
              </p>
            )}
          </div>
        )}

        {/* Version footer */}
        {personalMemory && (
          <p className="mt-4 text-[10px] font-mono text-[rgb(var(--foreground-muted))] border-t border-[rgba(255,255,255,0.06)] pt-2">
            v{personalMemory.version} · last updated {new Date(personalMemory.updated_at).toLocaleString()}
          </p>
        )}
      </Drawer>
    </div>
  );
};
