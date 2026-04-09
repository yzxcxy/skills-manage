import { Routes, Route, Navigate } from "react-router-dom";
import { AppShell } from "@/components/layout/AppShell";
import { PlatformView } from "@/pages/PlatformView";
import { CentralSkillsView } from "@/pages/CentralSkillsView";
import { SkillDetail } from "@/pages/SkillDetail";
import { CollectionView } from "@/pages/CollectionView";
import { SettingsView } from "@/pages/SettingsView";

function App() {
  return (
    <Routes>
      <Route path="/" element={<AppShell />}>
        {/* Default redirect to Central Skills */}
        <Route index element={<Navigate to="/central" replace />} />
        {/* Platform view: lists skills for a specific agent */}
        <Route path="platform/:agentId" element={<PlatformView />} />
        {/* Central Skills: canonical ~/.agents/skills/ view */}
        <Route path="central" element={<CentralSkillsView />} />
        {/* Skill detail page */}
        <Route path="skill/:skillId" element={<SkillDetail />} />
        {/* Collection view */}
        <Route path="collection/:collectionId" element={<CollectionView />} />
        {/* Settings */}
        <Route path="settings" element={<SettingsView />} />
      </Route>
    </Routes>
  );
}

export default App;
