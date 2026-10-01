import { useState, useEffect, useCallback, useRef, useMemo } from "react";
import {
  getPersonalMemory,
  getPersonalMemoryVersions,
  setActivePersonalMemoryVersion,
  savePersonalMemory,
  consolidatePersonalMemory,
  regeneratePersonalMemory,
  getMemoryRevisions,
  resolveMemoryRevisions,
  type PersonalMemoryRecord,
  type ObservationRecord,
  type MemoryRevisionView,
  type ConfirmationReason,
} from "@/services/memoryService";
import { useObservationsList, type ObservationFilter } from "@/shared/hooks/useObservationsList";
import { useRegisterPageDrawer } from "@/shared/context/PageDrawerContext";
import { useMemoryStore } from "@/store/memoryStore";
import { copyToClipboard } from "@/shared/lib/clipboard";
import type { StagingMode, MemoryComment, SelectionAnchor } from "@/shared/components/memory";
import { MEMORY_COPY } from "@/data/memoryCopy";

export interface UsePersonalMemoryDrawerProps {
  facts: ObservationRecord[];
  onRefreshFacts?: (silent?: boolean) => Promise<void>;
  onError?: (err: string) => void;
}

export interface UsePersonalMemoryDrawerReturn {
  // Drawer visibility & registration
  drawerOpen: boolean;
  openDrawer: () => void;
  closeDrawer: () => void;
  drawerBodyReady: boolean;

  // Personal memory model records
  personalMemory: PersonalMemoryRecord | null;
  displayedRecord: PersonalMemoryRecord | null;
  versions: PersonalMemoryRecord[];
  suggestions: MemoryRevisionView[];

  // Actions & Staging
  isRestoringVersion: boolean;
  handleSelectVersion: (rec: PersonalMemoryRecord) => void;
  handleRestoreActive: (version: number) => Promise<void>;
  handleCopyDoc: () => Promise<void>;
  copied: boolean;
  handleRegenerateFromFacts: () => Promise<void>;
  isRegenerating: boolean;
  saving: boolean;
  leftFlash: boolean;
  veilCycle: number;
  handleVeilReady: () => void;
  isCommitting: boolean;
  consolidating: boolean;

  // Staging Mode & Workflows
  stagingMode: StagingMode;
  setStagingMode: React.Dispatch<React.SetStateAction<StagingMode>>;
  handleStagingModeChange: (m: StagingMode) => void;
  handleSaveStaging: (content: string) => Promise<void>;
  handleConsolidateNow: (forced?: boolean) => Promise<void>;
  handleApplySuggestions: (decisionsMap: Record<string, "accept" | "reject">) => Promise<void>;
  isApplyingSuggestions: boolean;
  justCommitted: boolean;
  setJustCommitted: React.Dispatch<React.SetStateAction<boolean>>;

  // Commentary & Selection
  dossierContainerRef: React.RefObject<HTMLDivElement | null>;
  selectionAnchor: SelectionAnchor | null;
  isComposingComment: boolean;
  setIsComposingComment: (v: boolean) => void;
  handleAddComment: (comment: { line: number; quotedText: string; text: string; top: number }) => void;
  handleCancelComment: () => void;
  handleDeleteComment: (id: string) => void;
  handleUpdateComment: (id: string, text: string) => void;
  handleClearComments: () => void;
  comments: MemoryComment[];
  handleRegenerateWithComments: (commentsToApply: MemoryComment[], forced?: boolean) => Promise<void>;

  // Pending Confirmation
  pendingConfirmation: { reason: ConfirmationReason; pendingCount: number } | null;
  handleConfirmPendingIntegration: () => void;
  handleCancelPendingConfirmation: () => void;

  // Candidate Facts & Observations List
  identityCandidateFacts: ObservationRecord[];
  unconsolidatedIdentityCount: number;
  paginatedObservations: ObservationRecord[];
  obsStatusFilter: ObservationFilter;
  setObsStatusFilter: (f: ObservationFilter) => void;
  obsLoading: boolean;
  obsLoadingMore: boolean;
  obsHasMore: boolean;
  obsLoadMore: () => Promise<void>;

  // Manual refresh
  refreshPersonalMemory: (isSilent?: boolean) => Promise<void>;
}

export function usePersonalMemoryDrawer({
  facts,
  onRefreshFacts,
  onError,
}: UsePersonalMemoryDrawerProps): UsePersonalMemoryDrawerReturn {
  // Records & versions
  const [personalMemory, setPersonalMemory] = useState<PersonalMemoryRecord | null>(null);
  const [displayedRecord, setDisplayedRecord] = useState<PersonalMemoryRecord | null>(null);
  const [versions, setVersions] = useState<PersonalMemoryRecord[]>([]);
  const [suggestions, setSuggestions] = useState<MemoryRevisionView[]>([]);
  const [isApplyingSuggestions, setIsApplyingSuggestions] = useState(false);
  const [isRestoringVersion, setIsRestoringVersion] = useState(false);
  const [isRegenerating, setIsRegenerating] = useState(false);
  const [saving, setSaving] = useState(false);
  const [consolidating, setConsolidating] = useState(false);
  const [isCommitting, setIsCommitting] = useState(false);
  const [leftFlash, setLeftFlash] = useState(false);
  const [copied, setCopied] = useState(false);
  const [justCommitted, setJustCommitted] = useState(false);

  // Drawer open / body ready state
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [drawerBodyReady, setDrawerBodyReady] = useState(false);
  const [stagingMode, setStagingMode] = useState<StagingMode>("idle");

  // Confirmation dialog state
  const [pendingConfirmation, setPendingConfirmation] = useState<{
    reason: ConfirmationReason;
    pendingCount: number;
  } | null>(null);
  const [pendingActionType, setPendingActionType] = useState<"consolidate" | "regenerate">("consolidate");

  // Selection & comment state
  const [selectionAnchor, setSelectionAnchor] = useState<SelectionAnchor | null>(null);
  const [isComposingComment, setIsComposingComment] = useState(false);
  const isComposingCommentRef = useRef(false);
  useEffect(() => {
    isComposingCommentRef.current = isComposingComment;
  }, [isComposingComment]);

  const dossierContainerRef = useRef<HTMLDivElement>(null);

  // Zustand persistent comments
  const comments = useMemoryStore((s) => s.pendingComments);
  const reopenToComments = useMemoryStore((s) => s.reopenToComments);
  const storeAddComment = useMemoryStore((s) => s.addComment);
  const storeUpdateComment = useMemoryStore((s) => s.updateComment);
  const storeDeleteComment = useMemoryStore((s) => s.deleteComment);
  const storeClearComments = useMemoryStore((s) => s.clearComments);
  const storeSetReopenToComments = useMemoryStore((s) => s.setReopenToComments);

  // Observations list (paginated, filtered)
  const {
    observations: paginatedObservations,
    statusFilter: obsStatusFilter,
    setStatusFilter: setObsStatusFilter,
    isLoading: obsLoading,
    isLoadingMore: obsLoadingMore,
    hasMore: obsHasMore,
    loadMore: obsLoadMore,
  } = useObservationsList(stagingMode === "facts", undefined, "personal");

  // Mounted guard
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  // Tracked timers (§4.4)
  const timersRef = useRef<Set<ReturnType<typeof setTimeout>>>(new Set());
  useEffect(() => {
    const timers = timersRef.current;
    return () => {
      timers.forEach((id) => clearTimeout(id));
      timers.clear();
    };
  }, []);

  const later = useCallback((fn: () => void, ms: number) => {
    const id = setTimeout(() => {
      timersRef.current.delete(id);
      fn();
    }, ms);
    timersRef.current.add(id);
    return id;
  }, []);

  // Commit-veil sequencing
  const [veilCycle, setVeilCycle] = useState(0);
  const pendingSwapRef = useRef<PersonalMemoryRecord | null>(null);
  const veilReadyRef = useRef(false);

  const handleVeilReady = useCallback(() => {
    veilReadyRef.current = true;
    const pending = pendingSwapRef.current;
    if (pending) {
      pendingSwapRef.current = null;
      setPersonalMemory(pending);
      setDisplayedRecord(pending);
    }
  }, []);

  const stageSwap = useCallback((rec: PersonalMemoryRecord) => {
    if (veilReadyRef.current) {
      setPersonalMemory(rec);
      setDisplayedRecord(rec);
    } else {
      pendingSwapRef.current = rec;
    }
  }, []);

  const raiseVeil = useCallback(() => {
    veilReadyRef.current = false;
    pendingSwapRef.current = null;
    setVeilCycle((c) => c + 1);
    setLeftFlash(true);
    setIsCommitting(true);
  }, []);

  // Drawer handlers & Page Drawer registration
  const openDrawer = useCallback(() => {
    setStagingMode("idle");
    setDrawerOpen(true);
  }, []);

  const closeDrawer = useCallback(() => {
    if (comments.length > 0) {
      storeSetReopenToComments(true);
    }
    setDrawerOpen(false);
    setStagingMode("idle");
  }, [comments.length, storeSetReopenToComments]);

  const drawerHandlers = useMemo(
    () => ({
      open: openDrawer,
      close: closeDrawer,
    }),
    [openDrawer, closeDrawer]
  );
  useRegisterPageDrawer(drawerHandlers);

  // Deferred drawer body ready
  useEffect(() => {
    if (!drawerOpen) {
      setDrawerBodyReady(false);
      return;
    }
    let cancelled = false;
    const id1 = requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        if (!cancelled) setDrawerBodyReady(true);
      })
    );
    return () => {
      cancelled = true;
      cancelAnimationFrame(id1);
    };
  }, [drawerOpen]);

  // Auto-reopen to comment section if comments were pending when drawer was closed
  useEffect(() => {
    if (drawerOpen && reopenToComments && comments.length > 0) {
      setStagingMode("comment");
      storeSetReopenToComments(false);
    }
    return undefined;
  }, [drawerOpen, reopenToComments, comments.length, storeSetReopenToComments]);

  // Derived Candidate observations from facts
  const identityCandidateFacts = useMemo(() => {
    return facts.filter((f) => f.fact_type === "personal");
  }, [facts]);

  const unconsolidatedIdentityCount = identityCandidateFacts.length;

  // Refresh personal memory records
  const refreshPersonalMemory = useCallback(async (_isSilent = false) => {
    try {
      const [mem, allVersions, allSuggestions] = await Promise.all([
        getPersonalMemory(),
        getPersonalMemoryVersions(),
        getMemoryRevisions().catch(() => []),
      ]);
      if (!mountedRef.current) return;
      setPersonalMemory(mem);
      setDisplayedRecord(mem);
      setVersions(allVersions);
      setSuggestions(allSuggestions);
      if (allSuggestions.length > 0) {
        setStagingMode("suggestions");
      }
    } catch (e) {
      console.error("[usePersonalMemoryDrawer] Failed to load personal memory:", e);
      if (mountedRef.current && onError) {
        onError(MEMORY_COPY.loadFailedDesc);
      }
    }
  }, [onError]);

  useEffect(() => {
    refreshPersonalMemory();
  }, [refreshPersonalMemory]);

  // Version selection & restore
  const handleSelectVersion = useCallback((rec: PersonalMemoryRecord) => {
    setDisplayedRecord(rec);
  }, []);

  const handleRestoreActive = useCallback(
    async (version: number) => {
      setIsRestoringVersion(true);
      try {
        const restored = await setActivePersonalMemoryVersion(version);
        setPersonalMemory(restored);
        setDisplayedRecord(restored);
        const allVersions = await getPersonalMemoryVersions();
        setVersions(allVersions);
        await onRefreshFacts?.(true);
      } catch (e) {
        console.error("[usePersonalMemoryDrawer] Restore version failed:", e);
        if (onError) onError(MEMORY_COPY.loadFailedDesc);
      } finally {
        setIsRestoringVersion(false);
      }
    },
    [onRefreshFacts, onError]
  );

  const handleSaveStaging = useCallback(
    async (content: string) => {
      if (!personalMemory) return;
      setSaving(true);
      raiseVeil();

      try {
        const updated = await savePersonalMemory(content, personalMemory.version);
        stageSwap(updated);
        const allVersions = await getPersonalMemoryVersions();
        setVersions(allVersions);

        later(() => {
          setStagingMode("idle");
        }, 300);

        later(() => {
          setIsCommitting(false);
          setLeftFlash(false);
        }, 900);
      } catch (e) {
        console.error("[usePersonalMemoryDrawer] Save failed:", e);
        pendingSwapRef.current = null;
        setIsCommitting(false);
        setLeftFlash(false);
      } finally {
        setSaving(false);
      }
    },
    [personalMemory, later, raiseVeil, stageSwap]
  );

  const handleConsolidateNow = useCallback(
    async (forced = false) => {
      if (consolidating) return;
      setPendingActionType("consolidate");
      setConsolidating(true);
      raiseVeil();

      try {
        const outcome = await consolidatePersonalMemory(
          undefined,
          undefined,
          forced
        );

        if (outcome.status === "completed") {
          stageSwap(outcome.record);

          const [allVersions, pendingSuggestions] = await Promise.all([
            getPersonalMemoryVersions(),
            getMemoryRevisions().catch(() => []),
          ]);
          setVersions(allVersions);
          setSuggestions(pendingSuggestions);
          await onRefreshFacts?.(true);
          setIsCommitting(true);
          setPendingConfirmation(null);

          if (pendingSuggestions.length > 0) {
            setStagingMode("suggestions");
          }

          later(() => {
            setIsCommitting(false);
            setLeftFlash(false);
          }, 900);
        } else if (outcome.status === "confirmation_required") {
          pendingSwapRef.current = null;
          setIsCommitting(false);
          setLeftFlash(false);
          setPendingConfirmation({
            reason: outcome.reason,
            pendingCount: outcome.pending_count,
          });
        }
      } catch (e: unknown) {
        console.error("[usePersonalMemoryDrawer] Consolidate failed:", e);
        pendingSwapRef.current = null;
        setIsCommitting(false);
        setLeftFlash(false);
      } finally {
        setConsolidating(false);
      }
    },
    [consolidating, onRefreshFacts, later, raiseVeil, stageSwap]
  );

  const handleApplySuggestions = useCallback(
    async (decisionsMap: Record<string, "accept" | "reject">) => {
      const decisionList = Object.entries(decisionsMap).map(([id, action]) => ({
        id,
        action,
      }));
      if (decisionList.length === 0) return;
      setIsApplyingSuggestions(true);
      raiseVeil();

      try {
        const updated = await resolveMemoryRevisions({
          projectId: undefined,
          decisions: decisionList,
        });
        stageSwap(updated);

        const [allVersions, remainingSuggestions] = await Promise.all([
          getPersonalMemoryVersions(),
          getMemoryRevisions().catch(() => []),
        ]);
        setVersions(allVersions);
        setSuggestions(remainingSuggestions);
        await onRefreshFacts?.(true);
        setIsCommitting(true);

        if (remainingSuggestions.length === 0) {
          setJustCommitted(true);
          setStagingMode("idle");
        }

        later(() => {
          setIsCommitting(false);
          setLeftFlash(false);
        }, 900);
      } catch (e) {
        console.error("[usePersonalMemoryDrawer] Apply suggestions failed:", e);
        pendingSwapRef.current = null;
        setIsCommitting(false);
        setLeftFlash(false);
        throw e;
      } finally {
        setIsApplyingSuggestions(false);
      }
    },
    [onRefreshFacts, later, raiseVeil, stageSwap]
  );

  const handleCopyDoc = useCallback(async () => {
    const text = personalMemory?.markdown || personalMemory?.content;
    if (!text) return;
    const ok = await copyToClipboard(text);
    if (ok) {
      setCopied(true);
      later(() => setCopied(false), 2000);
    }
  }, [personalMemory?.markdown, personalMemory?.content, later]);

  const handleRegenerateFromFacts = useCallback(async () => {
    if (isRegenerating || saving) return;
    setIsRegenerating(true);
    raiseVeil();
    try {
      const record = await regeneratePersonalMemory();
      stageSwap(record);
      const allVersions = await getPersonalMemoryVersions();
      setVersions(allVersions);
      setIsCommitting(true);
      later(() => {
        setIsCommitting(false);
        setLeftFlash(false);
      }, 700);
      await onRefreshFacts?.(true);
    } catch (e) {
      pendingSwapRef.current = null;
      setIsCommitting(false);
      setLeftFlash(false);
      console.error("[usePersonalMemoryDrawer] Failed to regenerate memory from integrated facts:", e);
    } finally {
      setIsRegenerating(false);
    }
  }, [isRegenerating, saving, raiseVeil, stageSwap, later, onRefreshFacts]);

  // Fast line-number lookup table
  const lineStartsRef = useRef<number[]>([0]);
  const dossierContentRef = useRef("");
  useEffect(() => {
    const content = personalMemory?.content ?? "";
    dossierContentRef.current = content;
    const starts: number[] = [0];
    for (let i = 0; i < content.length; i++) {
      if (content.charCodeAt(i) === 10) starts.push(i + 1);
    }
    lineStartsRef.current = starts;
  }, [personalMemory?.content]);

  const lineNumberForIndex = useCallback((idx: number): number => {
    const starts = lineStartsRef.current;
    let lo = 0;
    let hi = starts.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (starts[mid] <= idx) lo = mid + 1;
      else hi = mid;
    }
    return lo;
  }, []);

  // Selection listener for comments
  useEffect(() => {
    if (!drawerOpen) return;
    let rafId: number | null = null;
    const handleSelectionChange = () => {
      if (rafId !== null) return;
      rafId = requestAnimationFrame(() => {
        rafId = null;
        if (isComposingCommentRef.current) return;

        const sel = window.getSelection();
        if (!sel || sel.isCollapsed || !sel.rangeCount) {
          setSelectionAnchor(null);
          return;
        }
        const container = dossierContainerRef.current;
        if (!container) return;

        const anchorNode = sel.anchorNode;
        if (!anchorNode || !container.contains(anchorNode)) {
          setSelectionAnchor(null);
          return;
        }

        const text = sel.toString().trim();
        if (!text || text.length < 2) {
          setSelectionAnchor(null);
          return;
        }

        const range = sel.getRangeAt(0);
        const rect = range.getBoundingClientRect();
        const containerRect = container.getBoundingClientRect();

        if (rect.width === 0 || rect.height === 0) {
          setSelectionAnchor(null);
          return;
        }

        const clientRects = Array.from(range.getClientRects());
        const selectionRects = clientRects.map((cr) => ({
          top: cr.top - containerRect.top + container.scrollTop,
          left: cr.left - containerRect.left,
          width: cr.width,
          height: cr.height,
        }));

        const fullContent = dossierContentRef.current;
        const snippetIdx = fullContent.indexOf(text);
        let lineNumber = 1;
        if (snippetIdx !== -1) {
          lineNumber = lineNumberForIndex(snippetIdx);
        } else {
          const firstWord = text.split(/\s+/)[0];
          const wordIdx = fullContent.indexOf(firstWord);
          if (wordIdx !== -1) {
            lineNumber = lineNumberForIndex(wordIdx);
          }
        }

        setSelectionAnchor({
          line: lineNumber,
          quotedText: text.length > 80 ? `${text.slice(0, 77)}…` : text,
          top: rect.top - containerRect.top + container.scrollTop,
          bottom: rect.bottom - containerRect.top + container.scrollTop,
          left: rect.left - containerRect.left,
          right: rect.right - containerRect.left,
          rects: selectionRects,
        });
      });
    };

    document.addEventListener("selectionchange", handleSelectionChange);
    return () => {
      document.removeEventListener("selectionchange", handleSelectionChange);
      if (rafId !== null) cancelAnimationFrame(rafId);
    };
  }, [drawerOpen, lineNumberForIndex]);

  const handleAddComment = useCallback(
    (newComment: { line: number; quotedText: string; text: string; top: number }) => {
      const commentItem: MemoryComment = {
        id: `comment_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`,
        line: newComment.line,
        quotedText: newComment.quotedText,
        text: newComment.text,
        top: newComment.top,
        createdAt: Date.now(),
      };
      storeAddComment(commentItem);
      setSelectionAnchor(null);
      setIsComposingComment(false);
      window.getSelection()?.removeAllRanges();
      setStagingMode("comment");
    },
    [storeAddComment]
  );

  const handleCancelComment = useCallback(() => {
    setSelectionAnchor(null);
    setIsComposingComment(false);
    window.getSelection()?.removeAllRanges();
  }, []);

  const handleDeleteComment = useCallback(
    (id: string) => {
      storeDeleteComment(id);
    },
    [storeDeleteComment]
  );

  const handleUpdateComment = useCallback(
    (id: string, text: string) => {
      storeUpdateComment(id, text);
    },
    [storeUpdateComment]
  );

  const handleClearComments = useCallback(() => {
    storeClearComments();
  }, [storeClearComments]);

  const handleRegenerateWithComments = useCallback(
    async (commentsToApply: MemoryComment[], forced?: boolean) => {
      if (!commentsToApply.length) return;
      setPendingActionType("regenerate");
      setSaving(true);
      raiseVeil();
      try {
        const formattedComments = commentsToApply.map(
          (c) => `Line ${c.line} ("${c.quotedText}"): ${c.text}`
        );
        const outcome = await consolidatePersonalMemory(
          formattedComments,
          undefined,
          forced ?? false
        );

        if (outcome.status === "completed") {
          stageSwap(outcome.record);
          const [allVersions, pendingSuggestions] = await Promise.all([
            getPersonalMemoryVersions(),
            getMemoryRevisions().catch(() => []),
          ]);
          setVersions(allVersions);
          setSuggestions(pendingSuggestions);
          storeClearComments();
          setIsCommitting(true);
          setPendingConfirmation(null);

          if (pendingSuggestions.length > 0) {
            setStagingMode("suggestions");
          } else {
            setStagingMode("idle");
          }

          later(() => {
            setIsCommitting(false);
            setLeftFlash(false);
          }, 700);
          await onRefreshFacts?.(true);
        } else if (outcome.status === "confirmation_required") {
          pendingSwapRef.current = null;
          setIsCommitting(false);
          setLeftFlash(false);
          setPendingConfirmation({
            reason: outcome.reason,
            pendingCount: outcome.pending_count,
          });
        }
      } catch (e) {
        pendingSwapRef.current = null;
        setIsCommitting(false);
        setLeftFlash(false);
        throw e;
      } finally {
        setSaving(false);
      }
    },
    [onRefreshFacts, storeClearComments, later, raiseVeil, stageSwap]
  );

  const handleConfirmPendingIntegration = useCallback(() => {
    if (pendingActionType === "regenerate" && comments.length > 0) {
      handleRegenerateWithComments(comments, true);
    } else {
      handleConsolidateNow(true);
    }
  }, [pendingActionType, comments, handleRegenerateWithComments, handleConsolidateNow]);

  const handleCancelPendingConfirmation = useCallback(() => {
    setPendingConfirmation(null);
  }, []);

  const handleStagingModeChange = useCallback(
    (m: StagingMode) => {
      setStagingMode(m);
      if (justCommitted) setJustCommitted(false);
    },
    [justCommitted]
  );

  return {
    drawerOpen,
    openDrawer,
    closeDrawer,
    drawerBodyReady,
    personalMemory,
    displayedRecord,
    versions,
    suggestions,
    isRestoringVersion,
    handleSelectVersion,
    handleRestoreActive,
    handleCopyDoc,
    copied,
    handleRegenerateFromFacts,
    isRegenerating,
    saving,
    leftFlash,
    veilCycle,
    handleVeilReady,
    isCommitting,
    consolidating,
    stagingMode,
    setStagingMode,
    handleStagingModeChange,
    handleSaveStaging,
    handleConsolidateNow,
    handleApplySuggestions,
    isApplyingSuggestions,
    justCommitted,
    setJustCommitted,
    dossierContainerRef,
    selectionAnchor,
    isComposingComment,
    setIsComposingComment,
    handleAddComment,
    handleCancelComment,
    handleDeleteComment,
    handleUpdateComment,
    handleClearComments,
    comments,
    handleRegenerateWithComments,
    pendingConfirmation,
    handleConfirmPendingIntegration,
    handleCancelPendingConfirmation,
    identityCandidateFacts,
    unconsolidatedIdentityCount,
    paginatedObservations,
    obsStatusFilter,
    setObsStatusFilter,
    obsLoading,
    obsLoadingMore,
    obsHasMore,
    obsLoadMore,
    refreshPersonalMemory,
  };
}
