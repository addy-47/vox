/**
 * Orbit field — a state-driven "living background" behind (and slightly in front of) the orb.
 * Large tilted elliptical orbits, travelling glowing nodes with trails, dotted dust tracks and
 * faint concentric waves. Framework-free canvas-2D engine; <OrbitField> is the thin React wrapper.
 *
 * Depth model: every orbit is a tilted circle seen from the front. Its near half (lower half in
 * the ring's own frame) is drawn on the FRONT canvas where it crosses the orb face; the far half
 * is simply never drawn inside the orb silhouette (back canvas has the disc cut out).
 *
 * Every numeric parameter is lerped toward the active preset, so state changes never pop.
 */

export interface FieldPreset {
  intensity: number; // overall brightness 0..1
  speed: number;     // node/dust speed multiplier
  orbits: number;    // how many orbits are visible (fractional = fade-in of the next one)
  nodes: number;     // nodes per orbit multiplier (fractional = fade-in)
  trail: number;     // comet trail length multiplier
  flow: number;      // concentric waves: + outward (scan), - inward (pull), 0 = still
  scale: number;     // orbit size multiplier (<1 = contracted)
}

/** Preset registry keys */
export type FieldPresetKey =
  | 'ready'
  | 'listening'
  | 'thinking'
  | 'speaking'
  | 'working'
  | 'web_search'
  | 'search_memory'
  | 'compaction';

export const FIELD_PRESETS: Record<FieldPresetKey, FieldPreset> = {
  ready:         { intensity: 0.35, speed: 0.35, orbits: 4,  nodes: 1,   trail: 0.5, flow: 0,    scale: 1 },
  listening:     { intensity: 0.55, speed: 0.40, orbits: 5,  nodes: 1,   trail: 0.6, flow: -0.4, scale: 0.92 },
  thinking:      { intensity: 0.75, speed: 0.60, orbits: 6,  nodes: 1,   trail: 0.75, flow: 0,   scale: 1 },
  speaking:      { intensity: 0.85, speed: 0.45, orbits: 7,  nodes: 1.5, trail: 1.0, flow: 0.6,  scale: 1.12 },
  working:       { intensity: 0.90, speed: 0.85, orbits: 7,  nodes: 1.5, trail: 0.9, flow: 0,    scale: 1 },
  web_search:    { intensity: 1,    speed: 1.35, orbits: 8,  nodes: 2,   trail: 1.3, flow: 1.0,  scale: 1 },
  search_memory: { intensity: 1,    speed: 0.95, orbits: 8,  nodes: 1.5, trail: 1.0, flow: -1.0, scale: 0.94 },
  compaction:    { intensity: 1,    speed: 1.10, orbits: 7,  nodes: 1,   trail: 0.9, flow: -1.4, scale: 0.86 },
};

export function isFieldPresetKey(k: string): k is FieldPresetKey {
  return k in FIELD_PRESETS;
}

/** Orb-preset names (DESIGN §8.2) → registry keys. Add aliases freely. */
export const FIELD_ALIASES: Record<string, string> = {
  web: 'web_search',
  globe: 'web_search',
  orbits: 'search_memory',
  morph: 'compaction',
  memory: 'search_memory',
};

export function resolveFieldKey(state?: string, activity?: string): string | null {
  if (state === 'Ready') return 'ready';
  if (state === 'Listening') return 'listening';
  if (state === 'Thinking') return 'thinking';
  if (state === 'Speaking') return 'speaking';
  if (state === 'Working') {
    const a = activity ? (FIELD_ALIASES[activity] ?? activity) : '';
    return isFieldPresetKey(a) ? a : 'working';
  }
  return null;
}

type Motion = 'static' | 'precess' | 'tumble' | 'breathe';
type Style = 'solid' | 'dust' | 'dash' | 'double' | 'band';

interface Orbit {
  a: number;       // semi-major axis, in orb radii
  k: number;       // initial flatten (cos of tilt): b = a*k
  phi: number;     // in-plane angle, degrees
  ox: number; oy: number; // centre offset, in orb radii
  w: number;       // node angular speed, rad/s (sign = direction)
  nodes: number;   // node slots (max 3), 0 = none
  style: Style;    // how the track is drawn
  motion: Motion;  // static = fixed plane; precess = plane rotates; tumble = ring flips end-over-end in 3D; breathe = size/tilt pulse
  rate: number;    // rad/s for the motion above (sign = direction)
  alpha: number;   // base brightness
  shimmer: number; // 0..1 slow independent brightness drift
}

/** Mix of static and moving rings, solid / dotted / dashed / double / band, bright and ghost-faint. */
const ORBITS: Orbit[] = [
  { a: 1.22, k: 0.30, phi: -10, ox: 0,    oy: 0.02, w: 0.9,   nodes: 1, style: 'solid',  motion: 'precess', rate: 0.11,  alpha: 0.6,  shimmer: 0.2 },
  { a: 1.40, k: 0.42, phi: 14,  ox: 0,    oy: 0,    w: -0.7,  nodes: 1, style: 'solid',  motion: 'tumble',  rate: 0.42,  alpha: 0.5,  shimmer: 0.3 },
  { a: 1.75, k: 0.27, phi: -21, ox: 0.05, oy: 0,    w: 0.5,   nodes: 1, style: 'dust',   motion: 'static',  rate: 0,     alpha: 0.65, shimmer: 0.4 },
  { a: 2.25, k: 0.34, phi: -8,  ox: 0,    oy: 0.05, w: 0.45,  nodes: 1, style: 'band',   motion: 'breathe', rate: 0.55,  alpha: 0.3,  shimmer: 0.5 },
  { a: 2.95, k: 0.30, phi: 12,  ox: 0.3,  oy: 0,    w: -0.38, nodes: 1, style: 'dash',   motion: 'precess', rate: -0.07, alpha: 0.5,  shimmer: 0.3 },
  { a: 3.7,  k: 0.42, phi: -26, ox: -0.4, oy: 0.1,  w: 0.3,   nodes: 0, style: 'solid',  motion: 'static',  rate: 0,     alpha: 0.2,  shimmer: 0.6 },
  { a: 4.6,  k: 0.24, phi: 6,   ox: 0.6,  oy: 0,    w: -0.25, nodes: 1, style: 'double', motion: 'tumble',  rate: 0.2,   alpha: 0.4,  shimmer: 0.3 },
  { a: 5.5,  k: 0.36, phi: -14, ox: -0.5, oy: -0.1, w: 0.2,   nodes: 1, style: 'dust',   motion: 'precess', rate: 0.045, alpha: 0.4,  shimmer: 0.4 },
  { a: 6.5,  k: 0.30, phi: 20,  ox: 0.2,  oy: 0,    w: -0.16, nodes: 0, style: 'solid',  motion: 'static',  rate: 0,     alpha: 0.16, shimmer: 0.5 },
  { a: 2.6,  k: 0.20, phi: 35,  ox: -0.2, oy: 0,    w: 0.6,   nodes: 1, style: 'dash',   motion: 'breathe', rate: 0.4,   alpha: 0.35, shimmer: 0.4 },
];
const MAX_NODES = 3;
const TAU = Math.PI * 2;
// Hoisted static dash patterns — avoids array allocation in 60fps render loop
const EMPTY_DASH: number[] = [];
const DUST_DASH: number[] = [0.4, 6];
const DASH_DASH: number[] = [26, 16];

export class OrbitFieldEngine {
  private bctx: CanvasRenderingContext2D;
  private fctx: CanvasRenderingContext2D;
  private cur: FieldPreset = { ...FIELD_PRESETS.thinking, intensity: 0 };
  private tgt: FieldPreset = { ...FIELD_PRESETS.thinking, intensity: 0 };
  private activeKey: string | null = null;
  private phase: number[][] = ORBITS.map((_o, i) => Array.from({ length: MAX_NODES }, (_, j) => (j * TAU) / MAX_NODES + i * 1.1));
  private motion: number[] = ORBITS.map(() => 0);
  private t = 0; private dust = 0; private flowPhase = 0;
  private accent: [number, number, number] = [0, 219, 233];
  private accentPrefix = 'rgba(0,219,233,';
  private light = false;
  private sprite: HTMLCanvasElement | null = null;
  // geometry (css px)
  private bw = 0; private bh = 0; private cx = 0; private cy = 0; private R = 100; private box = 200; private dpr = 1;
  private cleared = true;

  constructor(private back: HTMLCanvasElement, private front: HTMLCanvasElement, private wrap: HTMLElement) {
    this.bctx = back.getContext('2d')!;
    this.fctx = front.getContext('2d')!;
    this.buildSprite();
  }

  /** Orb silhouette radius as a fraction of the orb box (camera z=6.95, fov 45, sphere 2.30). */
  static ORB_FRAC = 0.423;

  setKey(key: string | null) {
    this.activeKey = key;
    if (key && isFieldPresetKey(key)) {
      this.tgt = { ...FIELD_PRESETS[key] };
    } else {
      this.tgt = { ...this.tgt, intensity: 0 };
    }
  }

  setColors(accent: [number, number, number], light: boolean) {
    this.accent = accent;
    this.accentPrefix = `rgba(${accent[0]},${accent[1]},${accent[2]},`;
    this.light = light;
    this.buildSprite();
  }

  /** True when fully faded out — the host may stop its frame loop. */
  isIdle() { return this.cur.intensity < 0.004 && this.tgt.intensity === 0; }

  layout(dprIn: number) {
    const r = this.wrap.getBoundingClientRect();
    this.dpr = Math.min(dprIn, 1.5);
    this.bw = window.innerWidth; this.bh = window.innerHeight;
    this.box = r.width;
    this.R = r.width * OrbitFieldEngine.ORB_FRAC;
    this.cx = r.left + r.width / 2; this.cy = r.top + r.height / 2;
    const b = this.back.style;
    b.left = `${-r.left}px`; b.top = `${-r.top}px`; b.width = `${this.bw}px`; b.height = `${this.bh}px`;
    this.back.width = Math.round(this.bw * this.dpr); this.back.height = Math.round(this.bh * this.dpr);
    this.front.width = Math.round(r.width * this.dpr); this.front.height = Math.round(r.height * this.dpr);
  }

  private buildSprite() {
    const s = document.createElement('canvas'); s.width = s.height = 64;
    const g = s.getContext('2d')!, [r, gg, b] = this.accent;
    const grd = g.createRadialGradient(32, 32, 0, 32, 32, 32);
    grd.addColorStop(0, `rgba(${r},${gg},${b},0.95)`);
    grd.addColorStop(0.25, `rgba(${r},${gg},${b},0.35)`);
    grd.addColorStop(1, `rgba(${r},${gg},${b},0)`);
    g.fillStyle = grd; g.fillRect(0, 0, 64, 64);
    this.sprite = s;
  }

  frame(dt: number) {
    const k = 1 - Math.exp(-dt * 2.2);
    const c = this.cur, g = this.tgt;
    (Object.keys(c) as (keyof FieldPreset)[]).forEach((key) => { c[key] += (g[key] - c[key]) * k; });
    if (this.isIdle()) {
      if (!this.cleared) { this.bctx.clearRect(0, 0, this.back.width, this.back.height); this.fctx.clearRect(0, 0, this.front.width, this.front.height); this.cleared = true; }
      return;
    }
    this.cleared = false;
    this.t += dt; this.dust += dt * 6 * c.speed; this.flowPhase += dt * 0.09 * c.flow;
    ORBITS.forEach((o, i) => {
      this.motion[i] += o.rate * c.speed * dt;
      for (let j = 0; j < MAX_NODES; j++) this.phase[i][j] += o.w * c.speed * dt;
    });

    // Subtle synchronized thinking pulse bob — locked to orb's breathing phase Math.sin(t * 2.3)
    const thinkBob = this.activeKey === 'thinking' ? 0.022 * Math.sin(this.t * 2.3) : 0;

    const d = this.dpr;
    // back canvas: everything, with the orb disc cut out
    const b = this.bctx; b.setTransform(d, 0, 0, d, 0, 0); b.clearRect(0, 0, this.bw, this.bh);
    b.save(); b.beginPath(); b.rect(0, 0, this.bw, this.bh); b.arc(this.cx, this.cy, this.R, 0, TAU); b.clip('evenodd');
    this.draw(b, false, this.cx, this.cy, thinkBob); b.restore();
    // front canvas: near halves only, inside the disc
    const f = this.fctx; f.setTransform(d, 0, 0, d, 0, 0); f.clearRect(0, 0, this.box, this.box);
    f.save(); f.beginPath(); f.arc(this.box / 2, this.box / 2, this.R + 1, 0, TAU); f.clip();
    this.draw(f, true, this.box / 2, this.box / 2, thinkBob); f.restore();
  }

  private rgba(a: number): string {
    const clamped = a > 1 ? 1 : a < 0 ? 0 : Math.round(a * 1000) / 1000;
    return `${this.accentPrefix}${clamped})`;
  }

  private draw(ctx: CanvasRenderingContext2D, frontOnly: boolean, cx: number, cy: number, thinkBob: number) {
    const c = this.cur, R = this.R, lw = this.light ? 1.3 : 1;
    const gain = this.light ? 1.25 : 1;
    ctx.lineCap = 'round';

    // concentric waves (back only — they never reach the orb face)
    if (!frontOnly) {
      ctx.lineWidth = lw; ctx.setLineDash(EMPTY_DASH);
      [1.9, 3.1, 4.6, 6.4].forEach((m) => {
        ctx.strokeStyle = this.rgba(0.05 * c.intensity * gain);
        ctx.beginPath(); ctx.arc(cx, cy, m * R * (c.scale + thinkBob), 0, TAU); ctx.stroke();
      });
      const fa = Math.min(1, Math.abs(c.flow));
      if (fa > 0.02) for (let q = 0; q < 3; q++) {
        const u = (((this.flowPhase + q / 3) % 1) + 1) % 1;
        ctx.strokeStyle = this.rgba(Math.sin(Math.PI * u) * 0.16 * fa * c.intensity * gain);
        ctx.beginPath(); ctx.arc(cx, cy, R * (1.3 + 5.5 * u), 0, TAU); ctx.stroke();
      }
    }

    ORBITS.forEach((o, i) => {
      const vis = Math.max(0, Math.min(1, c.orbits - i)) * c.intensity;
      if (vis < 0.01) return;
      const m = this.motion[i];
      // ── geometry: a tilted circle (tilt ψ), seen from the front ──
      let a = o.a * R * (c.scale + thinkBob), kk = o.k, phi = (o.phi * Math.PI) / 180;
      if (o.motion === 'precess') phi += m;
      if (o.motion === 'breathe') { a *= 1 + 0.07 * Math.sin(m); kk = Math.min(0.95, o.k * (1 + 0.3 * Math.sin(m * 0.7 + 1))); }
      const psi = o.motion === 'tumble' ? Math.acos(Math.min(0.99, o.k)) + m : Math.acos(Math.min(0.99, kk));
      const cosP = Math.cos(psi), sinP = Math.sin(psi);
      const ry = Math.max(0.02 * a, a * Math.abs(cosP));
      const ns = sinP * (cosP >= 0 ? 1 : -1) >= 0 ? 1 : -1;   // which half of the ellipse is nearest the viewer
      const lo = ns > 0 ? 0 : Math.PI;                          // near arc = [lo, lo+π], far arc = the rest
      const ox = cx + o.ox * R, oy = cy + o.oy * R;
      const cos = Math.cos(phi), sin = Math.sin(phi), dir = o.w >= 0 ? 1 : -1;
      const shim = 1 - o.shimmer + o.shimmer * (0.5 + 0.5 * Math.sin(this.t * 0.5 + i * 2.1));
      const base = o.alpha * vis * gain * shim;

      // ── track ──
      const pass = (aa: number, lwm: number, dash: number[], off: number, sc: number) => {
        ctx.lineWidth = lw * lwm; ctx.setLineDash(dash); ctx.lineDashOffset = off;
        if (!frontOnly) {
          ctx.strokeStyle = this.rgba(base * 0.7 * aa);
          ctx.beginPath(); ctx.ellipse(ox, oy, a * sc, ry * sc, phi, lo + Math.PI, lo + TAU); ctx.stroke();
        }
        ctx.strokeStyle = this.rgba(base * aa);
        ctx.beginPath(); ctx.ellipse(ox, oy, a * sc, ry * sc, phi, lo, lo + Math.PI); ctx.stroke();
      };
      switch (o.style) {
        case 'dust':   pass(1.1, 1.5, DUST_DASH, -this.dust * dir, 1); break;
        case 'dash':   pass(1, 1, DASH_DASH, -this.dust * 3 * dir, 1); break;
        case 'band':   pass(0.45, 4.5, EMPTY_DASH, 0, 1); pass(0.9, 1, EMPTY_DASH, 0, 1); break;
        case 'double': pass(1, 1, EMPTY_DASH, 0, 1); pass(0.6, 1, EMPTY_DASH, 0, 1.045); break;
        default:       pass(1, 1, EMPTY_DASH, 0, 1);
      }
      ctx.setLineDash(EMPTY_DASH);

      // ── nodes + comet trails ──
      const slots = o.nodes > 0 ? Math.min(MAX_NODES, Math.ceil(o.nodes * c.nodes)) : 0;
      for (let j = 0; j < slots; j++) {
        const na = Math.max(0, Math.min(1, o.nodes * c.nodes - j)) * vis * (0.55 + 0.45 * shim);
        if (na < 0.02) continue;
        const th = this.phase[i][j];
        const T = 11, step = 0.03 * c.trail;
        for (let s = T; s >= 0; s--) {
          const t2 = th - dir * s * step;
          const sy = Math.sin(t2);
          const near = sy * ns >= 0;
          if (frontOnly && !near) continue;
          const xr = a * Math.cos(t2), yr = ry * sy;
          const x = ox + xr * cos - yr * sin, y = oy + xr * sin + yr * cos;
          if (s === 0) {
            const gs = 15 + (near ? 3 : 0);
            ctx.globalCompositeOperation = this.light ? 'source-over' : 'lighter';
            ctx.globalAlpha = na * (near ? 1 : 0.7);
            ctx.drawImage(this.sprite!, x - gs, y - gs, gs * 2, gs * 2);
            ctx.globalCompositeOperation = 'source-over';
            ctx.fillStyle = this.rgba(na * (this.light ? 1 : 0.95));
            ctx.beginPath(); ctx.arc(x, y, 1.7, 0, TAU); ctx.fill();
            ctx.globalAlpha = 1;
          } else {
            const f = 1 - s / T;
            ctx.fillStyle = this.rgba(na * f * f * 0.85 * (near ? 1 : 0.7));
            ctx.beginPath(); ctx.arc(x, y, 1.1 * (0.5 + 0.5 * f), 0, TAU); ctx.fill();
          }
        }
      }
    });
  }

  dispose(): void {
    if (this.sprite) {
      this.sprite.width = 1;
      this.sprite.height = 1;
      this.sprite = null;
    }
    this.bctx.clearRect(0, 0, this.back.width, this.back.height);
    this.fctx.clearRect(0, 0, this.front.width, this.front.height);
  }
}
