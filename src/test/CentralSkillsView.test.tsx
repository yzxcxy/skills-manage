import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router-dom";
import { CentralSkillsView } from "../pages/CentralSkillsView";

vi.mock("../stores/collectionStore", () => ({
  useCollectionStore: vi.fn(),
}));

vi.mock("../stores/platformStore", () => ({
  usePlatformStore: vi.fn(),
}));

vi.mock("../stores/marketplaceStore", () => ({
  useMarketplaceStore: vi.fn(),
}));

import { useCollectionStore } from "../stores/collectionStore";
import { usePlatformStore } from "../stores/platformStore";
import { useMarketplaceStore } from "../stores/marketplaceStore";

const mockUseCollectionStore = useCollectionStore as unknown as ReturnType<
  typeof vi.fn
>;
const mockUsePlatformStore = usePlatformStore as unknown as ReturnType<
  typeof vi.fn
>;
const mockUseMarketplaceStore = useMarketplaceStore as unknown as ReturnType<
  typeof vi.fn
>;

describe("CentralSkillsView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders central skills directory path below title", () => {
    mockUseCollectionStore.mockImplementation(
      (selector: (s: unknown) => unknown) =>
        selector({
          collections: [],
          isLoading: false,
          loadCollections: vi.fn(),
          importCollection: vi.fn(),
          batchInstallCollection: vi.fn(),
          batchUninstallCollection: vi.fn(),
        })
    );

    mockUsePlatformStore.mockImplementation(
      (selector: (s: unknown) => unknown) =>
        selector({
          agents: [
            {
              id: "central",
              display_name: "Central Skills",
              category: "central",
              global_skills_dir: "/Users/test/.skillsmanage/central",
              is_detected: true,
              is_builtin: true,
              is_enabled: true,
            },
          ],
          refreshCounts: vi.fn(),
        })
    );

    mockUseMarketplaceStore.mockImplementation(
      (selector: (s: unknown) => unknown) =>
        selector({
          githubImport: {
            preview: null,
            error: null,
            isPreviewLoading: false,
            isImporting: false,
            importResult: null,
          },
          previewGitHubRepoImport: vi.fn(),
          importGitHubRepoSkills: vi.fn(),
          resetGitHubImport: vi.fn(),
        })
    );

    render(
      <MemoryRouter initialEntries={["/central"]}>
        <Routes>
          <Route path="/central" element={<CentralSkillsView />} />
        </Routes>
      </MemoryRouter>
    );

    // Should show the central skills directory path
    expect(
      screen.getByText("/Users/test/.skillsmanage/central")
    ).toBeInTheDocument();
  });
});
