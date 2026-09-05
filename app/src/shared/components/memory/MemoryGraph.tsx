import React, {
  useCallback,
  useEffect,
  useRef,
  useImperativeHandle,
  forwardRef,
  memo,
  Component,
  ErrorInfo,
  ReactNode,
  useState,
} from "react";
import { AnimatePresence, motion } from "framer-motion";
import * as THREE from "three";
import { MemoryNodeTopology, MemoryEdgeTopology, MemoryFactDetail } from "@/services/memoryService";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { OrbitalLoader } from "@/shared/components/common";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";
import { useMemoryGraphScene } from "@/shared/hooks/useMemoryGraphScene";
import { MemoryGraphClusterBadges } from "./MemoryGraphClusterBadges";
import {
  GNode,
  GLink,
  MemoryGraphRef,
  getCollectionColor,
  getRelationStyle,
  getCollectionIcon,
} from "./memoryGraphTypes";

// Re-export public interfaces and color helpers for backward-compatibility
export type { GNode, GLink, MemoryGraphRef };
export { getCollectionColor, getRelationStyle, getCollectionIcon };

interface GraphErrorBoundaryProps {
  children: ReactNode;
}

interface GraphErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

class GraphErrorBoundary extends Component<GraphErrorBoundaryProps, GraphErrorBoundaryState> {
  constructor(props: GraphErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("[MemoryGraph] WebGL canvas error:", error, info);
  }

  handleRetry = () => {
    this.setState({ hasError: false, error: null });
  };

  render() {
    if (this.state.hasError) {
      return (
        <div className="w-full h-full flex items-center justify-center p-6">
          <div className="glass-card max-w-sm w-full p-6 text-center space-y-4 rounded-2xl border border-[rgba(var(--accent),0.12)] bg-[rgb(var(--card))]/85 backdrop-blur-[20px] shadow-2xl">
            <div className="mx-auto w-12 h-12 rounded-2xl bg-red-500/10 border border-red-500/20 flex items-center justify-center text-red-400">
              <svg className="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <circle cx="12" cy="12" r="10" />
                <line x1="12" y1="8" x2="12" />
                <line x1="12" y1="16" x2="12.01" y2="16" />
              </svg>
            </div>
            <div>
              <h3 className="font-display text-[13px] font-sans font-bold uppercase tracking-wider text-[rgb(var(--foreground))]">
                {MEMORY_COPY.graphErrorTitle}
              </h3>
              <p className="text-[11px] text-[rgb(var(--foreground-muted))] mt-1 break-words">
                {this.state.error?.message || MEMORY_COPY.graphErrorFallback}
              </p>
            </div>
            <button
              onClick={this.handleRetry}
              className="px-4 py-2 text-[11px] font-bold uppercase tracking-widest glass-card hover:border-[rgb(var(--accent))]/50 transition-colors cursor-pointer rounded-xl"
            >
              {MEMORY_COPY.graphRetry}
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}

interface MemoryGraphProps {
  nodes: MemoryNodeTopology[];
  edges: MemoryEdgeTopology[];
  width: number;
  height: number;
  searchQuery: string;
  selectedCollection: string;
  selectedRelation: string;
  onSelectNode: (nodeId: string | null, pos?: { x: number; y: number }) => void;
  selectedFactId: string | null;
  selectedFactDetail?: MemoryFactDetail | null;
  conflictPairs?: { fact_a: MemoryNodeTopology; fact_b: MemoryNodeTopology }[];
  clearCacheOnUnmount?: boolean;
}

export const MemoryGraph = memo(
  forwardRef<MemoryGraphRef, MemoryGraphProps>(
    (
      {
        nodes,
        edges,
        width,
        height,
        searchQuery,
        selectedCollection,
        selectedRelation,
        onSelectNode,
        selectedFactId,
        selectedFactDetail,
        conflictPairs = [],
        clearCacheOnUnmount = false,
      },
      ref
    ) => {
      useMemoryTrace("MemoryGraph (WebGL InstancedMesh)");

      const canvasContainerRef = useRef<HTMLDivElement>(null);
      const mouseVecRef = useRef(new THREE.Vector2());
      const raycasterRef = useRef(new THREE.Raycaster());
      const tempVecRef = useRef(new THREE.Vector3());
      const [expandedBadge, setExpandedBadge] = useState<string | null>(null);

      // Encapsulated Three.js Scene, WebGL Buffers & Aspect-Ratio Responsive Simulation Hook
      const {
        isLayoutStable,
        isLightMode,
        clusterBadges,
        gNodesRef,
        cameraRef,
        rendererRef,
        instancedMeshRef,
        isNodeVisible,
        recenter,
        zoomIn,
        zoomOut,
      } = useMemoryGraphScene({
        canvasContainerRef,
        nodes,
        edges,
        width,
        height,
        searchQuery,
        selectedCollection,
        selectedRelation,
        selectedFactId,
        selectedFactDetail,
        conflictPairs,
        clearCacheOnUnmount,
      });

      // Expose imperative camera navigation methods
      useImperativeHandle(ref, () => ({
        recenter,
        zoomIn,
        zoomOut,
      }));

      // Dismiss expanded cluster badge card on click outside
      useEffect(() => {
        if (!expandedBadge) return;
        const handleDocClick = (e: MouseEvent | PointerEvent) => {
          const badgeElem = document.getElementById(`badge-card-${expandedBadge}`);
          if (badgeElem && !badgeElem.contains(e.target as Node)) {
            setExpandedBadge(null);
          }
        };
        const timer = setTimeout(() => {
          document.addEventListener("pointerdown", handleDocClick);
        }, 50);
        return () => {
          clearTimeout(timer);
          document.removeEventListener("pointerdown", handleDocClick);
        };
      }, [expandedBadge]);

      // Dismiss expanded badge card on Escape key
      useEffect(() => {
        if (!expandedBadge) return;
        const handleKeyDown = (e: KeyboardEvent) => {
          if (e.key === "Escape") {
            setExpandedBadge(null);
          }
        };
        window.addEventListener("keydown", handleKeyDown);
        return () => window.removeEventListener("keydown", handleKeyDown);
      }, [expandedBadge]);

      // Raycaster + Screen-Space Proximity Node Click Picking
      const handlePointerDown = useCallback(
        (e: React.PointerEvent<HTMLDivElement>) => {
          if (
            (e.target as HTMLElement).closest(".pointer-events-auto") &&
            (e.target as HTMLElement) !== canvasContainerRef.current
          ) {
            return;
          }

          const renderer = rendererRef.current;
          const camera = cameraRef.current;
          const instancedMesh = instancedMeshRef.current;
          const gNodes = gNodesRef.current;

          if (!renderer || !camera || !instancedMesh || gNodes.length === 0) return;

          const rect = renderer.domElement.getBoundingClientRect();
          const clickX = e.clientX - rect.left;
          const clickY = e.clientY - rect.top;

          // 1. Raycaster Direct Hit Test
          const mouse = mouseVecRef.current;
          mouse.set((clickX / rect.width) * 2 - 1, -(clickY / rect.height) * 2 + 1);

          const raycaster = raycasterRef.current;
          raycaster.setFromCamera(mouse, camera);

          const intersects = raycaster.intersectObject(instancedMesh);
          if (intersects.length > 0 && intersects[0].instanceId !== undefined) {
            const idx = intersects[0].instanceId;
            const node = gNodes[idx];
            if (node && isNodeVisible(node)) {
              onSelectNode(node.id, { x: e.clientX, y: e.clientY });
              return;
            }
          }

          // 2. Fallback Screen-Space Proximity Hit Test (24px Touch/Click Radius Threshold)
          let closestNode: GNode | null = null;
          let minSqDist = 24 * 24;
          const tempVec = tempVecRef.current;

          for (let i = 0; i < gNodes.length; i++) {
            const n = gNodes[i];
            if (!isNodeVisible(n)) continue;

            tempVec.set(n.x, n.y, n.z);
            tempVec.project(camera);

            const screenX = ((tempVec.x + 1) * width) / 2;
            const screenY = ((-tempVec.y + 1) * height) / 2;

            const dx = screenX - clickX;
            const dy = screenY - clickY;
            const sqDist = dx * dx + dy * dy;

            if (sqDist < minSqDist) {
              minSqDist = sqDist;
              closestNode = n;
            }
          }

          if (closestNode) {
            onSelectNode(closestNode.id, { x: e.clientX, y: e.clientY });
            return;
          }

          if (expandedBadge) {
            setExpandedBadge(null);
          }

          onSelectNode(null);
        },
        [onSelectNode, isNodeVisible, width, height, expandedBadge, rendererRef, cameraRef, instancedMeshRef, gNodesRef]
      );

      return (
        <GraphErrorBoundary>
          <div
            ref={canvasContainerRef}
            onPointerDown={handlePointerDown}
            className="relative w-full h-full cursor-grab active:cursor-grabbing select-none"
          >
            {/* Borderless Organic Memory Network Loader Overlay */}
            <AnimatePresence>
              {!isLayoutStable && (
                <motion.div
                  initial={{ opacity: 1 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.6, ease: "easeInOut" }}
                  className="absolute inset-0 z-30 flex flex-col items-center justify-center bg-[rgb(var(--background))]/90 backdrop-blur-3xl pointer-events-none select-none"
                >
                  <OrbitalLoader
                    size="lg"
                    title={MEMORY_COPY.graphBuilding}
                    subtitle={`${nodes.length.toLocaleString()} nodes · ${edges.length.toLocaleString()} edges`}
                    statusText="Optimizing layout and relationships"
                  />
                </motion.div>
              )}
            </AnimatePresence>

            {/* Prominent Cluster Centroid Badges & Overlay Cards */}
            {isLayoutStable && (
              <MemoryGraphClusterBadges
                clusterBadges={clusterBadges}
                expandedBadge={expandedBadge}
                onToggleBadge={setExpandedBadge}
                isLightMode={isLightMode}
              />
            )}
          </div>
        </GraphErrorBoundary>
      );
    }
  )
);

MemoryGraph.displayName = "MemoryGraph";
