import React, { memo, useCallback, useEffect, useRef } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import type { PersonalMemoryRecord } from "@/services/memoryService";
import { MEMORY_COPY } from "@/data/memoryCopy";

export interface PersonalMemoryVersionNavProps {
  versions: PersonalMemoryRecord[];
  activeVersionRecord: PersonalMemoryRecord | null;
  displayedRecord: PersonalMemoryRecord | null;
  onSelectVersion: (record: PersonalMemoryRecord) => void;
  onCommitActiveVersion: (version: number) => Promise<void>;
  isRestoring?: boolean;
}

export const PersonalMemoryVersionNav: React.FC<PersonalMemoryVersionNavProps> = memo(
  ({
    versions,
    activeVersionRecord,
    displayedRecord,
    onSelectVersion,
    onCommitActiveVersion,
    isRestoring = false,
  }) => {
    const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    // Clean up pending debounce on unmount
    useEffect(() => {
      return () => {
        if (debounceTimerRef.current) {
          clearTimeout(debounceTimerRef.current);
        }
      };
    }, []);

    const currentRecord = displayedRecord ?? activeVersionRecord;
    const currentVersion = currentRecord?.version ?? 1;

    // versions is sorted DESC by version (latest first at index 0)
    const currentIndex = versions.findIndex((v) => v.version === currentVersion);
    const hasOlder = currentIndex >= 0 && currentIndex < versions.length - 1;
    const hasNewer = currentIndex > 0;

    const navigateToRecord = useCallback(
      (targetRecord: PersonalMemoryRecord) => {
        // 1. Immediately preview the target version content
        onSelectVersion(targetRecord);

        // 2. Clear existing debounce timer
        if (debounceTimerRef.current) {
          clearTimeout(debounceTimerRef.current);
        }

        // 3. Debounce setting active version directly after 500ms
        debounceTimerRef.current = setTimeout(() => {
          onCommitActiveVersion(targetRecord.version).catch((e) => {
            console.error("[PersonalMemoryVersionNav] Failed to set active version:", e);
          });
        }, 500);
      },
      [onSelectVersion, onCommitActiveVersion]
    );

    const handleOlder = useCallback(() => {
      if (hasOlder && !isRestoring) {
        navigateToRecord(versions[currentIndex + 1]);
      }
    }, [hasOlder, isRestoring, versions, currentIndex, navigateToRecord]);

    const handleNewer = useCallback(() => {
      if (hasNewer && !isRestoring) {
        navigateToRecord(versions[currentIndex - 1]);
      }
    }, [hasNewer, isRestoring, versions, currentIndex, navigateToRecord]);

    return (
      <div className="flex items-center gap-1.5 text-[12px] font-mono text-[rgb(var(--foreground-muted))] select-none">
        <button
          type="button"
          onClick={handleOlder}
          disabled={!hasOlder || isRestoring}
          aria-label={MEMORY_COPY.versions.prevVersion}
          title={MEMORY_COPY.versions.prevVersion}
          className={cn(
            "p-1 rounded text-[rgb(var(--foreground-muted))] transition-colors",
            hasOlder && !isRestoring
              ? "hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer"
              : "opacity-30 cursor-not-allowed"
          )}
        >
          <ChevronLeft size={13} />
        </button>
        <span className="px-0.5">
          {MEMORY_COPY.version} {currentVersion}
        </span>
        <button
          type="button"
          onClick={handleNewer}
          disabled={!hasNewer || isRestoring}
          aria-label={MEMORY_COPY.versions.nextVersion}
          title={MEMORY_COPY.versions.nextVersion}
          className={cn(
            "p-1 rounded text-[rgb(var(--foreground-muted))] transition-colors",
            hasNewer && !isRestoring
              ? "hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer"
              : "opacity-30 cursor-not-allowed"
          )}
        >
          <ChevronRight size={13} />
        </button>
      </div>
    );
  }
);

PersonalMemoryVersionNav.displayName = "PersonalMemoryVersionNav";
