import React, { useEffect, useRef } from 'react';
import * as THREE from 'three';

interface AudioTelemetry {
  energy: number;
  vad_prob: number;
}

interface VoxOrbProps {
  telemetryRef?: React.MutableRefObject<{ energy: number; vad_prob: number }>;
  amplitude?: number;
  frequency?: number;
  interactionState?: "Idle" | "Listening" | "UserSpeaking" | "Thinking" | "AssistantSpeaking" | "Interrupted";
  isSleeping?: boolean;
}

// ─── 4 big, slow waves — enough to feel organic, never chaotic ─────────────
const WAVE_COUNT = 4;

interface WaveParams {
  anchor: [number, number, number];   // direction vector on the sphere
  speed: number;                       // orbit speed (rad/sec base)
  phase: number;                       // initial phase offset
  ampScale: number;                    // relative wave height
  width: number;                       // lateral spread (smaller = wider)
}

const WAVE_PARAMS: WaveParams[] = [
  { anchor: [ 0.7,  0.3,  0.6],  speed: 0.25, phase: 0.0,  ampScale: 1.0,  width: 0.35 },
  { anchor: [-0.4,  0.8, -0.4],  speed: 0.31, phase: 1.8,  ampScale: 0.8,  width: 0.40 },
  { anchor: [ 0.1, -0.95, 0.2],  speed: 0.20, phase: 3.2,  ampScale: 0.9,  width: 0.38 },
  { anchor: [-0.7, -0.4,  0.5],  speed: 0.28, phase: 5.0,  ampScale: 0.75, width: 0.42 },
];

export const VoxOrb: React.FC<VoxOrbProps> = ({
  telemetryRef,
  amplitude = 0.0,
  interactionState = "Idle",
  isSleeping = false,
}) => {
  const mountRef = useRef<HTMLDivElement>(null);
  const stateRef = useRef(interactionState);
  const internalTelemetryRef = useRef<AudioTelemetry>({ energy: 0, vad_prob: 0 });

  useEffect(() => { stateRef.current = interactionState; }, [interactionState]);

  useEffect(() => {
    if (!telemetryRef) {
      internalTelemetryRef.current = { energy: amplitude, vad_prob: 0 };
    }
  }, [amplitude, telemetryRef]);

  useEffect(() => {
    if (!mountRef.current) return;

    const container = mountRef.current;
    let width = container.clientWidth;
    let height = container.clientHeight;

    // ── Scene setup ────────────────────────────────────────────────────────
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(45, width / height, 0.1, 1000);
    const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
    renderer.setSize(width, height);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setClearColor(0x000000, 0);
    renderer.domElement.style.cssText = 'position:absolute;top:0;left:0;width:100%;height:100%;';
    container.appendChild(renderer.domElement);

    // Outer shell: perfect static sphere, never deforms
    const outerGeo = new THREE.SphereGeometry(2.0, 72, 72);
    // Inner surface: slightly smaller, displaced inward only
    const innerGeo = new THREE.SphereGeometry(1.96, 72, 72);

    // ── Pack wave parameters into uniforms ─────────────────────────────────
    const anchors   = new Float32Array(WAVE_PARAMS.flatMap(w => w.anchor));
    const speeds    = new Float32Array(WAVE_PARAMS.map(w => w.speed));
    const phases    = new Float32Array(WAVE_PARAMS.map(w => w.phase));
    const ampScales = new Float32Array(WAVE_PARAMS.map(w => w.ampScale));
    const widths    = new Float32Array(WAVE_PARAMS.map(w => w.width));

    // ── Get theme colors ──────────────────────────────────────────────────
    const getThemeColor = (cssVar: string, defaultHex: string) => {
      const val = getComputedStyle(document.documentElement).getPropertyValue(cssVar).trim();
      if (!val) return new THREE.Color(defaultHex);
      const rgb = val.split(',').map(v => parseInt(v.trim()));
      if (rgb.length === 3) return new THREE.Color(`rgb(${rgb[0]},${rgb[1]},${rgb[2]})`);
      return new THREE.Color(defaultHex);
    };

    const accentColor = getThemeColor('--accent', '#00dbe9');
    const accentGlow = accentColor.clone().multiplyScalar(1.2);

    const uniforms = {
      u_time:        { value: 0.0 },
      u_amplitude:   { value: 0.0 },
      u_frequency:   { value: 1.0 },
      u_color:       { value: accentColor },
      u_colorGlow:   { value: accentGlow },
      u_anchors:     { value: anchors },
      u_speeds:      { value: speeds },
      u_phases:      { value: phases },
      u_ampScales:   { value: ampScales },
      u_widths:      { value: widths },
      u_waveCount:   { value: WAVE_COUNT },
    };

    // ── Inner surface (wave-displaced, additive glow) ──────────────────────
    const innerMat = new THREE.ShaderMaterial({
      uniforms,
      transparent: true,
      depthWrite: false,
      depthTest: true,
      side: THREE.FrontSide,
      blending: THREE.AdditiveBlending,
      vertexShader: /* glsl */ `
        uniform float u_time;
        uniform float u_amplitude;
        uniform float u_frequency;
        uniform vec3  u_anchors[4];
        uniform float u_speeds[4];
        uniform float u_phases[4];
        uniform float u_ampScales[4];
        uniform float u_widths[4];
        uniform int   u_waveCount;
        varying vec3  vWorldPos;
        varying vec3  vNormal;
        varying float vWaveHeight;

        void main() {
          vec3 n = normalize(normalMatrix * normal);
          vNormal = n;
          vec4 worldPos = modelMatrix * vec4(position, 1.0);
          vWorldPos = worldPos.xyz;

          // Smooth frequency clamping to keep orbit speeds sane
          float f = clamp(u_frequency, 0.4, 2.5);

          float totalDisp = 0.0;
          vec3 posNorm = normalize(position);

          for (int i = 0; i < 4; i++) {
            if (i >= u_waveCount) break;

            // Orbit anchor around Y and X axes independently
            float a1 = u_time * u_speeds[i] * f + u_phases[i];
            vec3 anchor = u_anchors[i];
            // Rotate around Y
            float cy = cos(a1), sy = sin(a1);
            vec3 r1 = vec3(anchor.x * cy - anchor.z * sy, anchor.y, anchor.x * sy + anchor.z * cy);
            // Rotate around X
            float a2 = u_time * u_speeds[i] * 0.7 * f + u_phases[i] + 1.0;
            float cx = cos(a2), sx = sin(a2);
            vec3 r2 = vec3(r1.x, r1.y * cx - r1.z * sx, r1.y * sx + r1.z * cx);
            vec3 dir = normalize(r2);

            // Cosine‑lobe wave: smooth, wide, non‑zero over a large area
            float d = dot(posNorm, dir);
            // Rescale to [-1,1] then map to lobe
            float lobe = smoothstep(-0.15, 0.5, d) * smoothstep(1.1, 0.3, d);
            // Gentle breathing
            float pulse = 0.7 + 0.3 * sin(u_time * 0.8 * u_speeds[i] + u_phases[i]);

            totalDisp += lobe * pulse * u_ampScales[i] * u_widths[i] * 1.8;
          }

          // Normalise across waves so we never exceed ~1.2
          totalDisp = totalDisp / float(u_waveCount) * 1.2;

          // Add a high-frequency secondary noise for organic texture
          // Add multiple high-frequency noise layers for organic texture
          float noise1 = sin(posNorm.x * 30.0 + u_time * 1.2) * cos(posNorm.y * 35.0 - u_time * 0.7);
          float noise2 = sin(posNorm.z * 50.0 - u_time * 2.0) * cos(posNorm.x * 45.0 + u_time * 1.5);
          float combinedNoise = (noise1 * 0.6 + noise2 * 0.4) * 0.05;
          totalDisp += combinedNoise * (u_amplitude + 0.15);

          // Scale by global amplitude (idle base always present)
          float finalDisp = totalDisp * (u_amplitude * 0.75 + 0.07);

          // PUSH INWARD ONLY — outer silhouette stays untouched
          vec3 newPos = position - n * finalDisp;
          vWaveHeight = finalDisp;

          gl_Position = projectionMatrix * modelViewMatrix * vec4(newPos, 1.0);
        }
      `,
      fragmentShader: /* glsl */ `
        uniform vec3 u_color;
        uniform vec3 u_colorGlow;
        varying vec3 vWorldPos;
        varying vec3 vNormal;
        varying float vWaveHeight;

        void main() {
          vec3 viewDir = normalize(cameraPosition - vWorldPos);
          float fresnel = 1.0 - abs(dot(viewDir, vNormal));

          // More active glow for internal animation
          float waveGlow = smoothstep(0.005, 0.45, vWaveHeight);
          float rim      = smoothstep(0.35, 0.95, fresnel);

          // Enhanced glow: inner surface is a vibrant, glowing internal state.
          float alpha = rim * 0.3 + waveGlow * 0.55;
          vec3 color  = mix(u_color, u_colorGlow, waveGlow * 0.6 + rim * 0.15);
          alpha *= 0.35 + vWaveHeight * 0.25;

          gl_FragColor = vec4(color, alpha);
        }
      `,
    });

    // ── Outer shell (perfect rim mask) ─────────────────────────────────────
    const outerMat = new THREE.ShaderMaterial({
      uniforms: { u_color: { value: accentColor } },
      transparent: true,
      depthWrite: true,           // write depth so inner fragments behind it are discarded
      depthTest: true,
      side: THREE.FrontSide,
      blending: THREE.NormalBlending,
      vertexShader: /* glsl */ `
        varying vec3 vWorldPos;
        varying vec3 vNormal;
        void main() {
          vec4 worldPos = modelMatrix * vec4(position, 1.0);
          vWorldPos = worldPos.xyz;
          vNormal = normalize(normalMatrix * normal);
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
        }
      `,
      fragmentShader: /* glsl */ `
        uniform vec3 u_color;
        varying vec3 vWorldPos;
        varying vec3 vNormal;
        void main() {
          vec3 viewDir = normalize(cameraPosition - vWorldPos);
          float fresnel = 1.0 - abs(dot(viewDir, vNormal));

          // Thinner, dimmer rim for a more premium "glassy" feel
          float rim = smoothstep(0.45, 0.85, fresnel);
          float alpha = rim * 0.35;  // dimmed from 0.5

          if (alpha < 0.02) discard; 

          gl_FragColor = vec4(u_color * (0.85 + rim * 0.15), alpha);
        }
      `,
    });

    // Update colors when CSS variables or sleep state change
    const updateColors = (sleeping: boolean) => {
      const newAccent = getThemeColor('--accent', '#00dbe9');
      if (sleeping) {
          // Dim to 30% for Cold state
          newAccent.multiplyScalar(0.3);
      }
      uniforms.u_color.value = newAccent;
      uniforms.u_colorGlow.value = newAccent.clone().multiplyScalar(1.2);
      outerMat.uniforms.u_color.value = newAccent;
    };

    updateColors(isSleeping);

    // Observer for CSS variable changes
    const themeObserver = new MutationObserver(() => updateColors(isSleeping));
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['style', 'data-theme'] });

    const innerMesh = new THREE.Mesh(innerGeo, innerMat);
    const outerMesh = new THREE.Mesh(outerGeo, outerMat);

    // Render order: outer first to write depth, then inner
    outerMesh.renderOrder = 0;
    innerMesh.renderOrder = 1;

    const group = new THREE.Group();
    group.add(outerMesh);
    group.add(innerMesh);
    scene.add(group);

    // ── Dynamic camera distance (avoids clipping on narrow screens) ────────
    function updateCamera(w: number, h: number) {
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      renderer.setSize(w, h);

      const halfFovV = Math.tan((Math.PI * 45 / 180) * 0.5);
      const distV = 2.2 / halfFovV;                 // vertical fit
      const distH = 2.2 / (halfFovV * w / h);       // horizontal fit
      camera.position.z = Math.max(distV, distH, 5.5);
    }

    updateCamera(width, height);

    const resizeObs = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const { width: w, height: h } = entry.contentRect;
      if (w > 0 && h > 0) {
        updateCamera(w, h);
        renderer.render(scene, camera); // immediate re-draw
      }
    });
    resizeObs.observe(container);

    // ── Animation loop ─────────────────────────────────────────────────────
    let raf: number;
    const t0 = performance.now();

    function animate() {
      raf = requestAnimationFrame(animate);
      const t = (performance.now() - t0) / 1000;

      const state = stateRef.current;
      const target = {
        Idle:              { amp: 0.06, freq: 0.5 },
        Listening:         { amp: 0.18, freq: 0.7 },
        UserSpeaking:      { amp: 0.42, freq: 1.0 },
        Thinking:          { amp: 0.35, freq: 1.2 },
        AssistantSpeaking: { amp: 0.52, freq: 1.0 },
        Interrupted:       { amp: 0.06, freq: 0.4 },
      }[state] || { amp: 0.06, freq: 0.5 };

      let telemAmp = 0, telemFreq = 0;
      if (telemetryRef) {
        const e = telemetryRef.current.energy;
        // vad_prob intentionally unused for structural timing; mapping based on state energy
        switch (state) {
          case 'UserSpeaking':
            telemAmp = e * 0.4;
            telemFreq = e * 0.2;
            break;
          case 'AssistantSpeaking':
            telemAmp = e * 0.5;
            telemFreq = e * 0.15;
            break;
          case 'Listening':
            telemAmp = e * 0.15;
            break;
        }
      }
      if (state === 'Thinking') {
        telemAmp = Math.sin(t * 2.5) * 0.08 + 0.08;
      }

      // Asymmetric envelope (fast attack, slow release)
      const curAmp = uniforms.u_amplitude.value;
      const targetAmp = Math.min(target.amp + telemAmp, 0.85);
      const rate = targetAmp > curAmp ? 0.12 : 0.015;
      uniforms.u_amplitude.value += (targetAmp - curAmp) * rate;

      // Smooth frequency (the modulation you see)
      uniforms.u_frequency.value +=
        ((target.freq + telemFreq) - uniforms.u_frequency.value) * 0.05;

      uniforms.u_time.value = t;

      // Very slow group rotation for a subtle drift
      group.rotation.y += 0.002;
      group.rotation.x += 0.001;

      renderer.render(scene, camera);
    }
    animate();

    return () => {
      resizeObs.disconnect();
      themeObserver.disconnect();
      cancelAnimationFrame(raf);
      if (container.contains(renderer.domElement)) container.removeChild(renderer.domElement);
      outerGeo.dispose(); innerGeo.dispose();
      outerMat.dispose(); innerMat.dispose();
      renderer.dispose();
    };
  }, [isSleeping]);

  return (
    <div
      ref={mountRef}
      style={{ width: '100%', height: '100%', background: 'transparent', position: 'relative' }}
    />
  );
};