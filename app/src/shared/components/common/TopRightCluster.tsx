import React, { memo } from "react";
import { cn } from "@/shared/lib/utils";
import { HelpTriggerButton } from "@/shared/components/help/HelpTriggerButton";
import { NotificationBell } from "@/shared/components/home/NotificationBell";

interface TopRightClusterProps {
  /** Help deepLink for the current page (e.g. "page:home"). */
  deepLink: string;
  className?: string;
}

/**
 * Shared top-right help + notification cluster. Every page mounts this
 * instead of its own copy so placement, styling, and behavior stay uniform.
 */
export const TopRightCluster: React.FC<TopRightClusterProps> = memo(
  ({ deepLink, className }) => {
    return (
      <div className={cn("flex items-center gap-1.5 pointer-events-none", className)}>
        <HelpTriggerButton deepLink={deepLink} className="pointer-events-auto" />
        <NotificationBell />
      </div>
    );
  }
);
TopRightCluster.displayName = "TopRightCluster";
