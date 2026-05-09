import React, { useEffect, useRef } from 'react';
import * as THREE from 'three';

interface AudioTelemetry {
  amplitude: number;
  frequency: number;
}

interface VoxOrbProps {
  telemetryRef?: React.MutableRefObject<{ energy: number; vad_prob: number }>;
  amplitude?: number; // Keep for backward compatibility/static usage
  frequency?: number;
  interactionState?: "Idle" | "Listening" | "UserSpeaking" | "Thinking" | "AssistantSpeaking" | "Interrupted";
}

export const VoxOrb: React.FC<VoxOrbProps> = ({ telemetryRef, amplitude = 0.0, frequency = 1.0, interactionState = "Idle" }) => {
  const mountRef = useRef<HTMLDivElement>(null);
  const internalTelemetryRef = useRef<AudioTelemetry>({ amplitude, frequency });
  const stateRef = useRef(interactionState);

  useEffect(() => {
    stateRef.current = interactionState;
  }, [interactionState]);

  // Update internal ref if props are used
  useEffect(() => {
    if (!telemetryRef) {
      internalTelemetryRef.current = { amplitude, frequency };
    }
  }, [amplitude, frequency, telemetryRef]);

  useEffect(() => {
    if (!mountRef.current) return;

    const container = mountRef.current;
    const width = container.clientWidth;
    const height = container.clientHeight;

    // Scene
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(45, width / height, 0.1, 1000);
    camera.position.z = 6; // Move back slightly to prevent cutoff

    const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
    renderer.setSize(width, height);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setClearColor(0x000000, 0);
    renderer.domElement.style.position = 'absolute';
    renderer.domElement.style.top = '0';
    renderer.domElement.style.left = '0';
    renderer.domElement.style.width = '100%';
    renderer.domElement.style.height = '100%';
    container.appendChild(renderer.domElement);

    // Geometry
    const geometry = new THREE.SphereGeometry(2.0, 64, 64);

    // Uniforms
    const uniforms = {
      u_time: { value: 0.0 },
      u_amplitude: { value: 0.0 },
      u_frequency: { value: 1.0 },
      u_color: { value: new THREE.Color('#00dbe9') },
    };

    const material = new THREE.ShaderMaterial({
      uniforms,
      wireframe: false,
      transparent: true,
      depthWrite: false,
      side: THREE.DoubleSide,
      blending: THREE.AdditiveBlending,
      vertexShader: `
        uniform float u_time;
        uniform float u_amplitude;
        uniform float u_frequency;
        varying vec3 vNormal;
        varying vec3 vPosition;
        varying float vNoise;

        // Simplex 3D Noise
        vec3 mod289(vec3 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
        vec4 mod289(vec4 x) { return x - floor(x * (1.0 / 289.0)) * 289.0; }
        vec4 permute(vec4 x) { return mod289(((x*34.0)+10.0)*x); }
        vec4 taylorInvSqrt(vec4 r) { return 1.79284291400159 - 0.85373472095314 * r; }

        float snoise(vec3 v) {
          const vec2  C = vec2(1.0/6.0, 1.0/3.0) ;
          const vec4  D = vec4(0.0, 0.5, 1.0, 2.0);
          vec3 i  = floor(v + dot(v, C.yyy) );
          vec3 x0 = v - i + dot(i, C.xxx) ;
          vec3 g = step(x0.yzx, x0.xyz);
          vec3 l = 1.0 - g;
          vec3 i1 = min( g.xyz, l.zxy );
          vec3 i2 = max( g.xyz, l.zxy );
          vec3 x1 = x0 - i1 + C.xxx;
          vec3 x2 = x0 - i2 + C.yyy; 
          vec3 x3 = x0 - D.yyy;      
          i = mod289(i);
          vec4 p = permute( permute( permute(
                     i.z + vec4(0.0, i1.z, i2.z, 1.0 ))
                   + i.y + vec4(0.0, i1.y, i2.y, 1.0 ))
                   + i.x + vec4(0.0, i1.x, i2.x, 1.0 ));
          float n_ = 0.142857142857;
          vec3  ns = n_ * D.wyz - D.xzx;
          vec4 j = p - 49.0 * floor(p * ns.z * ns.z);
          vec4 x_ = floor(j * ns.z);
          vec4 y_ = floor(j - 7.0 * x_ );
          vec4 x = x_ *ns.x + ns.yyyy;
          vec4 y = y_ *ns.x + ns.yyyy;
          vec4 h = 1.0 - abs(x) - abs(y);
          vec4 b0 = vec4( x.xy, y.xy );
          vec4 b1 = vec4( x.zw, y.zw );
          vec4 s0 = floor(b0)*2.0 + 1.0;
          vec4 s1 = floor(b1)*2.0 + 1.0;
          vec4 sh = -step(h, vec4(0.0));
          vec4 a0 = b0.xzyw + s0.xzyw*sh.xxyy ;
          vec4 a1 = b1.xzyw + s1.xzyw*sh.zzww ;
          vec3 p0 = vec3(a0.xy,h.x);
          vec3 p1 = vec3(a0.zw,h.y);
          vec3 p2 = vec3(a1.xy,h.z);
          vec3 p3 = vec3(a1.zw,h.w);
          vec4 norm = taylorInvSqrt(vec4(dot(p0,p0), dot(p1,p1), dot(p2, p2), dot(p3,p3)));
          p0 *= norm.x; p1 *= norm.y; p2 *= norm.z; p3 *= norm.w;
          vec4 m = max(0.5 - vec4(dot(x0,x0), dot(x1,x1), dot(x2,x2), dot(x3,x3)), 0.0);
          m = m * m;
          return 42.0 * dot( m*m, vec4( dot(p0,x0), dot(p1,x1), dot(p2,x2), dot(p3,x3) ) );
        }

        // FBM for unified, complex fluid flow - Simplified to 2 octaves for 'merged' look
        float fbm(vec3 p) {
          float v = 0.0;
          float a = 0.6; // Slightly more persistence for the first octave
          for (int i = 0; i < 2; i++) {
            v += a * snoise(p);
            p *= 2.0;
            a *= 0.4;
          }
          return v;
        }

        void main() {
          vNormal = normalize(normalMatrix * normal);
          vPosition = position;

          // Random, asymmetrical swirling of noise coordinates instead of rotating the whole mesh
          float s = sin(u_time * 0.15);
          float c = cos(u_time * 0.15);
          mat3 rot = mat3(
             c, 0.0, s,
            0.0, 1.0, 0.0,
            -s, 0.0, c
          );
          
          vec3 noisePos = rot * position * u_frequency + vec3(u_time * 0.1, u_time * 0.2, -u_time * 0.1);
          float n = fbm(noisePos);
          vNoise = n;

          // Low-frequency 'blob mask' to merge areas randomly
          float mask = snoise(rot * position * 0.4 + u_time * 0.1);
          float combinedNoise = n * (mask * 0.4 + 0.6);

          // Scaled down displacement slightly to prevent extreme oval distortion
          float displacement = combinedNoise * (u_amplitude * 0.4 + 0.05);
          vec3 newPosition = position + normal * displacement;
          gl_Position = projectionMatrix * modelViewMatrix * vec4(newPosition, 1.0);
        }
      `,
      fragmentShader: `
        uniform vec3 u_color;
        uniform float u_amplitude;
        varying vec3 vNormal;
        varying vec3 vPosition;
        varying float vNoise;

        void main() {
          vec3 viewDirection = normalize(cameraPosition - vPosition);
          float fresnel = 1.0 - abs(dot(viewDirection, vNormal));
          
          // Layer 1: Base hollow shell (the "dormant" look) - Broadened rim
          float alpha1 = smoothstep(0.4, 1.0, fresnel);
          
          // Layer 2: Unified asymmetric flow overlay - Broadened flow
          float alpha2 = smoothstep(0.2, 1.0, fresnel * (vNoise * 0.5 + 0.5));
          
          // Layer 3: High-energy liquid peaks
          float alpha3 = smoothstep(0.65, 1.0, vNoise) * (u_amplitude + 0.1);

          // Superimpose layers for a volumetric unified flow
          float finalAlpha = (alpha1 * 0.5 + alpha2 * 0.4 + alpha3 * 0.4) * 0.7;

          // Unified cyan color with intensity variation based on noise flow
          vec3 color = u_color * (1.0 + vNoise * 0.4);

          gl_FragColor = vec4(color, finalAlpha);
        }
      `,
    });

    const mesh = new THREE.Mesh(geometry, material);
    scene.add(mesh);

    // Resize handler using ResizeObserver for better reliability in flex/grid layouts
    const resizeObserver = new ResizeObserver((entries) => {
      if (!entries[0]) return;
      const { width: w, height: h } = entries[0].contentRect;

      // Prevent division by zero and unnecessary updates
      if (w <= 0 || h <= 0) return;

      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      renderer.setSize(w, h);

      // Force a re-render immediately on resize to prevent black flashes
      renderer.render(scene, camera);
    });
    resizeObserver.observe(container);

    // Animation loop
    let animationFrameId: number;
    const startTime = performance.now();

    const animate = () => {
      animationFrameId = requestAnimationFrame(animate);
      const t = (performance.now() - startTime) / 1000; // Convert to seconds

      const state = stateRef.current;
      const target = {
        Idle: { amp: 0.05, freq: 0.5, speed: 0.1 },
        Listening: { amp: 0.15, freq: 0.8, speed: 0.4 },
        UserSpeaking: { amp: 0.4, freq: 1.0, speed: 1.2 },
        Thinking: { amp: 0.35, freq: 1.2, speed: 1.8 },
        AssistantSpeaking: { amp: 0.55, freq: 1.1, speed: 1.5 },
        Interrupted: { amp: 0.05, freq: 0.4, speed: 0.1 },
      }[state] || { amp: 0.05, freq: 0.5, speed: 0.1 };

      // Determine telemetry impact
      let teleAmp = 0;
      let teleFreq = 0;
      
      if (state === 'UserSpeaking' && telemetryRef) {
        teleAmp = telemetryRef.current.energy * 0.5;
        teleFreq = telemetryRef.current.energy * 2.0;
      } else if (state === 'AssistantSpeaking' && telemetryRef) {
        // Actual audio telemetry from TTS playback
        teleAmp = telemetryRef.current.energy * 0.6; 
        teleFreq = telemetryRef.current.energy * 3.0;
      } else if (state === 'Thinking') {
        // Precomputed pulse for Thinking
        teleAmp = Math.sin(t * 4.0) * 0.1 + 0.1;
      } else if (state === 'Listening' && telemetryRef) {
        teleAmp = telemetryRef.current.energy * 0.2;
      }

      uniforms.u_time.value = t;

      // Asymmetric attack/release envelope on amplitude
      // Attack fast: orb reacts instantly to audio
      // Release slow: orb doesn't collapse between TTS chunks
      const currentAmp = uniforms.u_amplitude.value;
      const targetAmpFinal = target.amp + teleAmp;
      const rate = targetAmpFinal > currentAmp ? 0.12 : 0.025;
      uniforms.u_amplitude.value += (targetAmpFinal - currentAmp) * rate;

      uniforms.u_frequency.value +=
        ((target.freq + teleFreq) - uniforms.u_frequency.value) * 0.04;

      // Note: Fluidity is shader-driven via noise coordinate swirling.

      renderer.render(scene, camera);
    };

    animate();

    return () => {
      resizeObserver.disconnect();
      cancelAnimationFrame(animationFrameId);
      if (container.contains(renderer.domElement)) {
        container.removeChild(renderer.domElement);
      }
      geometry.dispose();
      material.dispose();
      renderer.dispose();
    };
  }, []);

  return (
    <div
      ref={mountRef}
      style={{ width: '100%', height: '100%', background: 'transparent', position: 'relative' }}
    />
  );
};