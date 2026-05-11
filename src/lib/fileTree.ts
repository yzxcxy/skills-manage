import type { SkillDirectoryNode, SkillsShFileEntry } from "@/types";

function sortNodes(nodes: SkillDirectoryNode[]) {
  nodes.sort((left, right) => {
    if (left.is_dir !== right.is_dir) return left.is_dir ? -1 : 1;
    return left.name.localeCompare(right.name);
  });
  for (const node of nodes) {
    sortNodes(node.children);
  }
}

export function buildSkillDirectoryTree(entries: SkillsShFileEntry[]): SkillDirectoryNode[] {
  const nodes = new Map<string, SkillDirectoryNode>();

  for (const entry of entries) {
    nodes.set(entry.path, {
      name: entry.name,
      path: entry.path,
      relative_path: entry.path,
      is_dir: entry.is_dir,
      children: [],
    });
  }

  const roots: SkillDirectoryNode[] = [];
  for (const node of nodes.values()) {
    const parentIndex = node.path.lastIndexOf("/");
    const parentPath = parentIndex > 0 ? node.path.slice(0, parentIndex) : null;
    const parent = parentPath ? nodes.get(parentPath) : null;
    if (parent) {
      parent.children.push(node);
    } else {
      roots.push(node);
    }
  }

  sortNodes(roots);
  return roots;
}
