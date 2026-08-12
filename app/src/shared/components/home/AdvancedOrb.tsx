import React, { useEffect, useRef, useState, useCallback } from 'react';
import * as THREE from 'three';
import { useDynamicFPS } from '@/shared/hooks/useDynamicFPS';
import { type InteractionState } from '@/services/eventsService';

// ─── Types ──────────────────────────────────────────────────────────────────

interface VoxOrbProps {
  telemetryRef?: React.MutableRefObject<{ energy: number; vad_prob: number; low: number; mid: number; high: number }>;
  amplitude?: number;
  interactionState?: InteractionState;
  isSleeping?: boolean;
  isTesting?: boolean;
}


// ─── Constants ───────────────────────────────────────────────────────────────

/**
 * Disc radius = sphere surface radius where each disc projects to.
 * Injected into GLSL as a literal so the shader can reference it as a const.
 */
const DISC_R = 2.26;

/** Outer glow shell — slightly larger than the disc hemisphere radius. */
const SHELL_R = 2.30;

/** Number of silk-sheet disc layers. */
const NUM_SHEETS = 7;

/** Target amplitude per interaction state. */
const BASE_AMP: Record<string, number> = {
  Idle:              0.02,
  Listening:         0.14,
  UserSpeaking:      0.58,
  Thinking:          0.30,
  AssistantSpeaking: 0.58,
  Interrupted:       0.02,
};

// Helper to parse CSS variables that contain comma-separated RGB values (e.g., "0, 219, 233") or hex colors
const COLOR_CACHE = new Map<string, THREE.Color>();

function getCSSColor(varName: string, fallbackHex: string): THREE.Color {
  if (typeof window === 'undefined') return new THREE.Color(fallbackHex);
  if (COLOR_CACHE.has(varName)) return COLOR_CACHE.get(varName)!.clone();
  const val = getComputedStyle(document.documentElement).getPropertyValue(varName).trim();
  if (!val) {
    const col = new THREE.Color(fallbackHex);
    COLOR_CACHE.set(varName, col);
    return col.clone();
  }
  const parts = val.split(',').map(s => parseInt(s.trim(), 10));
  if (parts.length === 3 && !parts.some(isNaN)) {
    const col = new THREE.Color(parts[0] / 255, parts[1] / 255, parts[2] / 255);
    COLOR_CACHE.set(varName, col);
    return col.clone();
  }
  const col = new THREE.Color(val);
  COLOR_CACHE.set(varName, col);
  return col.clone();
}

// ─── Geometry ────────────────────────────────────────────────────────────────

/**
 * Disc with concentric ring subdivision so the vertex shader's noise
 * displacement has adequate spatial resolution.
 *
 * UV layout: (0.5, 0.5) = centre; distance 0.5 from centre = outer edge.
 */
function createDiscGeometry(
  radius: number,
  radialSegs: number,
  ringSegs: number,
): THREE.BufferGeometry {
  const positions: number[] = [];
  const uvs: number[] = [];
  const indices: number[] = [];

  // Centre vertex
  positions.push(0, 0, 0);
  uvs.push(0.5, 0.5);

  for (let ring = 1; ring <= ringSegs; ring++) {
    const rFrac = ring / ringSegs;
    const r = rFrac * radius;
    for (let seg = 0; seg < radialSegs; seg++) {
      const angle = (seg / radialSegs) * Math.PI * 2;
      const cos = Math.cos(angle);
      const sin = Math.sin(angle);
      positions.push(r * cos, r * sin, 0);
      uvs.push(0.5 + rFrac * 0.5 * cos, 0.5 + rFrac * 0.5 * sin);
    }
  }

  // Centre → first ring fan
  for (let seg = 0; seg < radialSegs; seg++) {
    const next = (seg + 1) % radialSegs;
    indices.push(0, 1 + seg, 1 + next);
  }

  // Ring quads
  for (let ring = 0; ring < ringSegs - 1; ring++) {
    const base = 1 + ring * radialSegs;
    const nextBase = 1 + (ring + 1) * radialSegs;
    for (let seg = 0; seg < radialSegs; seg++) {
      const next = (seg + 1) % radialSegs;
      const a = base + seg;
      const b = base + next;
      const c = nextBase + seg;
      const d = nextBase + next;
      indices.push(a, c, b, b, c, d);
    }
  }

  const geo = new THREE.BufferGeometry();
  geo.setAttribute('position', new THREE.Float32BufferAttribute(positions, 3));
  geo.setAttribute('uv', new THREE.Float32BufferAttribute(uvs, 2));
  geo.setIndex(indices);
  geo.computeVertexNormals();
  return geo;
}

// ─── GLSL: 3-D Simplex noise ─────────────────────────────────────────────────

const SNOISE_GLSL = `
  vec3 mod289(vec3 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
  vec4 mod289(vec4 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
  vec4 permute(vec4 x) { return mod289(((x * 34.0) + 1.0) * x); }
  vec4 taylorInvSqrt(vec4 r) { return 1.79284291400159 - 0.85373472095314 * r; }
  float snoise(vec3 v) {
    const vec2 C = vec2(1.0/6.0, 1.0/3.0);
    const vec4 D = vec4(0.0, 0.5, 1.0, 2.0);
    vec3 i  = floor(v + dot(v, C.yyy));
    vec3 x0 = v - i + dot(i, C.xxx);
    vec3 g  = step(x0.yzx, x0.xyz);
    vec3 l  = 1.0 - g;
    vec3 i1 = min(g.xyz, l.zxy);
    vec3 i2 = max(g.xyz, l.zxy);
    vec3 x1 = x0 - i1 + C.xxx;
    vec3 x2 = x0 - i2 + C.yyy;
    vec3 x3 = x0 - D.yyy;
    i = mod289(i);
    vec4 p = permute(permute(permute(
      i.z + vec4(0.0, i1.z, i2.z, 1.0))
      + i.y + vec4(0.0, i1.y, i2.y, 1.0))
      + i.x + vec4(0.0, i1.x, i2.x, 1.0));
    float n_ = 1.0/7.0;
    vec3 ns = n_ * D.wyz - D.xzx;
    vec4 j = p - 49.0 * floor(p * ns.z * ns.z);
    vec4 x_ = floor(j * ns.z);
    vec4 y_ = floor(j - 7.0 * x_);
    vec4 x = x_ * ns.x + ns.yyyy;
    vec4 y = y_ * ns.x + ns.yyyy;
    vec4 h = 1.0 - abs(x) - abs(y);
    vec4 b0 = vec4(x.xy, y.xy);
    vec4 b1 = vec4(x.zw, y.zw);
    vec4 s0 = floor(b0)*2.0 + 1.0;
    vec4 s1 = floor(b1)*2.0 + 1.0;
    vec4 sh = -step(h, vec4(0.0));
    vec4 a0 = b0.xzyw + s0.xzyw*sh.xxyy;
    vec4 a1 = b1.xzyw + s1.xzyw*sh.zzww;
    vec3 p0 = vec3(a0.xy, h.x);
    vec3 p1 = vec3(a0.zw, h.y);
    vec3 p2 = vec3(a1.xy, h.z);
    vec3 p3 = vec3(a1.zw, h.w);
    vec4 norm = taylorInvSqrt(vec4(
      dot(p0,p0), dot(p1,p1), dot(p2,p2), dot(p3,p3)));
    p0 *= norm.x; p1 *= norm.y; p2 *= norm.z; p3 *= norm.w;
    vec4 m = max(0.6 - vec4(
      dot(x0,x0), dot(x1,x1), dot(x2,x2), dot(x3,x3)), 0.0);
    m = m * m;
    return 42.0 * dot(m*m, vec4(
      dot(p0,x0), dot(p1,x1), dot(p2,x2), dot(p3,x3)));
  }
`;

// ─── Shaders ─────────────────────────────────────────────────────────────────

/**
 * DISC VERTEX SHADER
 *
 * Core idea — hemisphere projection:
 *   Each flat disc vertex (x, y, 0) is lifted to the sphere surface:
 *   z = sqrt(SR² − x² − y²)
 *   giving point (x, y, z) on the front hemisphere of radius SR.
 *
 * The outward sphere normal at that point is normalize(x, y, z).
 * After the model-matrix rotation, the Fresnel in the fragment shader
 * is naturally highest at the equatorial rim (edge-on) and zero at the
 * pole (face-on) — producing the hairline-to-ribbon gradient the eye sees.
 *
 * Noise displacement is applied radially: it crumples the hemisphere
 * organically so the rim is no longer a perfect circle but a wavy curve,
 * which is exactly the undulating silk look in the target images.
 */
const DISC_VERT = `
  uniform float u_time;
  uniform float u_amplitude;
  uniform float u_phase;
  uniform float u_waveScale;
  uniform float u_highs;

  varying vec3  vWorldPos;
  varying vec3  vNormal;    // world-space sphere normal
  varying float vRadius;    // normalised radial coord 0 (centre) → 1 (equator)

  const float SR = ${DISC_R.toFixed(2)};

  ${SNOISE_GLSL}

  void main() {
    float px = position.x;
    float py = position.y;
    float r2 = px * px + py * py;

    // Project disc XY to front hemisphere surface
    float z    = sqrt(max(0.0, SR * SR - r2));
    vec3  hemi = vec3(px, py, z);

    // Outward sphere normal in local space
    vec3 sNorm = normalize(hemi);
    vRadius    = sqrt(r2) / SR;           // 0 at centre, 1 at equator

    // ── Noise radial displacement ──────────────────────────────────────
    // Two noise octaves at different frequencies / drift speeds.
    // Displacement tapers smoothly to zero near the equator so the rim
    // stays well-defined; maximum crumple is at ~60 % of the radius.
    // Make displacement negative (inward) only to prevent overflowing the outer rim boundary
    float dispScale = 0.22 + u_amplitude * 0.95;
    float taper     = sin(vRadius * 3.14159) * smoothstep(1.0, 0.6, vRadius);

    float currentWaveScale = u_waveScale * (1.0 + u_highs * 0.4);
    vec3 nc1 = hemi * currentWaveScale
               + vec3(u_time * 0.11, u_time * 0.08,  u_phase);
    vec3 nc2 = hemi * currentWaveScale * 2.5
               + vec3(-u_time * 0.17, u_time * 0.14, u_phase + 5.3);
    float disp = -abs(snoise(nc1) + snoise(nc2) * 0.45) * dispScale * taper * 0.85;

    vec3 pos = hemi + sNorm * disp;

    // World-space normal — since the mesh only has rotation (no scale),
    // mat3(modelMatrix) correctly rotates the local normal to world space.
    vNormal   = normalize(mat3(modelMatrix) * sNorm);

    vec4 worldPos = modelMatrix * vec4(pos, 1.0);
    vWorldPos     = worldPos.xyz;

    gl_Position = projectionMatrix * viewMatrix * worldPos;
  }
`;

/**
 * DISC FRAGMENT SHADER
 *
 * Fresnel on the hemisphere surface:
 *   • facing = dot(viewDir, sphereNormal)
 *   • edgeGlow = 1 − facing         → 0 at the pole, 1 at the equator
 *
 * This is the "thin-line ↔ ribbon" mechanism: the same geometry appears
 * as a hairline when nearly edge-on and a wide translucent band when the
 * hemisphere is rotated so the equator sweeps across the centre of view.
 */
const DISC_FRAG = `
  uniform vec3  u_colorGlow;
  uniform vec3  u_colorAccent;
  uniform float u_baseOpacity;
  uniform float u_amplitude;
  uniform float u_sleeping;

  varying vec3  vWorldPos;
  varying vec3  vNormal;
  varying float vRadius;

  void main() {
    vec3  viewDir = normalize(cameraPosition - vWorldPos);

    // Fresnel: 0 = pole (face-on, dark), 1 = equator (edge-on, bright)
    float facing   = abs(dot(viewDir, vNormal));
    float edgeGlow = 1.0 - facing;
    edgeGlow       = pow(edgeGlow, 1.7);   // concentrate glow near rim

    // Radial blend — gently suppresses the extreme centre so the dark
    // void reads clearly at idle amplitude.
    float radialBlend = smoothstep(0.12, 0.80, vRadius);

    // Tiny fill at high amplitude floods the interior (matching screenshot 1)
    float fill = u_amplitude * 0.055 * (1.0 - radialBlend * 0.6);

    float alpha = (edgeGlow * radialBlend * 0.92 + fill) * u_baseOpacity;
    alpha *= (1.0 + u_amplitude * 4.5);
    alpha *= mix(1.0, 0.05, u_sleeping);

    if (alpha < 0.004) discard;

    // Colour: glow base → accent white-cyan at the bright rim
    vec3 color = mix(u_colorGlow, u_colorAccent, edgeGlow);

    gl_FragColor = vec4(color, clamp(alpha, 0.0, 0.95));
  }
`;

/**
 * OUTER SHELL VERTEX SHADER
 * Passes world-space position and normal for Fresnel rim calculation.
 */
const OUTER_VERT = `
  varying vec3 vWorldPos;
  varying vec3 vNormal;
  void main() {
    vec4 worldPos = modelMatrix * vec4(position, 1.0);
    vWorldPos     = worldPos.xyz;
    vNormal       = normalize(mat3(modelMatrix) * normal);
    gl_Position   = projectionMatrix * viewMatrix * worldPos;
  }
`;

/**
 * OUTER SHELL FRAGMENT SHADER
 * Sharp, bright Fresnel rim — the iconic glowing circle border.
 */
const OUTER_FRAG = `
  uniform vec3  u_colorGlow;
  uniform vec3  u_colorAccent;
  uniform float u_amplitude;
  uniform float u_sleeping;
  varying vec3  vWorldPos;
  varying vec3  vNormal;

  void main() {
    vec3  viewDir = normalize(cameraPosition - vWorldPos);
    float fresnel = 1.0 - abs(dot(viewDir, vNormal));

    // Very sharp rim — tight but bright
    float rim   = smoothstep(0.52, 1.0, fresnel);
    float alpha = rim * (0.65 + u_amplitude * 0.30);
    alpha *= mix(1.0, 0.12, u_sleeping);

    if (alpha < 0.008) discard;

    vec3 color = mix(u_colorGlow, u_colorAccent, pow(rim, 1.1));
    gl_FragColor = vec4(color, alpha);
  }
`;

// ─── Component ───────────────────────────────────────────────────────────────

export const VoxOrb = React.memo(({
  telemetryRef,
  amplitude = 0.0,
  interactionState = 'Idle',
  isSleeping = false,
  isTesting = false,
}: VoxOrbProps) => {
  const mountRef     = useRef<HTMLDivElement>(null);
  const stateRef     = useRef(interactionState);
  const sleepingRef  = useRef(isSleeping);
  const amplitudeRef = useRef(amplitude);
  const testingRef   = useRef(isTesting);

  useEffect(() => { stateRef.current     = interactionState; }, [interactionState]);
  useEffect(() => { sleepingRef.current  = isSleeping;       }, [isSleeping]);
  useEffect(() => { amplitudeRef.current = amplitude;         }, [amplitude]);
  useEffect(() => { testingRef.current   = isTesting;         }, [isTesting]);

  // ── Page visibility tracking ─────────────────────────────────────────────
  const [isPageVisible, setIsPageVisible] = useState(
    typeof document !== 'undefined' ? document.visibilityState === 'visible' : true,
  );

  useEffect(() => {
    const handler = () => setIsPageVisible(document.visibilityState === 'visible');
    document.addEventListener('visibilitychange', handler);
    return () => document.removeEventListener('visibilitychange', handler);
  }, []);

  // ── Theme observer to prevent Layout Thrashing during 60fps rendering ────
  useEffect(() => {
    let observer: MutationObserver | null = null;

    const updateTheme = () => {
      if (typeof document === 'undefined') return;
      const currentTheme = document.documentElement.getAttribute('data-theme') || 'dark';
      const accent = getCSSColor('--accent', '#00dbe9');
      const glow = currentTheme === 'light'
        ? getCSSColor('--accent-dark', '#0891b2')
        : accent.clone().multiplyScalar(0.40);

      themeRef.current = {
        theme: currentTheme,
        accent,
        glow
      };
    };

    updateTheme();

    if (typeof window !== 'undefined') {
      observer = new MutationObserver(updateTheme);
      observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    }

    return () => {
      if (observer) {
        observer.disconnect();
      }
    };
  }, []);

  // ── IntersectionObserver for component visibility ────────────────────────
  const [isVisible, setIsVisible] = useState(true);

  useEffect(() => {
    const el = mountRef.current;
    if (!el) return;
    const obs = new IntersectionObserver(
      ([entry]) => setIsVisible(entry.isIntersecting),
      { threshold: 0.1 },
    );
    obs.observe(el);
    return () => obs.disconnect();
  }, []);

  // ── Dynamic FPS controls ─────────────────────────────────────────────────
  const isActive = interactionState !== 'Idle' && interactionState !== 'Interrupted';

  /**
   * Runtime scene context — populated by the init effect below.
   * The tick function reads from this ref so it doesn't need closure captures.
   */
  interface SceneContext {
    renderer: THREE.WebGLRenderer;
    scene: THREE.Scene;
    camera: THREE.PerspectiveCamera;
    sharedUni: {
      u_time: { value: number };
      u_amplitude: { value: number };
      u_colorGlow: { value: THREE.Color };
      u_colorAccent: { value: THREE.Color };
      u_sleeping: { value: number };
      u_highs: { value: number };
    };
    discGroup: THREE.Group;
    group: THREE.Group;
    outerMat: THREE.ShaderMaterial;
    discAnims: { speedX: number; speedY: number; speedZ: number }[];
    curGlow: THREE.Color;
    curAccent: THREE.Color;
    tgtGlow: THREE.Color;
    tgtAccent: THREE.Color;
    whiteColor: THREE.Color;
    t0: number;
    discMats: THREE.ShaderMaterial[];
    outerGeo: THREE.SphereGeometry;
    resizeObs: ResizeObserver;
    container: HTMLDivElement;
    curScale: number;
    curBaseVal: number;
    heartbeatPhase: number;
    noiseTime: number;
    smoothedEnergy: number;
  }

  const sceneRef = useRef<SceneContext | null>(null);
  const themeRef = useRef({
    theme: "",
    accent: new THREE.Color('#00dbe9'),
    glow: new THREE.Color('#0891b2'),
  });

  const tickFn = useCallback((dt: number) => {
    const ctx = sceneRef.current;
    if (!ctx) return;

    const dtSec = dt / 1000;
    const t = (performance.now() - ctx.t0) / 1000;
    const state = stateRef.current;
    const sleeping = sleepingRef.current;

    // 1. Calculate raw voice/VAD energy input based on current state
    let rawEnergy = 0;
    let rawHigh = 0;

    if (telemetryRef?.current) {
      const e = telemetryRef.current.energy;
      const h = telemetryRef.current.high;
      const v = telemetryRef.current.vad_prob || 0;
      
      if (state === 'UserSpeaking' || state === 'AssistantSpeaking') {
        rawEnergy = e;
        rawHigh = h;
      } else if (state === 'Listening') {
        // Subtle energy feedback when listening based on mic energy & VAD
        rawEnergy = e * 0.4 + v * 0.2;
        rawHigh = h * 0.3;
      }
    } else if (amplitudeRef.current > 0) {
      rawEnergy = amplitudeRef.current;
      rawHigh = rawEnergy;
    }

    // 2. Smoothed audio energy (fast attack, slow graceful release envelope follower)
    const targetEnergy = Math.min(rawEnergy, 1.0);
    const prevEnergy = ctx.smoothedEnergy ?? 0;
    const energyRate = targetEnergy > prevEnergy ? 0.35 : 0.04;
    ctx.smoothedEnergy = prevEnergy + (targetEnergy - prevEnergy) * energyRate;
    const audioEnergy = ctx.smoothedEnergy;

    // 3. Stable Holographic Boundary (No physical bouncing or heartbeat scaling)
    ctx.group.scale.set(1.0, 1.0, 1.0);

    // 4. Smoothly transition base offset value on state changes (eliminates instant state-jump visual pops)
    const targetBase = BASE_AMP[state] ?? 0.02;
    const baseVal = (sleeping ? targetBase * 0.08 : targetBase);
    if (ctx.curBaseVal === undefined) {
      ctx.curBaseVal = baseVal;
    }
    ctx.curBaseVal += (baseVal - ctx.curBaseVal) * 0.06;

    // 5. Calculate internal vertex deformation amplitude (u_amplitude) directly from smoothed audio energy
    // This allows the internal "silk" to deepen fluidly and hold its shape during speech, without rhythmic bouncing.
    const targetAmp = ctx.curBaseVal + audioEnergy * 0.4;
    const curAmp = ctx.sharedUni.u_amplitude.value;
    const ampRate = targetAmp > curAmp ? 0.25 : 0.08;
    ctx.sharedUni.u_amplitude.value += (targetAmp - curAmp) * ampRate;

    // 7. Internal noise/texture boiling (accelerate time drift based on voice activity)
    const speedBoost = 1.0 + audioEnergy * 3.5;
    ctx.noiseTime += dtSec * speedBoost;
    ctx.sharedUni.u_time.value = ctx.noiseTime;

    ctx.sharedUni.u_highs.value = rawHigh * (1.0 + audioEnergy * 0.5);
    ctx.sharedUni.u_sleeping.value += (Number(sleeping) - ctx.sharedUni.u_sleeping.value) * 0.08;

    const themeAccent = themeRef.current.accent;
    const themeGlow = themeRef.current.glow;

    if (state === 'Idle' || state === 'Interrupted') {
      ctx.tgtGlow.copy(themeGlow);
      ctx.tgtAccent.copy(themeGlow);
    } else if (state === 'AssistantSpeaking') {
      ctx.tgtGlow.copy(themeAccent);
      const isDark = themeRef.current.theme !== 'light';
      if (isDark) {
        ctx.tgtAccent.copy(themeAccent).lerp(ctx.whiteColor, 0.75);
      } else {
        ctx.tgtAccent.copy(ctx.whiteColor);
      }
    } else {
      ctx.tgtGlow.copy(themeGlow);
      ctx.tgtAccent.copy(themeAccent);
    }

    // Response morph rate
    ctx.curGlow.lerp(ctx.tgtGlow, 0.08);
    ctx.curAccent.lerp(ctx.tgtAccent, 0.08);

    ctx.sharedUni.u_colorGlow.value.copy(ctx.curGlow);
    ctx.sharedUni.u_colorAccent.value.copy(ctx.curAccent);

    ctx.outerMat.uniforms['u_colorGlow'].value.copy(ctx.curGlow);
    ctx.outerMat.uniforms['u_colorAccent'].value.copy(ctx.curAccent);

    // 8. Treble/Highs drive rotation speed multiplier (tied to smoothed energy to prevent frantic jumps)
    const speedMult = 1.0 + audioEnergy * 0.8;
    for (let i = 0; i < NUM_SHEETS; i++) {
      const mesh = ctx.discGroup.children[i] as THREE.Mesh;
      const anim = ctx.discAnims[i];
      mesh.rotation.x += anim.speedX * speedMult;
      mesh.rotation.y += anim.speedY * speedMult;
      mesh.rotation.z += anim.speedZ * speedMult;
    }

    ctx.group.rotation.y = t * 0.033;
    ctx.group.rotation.x = Math.sin(t * 0.021) * 0.055;

    ctx.renderer.render(ctx.scene, ctx.camera);
  }, [telemetryRef]);

  useDynamicFPS({
    onFrame: tickFn,
    isVisible,
    isPageVisible,
    fpsActive: 60,
    fpsIdle: 15,
    isActive,
    isPaused: isSleeping,
  });

  // ── Scene initialisation (runs once) ─────────────────────────────────────
  useEffect(() => {
    if (!mountRef.current) return;

    const container = mountRef.current;
    let width = container.clientWidth;
    let height = container.clientHeight;

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(45, width / height, 0.1, 1000);
    const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
    renderer.setSize(width, height, false);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setClearColor(0x000000, 0);
    renderer.domElement.style.cssText =
      'position:absolute;top:0;left:0;width:100%;height:100%;margin:0;padding:0;';
    container.appendChild(renderer.domElement);

    const initGlow = getCSSColor('--accent-dark', '#0891b2');
    const initAccent = getCSSColor('--accent', '#00dbe9');

    const sharedUni = {
      u_time: { value: 0.0 },
      u_amplitude: { value: 0.02 },
      u_colorGlow: { value: initGlow.clone() },
      u_colorAccent: { value: initAccent.clone() },
      u_sleeping: { value: 0.0 },
      u_highs: { value: 0.0 },
    };

    const discGroup = new THREE.Group();
    const discMats: THREE.ShaderMaterial[] = [];

    type DiscAnim = { speedX: number; speedY: number; speedZ: number };
    const discAnims: DiscAnim[] = [];

    // Share a single BufferGeometry across all 7 silk-sheet meshes
    const sharedDiscGeo = createDiscGeometry(DISC_R, 36, 12);

    for (let i = 0; i < NUM_SHEETS; i++) {
      const mat = new THREE.ShaderMaterial({
        uniforms: {
          u_time: sharedUni.u_time,
          u_amplitude: sharedUni.u_amplitude,
          u_colorGlow: sharedUni.u_colorGlow,
          u_colorAccent: sharedUni.u_colorAccent,
          u_sleeping: sharedUni.u_sleeping,
          u_highs: sharedUni.u_highs,
          u_phase: { value: i * 1.73 + Math.random() * 2.1 },
          u_waveScale: { value: 0.38 + Math.random() * 0.48 },
          u_baseOpacity: { value: 0.11 + Math.random() * 0.28 },
        },
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
        blending: THREE.NormalBlending,
        vertexShader: DISC_VERT,
        fragmentShader: DISC_FRAG,
      });
      discMats.push(mat);

      const mesh = new THREE.Mesh(sharedDiscGeo, mat);
      mesh.rotation.order = 'ZYX';
      mesh.rotation.x = (Math.random() - 0.5) * Math.PI;
      mesh.rotation.y = Math.random() * Math.PI * 2;
      mesh.rotation.z = (Math.random() - 0.5) * Math.PI * 0.5;
      discGroup.add(mesh);

      discAnims.push({
        speedX: (Math.random() - 0.5) * 0.0024,
        speedY: (Math.random() - 0.5) * 0.0017,
        speedZ: (Math.random() - 0.5) * 0.0031,
      });
    }

    const outerGeo = new THREE.SphereGeometry(SHELL_R, 64, 64);
    const outerMat = new THREE.ShaderMaterial({
      uniforms: {
        u_colorGlow: { value: initGlow.clone() },
        u_colorAccent: { value: initAccent.clone() },
        u_amplitude: sharedUni.u_amplitude,
        u_sleeping: sharedUni.u_sleeping,
      },
      transparent: true,
      depthWrite: false,
      side: THREE.FrontSide,
      blending: THREE.NormalBlending,
      vertexShader: OUTER_VERT,
      fragmentShader: OUTER_FRAG,
    });
    const outerMesh = new THREE.Mesh(outerGeo, outerMat);

    discGroup.renderOrder = 1;
    outerMesh.renderOrder = 2;

    const group = new THREE.Group();
    group.add(discGroup);
    group.add(outerMesh);
    scene.add(group);

    function updateCamera(w: number, h: number) {
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      renderer.setSize(w, h, false);
      const fovRad = (45 * Math.PI) / 180;
      const fitDist = SHELL_R / Math.tan(fovRad / 2);
      camera.position.z = Math.max(fitDist * 1.08, 6.5);
    }
    updateCamera(width, height);

    const resizeObs = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const { width: w, height: h } = entry.contentRect;
      if (w > 0 && h > 0) {
        updateCamera(w, h);
        if (sceneRef.current) {
          renderer.render(scene, camera);
        }
      }
    });
    resizeObs.observe(container);

    const curGlow = initGlow.clone();
    const curAccent = initAccent.clone();
    const tgtGlow = new THREE.Color();
    const tgtAccent = new THREE.Color();
    const whiteColor = new THREE.Color(1, 1, 1);

    sceneRef.current = {
      renderer,
      scene,
      camera,
      sharedUni,
      discGroup,
      group,
      outerMat,
      discAnims,
      curGlow,
      curAccent,
      tgtGlow,
      tgtAccent,
      whiteColor,
      t0: performance.now(),
      discMats,
      outerGeo,
      resizeObs,
      container,
      curScale: 1.0,
      curBaseVal: 0.02,
      heartbeatPhase: 0,
      noiseTime: 0,
      smoothedEnergy: 0,
    };

    return () => {
      sceneRef.current = null;
      resizeObs.disconnect();
      if (container.contains(renderer.domElement)) {
        container.removeChild(renderer.domElement);
      }
      sharedDiscGeo.dispose();
      outerGeo.dispose();
      outerMat.dispose();
      discMats.forEach((m) => m.dispose());
      renderer.dispose();
    };
  }, []);

  return (
    <div
      ref={mountRef}
      style={{ width: '100%', height: '100%', background: 'transparent', position: 'relative' }}
    />
  );
});

VoxOrb.displayName = "VoxOrb";
