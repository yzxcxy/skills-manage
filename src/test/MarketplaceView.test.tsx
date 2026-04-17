import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import type { MarketplaceSkill, SkillRegistry } from "@/types";

type StoreState = {
  registries: SkillRegistry[];
  skills: MarketplaceSkill[];
  selectedRegistryId: string;
  searchQuery: string;
  isLoading: boolean;
  isSyncing: boolean;
  installingIds: Set<string>;
  error: string | null;
};

const storeState: StoreState = {
  registries: [
    {
      id: "reg-1",
      name: "Repo One",
      source_type: "github",
      url: "https://github.com/acme/repo-one",
      is_builtin: false,
      is_enabled: true,
      last_synced: "2026-04-16T00:00:00Z",
      last_attempted_sync: "2026-04-16T00:10:00Z",
      last_sync_status: "success",
      last_sync_error: null,
      cache_updated_at: "2026-04-16T00:00:00Z",
      cache_expires_at: "2026-04-17T00:00:00Z",
      etag: null,
      last_modified: null,
      created_at: "2026-04-15T00:00:00Z",
    },
  ],
  skills: [
    {
      id: "skill-1",
      registry_id: "reg-1",
      name: "Cached Skill",
      description: "Skill from cache",
      download_url: "https://example.com/skill-1",
      is_installed: false,
      synced_at: "2026-04-16T00:00:00Z",
      cache_updated_at: "2026-04-16T00:00:00Z",
    },
  ],
  selectedRegistryId: "reg-1",
  searchQuery: "",
  isLoading: false,
  isSyncing: false,
  installingIds: new Set<string>(),
  error: null as string | null,
};

vi.mock("@/components/skill/UnifiedSkillCard", () => ({
  UnifiedSkillCard: ({ name, description }: { name: string; description?: string }) => (
    <div>
      <div>{name}</div>
      {description ? <div>{description}</div> : null}
    </div>
  ),
}));

vi.mock("@/components/marketplace/SkillPreviewDialog", () => ({
  SkillPreviewDialog: () => null,
}));

vi.mock("@/components/central/InstallDialog", () => ({
  InstallDialog: () => null,
}));

const mockLoadRegistries = vi.fn();
const mockSelectRegistry = vi.fn();
const mockSetSearchQuery = vi.fn();
const mockSyncRegistry = vi.fn();
const mockInstallSkill = vi.fn();
const mockAddRegistry = vi.fn();
const mockRemoveRegistry = vi.fn();
const mockFindDuplicateRegistry = vi.fn();
const mockRescan = vi.fn();
const mockLoadCentralSkills = vi.fn();
const mockInstallCentralSkill = vi.fn();

vi.mock("sonner", async () => {
  const actual = await vi.importActual<typeof import("sonner")>("sonner");
  return {
    ...actual,
    toast: {
      ...actual.toast,
      success: vi.fn(),
      error: vi.fn(),
    },
  };
});

vi.mock("@/stores/marketplaceStore", () => ({
  useMarketplaceStore: (selector: (state: typeof storeState & Record<string, unknown>) => unknown) =>
    selector({
      ...storeState,
      loadRegistries: mockLoadRegistries,
      selectRegistry: mockSelectRegistry,
      setSearchQuery: mockSetSearchQuery,
      syncRegistry: mockSyncRegistry,
      installSkill: mockInstallSkill,
      addRegistry: mockAddRegistry,
      removeRegistry: mockRemoveRegistry,
      findDuplicateRegistry: mockFindDuplicateRegistry,
    }),
}));

vi.mock("@/stores/platformStore", () => ({
  usePlatformStore: (selector: (state: { rescan: typeof mockRescan }) => unknown) =>
    selector({
      rescan: mockRescan,
    }),
}));

vi.mock("@/stores/centralSkillsStore", () => ({
  useCentralSkillsStore: Object.assign(
    (selector: (state: Record<string, unknown>) => unknown) =>
      selector({
        skills: [],
        agents: [],
        loadCentralSkills: mockLoadCentralSkills,
        installSkill: mockInstallCentralSkill,
      }),
    {
      getState: () => ({ skills: [] }),
    }
  ),
}));

import { MarketplaceView } from "@/pages/MarketplaceView";
import { toast } from "sonner";

const mockedToast = vi.mocked(toast);
const mockToastSuccess = mockedToast.success as unknown as ReturnType<typeof vi.fn>;
const mockToastError = mockedToast.error as unknown as ReturnType<typeof vi.fn>;

describe("MarketplaceView", () => {
  beforeEach(() => {
    mockLoadRegistries.mockReset();
    mockSelectRegistry.mockReset();
    mockSetSearchQuery.mockReset();
    mockSyncRegistry.mockReset();
    mockInstallSkill.mockReset();
    mockAddRegistry.mockReset();
    mockRemoveRegistry.mockReset();
    mockFindDuplicateRegistry.mockReset();
    mockRescan.mockReset();
    mockLoadCentralSkills.mockReset();
    mockInstallCentralSkill.mockReset();
    mockToastSuccess.mockReset();
    mockToastError.mockReset();

    storeState.registries = [
      {
        id: "reg-1",
        name: "Repo One",
        source_type: "github",
        url: "https://github.com/acme/repo-one",
        is_builtin: false,
        is_enabled: true,
        last_synced: "2026-04-16T00:00:00Z",
        last_attempted_sync: "2026-04-16T00:10:00Z",
        last_sync_status: "success",
        last_sync_error: null,
        cache_updated_at: "2026-04-16T00:00:00Z",
        cache_expires_at: "2026-04-17T00:00:00Z",
        etag: null,
        last_modified: null,
        created_at: "2026-04-15T00:00:00Z",
      },
    ];
    storeState.skills = [
      {
        id: "skill-1",
        registry_id: "reg-1",
        name: "Cached Skill",
        description: "Skill from cache",
        download_url: "https://example.com/skill-1",
        is_installed: false,
        synced_at: "2026-04-16T00:00:00Z",
        cache_updated_at: "2026-04-16T00:00:00Z",
      },
    ];
    storeState.selectedRegistryId = "reg-1";
    storeState.searchQuery = "";
    storeState.isLoading = false;
    storeState.isSyncing = false;
    storeState.installingIds = new Set<string>();
    storeState.error = null as string | null;
    mockFindDuplicateRegistry.mockImplementation(() => null);
  });

  function renderView() {
    return render(
      <MemoryRouter>
        <MarketplaceView />
      </MemoryRouter>
    );
  }

  it("shows cached status for the selected source and keeps cached skills visible", async () => {
    renderView();

    fireEvent.click(screen.getByRole("button", { name: "My Sources" }));

    expect(await screen.findByText("Cached Skill")).toBeInTheDocument();
    expect(screen.getByText(/Cached ·|缓存可用/i)).toBeInTheDocument();
    expect(screen.getByText(/Reopening this source reuses backend cache/i)).toBeInTheDocument();
    expect(screen.getByText(/Cache valid until:/i)).toBeInTheDocument();
  });

  it("uses cached update without forcing a refresh", async () => {
    mockSyncRegistry.mockResolvedValue(undefined);
    renderView();

    fireEvent.click(screen.getByRole("button", { name: "My Sources" }));
    fireEvent.click(screen.getByRole("button", { name: "Update" }));

    await waitFor(() => {
      expect(mockSyncRegistry).toHaveBeenCalledWith("reg-1", false);
    });
    expect(mockToastSuccess).not.toHaveBeenCalledWith("Marketplace cache updated");
  });

  it("force refreshes and reports cached fallback after a failure", async () => {
    mockSyncRegistry.mockRejectedValue(new Error("network down"));
    storeState.error = "Error: network down" as string | null;
    storeState.registries = [
      {
        ...storeState.registries[0],
        last_sync_status: "error",
        last_sync_error: "network down",
      },
    ];

    renderView();

    fireEvent.click(screen.getByRole("button", { name: "My Sources" }));
    fireEvent.click(screen.getByRole("button", { name: "Force Refresh" }));

    await waitFor(() => {
      expect(mockSyncRegistry).toHaveBeenCalledWith("reg-1", true);
    });
    expect(await screen.findByText(/Refresh failed, showing cached data/i)).toBeInTheDocument();
    expect(screen.getByText("Cached Skill")).toBeInTheDocument();
  });

  it("shows persisted source metadata and deletes a source from My Sources", async () => {
    mockRemoveRegistry.mockResolvedValue(undefined);
    renderView();

    fireEvent.click(screen.getByRole("button", { name: "My Sources" }));

    expect(screen.getByText("Repo One")).toBeInTheDocument();
    expect(screen.getByText(/Source identity and sync metadata persist/i)).toBeInTheDocument();
    expect(screen.getByText(/Cache updated:/i)).toBeInTheDocument();

    fireEvent.click(screen.getAllByRole("button", { name: "Delete" })[0]);

    await waitFor(() => {
      expect(mockRemoveRegistry).toHaveBeenCalledWith("reg-1");
    });
    expect(mockToastSuccess).toHaveBeenCalledWith("Source deleted");
  });

  it("warns when adding a source that duplicates an official source", async () => {
    mockAddRegistry.mockRejectedValue(
      new Error(
        'DUPLICATE_REGISTRY:{"id":"official-1","name":"Anthropic","url":"https://github.com/anthropics/skills","isBuiltin":true}'
      )
    );

    renderView();

    fireEvent.click(screen.getByRole("button", { name: "Official Directory" }));
    fireEvent.click(screen.getByRole("button", { name: /Anthropic/i }));
    fireEvent.click(screen.getAllByRole("button", { name: /\+ Add to My Sources/i })[0]);

    await waitFor(() => {
      expect(mockToastError).toHaveBeenCalledWith(
        "This repo already exists in Official Directory: Anthropic"
      );
    });
  });
});
