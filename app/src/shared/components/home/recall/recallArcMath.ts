/**
 * 3D Cylindrical Arc Math for Vox Recall Carousel.
 * Projects cards along a horizontal cylindrical arc overlapping the Orb bottom.
 * Pure math functions with zero heap allocation during 60 FPS animation/drag loops.
 */

export const RECALL_CARD_WIDTH = 224;
export const RECALL_CARD_HEIGHT = 168;

export interface ArcCardTransform {
  x: number;
  y: number;
  z: number;
  rotateY: number;
  scale: number;
  opacity: number;
  zIndex: number;
  transform: string;
}

/**
 * Calculates transform properties for a card at a continuous offset from the active center.
 * @param offset Continuous distance from the center slot (e.g. 0 = exact center, -1 = one card to the left)
 * @param stageWidth Total width of the carousel stage in pixels
 */
export function calculateArcTransform(offset: number, stageWidth: number): ArcCardTransform {
  // Angular step per card slot in radians (~11.5 degrees)
  const angularStep = 0.20;
  // Arc radius proportional to stage width, clamped to comfortable viewing bounds
  const radius = Math.min(Math.max(stageWidth * 0.75, 600), 1100);

  const angle = offset * angularStep;
  const absOffset = Math.abs(offset);

  // Horizontal translation along circular cylinder
  const x = radius * Math.sin(angle);

  // Depth translation: center card is at z=0, flanking cards recede into depth (z < 0)
  const z = radius * (Math.cos(angle) - 1);

  // Upward arc curvature matching the mockup: outer cards lift slightly in Y
  // Lift is proportional to the square of normalized horizontal displacement
  const normX = Math.min(Math.abs(x) / (radius * 0.85), 1.2);
  const y = -Math.pow(normX, 2) * 48;

  // Inward rotation: cards face toward the central focal point
  // Left cards (offset < 0) rotate positive Y; Right cards rotate negative Y
  const rotateY = -angle * (180 / Math.PI) * 0.75;

  // Scale falloff: center card is 1.0, outer cards attenuate down to 0.72
  const scale = Math.max(1.0 - absOffset * 0.065, 0.70);

  // Opacity falloff: center card is 1.0, outer cards fade gently
  const opacity = Math.max(1.0 - absOffset * 0.14, 0.25);

  // Discrete stacking index: center card is highest (z-50)
  const zIndex = Math.max(50 - Math.round(absOffset * 2), 1);

  const transform = `translate3d(${x.toFixed(1)}px, ${y.toFixed(1)}px, ${z.toFixed(1)}px) rotateY(${rotateY.toFixed(1)}deg) scale(${scale.toFixed(3)})`;

  return {
    x,
    y,
    z,
    rotateY,
    scale,
    opacity,
    zIndex,
    transform,
  };
}
