import React, { memo, useCallback, useRef, useEffect } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { cn } from "@/shared/lib/utils";
import type { PersonalMemoryRecord } from "@/services/memoryService";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { Tooltip } from "@/shared/ui/Tooltip";

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
  }) => {
    const currentRecord = displayedRecord ?? activeVersionRecord;
    const currentVersion = currentRecord?.version ?? 1;

    // versions is sorted DESC by version (latest first at index 0)
    const currentIndex = versions.findIndex((v) => v.version === currentVersion);
    const hasOlder = currentIndex >= 0 && currentIndex < versions.length - 1;
    const hasNewer = currentIndex > 0;

    const debounceTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

    useEffect(() => {
      return () => {
        if (debounceTimerRef.current) {
          clearTimeout(debounceTimerRef.current);
        }
      };
    }, []);

    // Instant visual navigation + 500ms debounced active version commitment
    const navigateToRecord = useCallback(
      (targetRecord: PersonalMemoryRecord) => {
        onSelectVersion(targetRecord);

        if (debounceTimerRef.current) {
          clearTimeout(debounceTimerRef.current);
        }

        debounceTimerRef.current = setTimeout(() => {
          onCommitActiveVersion(targetRecord.version);
        }, 500);
      },
      [onSelectVersion, onCommitActiveVersion]
    );

    const handleOlder = useCallback(() => {
      if (hasOlder) {
        navigateToRecord(versions[currentIndex + 1]);
      }
    }, [hasOlder, versions, currentIndex, navigateToRecord]);

    const handleNewer = useCallback(() => {
      if (hasNewer) {
        navigateToRecord(versions[currentIndex - 1]);
      }
    }, [hasNewer, versions, currentIndex, navigateToRecord]);

    return (
      <div className="flex items-center gap-1.5 text-[12px] font-mono text-[rgb(var(--foreground-muted))] select-none">
        <Tooltip label={MEMORY_COPY.versions.prevVersion}>
          <button
            type="button"
            onClick={handleOlder}
            disabled={!hasOlder}
            aria-label={MEMORY_COPY.versions.prevVersion}
            className={cn(
              "p-1 rounded text-[rgb(var(--foreground-muted))] transition-colors",
              hasOlder
                ? "hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer"
                : "opacity-30 cursor-not-allowed"
            )}
          >
            <ChevronLeft size={13} />
          </button>
        </Tooltip>
        <span className="px-0.5">
          {MEMORY_COPY.version} {currentVersion}
        </span>
        <Tooltip label={MEMORY_COPY.versions.nextVersion}>
          <button
            type="button"
            onClick={handleNewer}
            disabled={!hasNewer}
            aria-label={MEMORY_COPY.versions.nextVersion}
            className={cn(
              "p-1 rounded text-[rgb(var(--foreground-muted))] transition-colors",
              hasNewer
                ? "hover:text-[rgb(var(--foreground))] hover:bg-[rgba(var(--foreground),0.06)] cursor-pointer"
                : "opacity-30 cursor-not-allowed"
            )}
          >
            <ChevronRight size={13} />
          </button>
        </Tooltip>
      </div>
    );
  }
);

PersonalMemoryVersionNav.displayName = "PersonalMemoryVersionNav";
