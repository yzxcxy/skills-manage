import { useEffect, useRef, useState } from "react";
import {
  Plus,
  FileInput,
  Layers,
  Loader2,
  ArrowRight,
} from "lucide-react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { useCollectionStore } from "@/stores/collectionStore";
import { Collection } from "@/types";
import { CollectionEditor } from "@/components/collection/CollectionEditor";
import { cn } from "@/lib/utils";

// ─── CollectionsListView ─────────────────────────────────────────────────────

export function CollectionsListView() {
  const { t } = useTranslation();
  const navigate = useNavigate();

  const collections = useCollectionStore((s) => s.collections);
  const isLoading = useCollectionStore((s) => s.isLoading);
  const loadCollections = useCollectionStore((s) => s.loadCollections);
  const importCollection = useCollectionStore((s) => s.importCollection);

  const [isEditorOpen, setIsEditorOpen] = useState(false);
  const importInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    loadCollections();
  }, [loadCollections]);

  async function handleImportFile(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      const collection = await importCollection(text);
      navigate(`/collection/${collection.id}`);
    } catch (err) {
      toast.error(String(err));
    } finally {
      if (importInputRef.current) importInputRef.current.value = "";
    }
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="border-b border-border px-6 py-4">
        <div className="flex items-center justify-between gap-4">
          <h1 className="text-xl font-semibold">{t("sidebar.collections")}</h1>
          <div className="flex items-center gap-2 shrink-0">
            <Button
              variant="outline"
              size="sm"
              onClick={() => importInputRef.current?.click()}
            >
              <FileInput className="size-3.5" />
              <span>{t("sidebar.importCollection")}</span>
            </Button>
            <Button
              variant="default"
              size="sm"
              onClick={() => setIsEditorOpen(true)}
            >
              <Plus className="size-3.5" />
              <span>{t("sidebar.newCollectionLabel")}</span>
            </Button>
          </div>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto">
        {isLoading ? (
          <div className="flex items-center justify-center h-full gap-3 text-muted-foreground">
            <Loader2 className="size-5 animate-spin" />
            <span className="text-sm">{t("common.loading")}</span>
          </div>
        ) : collections.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-4 py-20">
            <div className="p-4 rounded-full bg-muted/60">
              <Layers className="size-12 text-muted-foreground opacity-60" />
            </div>
            <p className="text-sm font-medium text-muted-foreground">
              {t("collectionPicker.noCollections")}
            </p>
            <Button variant="default" size="sm" onClick={() => setIsEditorOpen(true)}>
              <Plus className="size-3.5" />
              {t("sidebar.newCollectionLabel")}
            </Button>
          </div>
        ) : (
          <div className="p-6 grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {collections.map((col) => (
              <CollectionCard
                key={col.id}
                collection={col}
                onClick={() => navigate(`/collection/${col.id}`)}
              />
            ))}
          </div>
        )}
      </div>

      <CollectionEditor open={isEditorOpen} onOpenChange={setIsEditorOpen} collection={null} />

      <input
        ref={importInputRef}
        type="file"
        accept=".json"
        className="hidden"
        onChange={handleImportFile}
      />
    </div>
  );
}

// ─── CollectionCard ──────────────────────────────────────────────────────────

function CollectionCard({
  collection,
  onClick,
}: {
  collection: Collection;
  onClick: () => void;
}) {

  return (
    <button
      onClick={onClick}
      className={cn(
        "w-full text-left rounded-xl bg-card ring-1 ring-border shadow-sm",
        "p-4 flex flex-col gap-3 transition-all",
        "hover:ring-primary/25 hover:bg-accent/30 cursor-pointer focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      )}
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-center gap-2.5 min-w-0">
          <div className="flex items-center justify-center size-9 rounded-lg bg-primary/10 text-primary shrink-0">
            <Layers className="size-5" />
          </div>
          <div className="min-w-0">
            <h3 className="text-sm font-semibold truncate text-foreground">
              {collection.name}
            </h3>
          </div>
        </div>
        <ArrowRight className="size-4 text-muted-foreground shrink-0 mt-2.5" />
      </div>

      {collection.description && (
        <p className="text-xs text-muted-foreground line-clamp-2 leading-relaxed">
          {collection.description}
        </p>
      )}

      <div className="mt-auto pt-2 border-t border-border/50 flex items-center justify-between">
        <span className="text-xs text-muted-foreground/60">
          {new Date(collection.updated_at).toLocaleDateString()}
        </span>
      </div>
    </button>
  );
}
