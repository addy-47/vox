import { memo } from "react";
import { SubModelCard } from "../SubModelCard";
import { cn } from "@/shared/lib/utils";

interface AuxiliaryWorkspaceProps {
  layoutMode?: "full-max" | "full-min" | "small";
  confirmDeleteId: string | null;
  setConfirmDeleteId: (id: string | null) => void;
  modelPresence: Record<string, boolean>;
  downloadStatuses: Record<string, any>;
  startDownload: (id: string) => void;
  deleteModel: (id: string) => void;
}

export const AuxiliaryWorkspace = memo(
  ({
    layoutMode,
    confirmDeleteId,
    setConfirmDeleteId,
    modelPresence,
    downloadStatuses,
    startDownload,
    deleteModel,
  }: AuxiliaryWorkspaceProps) => {
    return (
      <div className="space-y-3">
        <div
          className={cn(
            "grid gap-2.5",
            layoutMode === "small" ? "grid-cols-1" : "grid-cols-2"
          )}
        >
          <SubModelCard
            id="modernbert_memory_scope"
            name="ModernBERT MemoryScope Classifier"
            description="Neural Intent Sieve. Categorizes transcripts in <5ms to skip unnecessary LLM inference."
            parameters="66M ONNX"
            ramUsage="~130 MB"
            tradeoffs="Intent Classification"
            isDownloaded={!!modelPresence["modernbert_memory_scope"]}
            isActive={true}
            isRequired={false}
            layoutMode={layoutMode}
            onSelect={() => {}}
            confirmDeleteId={confirmDeleteId}
            setConfirmDeleteId={setConfirmDeleteId}
            downloadStatus={downloadStatuses["modernbert_memory_scope"]}
            startDownload={() => startDownload("modernbert_memory_scope")}
            deleteModel={() => deleteModel("modernbert_memory_scope")}
          />

          <SubModelCard
            id="minilm_l12_v2"
            name="MiniLM-L12-v2 Embeddings"
            description="Vector Memory Encoder. Generates 384-dim semantic embeddings for fast retrieval RAG."
            parameters="33M ONNX"
            ramUsage="~120 MB"
            tradeoffs="Memory Embeddings"
            isDownloaded={!!modelPresence["minilm_l12_v2"]}
            isActive={true}
            isRequired={false}
            layoutMode={layoutMode}
            onSelect={() => {}}
            confirmDeleteId={confirmDeleteId}
            setConfirmDeleteId={setConfirmDeleteId}
            downloadStatus={downloadStatuses["minilm_l12_v2"]}
            startDownload={() => startDownload("minilm_l12_v2")}
            deleteModel={() => deleteModel("minilm_l12_v2")}
          />

          <SubModelCard
            id="nli_deberta_v3_base"
            name="DeBERTa-v3 NLI Reranker"
            description="Cross-encoder entailment auditor. Verifies retrieved facts before injecting into LLM context."
            parameters="22M ONNX"
            ramUsage="~90 MB"
            tradeoffs="RAG Fact Verification"
            isDownloaded={!!modelPresence["nli_deberta_v3_base"]}
            isActive={true}
            isRequired={false}
            layoutMode={layoutMode}
            onSelect={() => {}}
            confirmDeleteId={confirmDeleteId}
            setConfirmDeleteId={setConfirmDeleteId}
            downloadStatus={downloadStatuses["nli_deberta_v3_base"]}
            startDownload={() => startDownload("nli_deberta_v3_base")}
            deleteModel={() => deleteModel("nli_deberta_v3_base")}
          />

          <SubModelCard
            id="vox_translit_rnn"
            name="Translit-RNN (Hinglish/Devanagari)"
            description="Phonetic transliteration RNN engine for multi-lingual speech normalization."
            parameters="Embedded ONNX"
            ramUsage="~15 MB"
            tradeoffs="Language Normalization"
            isDownloaded={!!modelPresence["vox_translit_rnn"]}
            isActive={true}
            isRequired={false}
            layoutMode={layoutMode}
            onSelect={() => {}}
            confirmDeleteId={confirmDeleteId}
            setConfirmDeleteId={setConfirmDeleteId}
            downloadStatus={downloadStatuses["vox_translit_rnn"]}
            startDownload={() => startDownload("vox_translit_rnn")}
            deleteModel={() => deleteModel("vox_translit_rnn")}
          />
          <SubModelCard
            id="modernbert_edge_creation"
            name="ModernBERT Cognitive Edge Classifier"
            description="Neural Edge Sieve. Classifies cross-collection inter-fact graph relationships."
            parameters="66M ONNX"
            ramUsage="~130 MB"
            tradeoffs="Edge Graph Topology"
            isDownloaded={!!modelPresence["modernbert_edge_creation"]}
            isActive={true}
            isRequired={false}
            layoutMode={layoutMode}
            onSelect={() => {}}
            confirmDeleteId={confirmDeleteId}
            setConfirmDeleteId={setConfirmDeleteId}
            downloadStatus={downloadStatuses["modernbert_edge_creation"]}
            startDownload={() => startDownload("modernbert_edge_creation")}
            deleteModel={() => deleteModel("modernbert_edge_creation")}
          />
        </div>
      </div>
    );
  }
);

AuxiliaryWorkspace.displayName = "AuxiliaryWorkspace";
