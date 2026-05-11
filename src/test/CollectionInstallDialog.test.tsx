import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { CollectionInstallDialog } from "../components/collection/CollectionInstallDialog";
import { AgentWithStatus, CollectionBatchInstallResult } from "../types";

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
    id: "gemini-cli",
    display_name: "Gemini CLI",
    category: "coding",
    global_skills_dir: "~/.gemini/skills/",
    is_detected: false,
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
  {
    id: "obsidian",
    display_name: "Obsidian",
    category: "obsidian",
    global_skills_dir: "~/Vault/.agents/skills/",
    is_detected: true,
    is_builtin: false,
    is_enabled: true,
  },
];

const mockOnOpenChange = vi.fn();
const mockOnInstall = vi.fn<() => Promise<CollectionBatchInstallResult>>();

function renderDialog(props: { mode?: "install" | "uninstall"; agents?: AgentWithStatus[] } = {}) {
  return render(
    <CollectionInstallDialog
      open={true}
      onOpenChange={mockOnOpenChange}
      collectionName="superpowers"
      skillCount={14}
      agents={props.agents ?? mockAgents}
      onInstall={mockOnInstall}
      mode={props.mode ?? "install"}
    />
  );
}

describe("CollectionInstallDialog", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("only shows detected install target platforms", () => {
    renderDialog();

    expect(screen.getByLabelText("Claude Code")).toBeInTheDocument();
    expect(screen.getByLabelText("Cursor")).toBeInTheDocument();
    expect(screen.queryByLabelText("Gemini CLI")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Central Skills")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Obsidian")).not.toBeInTheDocument();
    expect(screen.queryByText("(未检测到)")).not.toBeInTheDocument();
  });

  it("opens install mode with no platforms selected", () => {
    renderDialog();

    expect(screen.getByLabelText("Claude Code")).not.toBeChecked();
    expect(screen.getByLabelText("Cursor")).not.toBeChecked();
    expect(screen.getByRole("button", { name: /安装到 0 个平台/i })).toBeDisabled();
  });

  it("opens uninstall mode with no platforms selected", () => {
    renderDialog({ mode: "uninstall" });

    expect(screen.getByLabelText("Claude Code")).not.toBeChecked();
    expect(screen.getByLabelText("Cursor")).not.toBeChecked();
    expect(screen.getByRole("button", { name: /从 0 个平台卸载/i })).toBeDisabled();
  });

  it("calls onInstall with the selected detected platform", async () => {
    mockOnInstall.mockResolvedValueOnce({ succeeded: ["cursor"], failed: [] });
    renderDialog();

    fireEvent.click(screen.getByLabelText("Cursor"));
    fireEvent.click(screen.getByRole("button", { name: /安装到 1 个平台/i }));

    await waitFor(() => {
      expect(mockOnInstall).toHaveBeenCalledWith(["cursor"]);
    });
  });

  it("shows no-platform message when no detected install targets exist", () => {
    renderDialog({
      agents: mockAgents.map((agent) =>
        agent.id === "central" || agent.id === "obsidian"
          ? agent
          : { ...agent, is_detected: false }
      ),
    });

    expect(screen.getByText("未检测到平台。请在设置中添加平台。")).toBeInTheDocument();
    expect(screen.queryByLabelText("Claude Code")).not.toBeInTheDocument();
  });
});
