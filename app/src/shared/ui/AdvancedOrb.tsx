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
      u_color_a: { value: new THREE.Color('#00dbe9') },
      u_color_b: { value: new THREE.Color('#008c96') },
    };

    const material = new THREE.ShaderMaterial({
      uniforms,
      wireframe: true,
      transparent: true,
      depthWrite: false,
      vertexShader: `
        uniform float u_time;
        uniform float u_amplitude;
        uniform float u_frequency;
        varying vec3 vNormal;
        varying float vDisplacement;

        void main() {
          vNormal = normalize(normalMatrix * normal);

          // Subtle displacement - less aggressive
          float wave = sin(position.y * u_frequency * 2.0 + u_time * 1.0)
                     * cos(position.x * u_frequency * 1.5 + u_time * 0.8)
                     * (u_amplitude * 0.25 + 0.05);

          vDisplacement = wave;
          vec3 newPosition = position + normal * wave;
          gl_Position = projectionMatrix * modelViewMatrix * vec4(newPosition, 1.0);
        }
      `,
      fragmentShader: `
        uniform vec3 u_color_a;
        uniform vec3 u_color_b;
        varying vec3 vNormal;
        varying float vDisplacement;

        void main() {
          float fresnel = 1.0 - abs(dot(vNormal, vec3(0.0, 0.0, 1.0)));
          
          // More hollow space: adjust smoothstep range for a thinner shell
          // Higher start value (0.7 instead of 0.4) makes the center clearer
          float alpha = smoothstep(0.7, 1.0, fresnel) * 0.6;

          // Subtle gradient shift
          vec3 color = mix(u_color_b, u_color_a, clamp(vDisplacement * 10.0 + 0.5, 0.0, 1.0));

          gl_FragColor = vec4(color, alpha);
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
        Idle: { a: new THREE.Color('#00dbe9'), b: new THREE.Color('#008c96'), amp: 0.0, freq: 1.0, speed: 1.0 },
        Listening: { a: new THREE.Color('#00dbe9'), b: new THREE.Color('#008c96'), amp: 0.1, freq: 1.5, speed: 1.5 },
        UserSpeaking: { a: new THREE.Color('#00dbe9'), b: new THREE.Color('#008c96'), amp: 0.2, freq: 2.0, speed: 2.0 },
        Thinking: { a: new THREE.Color('#b500e9'), b: new THREE.Color('#7a0096'), amp: 0.15, freq: 3.0, speed: 3.0 },
        AssistantSpeaking: { a: new THREE.Color('#00e98c'), b: new THREE.Color('#00965a'), amp: 0.25, freq: 2.0, speed: 2.0 },
        Interrupted: { a: new THREE.Color('#e90000'), b: new THREE.Color('#960000'), amp: 0.05, freq: 0.5, speed: 0.5 },
      }[state] || { a: new THREE.Color('#00dbe9'), b: new THREE.Color('#008c96'), amp: 0.0, freq: 1.0, speed: 1.0 };

      // Smoothly interpolate colors
      uniforms.u_color_a.value.lerp(target.a, 0.05);
      uniforms.u_color_b.value.lerp(target.b, 0.05);

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
      uniforms.u_amplitude.value +=
        ((target.amp + teleAmp) - uniforms.u_amplitude.value) * 0.1;
      uniforms.u_frequency.value +=
        ((target.freq + teleFreq) - uniforms.u_frequency.value) * 0.05;

      mesh.rotation.y += 0.004 * target.speed;
      mesh.rotation.x += 0.0015 * target.speed;

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