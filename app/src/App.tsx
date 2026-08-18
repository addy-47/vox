import React, { Suspense, lazy, useEffect, useState } from "react";
import { BrowserRouter as Router, Routes, Route, Navigate } from "react-router-dom";
import { getOnboardingStatus } from "@/services/modelService";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ResponsiveLayout } from "@/layout/ResponsiveLayout";
import { WizardRoot } from "@/wizard/WizardRoot";
import { TitleBar } from "@/layout/TitleBar";
import { ErrorBoundary, OrbitalLoader } from "@/shared/components/common";
import { AnimatePresence, motion } from "framer-motion";

import { Home } from "@/pages/Home";

// Lazy load secondary pages for performance
const History = lazy(() => import("@/pages/History").then(m => ({ default: m.History })));
const Memory = lazy(() => import("@/pages/Memory").then(m => ({ default: m.Memory })));
const Settings = lazy(() => import("@/pages/Settings").then(m => ({ default: m.Settings })));
const Monitoring = lazy(() => import("@/pages/Monitoring").then(m => ({ default: m.Monitoring })));

// Premium Shared Orbital Loading Screen
const PageLoader = () => (
  <div className="flex h-screen w-full items-center justify-center bg-[rgb(var(--background))]">
    <OrbitalLoader
      size="lg"
      title="SYNCHRONIZING"
      subtitle="Preparing neural models and interface"
      statusText="VOX RUNTIME READY"
    />
  </div>
);

const App: React.FC = () => {
  const [setupCompleted, setSetupCompleted] = useState<boolean | null>(null);
  const [readyToTransition, setReadyToTransition] = useState(false);

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
        setSetupCompleted(forceWizard ? false : completed);
      } catch (e) {
        console.error('[App] Setup check failed:', e);
        setSetupCompleted(false);
      } finally {
        // Double rAF ensures the WebKit compositor and Window Manager have
        // completed layout calculation and maximization before revealing the window
        requestAnimationFrame(() => {
          requestAnimationFrame(() => {
            getCurrentWindow().show().catch(console.error);

            // Hold the orbital loader for a brief smooth beat so the user sees the loader,
            // then smoothly cross-fade to the home screen
            setTimeout(() => {
              setReadyToTransition(true);
            }, 300);
          });
        });
      }
    };
    checkSetup();

    return () => {
      window.removeEventListener('unhandledrejection', onRejection);
    };
  }, []);

  const isLoading = setupCompleted === null || !readyToTransition;

  return (
    <div className="relative h-screen w-full bg-[rgb(var(--background))] overflow-hidden">
      <ErrorBoundary name="App">
        <Router>
          {/* Main App content mounts and initializes behind the loader */}
          <div className="relative h-full w-full">
            {setupCompleted !== null && (
              <Suspense fallback={null}>
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
      </ErrorBoundary>
    </div>
  );
};

export default App;
