import React from "react";
import { cn } from "@/shared/lib/utils";

interface VoxLogoProps {
  className?: string;
  size?: number;
}

export const VoxLogo: React.FC<VoxLogoProps> = ({ className, size = 24 }) => {
  return (
    <div 
      className={cn("vox-logo-theme", className)} 
      style={{ width: size, height: size }}
    />
  );
};
