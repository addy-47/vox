import React, { useEffect, useRef } from 'react';
import * as THREE from 'three';

interface AudioTelemetry {
  amplitude: number;
  frequency: number;
}

interface VoxOrbProps {
  amplitude?: number;
  frequency?: number;
}

export const VoxOrb: React.FC<VoxOrbProps> = ({ amplitude = 0.0, frequency = 1.0 }) => {
  const mountRef = useRef<HTMLDivElement>(null);
  const telemetryRef = useRef<AudioTelemetry>({ amplitude, frequency });

  // Update telemetry via ref so animation loop always has latest value
  useEffect(() => {
    telemetryRef.current = { amplitude, frequency };
  }, [amplitude, frequency]);

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
    container.appendChild(renderer.domElement);

    // Geometry
    const geometry = new THREE.SphereGeometry(2.0, 128, 128);

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

    // Resize handler
    const handleResize = () => {
      const w = container.clientWidth;
      const h = container.clientHeight;
      camera.aspect = w / h;
      camera.updateProjectionMatrix();
      renderer.setSize(w, h);
    };
    window.addEventListener('resize', handleResize);

    // Animation loop
    let animationFrameId: number;
    const clock = new THREE.Clock();

    const animate = () => {
      animationFrameId = requestAnimationFrame(animate);
      const t = clock.getElapsedTime();

      uniforms.u_time.value = t;
      uniforms.u_amplitude.value +=
        (telemetryRef.current.amplitude - uniforms.u_amplitude.value) * 0.1;
      uniforms.u_frequency.value +=
        (telemetryRef.current.frequency - uniforms.u_frequency.value) * 0.05;

      mesh.rotation.y += 0.004;
      mesh.rotation.x += 0.0015;

      renderer.render(scene, camera);
    };

    animate();

    return () => {
      window.removeEventListener('resize', handleResize);
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
      style={{ width: '100%', height: '100%', background: 'transparent' }}
    />
  );
};