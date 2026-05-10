import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes, useLocation, type Location } from "react-router-dom";
import { useEffect } from "react";
import { CollectionDetailView } from "../pages/CollectionDetailView";
import {
  CollectionDetail,
  AgentWithStatus,
  SkillWithLinks,
} from "../types";

// ─── Mock stores ──────────────────────────────────────────────────────────────

vi.mock("../stores/collectionStore", () => ({
  useCollectionStore: vi.fn(),
}));

vi.mock("../stores/platformStore", () => ({
  usePlatformStore: vi.fn(),
}));

vi.mock("../stores/centralSkillsStore", () => ({
  useCentralSkillsStore: vi.fn(),
}));

vi.mock("../components/collection/CollectionEditor", () => ({
  CollectionEditor: () => null,
}));

vi.mock("../components/collection/SkillPickerDialog", () => ({
  SkillPickerDialog: () => null,
}));

vi.mock("../components/collection/CollectionInstallDialog", () => ({
  CollectionInstallDialog: () => null,
}));

vi.mock("../components/central/InstallDialog", () => ({
  InstallDialog: () => null,
}));

import { useCollectionStore } from "../stores/collectionStore";
import { usePlatformStore } from "../stores/platformStore";
import { useCentralSkillsStore } from "../stores/centralSkillsStore";

// ─── Fixtures ─────────────────────────────────────────────────────────────────

const mockAgents: AgentWithStatus[] = [
  {
    id: "claude-code",
    display_name: "Claude Code",
    category: "coding",
    global_skills_dir: "~/.claude/skills/",
    is_detected: true,
    is_builtin: true,
    is_enabled: true,
  },
];

const mockDetail: CollectionDetail = {
  id: "col-1",
  name: "Frontend",
  description: "Frontend skills",
  created_at: "2026-04-09T00:00:00Z",
  updated_at: "2026-04-09T00:00:00Z",
  skills: [
    {
      id: "frontend-design",
      name: "frontend-design",
      description: "Build distinctive frontend UIs",
      file_path: "~/.agents/skills/frontend-design/SKILL.md",
      is_central: true,
      scanned_at: "2026-04-09T00:00:00Z",
    },
  ],
};

const mockCentralSkills: SkillWithLinks[] = [
  {
    id: "frontend-design",
    name: "frontend-design",
    description: "Build distinctive frontend UIs",
    file_path: "~/.agents/skills/frontend-design/SKILL.md",
    canonical_path: "~/.agents/skills/frontend-design",
    is_central: true,
    scanned_at: "2026-04-09T00:00:00Z",
    linked_agents: [],
  },
];

const mockLoadCollectionDetail = vi.fn();
const mockRemoveSkill = vi.fn();
const mockDeleteCollection = vi.fn();
const mockBatchInstallCollection = vi.fn();
const mockBatchUninstallCollection = vi.fn();
const mockBatchDeleteCollectionSkills = vi.fn();
const mockExportCollection = vi.fn();
const mockAddSkillToCollection = vi.fn();
const mockRefreshCounts = vi.fn();
const mockLoadCentralSkills = vi.fn();
const mockInstallCentralSkill = vi.fn();

const mockUseCollectionStore = vi.mocked(useCollectionStore);
const mockUsePlatformStore = vi.mocked(usePlatformStore);
const mockUseCentralSkillsStore = vi.mocked(useCentralSkillsStore);

function applyStoreMocks(overrides: Record<string, unknown> = {}) {
  mockUseCollectionStore.mockImplementation((selector: unknown) => {
    const state = {
      currentDetail: mockDetail,
      isLoadingDetail: false,
      loadCollectionDetail: mockLoadCollectionDetail,
      removeSkillFromCollection: mockRemoveSkill,
      deleteCollection: mockDeleteCollection,
      batchInstallCollection: mockBatchInstallCollection,
      batchUninstallCollection: mockBatchUninstallCollection,
      batchDeleteCollectionSkills: mockBatchDeleteCollectionSkills,
      exportCollection: mockExportCollection,
      addSkillToCollection: mockAddSkillToCollection,
      ...overrides,
    };
    if (typeof selector === "function") return selector(state);
    return state;
  });
  mockUsePlatformStore.mockImplementation((selector: unknown) => {
    const state = {
      agents: mockAgents,
      refreshCounts: mockRefreshCounts,
    };
    if (typeof selector === "function") return selector(state);
    return state;
  });
  mockUseCentralSkillsStore.mockImplementation((selector: unknown) => {
    const state = {
      skills: mockCentralSkills,
      agents: mockAgents,
      loadCentralSkills: mockLoadCentralSkills,
      installSkill: mockInstallCentralSkill,
    };
    if (typeof selector === "function") return selector(state);
    return state;
  });
}

function LocationProbe({ onChange }: { onChange: (location: Location) => void }) {
  const location = useLocation();
  useEffect(() => {
    onChange(location);
  }, [location, onChange]);
  return null;
}

function renderDetail(
  collectionId = "col-1",
  onLocationChange?: (location: Location) => void
) {
  applyStoreMocks();
  return render(
    <MemoryRouter initialEntries={[`/collection/${collectionId}`]}>
      {onLocationChange && <LocationProbe onChange={onLocationChange} />}
      <Routes>
        <Route path="/collection/:collectionId" element={<CollectionDetailView />} />
        <Route path="/skill/:skillId" element={<div>skill-detail</div>} />
        <Route path="/central" element={<div>central-list</div>} />
      </Routes>
    </MemoryRouter>
  );
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("CollectionDetailView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("loads collection detail on mount", () => {
    renderDetail("col-1");
    expect(mockLoadCollectionDetail).toHaveBeenCalledWith("col-1");
  });

  it("renders collection name and description", async () => {
    renderDetail();
    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Frontend" })).toBeInTheDocument();
    });
    expect(screen.getByText("Frontend skills")).toBeInTheDocument();
  });

  it("renders skills in the collection", async () => {
    renderDetail();
    await waitFor(() => {
      expect(screen.getByText("frontend-design")).toBeInTheDocument();
    });
  });

  it("navigates to skill detail with from state when clicking a skill", async () => {
    const locations: Location[] = [];
    renderDetail("col-1", (loc) => locations.push(loc));

    await waitFor(() => {
      expect(screen.getByText("frontend-design")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: /查看 frontend-design 的详情/i }));

    await waitFor(() => {
      expect(locations.some((l) => l.pathname === "/skill/frontend-design")).toBe(true);
    });

    const skillLocation = locations.find((l) => l.pathname === "/skill/frontend-design");
    expect(skillLocation?.state?.from?.route).toBe("/collection/col-1");
  });

  it("shows back button and breadcrumb", async () => {
    renderDetail();
    await waitFor(() => {
      expect(screen.getByLabelText("返回")).toBeInTheDocument();
    });
    expect(screen.getByText("中央技能仓库")).toBeInTheDocument();
  });

  it("shows empty state when collection has no skills", async () => {
    applyStoreMocks({
      currentDetail: { ...mockDetail, skills: [] },
    });
    render(
      <MemoryRouter initialEntries={[`/collection/col-1`]}>
        <Routes>
          <Route path="/collection/:collectionId" element={<CollectionDetailView />} />
        </Routes>
      </MemoryRouter>
    );
    await waitFor(() => {
      expect(screen.getByText("此技能集还没有技能。")).toBeInTheDocument();
    });
  });

  it("shows loading state while detail is loading", () => {
    applyStoreMocks({ currentDetail: null, isLoadingDetail: true });
    render(
      <MemoryRouter initialEntries={[`/collection/col-1`]}>
        <Routes>
          <Route path="/collection/:collectionId" element={<CollectionDetailView />} />
        </Routes>
      </MemoryRouter>
    );
    expect(screen.getByText("正在加载技能集...")).toBeInTheDocument();
  });
});
