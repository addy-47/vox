import React, { memo, useCallback } from "react";
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
    // NOTE: `onCommitActiveVersion` is still declared on the props interface but
    // this component no longer calls it. Browsing is preview-only; promoting a
    // revision to canonical is an explicit user action (the dossier's Restore
    // button). Previously a 500ms debounce fired the DB write on *browse*, so a
    // stray double-click silently rewrote the user's active profile.
    // REVERT: restore the debounce in navigateToRecord below.
    onCommitActiveVersion: _onCommitActiveVersion,
  }) => {
    const currentRecord = displayedRecord ?? activeVersionRecord;
    const currentVersion = currentRecord?.version ?? 1;

    // versions is sorted DESC by version (latest first at index 0)
    const currentIndex = versions.findIndex((v) => v.version === currentVersion);
    const hasOlder = currentIndex >= 0 && currentIndex < versions.length - 1;
    const hasNewer = currentIndex > 0;

    // Preview only. No IPC, no debounce, no write.
    const navigateToRecord = useCallback(
      (targetRecord: PersonalMemoryRecord) => {
        onSelectVersion(targetRecord);
      },
      [onSelectVersion]
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
        <button
          type="button"
          onClick={handleOlder}
          disabled={!hasOlder}
          aria-label={MEMORY_COPY.versions.prevVersion}
          title={MEMORY_COPY.versions.prevVersion}
          className={cn(
            "p-1 rounded text-[rgb(var(--foreground-muted))] transition-colors",
            hasOlder
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
          disabled={!hasNewer}
          aria-label={MEMORY_COPY.versions.nextVersion}
          title={MEMORY_COPY.versions.nextVersion}
          className={cn(
            "p-1 rounded text-[rgb(var(--foreground-muted))] transition-colors",
            hasNewer
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
