import { useEffect, useState } from "react";
import {
  Plus,
  Pencil,
  Trash2,
  Download,
  PackagePlus,
  PackageMinus,
  Bomb,
  ArrowLeft,
  Loader2,
  BookOpen,
} from "lucide-react";
import { useParams, useNavigate, Link } from "react-router-dom";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";

import { Button } from "@/components/ui/button";
import { useCollectionStore } from "@/stores/collectionStore";
import { usePlatformStore } from "@/stores/platformStore";
import { useCentralSkillsStore } from "@/stores/centralSkillsStore";
import { CollectionEditor } from "@/components/collection/CollectionEditor";
import { SkillPickerDialog } from "@/components/collection/SkillPickerDialog";
import { CollectionInstallDialog } from "@/components/collection/CollectionInstallDialog";
import { InstallDialog } from "@/components/central/InstallDialog";
import { UnifiedSkillCard } from "@/components/skill/UnifiedSkillCard";
import { SkillWithLinks } from "@/types";

// ─── CollectionDetailView ────────────────────────────────────────────────────

export function CollectionDetailView() {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { collectionId } = useParams<{ collectionId: string }>();

  const loadCollectionDetail = useCollectionStore((s) => s.loadCollectionDetail);
  const currentDetail = useCollectionStore((s) => s.currentDetail);
  const isLoadingDetail = useCollectionStore((s) => s.isLoadingDetail);
  const removeSkillFromCollection = useCollectionStore((s) => s.removeSkillFromCollection);
  const deleteCollection = useCollectionStore((s) => s.deleteCollection);
  const batchInstallCollection = useCollectionStore((s) => s.batchInstallCollection);
  const batchUninstallCollection = useCollectionStore((s) => s.batchUninstallCollection);
  const batchDeleteCollectionSkills = useCollectionStore((s) => s.batchDeleteCollectionSkills);
  const exportCollection = useCollectionStore((s) => s.exportCollection);
  const addSkillToCollection = useCollectionStore((s) => s.addSkillToCollection);

  const agents = usePlatformStore((s) => s.agents);
  const refreshCounts = usePlatformStore((s) => s.refreshCounts);

  const centralSkills = useCentralSkillsStore((s) => s.skills);
  const centralAgents = useCentralSkillsStore((s) => s.agents);
  const loadCentralSkills = useCentralSkillsStore((s) => s.loadCentralSkills);
  const installCentralSkill = useCentralSkillsStore((s) => s.installSkill);

  const [isEditOpen, setIsEditOpen] = useState(false);
  const [isPickerOpen, setIsPickerOpen] = useState(false);
  const [isInstallOpen, setIsInstallOpen] = useState(false);
  const [isUninstallOpen, setIsUninstallOpen] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [installTargetSkill, setInstallTargetSkill] = useState<SkillWithLinks | null>(null);
  const [isSingleInstallOpen, setIsSingleInstallOpen] = useState(false);

  useEffect(() => {
    if (collectionId) {
      loadCollectionDetail(collectionId);
    }
  }, [collectionId, loadCollectionDetail]);

  useEffect(() => {
    if (centralSkills.length === 0) {
      loadCentralSkills();
    }
  }, [centralSkills.length, loadCentralSkills]);

  function handleGoBack() {
    navigate(-1);
  }

  function handleSkillClick(skillId: string) {
    navigate(`/skill/${skillId}`, {
      state: {
        from: {
          pageLabel: currentDetail?.name ?? t("sidebar.collections"),
          route: `/collection/${collectionId}`,
        },
      },
    });
  }

  function handleInstallSingleSkillClick(skillId: string) {
    const target = centralSkills.find((s) => s.id === skillId);
    if (!target) {
      toast.error(t("central.installError", { error: t("platform.notFound") }));
      return;
    }
    setInstallTargetSkill(target);
    setIsSingleInstallOpen(true);
  }

  async function handleInstallSingleSkill(skillId: string, agentIds: string[], method: string) {
    try {
      const result = await installCentralSkill(skillId, agentIds, method);
      await refreshCounts();
      if (result.failed.length > 0) {
        const failedNames = result.failed.map((f) => f.agent_id).join(", ");
        toast.error(t("central.installPartialFail", { platforms: failedNames }));
      }
    } catch (err) {
      toast.error(t("central.installError", { error: String(err) }));
    }
  }

  async function handleRemoveSkill(skillId: string) {
    if (!collectionId) return;
    try {
      await removeSkillFromCollection(collectionId, skillId);
    } catch (err) {
      toast.error(t("collection.removeSkillError", { error: String(err) }));
    }
  }

  async function handleDelete() {
    if (!collectionId || !currentDetail) return;
    if (!window.confirm(t("collection.deleteConfirm", { name: currentDetail.name }))) return;
    setIsDeleting(true);
    try {
      await deleteCollection(collectionId);
      navigate("/central");
    } catch (err) {
      toast.error(t("collection.deleteError", { error: String(err) }));
    } finally {
      setIsDeleting(false);
    }
  }

  async function handleExport() {
    if (!collectionId || !currentDetail) return;
    try {
      const json = await exportCollection(collectionId);
      const blob = new Blob([json], { type: "application/json" });
      const url = URL.createObjectURL(blob);
      const a = document.createElement("a");
      a.href = url;
      a.download = `${currentDetail.name.replace(/\s+/g, "-").toLowerCase()}-collection.json`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
    } catch (err) {
      toast.error(t("collection.exportError", { error: String(err) }));
    }
  }

  async function handleAddSkills(skillIds: string[]) {
    if (!collectionId) return;
    try {
      for (const skillId of skillIds) {
        await addSkillToCollection(collectionId, skillId);
      }
    } catch (err) {
      toast.error(t("collection.addSkillError", { error: String(err) }));
    }
  }

  async function handleBatchUninstall(agentIds: string[]) {
    if (!collectionId) {
      return { succeeded: [], failed: [] };
    }
    try {
      const result = await batchUninstallCollection(collectionId, agentIds);
      await refreshCounts();
      if (result.failed.length > 0) {
        const failedNames = result.failed.map((f) => f.agent_id).join(", ");
        toast.error(t("collection.uninstallPartialFail", { platforms: failedNames }));
      } else {
        toast.success(t("collection.uninstallSuccess"));
      }
      return result;
    } catch (err) {
      toast.error(t("collection.uninstallError", { error: String(err) }));
      return { succeeded: [], failed: [{ agent_id: "batch", error: String(err) }] };
    }
  }

  async function handleBatchDeleteSkills() {
    if (!collectionId || !currentDetail) return;
    if (!window.confirm(t("collection.deleteSkillsConfirm", { name: currentDetail.name, count: currentDetail.skills.length }))) return;
    setIsDeleting(true);
    try {
      const result = await batchDeleteCollectionSkills(collectionId);
      await refreshCounts();
      if (result.failed.length > 0) {
        const failedNames = result.failed.map((f) => f.skill_id).join(", ");
        toast.error(t("collection.deleteSkillsPartialFail", { skills: failedNames }));
      } else {
        toast.success(t("collection.deleteSkillsSuccess", { count: result.deletedSkillIds.length }));
      }
      navigate("/central");
    } catch (err) {
      toast.error(t("collection.deleteSkillsError", { error: String(err) }));
    } finally {
      setIsDeleting(false);
    }
  }

  const detailName = currentDetail?.name ?? "";

  return (
    <div className="flex flex-col h-full">
      {/* Header with back button and breadcrumb */}
      <div className="border-b border-border px-6 py-2 flex items-center gap-3 shrink-0">
        <button
          onClick={handleGoBack}
          className="p-1.5 rounded-md hover:bg-muted transition-colors text-muted-foreground hover:text-foreground"
          aria-label={t("detail.goBack")}
        >
          <ArrowLeft className="size-4" />
        </button>
        <nav aria-label={t("detail.breadcrumb")} className="min-w-0 flex-1">
          <ol className="flex items-center gap-1.5 text-xs text-muted-foreground min-w-0">
            <li className="shrink-0">
              <Link
                to="/central"
                className="hover:text-foreground hover:underline truncate"
              >
                {t("sidebar.centralSkills")}
              </Link>
            </li>
            <li aria-hidden="true" className="text-muted-foreground/60 shrink-0">
              ›
            </li>
            <li className="min-w-0 truncate text-foreground font-medium">
              {detailName}
            </li>
          </ol>
        </nav>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto">
        {isLoadingDetail || !currentDetail ? (
          <div className="flex items-center justify-center h-full gap-3 text-muted-foreground">
            <Loader2 className="size-5 animate-spin" />
            <span className="text-sm">{t("collection.loading")}</span>
          </div>
        ) : (
          <div className="flex flex-col">
            {/* Detail header */}
            <div className="flex items-center justify-between gap-4 px-6 py-3 border-b border-border">
              <div className="min-w-0">
                <h2 className="text-sm font-semibold truncate">{currentDetail.name}</h2>
                {currentDetail.description && (
                  <p className="text-xs text-muted-foreground mt-0.5 truncate">
                    {currentDetail.description}
                  </p>
                )}
              </div>
              <div className="flex items-center gap-2 shrink-0">
                <Button variant="outline" size="sm" onClick={() => setIsEditOpen(true)}>
                  <Pencil className="size-3.5" />
                  <span>{t("collection.edit")}</span>
                </Button>
                <Button variant="outline" size="sm" onClick={handleExport}>
                  <Download className="size-3.5" />
                  <span>{t("collection.export")}</span>
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleDelete}
                  disabled={isDeleting}
                  className="text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
                >
                  {isDeleting ? <Loader2 className="size-3.5 animate-spin" /> : <Trash2 className="size-3.5" />}
                  <span>{t("collection.delete")}</span>
                </Button>
              </div>
            </div>

            {/* Skills sub-header */}
            <div className="flex items-center justify-between px-6 py-3 border-b border-border">
              <span className="text-sm font-medium text-muted-foreground">
                {t("collection.skills", { count: currentDetail.skills.length })}
              </span>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setIsInstallOpen(true)}
                  disabled={currentDetail.skills.length === 0}
                >
                  <PackagePlus className="size-3.5" />
                  <span>{t("collection.batchInstall")}</span>
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setIsUninstallOpen(true)}
                  disabled={currentDetail.skills.length === 0}
                >
                  <PackageMinus className="size-3.5" />
                  <span>{t("collection.batchUninstall")}</span>
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleBatchDeleteSkills}
                  disabled={currentDetail.skills.length === 0 || isDeleting}
                  className="text-destructive hover:text-destructive hover:bg-destructive/10 border-destructive/30"
                >
                  {isDeleting ? <Loader2 className="size-3.5 animate-spin" /> : <Bomb className="size-3.5" />}
                  <span>{t("collection.batchDeleteSkills")}</span>
                </Button>
                <Button variant="default" size="sm" onClick={() => setIsPickerOpen(true)}>
                  <Plus className="size-3.5" />
                  <span>{t("collection.addSkill")}</span>
                </Button>
              </div>
            </div>

            {/* Skills list */}
            <div className="flex-1 overflow-auto">
              {currentDetail.skills.length === 0 ? (
                <div className="flex flex-col items-center justify-center h-full gap-4 py-20">
                  <div className="p-4 rounded-full bg-muted/60">
                    <BookOpen className="size-12 text-muted-foreground opacity-60" />
                  </div>
                  <p className="text-sm font-medium text-muted-foreground">{t("collection.noSkillsTitle")}</p>
                  <p className="text-xs text-muted-foreground/70">{t("collection.noSkillsDesc")}</p>
                  <Button variant="default" size="sm" onClick={() => setIsPickerOpen(true)}>
                    <Plus className="size-3.5" />
                    {t("collection.addFirstSkill")}
                  </Button>
                </div>
              ) : (
                <div className="mx-6 my-3 grid grid-cols-2 gap-4">
                  {currentDetail.skills.map((skill) => (
                    <UnifiedSkillCard
                      key={skill.id}
                      name={skill.name}
                      description={skill.description}
                      onDetail={() => handleSkillClick(skill.id)}
                      onInstallTo={() => handleInstallSingleSkillClick(skill.id)}
                      onRemove={() => handleRemoveSkill(skill.id)}
                    />
                  ))}
                </div>
              )}
            </div>
          </div>
        )}
      </div>

      {/* Dialogs */}
      {currentDetail && (
        <>
          <CollectionEditor
            open={isEditOpen}
            onOpenChange={setIsEditOpen}
            collection={{
              id: currentDetail.id,
              name: currentDetail.name,
              description: currentDetail.description,
              created_at: currentDetail.created_at,
              updated_at: currentDetail.updated_at,
            }}
          />
          <SkillPickerDialog
            open={isPickerOpen}
            onOpenChange={setIsPickerOpen}
            existingSkillIds={currentDetail.skills.map((s) => s.id)}
            onAdd={handleAddSkills}
          />
          <CollectionInstallDialog
            open={isInstallOpen}
            onOpenChange={setIsInstallOpen}
            collectionName={currentDetail.name}
            skillCount={currentDetail.skills.length}
            agents={agents}
            onInstall={(agentIds) => batchInstallCollection(currentDetail.id, agentIds)}
          />
          <CollectionInstallDialog
            open={isUninstallOpen}
            onOpenChange={setIsUninstallOpen}
            collectionName={currentDetail.name}
            skillCount={currentDetail.skills.length}
            agents={agents}
            onInstall={(agentIds) => handleBatchUninstall(agentIds)}
            mode="uninstall"
          />
        </>
      )}

      <InstallDialog
        open={isSingleInstallOpen}
        onOpenChange={setIsSingleInstallOpen}
        skill={installTargetSkill}
        agents={centralAgents}
        onInstall={handleInstallSingleSkill}
      />
    </div>
  );
}
