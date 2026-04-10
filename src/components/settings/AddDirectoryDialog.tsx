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

// ─── Props ────────────────────────────────────────────────────────────────────

interface AddDirectoryDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdd: (path: string) => Promise<void>;
}

// ─── AddDirectoryDialog ───────────────────────────────────────────────────────

export function AddDirectoryDialog({
  open,
  onOpenChange,
  onAdd,
}: AddDirectoryDialogProps) {
  const [path, setPath] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [validationError, setValidationError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Reset form when dialog opens.
  useEffect(() => {
    if (open) {
      setPath("");
      setValidationError(null);
      setError(null);
    }
  }, [open]);

  async function handleSubmit() {
    const trimmedPath = path.trim();
    if (!trimmedPath) {
      setValidationError("目录路径不能为空");
      return;
    }

    setIsSubmitting(true);
    setValidationError(null);
    setError(null);

    try {
      await onAdd(trimmedPath);
      onOpenChange(false);
    } catch (err) {
      setError(String(err));
    } finally {
      setIsSubmitting(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter" && !isSubmitting) {
      handleSubmit();
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>添加项目目录</DialogTitle>
          <DialogClose />
        </DialogHeader>

        <DialogBody className="space-y-4">
          <DialogDescription>
            输入要扫描的自定义目录路径。添加后将自动重新扫描。
          </DialogDescription>

          {/* Path field */}
          <div className="space-y-1.5">
            <label htmlFor="dir-path" className="text-sm font-medium">
              目录路径 <span className="text-destructive">*</span>
            </label>
            <Input
              id="dir-path"
              placeholder="例如: ~/projects/my-project"
              value={path}
              onChange={(e) => {
                setPath(e.target.value);
                if (validationError) setValidationError(null);
              }}
              onKeyDown={handleKeyDown}
              disabled={isSubmitting}
              autoFocus
            />
            {validationError && (
              <p className="text-xs text-destructive" role="alert">
                {validationError}
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
                添加中...
              </>
            ) : (
              "添加"
            )}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
