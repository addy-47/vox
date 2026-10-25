import { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { CatalogSkeletonGrid } from "./CatalogSkeletonGrid";

export interface CatalogModalShellProps {
  toolbar?: React.ReactNode;
  facetRail?: React.ReactNode;
  scrollRef?: React.RefObject<HTMLDivElement | null>;
  footer?: React.ReactNode;
  isLoading?: boolean;
  skeletonGrid?: React.ReactNode;
  children: React.ReactNode;
  guideView?: React.ReactNode;
  isGuideActive?: boolean;
  emptyState?: React.ReactNode;
  isEmpty?: boolean;
  contentClassName?: string;
}

export const CatalogModalShell = memo(function CatalogModalShell({
  toolbar,
  facetRail,
  scrollRef,
  footer,
  isLoading = false,
  skeletonGrid,
  children,
  guideView,
  isGuideActive = false,
  emptyState,
  isEmpty = false,
  contentClassName,
}: CatalogModalShellProps) {
  return (
    <div className="flex flex-col h-full min-h-0 gap-3">
      {toolbar ? toolbar : null}

      <div className="flex flex-1 min-h-0 gap-4 overflow-hidden">
        {/* Desktop Left Rail: 200px fixed width, hidden below lg (1024px) */}
        {facetRail && (
          <aside
            aria-label="Filter Rail"
            className="w-[200px] shrink-0 hidden lg:flex flex-col gap-6 border-r border-[rgba(var(--foreground),0.06)] pr-3 py-1 overflow-y-auto custom-scrollbar select-none"
          >
            {facetRail}
          </aside>
        )}

        {/* Main Content Area */}
        {isGuideActive && guideView ? (
          <div className="flex-1 min-h-0 h-full overflow-hidden animate-fade-in">
            {guideView}
          </div>
        ) : (
          <div
            ref={scrollRef}
            className={cn(
              "flex-1 min-h-0 overflow-y-auto overscroll-contain custom-scrollbar p-1 sm:p-2 contain-paint transform-gpu",
              contentClassName
            )}
          >
            {isLoading ? (
              skeletonGrid || <CatalogSkeletonGrid />
            ) : isEmpty && emptyState ? (
              emptyState
            ) : (
              children
            )}
          </div>
        )}
      </div>

      {footer && <div className="shrink-0 w-full">{footer}</div>}
    </div>
  );
});

CatalogModalShell.displayName = "CatalogModalShell";
