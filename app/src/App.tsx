import React, { Suspense, lazy } from "react";
import { BrowserRouter as Router, Routes, Route } from "react-router-dom";
import { ResponsiveLayout } from "@/layout/ResponsiveLayout";

// Lazy load pages for performance
const Home = lazy(() => import("@/pages/Home").then(m => ({ default: m.Home })));
const History = lazy(() => import("@/pages/History").then(m => ({ default: m.History })));
const Settings = lazy(() => import("@/pages/Settings").then(m => ({ default: m.Settings })));

// Premium Loading Overlay
const PageLoader = () => (
  <div className="flex h-screen w-full items-center justify-center bg-[rgb(var(--background))]">
    <div className="flex flex-col items-center gap-6">
      <div className="relative w-12 h-12">
        <div className="absolute inset-0 border-2 border-[rgb(var(--accent))]/20 rounded-full" />
        <div className="absolute inset-0 border-2 border-t-[rgb(var(--accent))] rounded-full animate-spin" />
      </div>
      <span className="text-[11px] font-bold tracking-[0.5em] text-[rgb(var(--accent))] uppercase animate-pulse">
        Synchronizing
      </span>
    </div>
  </div>
);

const App: React.FC = () => {
  return (
    <Router>
      <Suspense fallback={<PageLoader />}>
        <Routes>
          <Route element={<ResponsiveLayout />}>
            <Route path="/" element={<Home />} />
            <Route path="/history" element={<History />} />
            <Route path="/settings" element={<Settings />} />
          </Route>
        </Routes>
      </Suspense>
    </Router>
  );
};

export default App;

