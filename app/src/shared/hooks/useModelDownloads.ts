import { useState, useCallback, useEffect, useRef } from "react";
import { useSettingsStore } from "@/store/settingsStore";
import {
  downloadOptionalModel,
  deleteModel,
  checkModelExists,
} from "@/services/modelService";
import * as eventsService from "@/services/eventsService";

export interface ModelStatus {
  step: 'idle' | 'downloading' | 'extracting' | 'verifying' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  bytesDownloaded: number;
  totalBytes: number;
  error?: string;
}

// Module-level persistent cache for model download statuses across modal open/close
const globalDownloadStatuses: Record<string, ModelStatus> = {};

export function useModelDownloads() {
  const modelCatalog = useSettingsStore((s) => s.modelCatalog);

  const [downloadStatuses, setDownloadStatuses] = useState<Record<string, ModelStatus>>(() => ({
    ...globalDownloadStatuses,
  }));
  const [modelPresence, setModelPresence] = useState<Record<string, boolean>>({});
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const updateDownloadStatus = useCallback((modelId: string, status: Partial<ModelStatus>) => {
    setDownloadStatuses((prev) => {
      const updated = {
        ...(prev[modelId] || { step: "idle", progress: 0, bytesDownloaded: 0, totalBytes: 100 }),
        ...status,
      };
      globalDownloadStatuses[modelId] = updated as ModelStatus;
      return { ...prev, [modelId]: updated as ModelStatus };
    });
  }, []);

  // Check model files presence in parallel
  const refreshPresence = useCallback(async () => {
    try {
      const allModelIds = modelCatalog?.model_groups?.map((g) => g.id) || [
        ...(modelCatalog?.vad?.map((m) => m.id) || []),
        ...(modelCatalog?.stt?.map((m) => m.id) || []),
        ...(modelCatalog?.llm?.map((m) => m.id) || []),
        ...(modelCatalog?.tts?.map((m) => m.id) || []),
        ...(modelCatalog?.auxiliary?.map((m) => m.id) || []),
      ];

      const entries = await Promise.all(
        allModelIds.map(async (id) => [id, await checkModelExists(id)] as const)
      );
      setModelPresence(Object.fromEntries(entries));
    } catch (e) {
      console.error("Failed to fetch models presence:", e);
    }
  }, [modelCatalog]);

  useEffect(() => {
    refreshPresence();
  }, [refreshPresence]);

  // Per-file progress accumulator
  const fileProgressRef = useRef<Record<string, { progress: number; bytesDownloaded: number; totalBytes: number; done: boolean }>>({});

  // Model download events listener (unified model_progress)
  useEffect(() => {
    const unlistenProgress = eventsService.onModelProgress((payload) => {
      const { model_id, step, progress, bytes_downloaded, total_bytes, error } = payload || {};
      if (!model_id) return;

      const stepLower = String(step || "downloading").toLowerCase() as ModelStatus["step"];
      const fileProgress = typeof progress === "number" ? progress : 0;
      const fileBytes = bytes_downloaded || 0;
      const fileTotal = total_bytes || 100;

      fileProgressRef.current[model_id] = {
        progress: fileProgress,
        bytesDownloaded: fileBytes,
        totalBytes: fileTotal,
        done: stepLower === "completed" || (stepLower as string) === "complete",
      };

      const groups = modelCatalog?.model_groups || [];
      const parent = groups.find(
        (g) => g.id === model_id || (g.files || []).some((f) => f.id === model_id)
      );
      const targetId = parent ? parent.id : model_id;

      if (parent && parent.id !== model_id) {
        const files = parent.files || [];
        let totalBytes = 0;
        let doneBytes = 0;
        let allDone = files.length > 0;
        for (const f of files) {
          const fp = fileProgressRef.current[f.id];
          const tb = fp?.totalBytes || f.size || 0;
          totalBytes += tb;
          doneBytes += fp ? (fp.bytesDownloaded || (fp.progress / 100) * tb) : 0;
          if (!fp?.done) allDone = false;
        }
        if (stepLower === "failed" || stepLower === "cancelled") {
          updateDownloadStatus(targetId, {
            step: stepLower,
            progress: totalBytes > 0 ? (doneBytes / totalBytes) * 100 : 0,
            bytesDownloaded: Math.round(doneBytes),
            totalBytes: totalBytes || 100,
            error: error || undefined,
          });
        } else if (allDone) {
          updateDownloadStatus(targetId, {
            step: "completed",
            progress: 100,
            bytesDownloaded: totalBytes,
            totalBytes: totalBytes || 100,
            error: undefined,
          });
          refreshPresence();
        } else {
          updateDownloadStatus(targetId, {
            step: "downloading",
            progress: totalBytes > 0 ? (doneBytes / totalBytes) * 100 : 0,
            bytesDownloaded: Math.round(doneBytes),
            totalBytes: totalBytes || 100,
            error: undefined,
          });
        }
        return;
      }

      updateDownloadStatus(model_id, {
        step: stepLower,
        progress: fileProgress,
        bytesDownloaded: fileBytes,
        totalBytes: fileTotal,
        error: error || undefined,
      });

      if (stepLower === "completed" || (stepLower as string) === "complete") {
        refreshPresence();
      }
    });

    return () => {
      unlistenProgress();
    };
  }, [updateDownloadStatus, refreshPresence, modelCatalog]);

  const startDownload = async (modelId: string) => {
    try {
      updateDownloadStatus(modelId, {
        step: "downloading",
        progress: 1,
        bytesDownloaded: 0,
        totalBytes: 100,
        error: undefined,
      });
      await downloadOptionalModel(modelId);
    } catch (e: any) {
      console.error("Failed to start download:", e);
      updateDownloadStatus(modelId, {
        step: "failed",
        error: String(e),
      });
    }
  };

  const handleDeleteModelGroup = async (modelGroupId: string) => {
    try {
      await deleteModel(modelGroupId);
      setModelPresence((prev) => ({ ...prev, [modelGroupId]: false }));
      setConfirmDeleteId(null);
    } catch (e) {
      console.error("Failed to delete model group:", e);
    }
  };

  const isGroupRequired = useCallback((id: string) => {
    const g = modelCatalog?.model_groups?.find((x) => x.id === id);
    return !!g && (!!g.required || !!g.is_built_in);
  }, [modelCatalog]);

  return {
    downloadStatuses,
    modelPresence,
    confirmDeleteId,
    setConfirmDeleteId,
    startDownload,
    handleDeleteModelGroup,
    isGroupRequired,
    refreshPresence,
  };
}
