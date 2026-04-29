import React from "react";
import { cn } from "../lib/utils";

interface GlassCardProps extends React.HTMLAttributes<HTMLDivElement> {
  children: React.ReactNode;
  blur?: "sm" | "md" | "lg" | "xl" | "2xl" | "3xl";
}

export const GlassCard: React.FC<GlassCardProps> = ({ 
  children, 
  className, 
  blur = "2xl",
  ...props 
}) => {
  return (
    <div
      className={cn(
        "glass-card rounded-xl transition-all duration-300",
        blur === "sm" && "backdrop-blur-sm",
        blur === "md" && "backdrop-blur-md",
        blur === "lg" && "backdrop-blur-lg",
        blur === "xl" && "backdrop-blur-xl",
        blur === "2xl" && "backdrop-blur-2xl",
        blur === "3xl" && "backdrop-blur-3xl",
        className
      )}
      {...props}
    >
      {children}
    </div>
  );
};
