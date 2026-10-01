import React, { memo } from "react";
import { Sparkles, LucideIcon } from "lucide-react";
import { cn } from "@/shared/lib/utils";

export interface OrbitalLoaderProps {
  title?: string;
  subtitle?: string;
  statusText?: string;
  size?: "sm" | "md" | "lg";
  icon?: LucideIcon;
  overlay?: boolean;
  className?: string;
}

export const OrbitalLoader: React.FC<OrbitalLoaderProps> = memo(
  ({
    size = "md",
    icon: IconComponent = Sparkles,
    overlay = false,
    className,
  }) => {
    // Sizing scale maps (centered with no bottom margin offset)
    const sizeConfig = {
      sm: {
        container: "w-16 h-16",
        innerP: "p-2.5",
        iconSize: 18,
      },
      md: {
        container: "w-24 h-24",
        innerP: "p-3.5",
        iconSize: 24,
      },
      lg: {
        container: "w-28 h-28",
        innerP: "p-4",
        iconSize: 30,
      },
    }[size];

    const content = (
      <div className={cn("flex flex-col items-center justify-center select-none", className)}>
        {/* Orbital Glowing Central Core */}
        <div className={cn("relative flex items-center justify-center", sizeConfig.container)}>
          {/* Ambient outer pulse aura */}
          <div className="absolute inset-0 rounded-full bg-[rgb(var(--accent))]/10 animate-ping duration-1000" />

          {/* Clockwise rotating ring */}
          <div className="absolute inset-1.5 sm:inset-2 rounded-full border border-[rgb(var(--accent))]/25 animate-spin duration-[6000ms]" />

          {/* Counter-clockwise dashed resonance ring */}
          <div className="absolute inset-3.5 sm:inset-5 rounded-full border border-dashed border-[rgb(var(--accent))]/40 animate-spin duration-[10000ms] [animation-direction:reverse]" />

          {/* Central Glowing Orb Core */}
          <div
            className={cn(
              "relative z-10 rounded-full bg-[rgb(var(--accent))]/15 text-[rgb(var(--accent))] shadow-[0_0_40px_rgba(var(--accent),0.35)] border border-[rgba(var(--accent),0.3)]",
              sizeConfig.innerP
            )}
          >
            <IconComponent size={sizeConfig.iconSize} className="animate-pulse text-[rgb(var(--accent))]" />
          </div>
        </div>
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
