import type { SessionRow } from "@/services/persistenceService";

export interface BoundaryCardData {
  id: string;
  isBoundary: true;
  label: string;
  count: number;
}

export interface SkeletonCardData {
  id: string;
  isSkeleton: true;
  label?: string;
}

export type CarouselItemData = SessionRow | BoundaryCardData | SkeletonCardData;

export function isBoundaryCard(item: CarouselItemData): item is BoundaryCardData {
  return "isBoundary" in item && Boolean((item as BoundaryCardData).isBoundary);
}

export function isSkeletonCard(item: CarouselItemData): item is SkeletonCardData {
  return "isSkeleton" in item && Boolean((item as SkeletonCardData).isSkeleton);
}
