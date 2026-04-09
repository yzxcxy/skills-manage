import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { Sidebar } from "../components/layout/Sidebar";
import { usePlatformStore } from "../stores/platformStore";

// Mock the platformStore to avoid real Tauri invocations
vi.mock("../stores/platformStore", () => ({
  usePlatformStore: vi.fn(),
}));

const mockAgents = [
  {
    id: "claude-code",
    display_name: "Claude Code",
    category: "coding",
    global_skills_dir: "~/.claude/skills/",
    is_detected: true,
    is_builtin: true,
    is_enabled: true,
  },
  {
    id: "cursor",
    display_name: "Cursor",
    category: "coding",
    global_skills_dir: "~/.cursor/skills/",
    is_detected: true,
    is_builtin: true,
    is_enabled: true,
  },
  {
    id: "central",
    display_name: "Central Skills",
    category: "central",
    global_skills_dir: "~/.agents/skills/",
    is_detected: true,
    is_builtin: true,
    is_enabled: true,
  },
];

const defaultStoreState = {
  agents: mockAgents,
  skillsByAgent: {
    "claude-code": 5,
    cursor: 3,
    central: 10,
  },
  isLoading: false,
  error: null,
  initialize: vi.fn(),
  rescan: vi.fn(),
};

function renderSidebar(initialPath = "/central") {
  vi.mocked(usePlatformStore).mockReturnValue(defaultStoreState);
  return render(
    <MemoryRouter initialEntries={[initialPath]}>
      <Sidebar />
    </MemoryRouter>
  );
}

describe("Sidebar", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  // ── Rendering ─────────────────────────────────────────────────────────────

  it("renders the app title", () => {
    renderSidebar();
    expect(screen.getByText("skills-manage")).toBeInTheDocument();
  });

  it("renders By Tool section header", () => {
    renderSidebar();
    expect(screen.getByText("By Tool")).toBeInTheDocument();
  });

  it("renders platform agents in By Tool section", () => {
    renderSidebar();
    // Should show platform agents (not the central one)
    expect(screen.getByText("Claude Code")).toBeInTheDocument();
    expect(screen.getByText("Cursor")).toBeInTheDocument();
    // Central should not appear in By Tool
    // (it's rendered as a separate "Central Skills" nav item, not in By Tool)
  });

  it("shows skill count badges for each platform", () => {
    renderSidebar();
    // Claude Code has 5, Cursor has 3
    expect(screen.getByText("5")).toBeInTheDocument();
    expect(screen.getByText("3")).toBeInTheDocument();
  });

  it("renders Central Skills nav item", () => {
    renderSidebar();
    expect(screen.getByText("Central Skills")).toBeInTheDocument();
  });

  it("shows Central Skills count badge", () => {
    renderSidebar();
    expect(screen.getByText("10")).toBeInTheDocument();
  });

  it("renders Collections section", () => {
    renderSidebar();
    expect(screen.getByText("Collections")).toBeInTheDocument();
  });

  it("renders '+新建' button in Collections section", () => {
    renderSidebar();
    expect(screen.getByText("+ 新建")).toBeInTheDocument();
  });

  it("renders Settings link at bottom", () => {
    renderSidebar();
    expect(screen.getByText("设置")).toBeInTheDocument();
  });

  // ── Loading State ─────────────────────────────────────────────────────────

  it("shows loading indicator when isLoading is true", () => {
    vi.mocked(usePlatformStore).mockReturnValue({
      ...defaultStoreState,
      isLoading: true,
    });
    render(
      <MemoryRouter>
        <Sidebar />
      </MemoryRouter>
    );
    expect(screen.getByText("Scanning...")).toBeInTheDocument();
  });

  it("hides platform list when loading", () => {
    vi.mocked(usePlatformStore).mockReturnValue({
      ...defaultStoreState,
      isLoading: true,
    });
    render(
      <MemoryRouter>
        <Sidebar />
      </MemoryRouter>
    );
    expect(screen.queryByText("Claude Code")).not.toBeInTheDocument();
  });

  // ── Active Route Highlighting ─────────────────────────────────────────────

  it("highlights active platform route in sidebar", () => {
    renderSidebar("/platform/claude-code");
    const claudeButton = screen.getByRole("button", { name: /Claude Code/ });
    // Active item should have the active class applied
    expect(claudeButton.className).toContain("font-medium");
  });

  it("highlights Central Skills when on /central", () => {
    renderSidebar("/central");
    const centralButton = screen.getByRole("button", { name: /Central Skills/ });
    expect(centralButton.className).toContain("font-medium");
  });

  it("highlights Settings when on /settings", () => {
    renderSidebar("/settings");
    const settingsButton = screen.getByRole("button", { name: /设置/ });
    expect(settingsButton.className).toContain("font-medium");
  });

  // ── Empty States ──────────────────────────────────────────────────────────

  it("shows empty message when no platforms are detected", () => {
    vi.mocked(usePlatformStore).mockReturnValue({
      ...defaultStoreState,
      agents: [
        {
          id: "central",
          display_name: "Central Skills",
          category: "central",
          global_skills_dir: "~/.agents/skills/",
          is_detected: true,
          is_builtin: true,
          is_enabled: true,
        },
      ],
    });
    render(
      <MemoryRouter>
        <Sidebar />
      </MemoryRouter>
    );
    expect(screen.getByText("No platforms detected")).toBeInTheDocument();
  });

  // ── Navigation ────────────────────────────────────────────────────────────

  it("platform buttons are clickable", () => {
    renderSidebar();
    const claudeButton = screen.getByRole("button", { name: /Claude Code/ });
    expect(claudeButton).not.toBeDisabled();
    // Just verify it can be clicked without throwing
    fireEvent.click(claudeButton);
  });

  it("Central Skills button is clickable", () => {
    renderSidebar();
    const centralButton = screen.getByRole("button", { name: /Central Skills/ });
    expect(centralButton).not.toBeDisabled();
    fireEvent.click(centralButton);
  });
});
