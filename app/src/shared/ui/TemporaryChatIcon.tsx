import React, { memo } from "react";

interface TemporaryChatIconProps {
  size?: number;
  strokeWidth?: number;
  checked?: boolean;
  className?: string;
}

/**
 * Dotted circular chat-bubble icon for the temporary (private) session toggle.
 * Same 24px Lucide-style stroke grid as the neighbouring cluster icons.
 * Bubble outline is dashed (`_ _` gaps); a solid tick appears inside when active.
 */
export const TemporaryChatIcon: React.FC<TemporaryChatIconProps> = memo(
  ({ size = 14, strokeWidth = 1.75, checked = false, className }) => {
    return (
      <svg
        width={size}
        height={size}
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth={strokeWidth}
        strokeLinecap="round"
        strokeLinejoin="round"
        className={className}
        aria-hidden="true"
      >
        <path d="M7.9 20A9 9 0 1 0 4 16.1L2 22Z" strokeDasharray="2.6 2.2" />
        {checked && <path d="m9 11.2 2.2 2.2 4.3-4.4" strokeDasharray="none" />}
      </svg>
    );
  }
);
TemporaryChatIcon.displayName = "TemporaryChatIcon";
