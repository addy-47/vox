import { memo } from "react";
import { cn } from "@/shared/lib/utils";

export interface CatalogSkeletonGridProps {
  count?: number;
  columnsClassName?: string;
}

export const CatalogSkeletonGrid = memo(function CatalogSkeletonGrid({
  count = 6,
  columnsClassName = "grid-cols-1 lg:grid-cols-2 xl:grid-cols-3",
}: CatalogSkeletonGridProps) {
  return (
    <div className={cn("grid gap-3 select-none", columnsClassName)}>
      {Array.from({ length: count }).map((_, idx) => (
        <div
          key={`skeleton-card-${idx}`}
          className="rounded-xl border border-[rgba(var(--foreground),0.06)] bg-[rgba(var(--foreground),0.02)] p-4 flex flex-col justify-between h-[130px] animate-pulse"
        >
          <div className="flex items-start gap-3">
            <div className="w-8 h-8 rounded-lg bg-[rgba(var(--foreground),0.06)] shrink-0" />
            <div className="flex flex-col gap-1.5 flex-1 min-w-0">
              <div className="h-3.5 w-28 rounded bg-[rgba(var(--foreground),0.08)]" />
              <div className="h-2.5 w-44 rounded bg-[rgba(var(--foreground),0.04)]" />
            </div>
          </div>
          <div className="pt-2 border-t border-[rgba(var(--foreground),0.04)] flex items-center justify-between">
            <div className="h-2.5 w-16 rounded bg-[rgba(var(--foreground),0.05)]" />
            <div className="h-2.5 w-12 rounded bg-[rgba(var(--foreground),0.05)]" />
          </div>
        </div>
      ))}
    </div>
  );
});

CatalogSkeletonGrid.displayName = "CatalogSkeletonGrid";
