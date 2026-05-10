import { useEffect, useMemo, useRef, useState } from "react";
import {
  FileInput,
  Layers,
  PackageMinus,
  PackagePlus,
  Plus,
  RefreshCw,
  Search,
} from "lucide-react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";

import { useCollectionStore } from "@/stores/collectionStore";
import { usePlatformStore } from "@/stores/platformStore";
import { Collection, CollectionBatchInstallResult } from "@/types";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { CollectionEditor } from "@/components/collection/CollectionEditor";
import { CollectionInstallDialog } from "@/components/collection/CollectionInstallDialog";
import { cn } from "@/lib/utils";

// ─── Empty State ──────────────────────────────────────────────────────────────

function EmptyState({ message }: { message: string }) {
  return (
    <div className="flex flex-col items-center justify-center h-full gap-4 py-20">
      <div className="p-4 rounded-full bg-muted/60">
        <Layers className="size-12 text-muted-foreground opacity-60" />
      </div>
      <p className="text-sm text-muted-foreground font-medium">{message}</p>
    </div>
  );
}

// ─── Collection Card ─────────────────────────────────────────────────────────

function CollectionCard({
  collection,
  onNavigate,
  onInstall,
  onUninstall,
}: {
  collection: Collection;
  onNavigate: () => void;
  onInstall: (e: React.MouseEvent) => void;
  onUninstall: (e: React.MouseEvent) => void;
}) {
  const { t } = useTranslation();
  const skillCount = collection.skill_count ?? 0;
  return (
    <div
      className={cn(
        "w-full text-left rounded-xl bg-card ring-1 ring-border shadow-sm",
        "p-4 flex flex-col gap-3 transition-all",
        "hover:ring-primary/25 hover:bg-accent/30 cursor-pointer focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      )}
      onClick={onNavigate}
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
            <p className="text-xs text-muted-foreground">
              {t("collection.skills", { count: skillCount })}
            </p>
          </div>
        </div>
      </div>

      {collection.description && (
        <p className="text-xs text-muted-foreground line-clamp-2 leading-relaxed">
          {collection.description}
        </p>
      )}

      <div className="mt-auto pt-2 border-t border-border/50 flex items-center justify-between gap-2">
        <span className="text-xs text-muted-foreground/60">
          {t("detail.showDetails")}
        </span>
        <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            title={t("collection.batchInstallLabel")}
            aria-label={t("collection.batchInstallLabel")}
            onClick={onInstall}
            disabled={skillCount === 0}
          >
            <PackagePlus className="size-3.5" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            title={t("collection.batchUninstallLabel")}
            aria-label={t("collection.batchUninstallLabel")}
            onClick={onUninstall}
            disabled={skillCount === 0}
          >
            <PackageMinus className="size-3.5" />
          </Button>
        </div>
      </div>
    </div>
  );
}

// ─── CentralSkillsView ───────────────────────────────────────────────────────

export function CentralSkillsView() {
  const { t } = useTranslation();
  const navigate = useNavigate();

  const collections = useCollectionStore((s) => s.collections);
  const isLoadingCollections = useCollectionStore((s) => s.isLoading);
  const loadCollections = useCollectionStore((s) => s.loadCollections);
  const importCollection = useCollectionStore((s) => s.importCollection);
  const batchInstallCollection = useCollectionStore((s) => s.batchInstallCollection);
  const batchUninstallCollection = useCollectionStore((s) => s.batchUninstallCollection);

  const agents = usePlatformStore((s) => s.agents);
  const refreshCounts = usePlatformStore((s) => s.refreshCounts);

  const [searchQuery, setSearchQuery] = useState("");
  const [isEditorOpen, setIsEditorOpen] = useState(false);
  const importInputRef = useRef<HTMLInputElement>(null);

  const [installDialog, setInstallDialog] = useState<{
    open: boolean;
    collection: Collection | null;
    mode: "install" | "uninstall";
  }>({ open: false, collection: null, mode: "install" });

  useEffect(() => {
    loadCollections();
  }, [loadCollections]);

  const filteredCollections = useMemo(() => {
    if (!searchQuery.trim()) return collections;
    const q = searchQuery.toLowerCase();
    return collections.filter(
      (c) =>
        c.name.toLowerCase().includes(q) ||
        (c.description ?? "").toLowerCase().includes(q)
    );
  }, [collections, searchQuery]);

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

  async function handleRefresh() {
    await loadCollections();
  }

  function handleOpenInstall(collection: Collection) {
    setInstallDialog({ open: true, collection, mode: "install" });
  }

  function handleOpenUninstall(collection: Collection) {
    setInstallDialog({ open: true, collection, mode: "uninstall" });
  }

  async function handleBatchAction(agentIds: string[]): Promise<CollectionBatchInstallResult> {
    const { collection, mode } = installDialog;
    if (!collection) return { succeeded: [], failed: [] };
    try {
      let result: CollectionBatchInstallResult;
      if (mode === "install") {
        result = await batchInstallCollection(collection.id, agentIds);
        toast.success(t("collection.installSuccess"));
      } else {
        result = await batchUninstallCollection(collection.id, agentIds);
        toast.success(t("collection.uninstallSuccess"));
      }
      await refreshCounts();
      return result;
    } catch (err) {
      toast.error(String(err));
      return { succeeded: [], failed: [{ agent_id: "batch", error: String(err) }] };
    }
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="border-b border-border px-6 py-4">
        <div className="flex items-center justify-between gap-4">
          <div className="flex items-center gap-2">
            <h1 className="text-xl font-semibold">{t("sidebar.centralSkills")}</h1>
            <Button
              variant="ghost"
              size="icon"
              onClick={handleRefresh}
              disabled={isLoadingCollections}
              aria-label={t("common.refresh")}
            >
              <RefreshCw className={`size-4 ${isLoadingCollections ? "animate-spin" : ""}`} />
            </Button>
          </div>
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

      {/* Search bar */}
      <div className="px-6 py-3 border-b border-border">
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 size-4 text-muted-foreground pointer-events-none" />
          <Input
            placeholder={t("central.searchPlaceholder")}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="pl-8 bg-muted/40"
            aria-label={t("central.searchPlaceholder")}
          />
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        {isLoadingCollections ? (
          <EmptyState message={t("common.loading")} />
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
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {filteredCollections.map((col) => (
              <CollectionCard
                key={col.id}
                collection={col}
                onNavigate={() => navigate(`/collection/${col.id}`)}
                onInstall={(e) => {
                  e.stopPropagation();
                  handleOpenInstall(col);
                }}
                onUninstall={(e) => {
                  e.stopPropagation();
                  handleOpenUninstall(col);
                }}
              />
            ))}
          </div>
        )}
      </div>

      <CollectionEditor open={isEditorOpen} onOpenChange={setIsEditorOpen} collection={null} />

      <CollectionInstallDialog
        open={installDialog.open}
        onOpenChange={(open) => setInstallDialog((prev) => ({ ...prev, open }))}
        collectionName={installDialog.collection?.name ?? ""}
        skillCount={installDialog.collection?.skill_count ?? 0}
        agents={agents}
        onInstall={handleBatchAction}
        mode={installDialog.mode}
      />

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
