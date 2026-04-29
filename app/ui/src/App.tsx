import React from "react";
import { BrowserRouter as Router, Routes, Route } from "react-router-dom";
import { ResponsiveLayout } from "./layout/ResponsiveLayout";
import { Home } from "./pages/Home/Home";
import { History } from "./pages/History/History";
import { Settings } from "./pages/Settings/Settings";

const App: React.FC = () => {
  return (
    <Router>
      <ResponsiveLayout>
        <Routes>
          <Route path="/" element={<Home />} />
          <Route path="/history" element={<History />} />
          <Route path="/settings" element={<Settings />} />
        </Routes>
      </ResponsiveLayout>
    </Router>
  );
};

export default App;
