import React from "react";
import { cn } from "../lib/utils";

interface PillButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: "primary" | "secondary" | "ghost";
  size?: "sm" | "md" | "lg";
}

export const PillButton: React.FC<PillButtonProps> = ({
  className,
  variant = "primary",
  size = "md",
  ...props
}) => {
  return (
    <button
      className={cn(
        "rounded-full font-display font-semibold transition-all duration-300 active:scale-95 disabled:opacity-60 disabled:pointer-events-none",
        variant === "primary" && "bg-gradient-to-r from-primary-container to-blue-600 text-on-primary shadow-lg shadow-primary-container/20 hover:shadow-primary-container/40",
        variant === "secondary" && "bg-white/5 border border-white/10 text-white/60 hover:text-white hover:bg-white/10",
        variant === "ghost" && "bg-transparent text-white/40 hover:text-white",
        size === "sm" && "px-4 py-1.5 text-xs",
        size === "md" && "px-6 py-2 text-sm",
        size === "lg" && "px-10 py-4 text-base",
        className
      )}
      {...props}
    />
  );
};
