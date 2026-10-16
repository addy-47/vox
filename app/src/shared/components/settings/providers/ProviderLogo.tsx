import { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface ProviderLogoProps {
  logo?: string | null;
  providerId?: string;
  size?: number;
  className?: string;
}

export interface ProviderLogoDef {
  id: string;
  aliases: readonly string[];
  render: (props: { size: number; className?: string }) => JSX.Element;
}

export const FallbackServerLogo = ({ size, className }: { size: number; className?: string }) => (
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    strokeWidth="2"
    strokeLinecap="round"
    strokeLinejoin="round"
    className={cn("shrink-0 text-[rgb(var(--accent))]", className)}
  >
    <rect x="2" y="4" width="20" height="16" rx="2" />
    <line x1="6" y1="12" x2="6.01" y2="12" strokeWidth="2.5" />
    <line x1="10" y1="12" x2="18" y2="12" />
  </svg>
);

export const PROVIDER_LOGO_REGISTRY: ProviderLogoDef[] = [];

export function getProviderLogoDef(_norm: string): ProviderLogoDef | undefined {
  return undefined;
}

export const ProviderLogo = memo(function ProviderLogo({
  logo,
  size = 16,
  className,
}: ProviderLogoProps) {
  if (!logo) return <FallbackServerLogo size={size} className={className} />;
  return <img src={logo} width={size} height={size} alt="" className={cn("shrink-0", className)} />;
});

ProviderLogo.displayName = "ProviderLogo";