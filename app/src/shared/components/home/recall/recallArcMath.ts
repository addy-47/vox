/**
 * 3D Progressive Spatial Arc Math for Vox Recall Carousel.
 * Projects session cards into a deep, progressive spatial composition
 * overlapping the lower horizon of the Orb.
 *
 * Visual hierarchy:
 * - Center selected card: 100% scale (z=0, y=0, opacity=1.00)
 * - Distance 1 neighbors: ~88% scale (85–90%), subtle depth & inward angle, text fully readable
 * - Distance 2 neighbors: ~76% scale (72–80%), receding in perspective
 * - Distance 3+ neighbors: ~62% down to 50% scale (55–65%), ambient peripheral depth
 *
 * Pure math functions with zero heap allocation during 60 FPS animation/drag loops.
 */

export const RECALL_CARD_WIDTH = 240;
export const RECALL_CARD_HEIGHT = 180;

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
 * Calculates progressive scale based on distance from center slot:
 * - d = 0: 100% (1.00)
 * - d = 1: 88% (85–90%)
 * - d = 2: 76% (72–80%)
 * - d = 3: 62% (55–65%)
 * - d >= 4: 50%
 */
export function getProgressiveScale(d: number): number {
  if (d <= 1) return 1.0 - d * 0.12;
  if (d <= 2) return 0.88 - (d - 1) * 0.12;
  if (d <= 3) return 0.76 - (d - 2) * 0.14;
  return Math.max(0.62 - (d - 3) * 0.12, 0.48);
}

/**
 * Calculates horizontal displacement with progressive spacing so
 * overlapping cards maintain full title and metadata readability.
 */
export function getProgressiveX(offset: number, stageWidth: number): number {
  const sign = Math.sign(offset);
  const d = Math.abs(offset);
  const widthFactor = Math.min(Math.max(stageWidth / 1100, 0.88), 1.15);

  let baseDist: number;
  if (d <= 1) {
    baseDist = d * 202;
  } else if (d <= 2) {
    baseDist = 202 + (d - 1) * 172;
  } else if (d <= 3) {
    baseDist = 374 + (d - 2) * 148;
  } else {
    baseDist = 522 + (d - 3) * 120;
  }

  return sign * baseDist * widthFactor;
}

/**
 * Calculates spatial 3D transform properties for a card at a continuous offset from the active center.
 * @param offset Continuous distance from the center slot (0 = exact center, -1 = left neighbor, +1 = right neighbor)
 * @param stageWidth Total width of the carousel stage in pixels
 */
export function calculateArcTransform(offset: number, stageWidth: number): ArcCardTransform {
  const sign = Math.sign(offset);
  const d = Math.abs(offset);

  // Progressive horizontal placement
  const x = getProgressiveX(offset, stageWidth);

  // Perspective depth translation: center card is at z=0; flanking cards recede into z < 0
  const z = -Math.pow(d, 1.25) * 80;

  // Gentle progressive vertical displacement: subtle inward curve towards page center
  const y = -Math.pow(d, 1.2) * 8;

  // Gentle perspective inward rotation: flanking cards face slightly toward the central focus
  const rotateY = -sign * Math.min(d * 9, 22);

  // Progressive scale: 100% -> 88% -> 76% -> 62% -> 50%
  const scale = getProgressiveScale(d);

  // Progressive opacity falloff: preserving text readability on neighbors
  const opacity = Math.max(0.18, 1.0 - Math.pow(d, 1.15) * 0.18);

  // Discrete stacking index: center card is highest (z-50)
  const zIndex = Math.max(50 - Math.round(d * 10), 1);

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
