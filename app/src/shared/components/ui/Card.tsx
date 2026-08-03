import React from "react";
import { cn } from "@/shared/lib/utils";

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  children: React.ReactNode;
  blur?: "sm" | "md" | "lg" | "xl" | "2xl" | "3xl";
  elevation?: "whisper" | "surface" | "card" | "elevated";
  elevated?: boolean;
  layoutMode?: "full-max" | "full-min" | "small";
}

export const Card: React.FC<CardProps> = ({
  children,
  className,
  blur = "2xl",
  elevation,
  elevated = false,
  layoutMode,
  ...props
}) => {
  const isSmall = layoutMode === "small";

  const getElevationClass = () => {
    if (elevation === "whisper") return "bg-[rgba(var(--foreground),0.01)] border border-[rgba(var(--accent),0.04)]";
    if (elevation === "surface") return "bg-[rgba(var(--foreground),0.02)] border border-[rgba(var(--accent),0.08)]";
    if (elevation === "card" || elevated || elevation === "elevated") return "glass-card";
    return "glass";
  };

  return (
    <div
      className={cn(
        getElevationClass(),
        "rounded-xl transition-all duration-400 ease-in-out",
        blur === "sm" && "backdrop-blur-sm",
        blur === "md" && "backdrop-blur-md",
        blur === "lg" && "backdrop-blur-lg",
        blur === "xl" && "backdrop-blur-xl",
        blur === "2xl" && "backdrop-blur-2xl",
        blur === "3xl" && "backdrop-blur-3xl",
        isSmall && "w-full bg-transparent p-0 shadow-none border-transparent",
        className
      )}
      {...props}
    >
      {children}
    </div>
  );
};
