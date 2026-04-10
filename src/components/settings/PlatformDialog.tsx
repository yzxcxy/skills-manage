import { useState, useEffect } from "react";
import { Loader2 } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogBody,
  DialogFooter,
  DialogClose,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { AgentWithStatus } from "@/types";

// ─── Props ────────────────────────────────────────────────────────────────────

interface PlatformDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Pass a platform to edit it; null for create mode. */
  platform: AgentWithStatus | null;
  onAdd?: (displayName: string, globalSkillsDir: string) => Promise<void>;
  onEdit?: (displayName: string, globalSkillsDir: string) => Promise<void>;
}

// ─── PlatformDialog ───────────────────────────────────────────────────────────

export function PlatformDialog({
  open,
  onOpenChange,
  platform,
  onAdd,
  onEdit,
}: PlatformDialogProps) {
  const isEditMode = platform !== null;

  const [displayName, setDisplayName] = useState("");
  const [globalSkillsDir, setGlobalSkillsDir] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [nameError, setNameError] = useState<string | null>(null);
  const [dirError, setDirError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Reset form when dialog opens.
  useEffect(() => {
    if (open) {
      setDisplayName(platform?.display_name ?? "");
      setGlobalSkillsDir(platform?.global_skills_dir ?? "");
      setNameError(null);
      setDirError(null);
      setError(null);
    }
  }, [open, platform]);

  async function handleSubmit() {
    const trimmedName = displayName.trim();
    const trimmedDir = globalSkillsDir.trim();

    let hasError = false;
    if (!trimmedName) {
      setNameError("平台名称不能为空");
      hasError = true;
    } else {
      setNameError(null);
    }
    if (!trimmedDir) {
      setDirError("技能目录路径不能为空");
      hasError = true;
    } else {
      setDirError(null);
    }

    if (hasError) return;

    setIsSubmitting(true);
    setError(null);

    try {
      if (isEditMode && onEdit) {
        await onEdit(trimmedName, trimmedDir);
      } else if (!isEditMode && onAdd) {
        await onAdd(trimmedName, trimmedDir);
      }
      onOpenChange(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {isEditMode ? "编辑自定义平台" : "添加自定义平台"}
          </DialogTitle>
          <DialogClose />
        </DialogHeader>

        <DialogBody className="space-y-4">
          <DialogDescription>
            {isEditMode
              ? "修改自定义平台的名称和技能目录。"
              : "注册一个新的自定义平台，添加后将自动重新扫描。"}
          </DialogDescription>

          {/* Display name field */}
          <div className="space-y-1.5">
            <label htmlFor="platform-name" className="text-sm font-medium">
              平台名称 <span className="text-destructive">*</span>
            </label>
            <Input
              id="platform-name"
              placeholder="例如: QClaw"
              value={displayName}
              onChange={(e) => {
                setDisplayName(e.target.value);
                if (nameError) setNameError(null);
              }}
              disabled={isSubmitting}
              autoFocus
            />
            {nameError && (
              <p className="text-xs text-destructive" role="alert">
                {nameError}
              </p>
            )}
          </div>

          {/* Global skills dir field */}
          <div className="space-y-1.5">
            <label htmlFor="platform-dir" className="text-sm font-medium">
              技能目录路径 <span className="text-destructive">*</span>
            </label>
            <Input
              id="platform-dir"
              placeholder="例如: ~/.qclaw/skills/"
              value={globalSkillsDir}
              onChange={(e) => {
                setGlobalSkillsDir(e.target.value);
                if (dirError) setDirError(null);
              }}
              disabled={isSubmitting}
            />
            {dirError && (
              <p className="text-xs text-destructive" role="alert">
                {dirError}
              </p>
            )}
          </div>

          {/* Backend error */}
          {error && (
            <p className="text-xs text-destructive" role="alert">
              {error}
            </p>
          )}
        </DialogBody>

        <DialogFooter>
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={isSubmitting}
          >
            取消
          </Button>
          <Button onClick={handleSubmit} disabled={isSubmitting}>
            {isSubmitting ? (
              <>
                <Loader2 className="size-3.5 animate-spin" />
                {isEditMode ? "保存中..." : "添加中..."}
              </>
            ) : isEditMode ? (
              "保存"
            ) : (
              "添加"
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
