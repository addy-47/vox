import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { VoxLogoLoader } from "./VoxLogoLoader";

export { VoxLogoLoader };
export const ParticleOrb = VoxLogoLoader;

export interface OrbitalLoaderProps {
  /** Size variant: "sm" (compact card/popover), "md" (standard), "lg" (full screen/page) */
  size?: "sm" | "md" | "lg";
  /** Optional custom center graphic (defaults to VoxLogoLoader) */
  icon?: React.ComponentType<{ size?: number | "sm" | "md" | "lg"; className?: string }>;
  /** Whether to render as a full-screen fixed/absolute backdrop overlay */
  overlay?: boolean;
  /** Custom className for the container */
  className?: string;
}

/**
 * Textless, minimal ambient loading indicator per design spec §7.1.
 * Pure stroke-style Vox logo SVG loader with hardware-accelerated animations.
 */
export const OrbitalLoader: React.FC<OrbitalLoaderProps> = memo(
  ({
    size = "md",
    icon: IconComponent = VoxLogoLoader,
    overlay = false,
    className,
  }) => {
    const sizeConfig = {
      sm: 28,
      md: 44,
      lg: 68,
    }[size];

    const content = (
      <div className={cn("flex flex-col items-center justify-center select-none", className)}>
        <IconComponent size={sizeConfig} className="text-[rgb(var(--accent))]" />
      </div>
    );

    if (overlay) {
      return (
        <div className="absolute inset-0 z-30 flex flex-col items-center justify-center bg-[rgb(var(--background))]/90 backdrop-blur-3xl pointer-events-none select-none">
          {content}
        </div>
      );
    }

    return content;
  }
);

OrbitalLoader.displayName = "OrbitalLoader";
