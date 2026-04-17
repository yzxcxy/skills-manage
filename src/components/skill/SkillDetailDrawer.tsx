import { RefObject, useEffect, useId } from "react";
import { Dialog, DialogOverlay, DialogPortal } from "@/components/ui/dialog";
import { SkillDetailView } from "@/components/skill/SkillDetailView";
import { Button } from "@/components/ui/button";
import { XIcon } from "lucide-react";
import { cn } from "@/lib/utils";

export interface SkillDetailDrawerProps {
  open: boolean;
  skillId: string | null;
  onOpenChange: (open: boolean) => void;
  returnFocusRef?: RefObject<HTMLElement | null>;
}

export function SkillDetailDrawer({
  open,
  skillId,
  onOpenChange,
  returnFocusRef,
}: SkillDetailDrawerProps) {
  const titleId = useId();

  useEffect(() => {
    if (open) {
      return;
    }
    const target = returnFocusRef?.current ?? document.body;
    target?.focus?.();
  }, [open, returnFocusRef]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogPortal>
        <DialogOverlay
          data-testid="skill-detail-drawer-overlay"
          className="bg-black/30"
        />
        {open && skillId ? (
          <div className="fixed inset-0 z-50 flex justify-end pointer-events-none">
            <div
              role="dialog"
              aria-modal="true"
              aria-labelledby={titleId}
              data-testid="skill-detail-drawer"
              className={cn(
                "pointer-events-auto flex h-full w-screen flex-col bg-background shadow-2xl ring-1 ring-border outline-none",
                "md:w-[min(900px,90vw)]"
              )}
            >
              <div className="flex h-10 shrink-0 items-center justify-end border-b border-border px-2">
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Close"
                  onClick={() => onOpenChange(false)}
                >
                  <XIcon />
                </Button>
              </div>
              <div className="min-h-0 flex-1">
                <SkillDetailView
                  skillId={skillId}
                  variant="drawer"
                  leading={null}
                  onRequestClose={() => onOpenChange(false)}
                  titleId={titleId}
                />
              </div>
            </div>
          </div>
        ) : null}
      </DialogPortal>
    </Dialog>
  );
}
