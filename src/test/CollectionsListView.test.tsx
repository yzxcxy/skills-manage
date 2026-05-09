import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { MemoryRouter, Route, Routes, useLocation, type Location } from "react-router-dom";
import { useEffect } from "react";
import { CollectionsListView } from "../pages/CollectionsListView";
import { Collection } from "../types";

// ─── Mock stores ──────────────────────────────────────────────────────────────

vi.mock("../stores/collectionStore", () => ({
  useCollectionStore: vi.fn(),
}));

vi.mock("../components/collection/CollectionEditor", () => ({
  CollectionEditor: ({ open }: { open: boolean }) =>
    open ? <div data-testid="collection-editor">editor</div> : null,
}));

import { useCollectionStore } from "../stores/collectionStore";

const mockLoadCollections = vi.fn();
const mockImportCollection = vi.fn();
const mockUseCollectionStore = vi.mocked(useCollectionStore);

const mockCollections: Collection[] = [
  {
    id: "col-1",
    name: "Frontend",
    description: "Frontend skills",
    created_at: "2026-04-09T00:00:00Z",
    updated_at: "2026-04-09T00:00:00Z",
  },
  {
    id: "col-2",
    name: "Backend",
    description: "Backend skills",
    created_at: "2026-04-09T00:00:00Z",
    updated_at: "2026-04-09T00:00:00Z",
  },
];

function applyStoreMocks(overrides: Record<string, unknown> = {}) {
  mockUseCollectionStore.mockImplementation((selector: unknown) => {
    const state = {
      collections: mockCollections,
      isLoading: false,
      loadCollections: mockLoadCollections,
      importCollection: mockImportCollection,
      ...overrides,
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

function renderList(
  initialEntry = "/collections",
  onLocationChange?: (location: Location) => void
) {
  applyStoreMocks();
  return render(
    <MemoryRouter initialEntries={[initialEntry]}>
      {onLocationChange && <LocationProbe onChange={onLocationChange} />}
      <Routes>
        <Route path="/collections" element={<CollectionsListView />} />
        <Route path="/collection/:collectionId" element={<div>collection-detail</div>} />
      </Routes>
    </MemoryRouter>
  );
}

// ─── Tests ────────────────────────────────────────────────────────────────────

describe("CollectionsListView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders collection cards for all collections", () => {
    renderList();
    expect(screen.getByRole("button", { name: /Frontend/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Backend/i })).toBeInTheDocument();
  });

  it("loads collections on mount", () => {
    renderList();
    expect(mockLoadCollections).toHaveBeenCalled();
  });

  it("navigates to collection detail when a card is clicked", async () => {
    const locations: Location[] = [];
    renderList("/collections", (loc) => locations.push(loc));

    fireEvent.click(screen.getByRole("button", { name: /Frontend/i }));

    await waitFor(() => {
      expect(locations.some((l) => l.pathname === "/collection/col-1")).toBe(true);
    });
  });

  it("shows empty state when there are no collections", () => {
    applyStoreMocks({ collections: [], isLoading: false });
    render(
      <MemoryRouter initialEntries={["/collections"]}>
        <Routes>
          <Route path="/collections" element={<CollectionsListView />} />
        </Routes>
      </MemoryRouter>
    );
    expect(screen.getByText("暂无技能集。请先在下方创建一个。")).toBeInTheDocument();
  });

  it("shows loading state", () => {
    applyStoreMocks({ isLoading: true });
    render(
      <MemoryRouter initialEntries={["/collections"]}>
        <Routes>
          <Route path="/collections" element={<CollectionsListView />} />
        </Routes>
      </MemoryRouter>
    );
    expect(screen.getByText("加载中...")).toBeInTheDocument();
  });

  it("opens the collection editor when clicking new collection", () => {
    renderList();
    fireEvent.click(screen.getByRole("button", { name: /新建技能集/i }));
    expect(screen.getByTestId("collection-editor")).toBeInTheDocument();
  });

});
