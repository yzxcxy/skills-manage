import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import App from "../App";

// Mock platformStore to prevent real Tauri invoke calls during tests
vi.mock("../stores/platformStore", () => ({
  usePlatformStore: vi.fn().mockImplementation((selector?: unknown) => {
    const state = {
      agents: [],
      skillsByAgent: {},
      isLoading: false,
      error: null,
      initialize: vi.fn(),
      rescan: vi.fn(),
    };
    if (typeof selector === "function") {
      return selector(state);
    }
    return state;
  }),
}));

describe("App", () => {
  it("renders the app shell with sidebar", () => {
    render(
      <MemoryRouter initialEntries={["/central"]}>
        <App />
      </MemoryRouter>
    );
    // Sidebar header is visible
    expect(screen.getByText("skills-manage")).toBeInTheDocument();
  });

  it("renders sidebar navigation sections", () => {
    render(
      <MemoryRouter initialEntries={["/central"]}>
        <App />
      </MemoryRouter>
    );
    expect(screen.getByText("By Tool")).toBeInTheDocument();
    // "Central Skills" appears in both the sidebar nav button and the main content header
    expect(screen.getAllByText("Central Skills").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText("Collections")).toBeInTheDocument();
    expect(screen.getByText("设置")).toBeInTheDocument();
  });
});
