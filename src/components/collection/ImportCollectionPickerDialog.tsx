import { useState, useEffect } from "react";
import { Loader2, Plus, Layers, ArrowRight } from "lucide-react";
import { useTranslation } from "react-i18next";

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
import { useCollectionStore } from "@/stores/collectionStore";
import { Collection } from "@/types";
import { cn } from "@/lib/utils";

// ─── Types ────────────────────────────────────────────────────────────────────

export interface ImportCollectionChoice {
  type: "existing" | "new" | "default";
  collectionId?: string;
  collectionName?: string;
}

interface ImportCollectionPickerDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Called when user confirms a selection. */
  onConfirm: (choice: ImportCollectionChoice) => void;
  /** Default collection name suggestion for "new" mode. */
  defaultNewName?: string;
}

// ─── ImportCollectionPickerDialog ─────────────────────────────────────────────

export function ImportCollectionPickerDialog({
  open,
  onOpenChange,
  onConfirm,
  defaultNewName = "",
}: ImportCollectionPickerDialogProps) {
  const { t } = useTranslation();
  const collections = useCollectionStore((s) => s.collections);
  const isLoading = useCollectionStore((s) => s.isLoading);
  const loadCollections = useCollectionStore((s) => s.loadCollections);

  const [mode, setMode] = useState<"select" | "create">("select");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [newName, setNewName] = useState(defaultNewName);
  const [newNameError, setNewNameError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setMode("select");
      setSelectedId(null);
      setNewName(defaultNewName);
      setNewNameError(null);
      loadCollections();
    }
  }, [open, loadCollections, defaultNewName]);

  function handleSelectExisting(collection: Collection) {
    setSelectedId(collection.id);
  }

  function handleConfirm() {
    if (mode === "select") {
      if (selectedId) {
        onConfirm({ type: "existing", collectionId: selectedId });
      } else {
        // No selection → default collection.
        onConfirm({ type: "default" });
      }
    } else if (mode === "create") {
      const trimmed = newName.trim();
      if (!trimmed) {
        setNewNameError(t("collectionEditor.nameRequired"));
        return;
      }
      onConfirm({ type: "new", collectionName: trimmed });
    }
  }

  const defaultCollection = collections.find((c) => c.is_default);
  const nonDefaultCollections = collections.filter((c) => !c.is_default);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t("importCollectionPicker.title")}</DialogTitle>
          <DialogClose />
        </DialogHeader>

        <DialogBody className="space-y-4">
          <DialogDescription>{t("importCollectionPicker.desc")}</DialogDescription>

          {mode === "select" ? (
            <>
              {/* Collections list */}
              <div
                className="max-h-56 overflow-y-auto space-y-1 border border-border rounded-md p-2"
                role="radiogroup"
                aria-label={t("importCollectionPicker.selectCollection")}
              >
                {isLoading ? (
                  <div className="flex items-center justify-center py-6 gap-2 text-muted-foreground text-sm">
                    <Loader2 className="size-4 animate-spin" />
                    {t("collectionPicker.loading")}
                  </div>
                ) : collections.length === 0 ? (
                  <p className="text-sm text-muted-foreground text-center py-6">
                    {t("collectionPicker.noCollections")}
                  </p>
                ) : (
                  <>
                    {/* Default collection option */}
                    {defaultCollection && (
                      <button
                        type="button"
                        role="radio"
                        aria-checked={selectedId === null}
                        onClick={() => setSelectedId(null)}
                        className={cn(
                          "w-full text-left flex items-center gap-2.5 px-2 py-2 rounded transition-colors",
                          selectedId === null
                            ? "bg-primary/10 ring-1 ring-primary/30"
                            : "hover:bg-hover-bg/20"
                        )}
                      >
                        <Layers className="size-4 text-muted-foreground shrink-0" />
                        <div className="min-w-0 flex-1">
                          <div className="text-sm font-medium">{defaultCollection.name}</div>
                          <div className="text-xs text-muted-foreground">
                            {t("importCollectionPicker.defaultHint")}
                          </div>
                        </div>
                        {selectedId === null && (
                          <ArrowRight className="size-3.5 text-primary shrink-0" />
                        )}
                      </button>
                    )}

                    {/* Other collections */}
                    {nonDefaultCollections.map((collection) => (
                      <button
                        key={collection.id}
                        type="button"
                        role="radio"
                        aria-checked={selectedId === collection.id}
                        onClick={() => handleSelectExisting(collection)}
                        className={cn(
                          "w-full text-left flex items-center gap-2.5 px-2 py-2 rounded transition-colors",
                          selectedId === collection.id
                            ? "bg-primary/10 ring-1 ring-primary/30"
                            : "hover:bg-hover-bg/20"
                        )}
                      >
                        <Layers className="size-4 text-muted-foreground shrink-0" />
                        <div className="min-w-0 flex-1">
                          <div className="text-sm font-medium">{collection.name}</div>
                          {collection.description && (
                            <div className="text-xs text-muted-foreground line-clamp-1">
                              {collection.description}
                            </div>
                          )}
                        </div>
                        {selectedId === collection.id && (
                          <ArrowRight className="size-3.5 text-primary shrink-0" />
                        )}
                      </button>
                    ))}
                  </>
                )}
              </div>

              {/* Create new collection */}
              <Button
                variant="ghost"
                size="sm"
                className="w-full gap-1.5 text-muted-foreground hover:text-foreground"
                onClick={() => setMode("create")}
              >
                <Plus className="size-3.5" />
                {t("importCollectionPicker.createNew")}
              </Button>
            </>
          ) : (
            /* Create new collection mode */
            <div className="space-y-3">
              <div>
                <label className="text-sm font-medium">
                  {t("collectionEditor.nameLabel")}
                </label>
                <Input
                  value={newName}
                  onChange={(e) => {
                    setNewName(e.target.value);
                    setNewNameError(null);
                  }}
                  placeholder={t("collectionEditor.namePlaceholder")}
                  className="mt-1"
                  autoFocus
                />
                {newNameError && (
                  <p className="text-xs text-destructive mt-1">{newNameError}</p>
                )}
              </div>
              <Button
                variant="ghost"
                size="sm"
                className="w-full"
                onClick={() => setMode("select")}
              >
                {t("importCollectionPicker.backToSelect")}
              </Button>
            </div>
          )}
        </DialogBody>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t("common.cancel")}
          </Button>
          <Button onClick={handleConfirm}>
            {mode === "select" && selectedId === null
              ? t("importCollectionPicker.useDefault")
              : t("common.confirm")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
