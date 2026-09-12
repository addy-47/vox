import React, {
  useCallback,
  useRef,
  useImperativeHandle,
  forwardRef,
  memo,
  Component,
  ErrorInfo,
  ReactNode,
  useState,
} from "react";
import * as THREE from "three";
import { FactRecord } from "@/services/memoryService";
import { MEMORY_COPY } from "@/data/memoryCopy";
import { useMemoryTrace } from "@/shared/hooks/useMemoryTrace";
import { useMemoryGraphScene } from "@/shared/hooks/useMemoryGraphScene";
import { MemoryGraphClusterBadges } from "./MemoryGraphClusterBadges";
import {
  GNode,
  GLink,
  MemoryGraphRef,
  getCollectionColor,
  getCollectionIcon,
} from "./memoryGraphTypes";

export type { GNode, GLink, MemoryGraphRef };
export { getCollectionColor, getCollectionIcon };

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
  facts: FactRecord[];
  width: number;
  height: number;
  searchQuery: string;
  selectedCollection: string;
  onSelectNode: (fact: FactRecord | null, pos?: { x: number; y: number }) => void;
  onCoreClick?: () => void;
  selectedFactId: string | null;
}

export const MemoryGraph = memo(
  forwardRef<MemoryGraphRef, MemoryGraphProps>(
    (
      {
        facts,
        width,
        height,
        searchQuery,
        selectedCollection,
        onSelectNode,
        onCoreClick,
        selectedFactId,
      },
      ref
    ) => {
      useMemoryTrace("MemoryGraph (WebGL InstancedMesh)");

      const canvasContainerRef = useRef<HTMLDivElement>(null);
      const mouseVecRef = useRef(new THREE.Vector2());
      const raycasterRef = useRef(new THREE.Raycaster());
      const tempVecRef = useRef(new THREE.Vector3());
      const [expandedBadge, setExpandedBadge] = useState<string | null>(null);

      const {
        isLightMode,
        clusterBadges,
        gNodesRef,
        cameraRef,
        rendererRef,
        instancedMeshRef,
        coreMeshRef,
        recenter,
        focusCore,
        zoomIn,
        zoomOut,
      } = useMemoryGraphScene({
        canvasContainerRef,
        facts,
        width,
        height,
        searchQuery,
        selectedCollection,
        selectedFactId,
        onCoreClick,
      });

      useImperativeHandle(ref, () => ({
        recenter,
        zoomIn,
        zoomOut,
        focusCore,
      }));


      // Raycaster + Proximity Picking on Node or Core Click
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
          const coreMesh = coreMeshRef.current;
          const gNodes = gNodesRef.current;

          if (!renderer || !camera || !instancedMesh || gNodes.length === 0) return;

          const rect = renderer.domElement.getBoundingClientRect();
          const clickX = e.clientX - rect.left;
          const clickY = e.clientY - rect.top;

          const mouse = mouseVecRef.current;
          mouse.set((clickX / rect.width) * 2 - 1, -(clickY / rect.height) * 2 + 1);

          const raycaster = raycasterRef.current;
          raycaster.setFromCamera(mouse, camera);

          // 1. Raycast against Central Core
          if (coreMesh && onCoreClick) {
            const coreIntersects = raycaster.intersectObject(coreMesh);
            if (coreIntersects.length > 0) {
              onCoreClick();
              return;
            }
          }

          // 2. Raycast Direct Hit on InstancedMesh Nodes
          const intersects = raycaster.intersectObject(instancedMesh);
          if (intersects.length > 0 && intersects[0].instanceId !== undefined) {
            const idx = intersects[0].instanceId;
            const node = gNodes[idx];
            if (node) {
              onSelectNode(node.factRecord, { x: e.clientX, y: e.clientY });
              return;
            }
          }

          // 3. Fallback Screen-Space Proximity Hit Test (22px Radius)
          let closestNode: GNode | null = null;
          let minSqDist = 22 * 22;
          const tempVec = tempVecRef.current;

          for (let i = 0; i < gNodes.length; i++) {
            const n = gNodes[i];
            tempVec.set(n.x, n.y, n.z);
            tempVec.project(camera);

            if (tempVec.z > 1) continue; // Behind camera

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
            onSelectNode(closestNode.factRecord, { x: e.clientX, y: e.clientY });
            return;
          }

          if (expandedBadge) {
            setExpandedBadge(null);
          }

          onSelectNode(null);
        },
        [onSelectNode, onCoreClick, width, height, expandedBadge, rendererRef, cameraRef, instancedMeshRef, coreMeshRef, gNodesRef]
      );

      return (
        <GraphErrorBoundary>
          <div
            ref={canvasContainerRef}
            onPointerDown={handlePointerDown}
            className="relative w-full h-full cursor-grab active:cursor-grabbing select-none"
          >
            {/* Cluster Badges overlay */}
            <MemoryGraphClusterBadges
              clusterBadges={clusterBadges}
              expandedBadge={expandedBadge}
              onToggleBadge={setExpandedBadge}
              isLightMode={isLightMode}
            />
          </div>
        </GraphErrorBoundary>
      );
    }
  )
);

MemoryGraph.displayName = "MemoryGraph";
