import React, { Suspense, lazy, useEffect, useState } from "react";
import { BrowserRouter as Router, Routes, Route, Navigate } from "react-router-dom";
import { getOnboardingStatus } from "@/services/modelService";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { ResponsiveLayout } from "@/layout/ResponsiveLayout";
import { WizardRoot } from "@/wizard/WizardRoot";
import { TitleBar } from "@/layout/TitleBar";
import { ErrorBoundary } from "@/shared/components/common";

// Lazy load pages for performance
const Home = lazy(() => import("@/pages/Home").then(m => ({ default: m.Home })));
const History = lazy(() => import("@/pages/History").then(m => ({ default: m.History })));
const Memory = lazy(() => import("@/pages/Memory").then(m => ({ default: m.Memory })));
const Settings = lazy(() => import("@/pages/Settings").then(m => ({ default: m.Settings })));
const Monitoring = lazy(() => import("@/pages/Monitoring").then(m => ({ default: m.Monitoring })));

// Premium Loading Overlay
const PageLoader = () => (
  <div className="flex h-screen w-full items-center justify-center bg-[rgb(var(--background))]">
    <div className="flex flex-col items-center gap-6">
      <div className="relative w-12 h-12">
        <div className="absolute inset-0 border-2 border-[rgb(var(--accent))]/20 rounded-full" />
        <div className="absolute inset-0 border-2 border-t-[rgb(var(--accent))] rounded-full animate-spin" />
      </div>
      <span className="text-[12px] font-bold tracking-[0.5em] text-[rgb(var(--accent))] uppercase animate-pulse">
        Synchronizing
      </span>
    </div>
  </div>
);

const App: React.FC = () => {
  const [setupCompleted, setSetupCompleted] = useState<boolean | null>(null);

  useEffect(() => {
    // Global error handler for unhandled promise rejections
    const onRejection = (event: PromiseRejectionEvent) => {
      console.error('[GLOBAL] Unhandled Promise Rejection:', event.reason);
    };
    window.addEventListener('unhandledrejection', onRejection);

    // Preload all other primary page route chunks in the background after mounting App
    // to guarantee instant transitions between tabs
    import("@/pages/Home").catch(() => {});
    import("@/pages/History").catch(() => {});
    import("@/pages/Settings").catch(() => {});
    import("@/pages/Monitoring").catch(() => {});

    // Prewarm the 7 settings card chunks at boot so the radial hub opens without lag
    import("@/shared/components/settings/persona/PersonaCard").catch(() => {});
    import("@/shared/components/settings/models/ModelsCard").catch(() => {});
    import("@/shared/components/settings/realtime/RealtimeCard").catch(() => {});
    import("@/shared/components/settings/history/HistoryCard").catch(() => {});
    import("@/shared/components/settings/memory/MemoryCard").catch(() => {});
    import("@/shared/components/settings/appearance/AppearanceCard").catch(() => {});
    import("@/shared/components/settings/interaction/InteractionCard").catch(() => {});

    // Show window immediately once JS is ready to display the loader
    getCurrentWindow().show().catch(console.error);

    const checkSetup = async () => {
      console.log('[App] Checking setup status...');
      const urlParams = new URLSearchParams(window.location.search);
      const forceWizard = urlParams.get('wizard') === 'true';

      try {
        const completed = await getOnboardingStatus();
        console.log('[App] Setup completed:', completed);
        setSetupCompleted(forceWizard ? false : completed);
      } catch (e) {
        console.error('[App] Setup check failed:', e);
        setSetupCompleted(false);
      }
    };
    checkSetup();

    return () => {
      window.removeEventListener('unhandledrejection', onRejection);
    };
  }, []);

  return (
    <ErrorBoundary name="App">
      <Router>
        {setupCompleted === null ? (
          <div className="flex flex-col h-screen w-full bg-[rgb(var(--background))] overflow-hidden">
            <TitleBar />
            <PageLoader />
          </div>
        ) : (
          <Suspense fallback={<PageLoader />}>
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
      </Router>
    </ErrorBoundary>
  );
};

export default App;
