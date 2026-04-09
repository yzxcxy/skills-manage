import { useEffect } from "react";
import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { usePlatformStore } from "@/stores/platformStore";

/**
 * Top-level app shell: fixed sidebar + scrollable main content area.
 * Triggers the initial platform scan on mount.
 */
export function AppShell() {
  const initialize = usePlatformStore((state) => state.initialize);

  useEffect(() => {
    initialize();
    // Only run once on mount — disable exhaustive-deps warning deliberately
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="flex h-screen bg-background text-foreground overflow-hidden">
      <Sidebar />
      <main className="flex-1 overflow-auto min-w-0">
        <Outlet />
      </main>
    </div>
  );
}
