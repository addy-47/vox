import React, { Suspense, lazy, useCallback, useEffect, useState } from "react";
import { RotateCw, TriangleAlert } from "lucide-react";
import { BrowserRouter as Router, Routes, Route, Navigate } from "react-router-dom";
import { getOnboardingStatus } from "@/services/setupService";
import { ResponsiveLayout } from "@/layout/ResponsiveLayout";
import { WizardRoot } from "@/wizard/WizardRoot";
import { TitleBar } from "@/layout/TitleBar";
import { ErrorBoundary, OrbitalLoader, WizardStandIn } from "@/shared/components/common";
import { LAYOUT_COPY } from "@/data/layoutCopy";
import { MemoryProfilerProvider } from "@/shared/context/MemoryProfilerContext";
import { VoiceSessionProvider } from "@/shared/context/VoiceSessionContext";
import { ProfilerDrawerProvider } from "@/shared/components/profiler/ProfilerDrawer";
import { PanelStateProvider } from "@/shared/hooks/usePanelState";
import { useNotificationStore } from "@/store/notificationStore";
import { installOverlayStack } from "@/shared/lib/overlayStack";
import { getCurrentWindowLabel } from "@/services/windowService";
import { AnimatePresence, motion, MotionConfig } from "framer-motion";

// Route-level preloads
const preloadRouteChunks = () => {
  void import("@/pages/History");
  void import("@/pages/Memory");
  void import("@/pages/Settings");
  void import("@/pages/Monitoring");
};

import { Home } from "@/pages/Home";
import { PageDrawerProvider } from "@/shared/context/PageDrawerContext";
import { initSpatialNavigation } from "@/shared/lib/spatialNavigation";

// Lazy load secondary pages for performance
const History = lazy(() => import("@/pages/History").then(m => ({ default: m.History })));
const Memory = lazy(() => import("@/pages/Memory").then(m => ({ default: m.Memory })));
const Settings = lazy(() => import("@/pages/Settings").then(m => ({ default: m.Settings })));
const Monitoring = lazy(() => import("@/pages/Monitoring").then(m => ({ default: m.Monitoring })));


// Premium Shared Orbital Loading Screen
const PageLoader = () => (
  <div className="flex h-screen w-full items-center justify-center bg-[rgb(var(--background))]">
    <OrbitalLoader size="lg" />
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
  /**
   * Onboarding status is UNKNOWN. Distinct from both `true` and `false`:
   * `false` means "not configured, show the wizard"; unknown means "we could not
   * ask". Previously a rejected `get_onboarding_status` collapsed to `false`,
   * which inescapable-routed a fully-configured install into a 6-step wizard
   * ending in a multi-GB model download.
   */
  const [setupUnreachable, setSetupUnreachable] = useState(false);
  const [setupRetryAttempt, setSetupRetryAttempt] = useState(0);
  const [userChoseRoute, setUserChoseRoute] = useState<"wizard" | "app" | null>(null);

  // Boot loader fade is deferred until the app shell has finished initial routing
  // and is ready to display content, avoiding the blank screen gap.

  /**
   * True when this bundle is executing inside the dedicated `wizard` webview.
   *
   * First-run previously mounted `WizardRoot` twice: once in the `wizard`
   * webview (created at bootstrap by `ensure_wizard_window`) and once in
   * `main`, because `main` is also configured `visible: true` and routed its own
   * `setupCompleted === false` into the wizard. Both instances registered
   * `revealWizard()`, both ran `initSpatialNavigation()` and
   * `installOverlayStack()`, and both booted `VoiceSessionProvider` (6 pipeline
   * listeners + 3 IPC calls). Only one is the real setup window.
   */
  const [isWizardWebview, setIsWizardWebview] = useState<boolean | null>(null);

  useEffect(() => {
    let active = true;
    void getCurrentWindowLabel().then((label) => {
      if (active) setIsWizardWebview(label === "wizard");
    });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    // Global error handler for unhandled promise rejections
    const onRejection = (event: PromiseRejectionEvent) => {
      console.error('[GLOBAL] Unhandled Promise Rejection:', event.reason);
    };
    window.addEventListener('unhandledrejection', onRejection);

    // Warm secondary route chunks off the critical path
    const scheduleIdle = window.requestIdleCallback
      ? (cb: () => void) => window.requestIdleCallback(cb, { timeout: 2000 })
      : (cb: () => void) => setTimeout(cb, 1200);
    scheduleIdle(preloadRouteChunks);

    /**
     * Reads onboarding status with bounded retry/backoff.
     *
     * A rejected read is NOT treated as "not configured". It lands in the
     * `setupUnreachable` state, which renders an explicit screen with a manual
     * choice. The alternative — collapsing failure to `false` — is what made a
     * locked settings file or a not-yet-ready backend send a working install
     * through the whole first-run wizard.
     */
    const checkSetup = async () => {
      // In the wizard webview, setup status is known by construction (bootstrap
      // only creates that window when setup is incomplete), and `?wizard=true`
      // is meaningless here. Skip the IPC entirely.
      if (isWizardWebview) {
        setSetupCompleted(false);
        return;
      }

      const urlParams = new URLSearchParams(window.location.search);
      const forceWizard = urlParams.get('wizard') === 'true';

      const MAX_ATTEMPTS = 3;
      const BACKOFF_MS = [400, 1200, 3000];

      for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt++) {
        try {
          const completed = await getOnboardingStatus();
          const finalStatus = forceWizard ? false : completed;
          setSetupCompleted(finalStatus);
          setSetupUnreachable(false);
          try {
            if (finalStatus) {
              localStorage.setItem("vox_setup_completed", "true");
            } else {
              localStorage.removeItem("vox_setup_completed");
            }
          } catch (_) {}
          return;
        } catch (e) {
          console.error(`[App] Setup check failed (attempt ${attempt}/${MAX_ATTEMPTS}):`, e);
          setSetupRetryAttempt(attempt);
          if (attempt < MAX_ATTEMPTS) {
            await new Promise<void>((resolve) =>
              setTimeout(resolve, BACKOFF_MS[attempt - 1])
            );
          }
        }
      }

      // Exhausted. Last resort: if this machine previously completed setup, the
      // cached flag is strong evidence the backend is simply not answering yet —
      // prefer the app over the wizard, but never silently: the user is told.
      let cachedCompleted = false;
      try {
        cachedCompleted = localStorage.getItem("vox_setup_completed") === "true";
      } catch (_) {}

      setSetupUnreachable(true);
      setSetupCompleted(null);
      if (cachedCompleted) {
        setUserChoseRoute("app");
      }
    };
    checkSetup();

    return () => {
      window.removeEventListener('unhandledrejection', onRejection);
    };
  }, [isWizardWebview]);

  // Hold the orbital loader for a brief smooth beat so the user sees the loader
  // cross-fade to the home screen.
  useEffect(() => {
    if (setupCompleted === null && !setupUnreachable) return;
    const t = setTimeout(() => setReadyToTransition(true), 150);
    return () => clearTimeout(t);
  }, [setupCompleted, setupUnreachable]);

  /** Manual retry from the unreachable screen. */
  const retrySetupCheck = useCallback(() => {
    setSetupCompleted(null);
    setSetupUnreachable(false);
    setReadyToTransition(false);
    setSetupRetryAttempt(0);
    void (async () => {
      try {
        const completed = await getOnboardingStatus();
        setSetupCompleted(completed);
        setSetupUnreachable(false);
      } catch (e) {
        console.error('[App] Setup retry failed:', e);
        setSetupUnreachable(true);
      }
    })();
  }, []);

  /**
   * Effective routing decision. `setupUnreachable` deliberately blocks the
   * wildcard → /wizard redirect: we must not enter setup implicitly.
   */
  const routeToWizard = userChoseRoute
    ? userChoseRoute === "wizard"
    : setupCompleted === false;
  const routeToApp = userChoseRoute
    ? userChoseRoute === "app"
    : setupCompleted === true;

  const isLoading =
    (setupCompleted === null && !setupUnreachable) || !readyToTransition;

  // Once the app shell is loaded and readyToTransition is satisfied, smoothly fade out the HTML boot loader
  useEffect(() => {
    if (!isLoading) {
      window.__VOX_HIDE_BOOT_LOADER?.();
    }
  }, [isLoading]);

  // Single app-level notification fetch + listener lifecycle. Panels are
  // pure lists; this effect runs once (store actions are stable references).
  const fetchNotifications = useNotificationStore((s) => s.fetchNotifications);
  const initListeners = useNotificationStore((s) => s.initListeners);
  // The setup wizard has no notification panel and no pipeline, so this fetch +
  // listener registration is skipped there too.
  useEffect(() => {
    if (isWizardWebview) return;
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
  }, [fetchNotifications, initListeners, isWizardWebview]);

  // Global overlay stack + spatial navigation install FOUR window-level
  // listeners each (including a mousemove handler). They are app-shell
  // concerns, so they are skipped in the `wizard` webview, which renders only
  // the setup flow and would otherwise pay for navigation machinery it never
  // uses.
  useEffect(() => {
    if (isWizardWebview) return;
    const schedule = window.requestIdleCallback;
    const cb = () => installOverlayStack();
    if (typeof schedule === "function") {
      schedule(cb, { timeout: 1000 });
    } else {
      setTimeout(cb, 200);
    }
    initSpatialNavigation();
  }, [isWizardWebview]);

  return (
    <div className="relative h-screen w-full bg-[rgb(var(--background))] overflow-hidden">
      <ErrorBoundary name="App">
        {/* Dedicated setup wizard window */}
        {isWizardWebview === null ? null : isWizardWebview ? (
          <Router>
            <Suspense fallback={<PageLoader />}>
              <Routes>
                <Route path="/wizard" element={<WizardRoot />} />
                <Route path="*" element={<Navigate to="/wizard" replace />} />
              </Routes>
            </Suspense>
          </Router>
        ) : (
        <MemoryProfilerProvider>
          <VoiceSessionProvider>
            <Router>
              {/* Main App content mounts and initializes behind the loader */}
              {/* MotionConfig: all framer-motion animation below honours the OS
                  reduced-motion setting. Zero behaviour change when it is off. */}
              <MotionConfig reducedMotion="user">
              <div className="relative h-full w-full">
                {(routeToApp || routeToWizard) && (
                  <Suspense fallback={<PageLoader />}>
                    <PanelStateProvider>
                      <ProfilerDrawerProvider>
                        <PageDrawerProvider>
                          <Routes>
                          {/* Setup genuinely not completed. The real setup UI is the dedicated
                              `wizard` webview; `main` only hosts a stand-in that
                              surfaces it, so the setup flow is never mounted
                              twice on a first run. */}
                          {routeToWizard && (
                            <>
                              <Route path="/wizard" element={<WizardStandIn />} />
                              <Route path="*" element={<Navigate to="/wizard" replace />} />
                            </>
                          )}

                          {/* Main App Routes. Also reachable when the user
                              explicitly chose the app from the unreachable
                              screen. */}
                          {routeToApp && (
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
                        </PageDrawerProvider>
                      </ProfilerDrawerProvider>
                    </PanelStateProvider>
                  </Suspense>
                )}

                {/* Backend unreachable — explicit choice, never an implicit
                    redirect into first-run setup. */}
                <AnimatePresence>
                  {setupUnreachable && !userChoseRoute && (
                    <motion.div
                      key="setup-unreachable"
                      initial={{ opacity: 0 }}
                      animate={{ opacity: 1 }}
                      exit={{ opacity: 0 }}
                      transition={{ duration: 0.3, ease: [0.16, 1, 0.3, 1] }}
                      role="alert"
                      className="absolute inset-0 z-50 flex flex-col items-center justify-center gap-5 px-6 bg-[rgb(var(--background))]"
                    >
                      <div className="flex flex-col items-center gap-2 text-center max-w-md">
                        <TriangleAlert
                          size={28}
                          className="text-[rgb(var(--danger))]"
                          aria-hidden
                        />
                        <h1 className="text-[17px] font-display font-black uppercase tracking-[0.14em] text-[rgb(var(--foreground))]">
                          {LAYOUT_COPY.setupUnreachable.title}
                        </h1>
                        <p className="text-[12.5px] leading-relaxed text-[rgb(var(--foreground-muted))]">
                          {setupRetryAttempt > 0
                            ? LAYOUT_COPY.setupUnreachable.bodyFinal
                            : LAYOUT_COPY.setupUnreachable.body}
                        </p>
                        {setupRetryAttempt > 0 && (
                          <p className="text-[11px] font-mono text-[rgb(var(--foreground-muted))]/70">
                            {LAYOUT_COPY.setupUnreachable.retrying(setupRetryAttempt)}
                          </p>
                        )}
                      </div>

                      <div className="flex flex-wrap items-center justify-center gap-2.5">
                        <button
                          type="button"
                          onClick={retrySetupCheck}
                          className="flex items-center gap-1.5 px-4 py-2 rounded-xl text-[12px] font-mono bg-[rgba(var(--accent),0.15)] border border-[rgba(var(--accent),0.35)] text-[rgb(var(--accent))] hover:bg-[rgba(var(--accent),0.25)] transition-colors duration-150 cursor-pointer"
                        >
                          <RotateCw size={13} />
                          {LAYOUT_COPY.setupUnreachable.retry}
                        </button>
                        <button
                          type="button"
                          onClick={() => setUserChoseRoute("app")}
                          className="px-4 py-2 rounded-xl text-[12px] font-mono border border-[rgba(var(--border),0.24)] bg-[rgba(var(--foreground),0.04)] text-[rgb(var(--foreground))] hover:border-[rgba(var(--accent),0.4)] transition-colors duration-150 cursor-pointer"
                        >
                          {LAYOUT_COPY.setupUnreachable.continueAnyway}
                        </button>
                        <button
                          type="button"
                          onClick={() => setUserChoseRoute("wizard")}
                          className="px-4 py-2 rounded-xl text-[12px] font-mono text-[rgb(var(--foreground-muted))] hover:text-[rgb(var(--foreground))] transition-colors duration-150 cursor-pointer"
                        >
                          {LAYOUT_COPY.setupUnreachable.startSetup}
                        </button>
                      </div>
                    </motion.div>
                  )}
                </AnimatePresence>

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
              </MotionConfig>
            </Router>
          </VoiceSessionProvider>
        </MemoryProfilerProvider>
        )}
      </ErrorBoundary>
    </div>
  );
};

export default App;
