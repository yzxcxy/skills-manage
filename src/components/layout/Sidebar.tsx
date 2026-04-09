import { useNavigate, useLocation } from "react-router-dom";
import { Settings, Plus, Loader2 } from "lucide-react";
import { usePlatformStore } from "@/stores/platformStore";
import { cn } from "@/lib/utils";

// ─── Badge ────────────────────────────────────────────────────────────────────

function Badge({
  count,
  variant = "muted",
}: {
  count: number;
  variant?: "muted" | "primary";
}) {
  return (
    <span
      className={cn(
        "ml-auto shrink-0 rounded-full text-xs px-1.5 py-0.5 min-w-[1.25rem] text-center leading-tight",
        variant === "primary"
          ? "bg-primary/10 text-primary font-medium"
          : "bg-muted text-muted-foreground"
      )}
    >
      {count}
    </span>
  );
}

// ─── Nav Item ─────────────────────────────────────────────────────────────────

function NavItem({
  label,
  badge,
  isActive,
  onClick,
  badgeVariant = "muted",
}: {
  label: string;
  badge?: number;
  isActive: boolean;
  onClick: () => void;
  badgeVariant?: "muted" | "primary";
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "flex items-center w-full px-3 py-1.5 text-sm rounded-md mx-1 transition-colors",
        "hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
        isActive &&
          "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
      )}
      style={{ width: "calc(100% - 0.5rem)" }}
    >
      <span className="truncate">{label}</span>
      {badge !== undefined && (
        <Badge count={badge} variant={badgeVariant} />
      )}
    </button>
  );
}

// ─── Section Header ───────────────────────────────────────────────────────────

function SectionHeader({
  label,
  action,
}: {
  label: string;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex items-center justify-between px-3 py-1.5">
      <span className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
        {label}
      </span>
      {action}
    </div>
  );
}

// ─── Sidebar ──────────────────────────────────────────────────────────────────

export function Sidebar() {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const { agents, skillsByAgent, isLoading } = usePlatformStore();

  // Separate platform agents from the central one
  const platformAgents = agents.filter(
    (a) => a.id !== "central" && a.is_enabled
  );
  const centralCount = skillsByAgent["central"] ?? 0;

  return (
    <nav
      className="flex flex-col w-60 shrink-0 h-full border-r border-border bg-sidebar text-sidebar-foreground"
      aria-label="Main navigation"
    >
      {/* App header */}
      <div className="px-4 py-3 border-b border-border">
        <h1 className="text-sm font-semibold tracking-tight">skills-manage</h1>
      </div>

      {/* Scrollable sections */}
      <div className="flex-1 overflow-y-auto py-3 space-y-4">
        {/* ── By Tool ─────────────────────────────────────────── */}
        <section aria-label="By Tool">
          <SectionHeader label="By Tool" />
          {isLoading ? (
            <div className="flex items-center gap-2 px-3 py-2 text-muted-foreground text-sm">
              <Loader2 className="size-3.5 animate-spin" />
              <span>Scanning...</span>
            </div>
          ) : platformAgents.length === 0 ? (
            <p className="px-3 py-1.5 text-xs text-muted-foreground">
              No platforms detected
            </p>
          ) : (
            platformAgents.map((agent) => (
              <NavItem
                key={agent.id}
                label={agent.display_name}
                badge={skillsByAgent[agent.id] ?? 0}
                isActive={pathname === `/platform/${agent.id}`}
                onClick={() => navigate(`/platform/${agent.id}`)}
              />
            ))
          )}
        </section>

        {/* ── Central Skills ───────────────────────────────────── */}
        <section aria-label="Central Skills">
          <NavItem
            label="Central Skills"
            badge={isLoading ? undefined : centralCount}
            isActive={pathname === "/central"}
            onClick={() => navigate("/central")}
            badgeVariant="primary"
          />
        </section>

        {/* ── Collections ──────────────────────────────────────── */}
        <section aria-label="Collections">
          <SectionHeader
            label="Collections"
            action={
              <button
                className="p-0.5 rounded hover:bg-sidebar-accent text-muted-foreground hover:text-sidebar-accent-foreground transition-colors"
                onClick={() => {
                  /* TODO: open create collection dialog */
                }}
                aria-label="新建 Collection"
              >
                <Plus className="size-3.5" />
              </button>
            }
          />
          {/* Empty state with create prompt */}
          <div className="px-3 py-1">
            <button
              className="text-xs text-muted-foreground hover:text-foreground transition-colors"
              onClick={() => {
                /* TODO: open create collection dialog */
              }}
            >
              + 新建
            </button>
          </div>
        </section>
      </div>

      {/* Settings pinned at bottom */}
      <div className="border-t border-border p-2">
        <button
          onClick={() => navigate("/settings")}
          className={cn(
            "flex items-center gap-2 w-full px-3 py-1.5 text-sm rounded-md transition-colors",
            "hover:bg-sidebar-accent hover:text-sidebar-accent-foreground",
            pathname === "/settings" &&
              "bg-sidebar-accent text-sidebar-accent-foreground font-medium"
          )}
        >
          <Settings className="size-4" />
          <span>设置</span>
        </button>
      </div>
    </nav>
  );
}
