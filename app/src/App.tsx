import React, { Suspense, lazy, useCallback, useEffect, useState } from "react";
import { BrowserRouter as Router, Routes, Route, Navigate } from "react-router-dom";
import { getOnboardingStatus } from "@/services/modelService";
import { ResponsiveLayout } from "@/layout/ResponsiveLayout";
import { WizardRoot } from "@/wizard/WizardRoot";
import { TitleBar } from "@/layout/TitleBar";
import { ErrorBoundary, OrbitalLoader, HelpPanel, NotificationPanel } from "@/shared/components/common";
import { EdgePanel } from "@/shared/ui";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import { HELP_DRAWER_COPY } from "@/data/helpCopy";
import { NOTIFICATION_COPY } from "@/data/notificationCopy";
import { MemoryProfilerProvider } from "@/shared/context/MemoryProfilerContext";
import { VoiceSessionProvider } from "@/shared/context/VoiceSessionContext";
import { ProfilerDrawerProvider } from "@/shared/components/profiler/ProfilerDrawer";
import { PanelStateProvider, usePanelStateContext } from "@/shared/hooks/usePanelState";
import { useNotificationStore } from "@/store/notificationStore";
import { installOverlayStack } from "@/shared/lib/overlayStack";
import { AnimatePresence, motion } from "framer-motion";

import { Home } from "@/pages/Home";

// Lazy load secondary pages for performance
const History = lazy(() => import("@/pages/History").then(m => ({ default: m.History })));
const Memory = lazy(() => import("@/pages/Memory").then(m => ({ default: m.Memory })));
const Settings = lazy(() => import("@/pages/Settings").then(m => ({ default: m.Settings })));
const Monitoring = lazy(() => import("@/pages/Monitoring").then(m => ({ default: m.Monitoring })));

/**
 * Right-edge rails (Help / Notifications) mounted once at app level.
 * Help and Notifications share one exclusive group via usePanelState.
 */
const AppPanels: React.FC = () => {
  const { isPanelOpen, closePanel } = usePanelStateContext();

  const closeHelp = useCallback(() => closePanel("help"), [closePanel]);
  const closeNotifications = useCallback(() => closePanel("notifications"), [closePanel]);

  return (
    <>
      <EdgePanel side="right" open={isPanelOpen("help")} onClose={closeHelp} title={HELP_DRAWER_COPY.headerTitle}>
        <HelpPanel onClose={closeHelp} />
      </EdgePanel>
      <EdgePanel side="right" open={isPanelOpen("notifications")} onClose={closeNotifications} title={NOTIFICATION_COPY.title}>
        <NotificationPanel onClose={closeNotifications} />
      </EdgePanel>
    </>
  );
};

// Premium Shared Orbital Loading Screen
const PageLoader = () => (
  <div className="flex h-screen w-full items-center justify-center bg-[rgb(var(--background))]">
    <OrbitalLoader
      size="lg"
      title={LAYOUT_COPY.boot.title}
      subtitle={LAYOUT_COPY.boot.subtitle}
      statusText={LAYOUT_COPY.boot.status}
    />
  </div>
);

const App: React.FC = () => {
  const [setupCompleted, setSetupCompleted] = useState<boolean | null>(() => {
    try {
      const cached = localStorage.getItem("vox_setup_completed");
      return cached === "true" ? true : null;
    } catch {
      return null;
    }
  });
  const [readyToTransition, setReadyToTransition] = useState(false);

  // Fade and remove the pre-React boot loader (rendered by index.html's
  // #vox-boot-loader element) as soon as the App component mounts. This
  // ensures the user sees the animated loader during Vite cold start and
  // React tree mount, with a smooth cross-fade to the React OrbitalLoader.
  useEffect(() => {
    (window as unknown as { __VOX_HIDE_BOOT_LOADER?: () => void }).__VOX_HIDE_BOOT_LOADER?.();
  }, []);

  useEffect(() => {
    // Global error handler for unhandled promise rejections
    const onRejection = (event: PromiseRejectionEvent) => {
      console.error('[GLOBAL] Unhandled Promise Rejection:', event.reason);
    };
    window.addEventListener('unhandledrejection', onRejection);

    // Preload secondary page route chunks in the background
    import("@/pages/History").catch(() => {});
    import("@/pages/Settings").catch(() => {});
    import("@/pages/Monitoring").catch(() => {});

    const checkSetup = async () => {
      const urlParams = new URLSearchParams(window.location.search);
      const forceWizard = urlParams.get('wizard') === 'true';

      try {
        const completed = await getOnboardingStatus();
        const finalStatus = forceWizard ? false : completed;
        setSetupCompleted(finalStatus);
        try {
          if (finalStatus) {
            localStorage.setItem("vox_setup_completed", "true");
          } else {
            localStorage.removeItem("vox_setup_completed");
          }
        } catch (_) {}
      } catch (e) {
        console.error('[App] Setup check failed:', e);
        setSetupCompleted(false);
      } finally {
        // Hold the orbital loader for a brief smooth beat so the user sees the
        // loader cross-fade to the home screen.
        setTimeout(() => {
          setReadyToTransition(true);
        }, 150);
      }
    };
    checkSetup();

    return () => {
      window.removeEventListener('unhandledrejection', onRejection);
    };
  }, []);

  const isLoading = setupCompleted === null || !readyToTransition;

  // Single app-level notification fetch + listener lifecycle. Panels are
  // pure lists; this effect runs once (store actions are stable references).
  const fetchNotifications = useNotificationStore((s) => s.fetchNotifications);
  const initListeners = useNotificationStore((s) => s.initListeners);
  useEffect(() => {
    let isMounted = true;
    let cleanupListeners: (() => void) | null = null;

    fetchNotifications().catch(() => {});

    initListeners()
      .then((cleanup) => {
        if (isMounted) {
          cleanupListeners = cleanup;
        } else {
          cleanup();
        }
      })
      .catch(() => {});

    return () => {
      isMounted = false;
      if (cleanupListeners) cleanupListeners();
    };
  }, [fetchNotifications, initListeners]);

  // Global overlay stack — single authority for FILO Escape / outside-click
  // dismissal. Deferred to requestIdleCallback so it doesn't run during the
  // critical first-paint path. The overlay system is only needed once the
  // user has interacted with any overlay (drawer, popover, panel), so a
  // 100-500ms delay after first paint is imperceptible.
  useEffect(() => {
    const schedule = (window as unknown as { requestIdleCallback?: (cb: () => void, opts?: { timeout: number }) => number }).requestIdleCallback;
    const cb = () => installOverlayStack();
    if (typeof schedule === "function") {
      schedule(cb, { timeout: 1000 });
    } else {
      setTimeout(cb, 200);
    }
  }, []);

  return (
    <div className="relative h-screen w-full bg-[rgb(var(--background))] overflow-hidden">
      <ErrorBoundary name="App">
        <MemoryProfilerProvider>
          <VoiceSessionProvider>
            <Router>
              {/* Main App content mounts and initializes behind the loader */}
              <div className="relative h-full w-full">
                {setupCompleted !== null && (
                  <Suspense fallback={null}>
                    <PanelStateProvider>
                      <ProfilerDrawerProvider>
                        <Routes>
                        {/* If setup not completed, always redirect to wizard */}
                        {!setupCompleted && (
                          <>
                            <Route path="/wizard" element={<WizardRoot />} />
                            <Route path="*" element={<Navigate to="/wizard" replace />} />
                          </>
                        )}

                        {/* Main App Routes */}
                        {setupCompleted && (
                          <Route element={<ResponsiveLayout />}>
                            <Route path="/" element={<ErrorBoundary name="Home"><Home /></ErrorBoundary>} />
                            <Route path="/history" element={<ErrorBoundary name="History"><History /></ErrorBoundary>} />
                            <Route path="/memory" element={<ErrorBoundary name="Memory"><Memory /></ErrorBoundary>} />
                            <Route path="/settings" element={<ErrorBoundary name="Settings"><Settings /></ErrorBoundary>} />
                            <Route path="/monitoring" element={<ErrorBoundary name="Monitoring"><Monitoring /></ErrorBoundary>} />
                            <Route path="/wizard" element={<Navigate to="/" replace />} />
                            <Route path="*" element={<Navigate to="/" replace />} />
                          </Route>
                        )}
                      </Routes>
                        <AppPanels />
                      </ProfilerDrawerProvider>
                    </PanelStateProvider>
                  </Suspense>
                )}

                {/* Seamless Orbital Loader Overlay that cross-fades out once ready */}
                <AnimatePresence>
                  {isLoading && (
                    <motion.div
                      key="boot-loader"
                      initial={{ opacity: 1 }}
                      exit={{ opacity: 0, scale: 0.98 }}
                      transition={{ duration: 0.45, ease: [0.16, 1, 0.3, 1] }}
                      className="absolute inset-0 z-50 flex flex-col items-center justify-center bg-[rgb(var(--background))]"
                    >
                      <TitleBar />
                      <div className="flex-1 flex items-center justify-center w-full">
                        <PageLoader />
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>
              </div>
            </Router>
          </VoiceSessionProvider>
        </MemoryProfilerProvider>
      </ErrorBoundary>
    </div>
  );
};

export default App;
