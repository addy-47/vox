import type { MemoryRevisionView } from "@/services/memoryService";

export type StagingMode =
  | "idle"
  | "import"
  | "edit"
  | "comment"
  | "facts"
  | "suggestions";

export interface MemoryComment {
  id: string;
  line: number;
  quotedText: string;
  text: string;
  top: number;
  createdAt: number;
}

export interface ParsedBlock {
  secId?: string;
  secTitle?: string;
  blkId?: string;
  blkType?: string;
  text: string;
  isHeading?: boolean;
}

export interface SuggestionDiffItem {
  id: string;
  rev: MemoryRevisionView;
  block?: ParsedBlock;
  decision?: "accept" | "reject";
}

export interface PendingConfirmation {
  reason: "pending_queue_items" | "compaction_in_progress" | string;
  pendingCount: number;
}

