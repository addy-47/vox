import React from "react";
import { cn } from "../lib/utils";

interface TypographyProps {
  children: React.ReactNode;
  className?: string;
}

export const H1: React.FC<TypographyProps> = ({ children, className }) => (
  <h1 className={cn("font-display text-4xl md:text-5xl font-bold tracking-tight text-white", className)}>
    {children}
  </h1>
);

export const H2: React.FC<TypographyProps> = ({ children, className }) => (
  <h2 className={cn("font-display text-2xl md:text-3xl font-semibold tracking-tight text-white", className)}>
    {children}
  </h2>
);

export const Body: React.FC<TypographyProps> = ({ children, className }) => (
  <p className={cn("font-sans text-base leading-relaxed text-on-surface", className)}>
    {children}
  </p>
);

export const Label: React.FC<TypographyProps> = ({ children, className }) => (
  <span className={cn("font-display text-[11px] md:text-xs font-bold tracking-[0.2em] uppercase text-[rgb(var(--foreground))]/40", className)}>
    {children}
  </span>
);
