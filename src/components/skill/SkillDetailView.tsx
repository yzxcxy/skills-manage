import { useEffect, useState, type Ref, type ReactNode } from "react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  Tag,
  Plus,
  FileText,
  Code,
  Bot,
  RefreshCw,
  Loader2,
  ChevronDown,
  ChevronRight,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { PlatformIcon } from "@/components/platform/PlatformIcon";
import { useSkillDetailStore } from "@/stores/skillDetailStore";
import { usePlatformStore } from "@/stores/platformStore";
import { CollectionPickerDialog } from "@/components/collection/CollectionPickerDialog";
import { AgentWithStatus, SkillInstallation } from "@/types";
import { cn } from "@/lib/utils";
import { isTauriRuntime } from "@/lib/tauri";

// ─── Section Label ─────────────────────────────────────────────────────────────

function SectionLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-[10px] font-bold uppercase tracking-widest text-muted-foreground/80 mb-2">
      {children}
    </div>
  );
}

// ─── MetadataRow (compact) ───────────────────────────────────────────────────

function MetadataRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="space-y-0.5">
      <div className="text-[10px] text-muted-foreground/70 uppercase tracking-wider">{label}</div>
      <div className="font-mono text-xs text-foreground break-all leading-relaxed">
        {value}
      </div>
    </div>
  );
}

// ─── Platform Toggle Icon (compact install/uninstall) ─────────────────────────

interface PlatformToggleIconProps {
  agent: AgentWithStatus;
  skillName: string;
  isInstalled: boolean;
  isLoading: boolean;
  onToggle: () => void;
}

function PlatformToggleIcon({ agent, skillName, isInstalled, isLoading, onToggle }: PlatformToggleIconProps) {
  const { t } = useTranslation();
  return (
    <button
      className={cn(
        "p-1.5 rounded-md transition-colors cursor-pointer",
        isInstalled
          ? "text-primary hover:bg-primary/15"
          : "text-muted-foreground/40 hover:bg-muted/60 hover:text-muted-foreground",
        isLoading && "animate-pulse pointer-events-none"
      )}
      title={`${agent.display_name}${isInstalled ? ` — ${t("central.linked")}` : ""}`}
      aria-label={t("central.toggleInstallLabel", { platform: agent.display_name, skill: skillName })}
      disabled={isLoading}
      onClick={onToggle}
    >
      <PlatformIcon agentId={agent.id} className="size-4 shrink-0" size={16} />
    </button>
  );
}

// ─── Tab Toggle ───────────────────────────────────────────────────────────────

type PreviewTab = "markdown" | "raw" | "explanation";

interface TabToggleProps {
  activeTab: PreviewTab;
  onChange: (tab: PreviewTab) => void;
}

function TabToggle({ activeTab, onChange }: TabToggleProps) {
  const { t } = useTranslation();
  return (
    <div className="flex border border-border rounded-lg p-0.5 gap-0.5 bg-muted/40">
      <button
        role="tab"
        aria-selected={activeTab === "markdown"}
        onClick={() => onChange("markdown")}
        className={cn(
          "flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors",
          activeTab === "markdown"
            ? "bg-background text-foreground shadow-sm"
            : "text-muted-foreground hover:text-foreground"
        )}
      >
        <FileText className="size-3.5" />
        {t("detail.markdown")}
      </button>
      <button
        role="tab"
        aria-selected={activeTab === "raw"}
        onClick={() => onChange("raw")}
        className={cn(
          "flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors",
          activeTab === "raw"
            ? "bg-background text-foreground shadow-sm"
            : "text-muted-foreground hover:text-foreground"
        )}
      >
        <Code className="size-3.5" />
        {t("detail.rawSource")}
      </button>
      <button
        role="tab"
        aria-selected={activeTab === "explanation"}
        onClick={() => onChange("explanation")}
        className={cn(
          "flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-colors",
          activeTab === "explanation"
            ? "bg-background text-foreground shadow-sm"
            : "text-muted-foreground hover:text-foreground"
        )}
      >
        <Bot className="size-3.5" />
        {t("detail.aiExplanation")}
      </button>
    </div>
  );
}

function stripFrontmatter(markdown: string) {
  const match = markdown.match(/^---\s*\n[\s\S]*?\n---\s*\n?([\s\S]*)$/);
  return match ? match[1].trimStart() : markdown;
}

const detailTypographyClassName = cn(
  "text-[13px] leading-6 text-foreground/90",
  "[&_p]:text-[13px] [&_p]:leading-6",
  "[&_li]:text-[13px] [&_li]:leading-6",
  "[&_blockquote]:text-[13px] [&_blockquote]:leading-6",
  "[&_h1]:text-lg [&_h1]:leading-7 [&_h1]:font-semibold",
  "[&_h2]:text-base [&_h2]:leading-6 [&_h2]:font-semibold",
  "[&_h3]:text-sm [&_h3]:leading-6 [&_h3]:font-semibold",
  "[&_h4]:text-[13px] [&_h4]:leading-6 [&_h4]:font-semibold",
  "[&_th]:text-xs [&_th]:leading-5",
  "[&_td]:text-xs [&_td]:leading-5",
  "[&_code]:text-[12px]",
  "[&_pre]:text-[12px] [&_pre]:leading-5",
  "[&_pre_code]:text-[12px] [&_pre_code]:leading-5"
);

// ─── SkillDetailView ──────────────────────────────────────────────────────────

/**
 * Shared presentation component for skill detail. Rendered by both the
 * full-page route wrapper (`SkillDetailPage`) and the list-entry drawer
 * (`SkillDetailDrawer`). This component owns:
 *   - ViewHeader (title/description/TabToggle + optional leading slot)
 *   - TwoColumnLayout (LeftPreview tab panel + RightSidebar metadata/install/collections)
 *   - CollectionPicker portal
 *
 * It does NOT render a back button, breadcrumb, or close button. Those belong
 * to the outer shell. It also does NOT call `useNavigate` / `useParams`; all
 * route/shell concerns are handled outside.
 */
export interface SkillDetailViewProps {
  /** The skill id to load. */
  skillId: string;
  /** Affects local styling only, never behavior. */
  variant: "page" | "drawer";
  /** ViewHeader leftmost slot; currently null from both shells. */
  leading?: ReactNode;
  /** Drawer-only: used so the view can request its shell to close (e.g. on Esc). */
  onRequestClose?: () => void;
  /** Optional: exposes the left-preview scroll container to the outer shell. */
  scrollContainerRef?: Ref<HTMLDivElement>;
  /** Optional id applied to the ViewHeader h1 for shell-level aria-labelledby. */
  titleId?: string;
}

export function SkillDetailView({
  skillId,
  variant,
  leading = null,
  onRequestClose: _onRequestClose,
  scrollContainerRef,
  titleId,
}: SkillDetailViewProps) {
  const { t, i18n } = useTranslation();

  // Store data
  const detail = useSkillDetailStore((s) => s.detail);
  const content = useSkillDetailStore((s) => s.content);
  const isLoading = useSkillDetailStore((s) => s.isLoading);
  const installingAgentId = useSkillDetailStore((s) => s.installingAgentId);
  const error = useSkillDetailStore((s) => s.error);
  const loadDetail = useSkillDetailStore((s) => s.loadDetail);
  const installSkill = useSkillDetailStore((s) => s.installSkill);
  const uninstallSkill = useSkillDetailStore((s) => s.uninstallSkill);
  const refreshInstallations = useSkillDetailStore((s) => s.refreshInstallations);
  const explanation = useSkillDetailStore((s) => s.explanation);
  const isExplanationLoading = useSkillDetailStore((s) => s.isExplanationLoading);
  const isExplanationStreaming = useSkillDetailStore((s) => s.isExplanationStreaming);
  const explanationError = useSkillDetailStore((s) => s.explanationError);
  const explanationErrorInfo = useSkillDetailStore((s) => s.explanationErrorInfo);
  const loadCachedExplanation = useSkillDetailStore((s) => s.loadCachedExplanation);
  const generateExplanation = useSkillDetailStore((s) => s.generateExplanation);
  const refreshExplanation = useSkillDetailStore((s) => s.refreshExplanation);
  const reset = useSkillDetailStore((s) => s.reset);

  // Platform agents (loaded at app init)
  const agents = usePlatformStore((s) => s.agents);
  const refreshCounts = usePlatformStore((s) => s.refreshCounts);

  // Local UI state
  const [activeTab, setActiveTab] = useState<PreviewTab>("markdown");
  const [isCollectionPickerOpen, setIsCollectionPickerOpen] = useState(false);
  const [showErrorDetails, setShowErrorDetails] = useState(false);

  // Load detail on mount / skillId change, reset on unmount.
  // `reset()` must be safe to call regardless of shell and aborts any
  // in-flight AI explanation request via its monotonic request-id logic.
  useEffect(() => {
    if (skillId) {
      loadDetail(skillId);
    }
    return () => {
      reset();
    };
  }, [skillId, loadDetail, reset]);

  useEffect(() => {
    if (skillId && content) {
      loadCachedExplanation(skillId, i18n.language);
    }
  }, [skillId, content, i18n.language, loadCachedExplanation]);

  // ── Derived values ───────────────────────────────────────────────────────

  const targetAgents = agents.filter((a) => a.id !== "central");
  const lobsterAgents = targetAgents.filter((a) => a.category === "lobster");
  const codingAgents = targetAgents.filter((a) => a.category !== "lobster");

  const installationMap = new Map<string, SkillInstallation>(
    (detail?.installations ?? []).map((inst) => [inst.agent_id, inst])
  );

  // ── Handlers ─────────────────────────────────────────────────────────────

  async function handleToggle(agentId: string) {
    if (!skillId) return;
    const isInstalled = installationMap.has(agentId);
    try {
      if (isInstalled) {
        await uninstallSkill(skillId, agentId);
      } else {
        await installSkill(skillId, agentId);
      }
      await Promise.all([
        refreshCounts(),
        refreshInstallations(skillId),
      ]);
    } catch (err) {
      toast.error(
        isInstalled
          ? t("detail.uninstallError", { error: String(err) })
          : t("detail.installError", { error: String(err) })
      );
    }
  }

  function handleCollectionAdded() {
    if (skillId) {
      loadDetail(skillId);
    }
  }

  function handleGenerateExplanation() {
    if (skillId && content) {
      generateExplanation(skillId, content, i18n.language);
    }
  }

  function handleRefreshExplanation() {
    if (skillId && content) {
      refreshExplanation(skillId, content, i18n.language);
    }
  }

  const markdownContent = content ? stripFrontmatter(content) : "";
  const isBrowserFallback = !isTauriRuntime() && !isLoading && !detail && !error;

  // ── Render ────────────────────────────────────────────────────────────────

  return (
    <div className={cn("flex flex-col h-full", variant === "drawer" && "min-h-0")}>
      {/* ── ViewHeader: leading slot + title/description + TabToggle ─────── */}
      <div className="border-b border-border px-6 py-3 flex items-center gap-3 shrink-0">
        {leading}
        <div className="min-w-0 flex-1">
          <h1 id={titleId} className="text-lg font-semibold truncate">
            {isLoading ? (skillId ?? "") : (detail?.name ?? skillId ?? "")}
          </h1>
          {detail?.description && (
            <p className="text-xs text-muted-foreground truncate mt-0.5">
              {detail.description}
            </p>
          )}
        </div>
        <TabToggle activeTab={activeTab} onChange={setActiveTab} />
      </div>

      {/* ── ContentArea ──────────────────────────────────────────────────── */}
      <div className="flex-1 min-h-0 overflow-hidden">
        {/* Loading state */}
        {isLoading && (
          <div className="flex items-center justify-center h-full gap-2 text-muted-foreground">
            <Loader2 className="size-5 animate-spin" />
            <span className="text-sm">{t("detail.loading")}</span>
          </div>
        )}

        {/* Error state */}
        {!isLoading && error && (
          <div className="flex items-center justify-center h-full">
            <div className="text-center space-y-2">
              <p className="text-sm text-destructive">{error}</p>
              <Button
                variant="outline"
                size="sm"
                onClick={() => skillId && loadDetail(skillId)}
              >
                {t("detail.retry")}
              </Button>
            </div>
          </div>
        )}

        {!isLoading && !error && isBrowserFallback && (
          <div className="flex items-center justify-center h-full">
            <div className="text-center space-y-3 max-w-md px-6">
              <Bot className="size-8 mx-auto text-muted-foreground/60" />
              <div className="space-y-1">
                <p className="text-sm font-medium">{t("detail.browserFallbackTitle")}</p>
                <p className="text-sm text-muted-foreground">
                  {t("detail.browserFallbackDesc")}
                </p>
              </div>
            </div>
          </div>
        )}

        {/* ── TwoColumnLayout: LeftPreview + RightSidebar ────────────────── */}
        {!isLoading && !error && detail && (
          <div
            className="flex h-full flex-col md:flex-row"
            data-testid="skill-detail-two-column-layout"
          >
            {/* ── Left: SKILL.md Preview ─────────────────────────────── */}
            <div
              ref={scrollContainerRef}
              className="flex-1 min-w-0 overflow-auto"
            >
              {activeTab === "markdown" ? (
                <div
                  className={cn("markdown-body p-6", detailTypographyClassName)}
                  role="tabpanel"
                  aria-label={t("detail.markdown")}
                >
                  {content ? (
                    <ReactMarkdown remarkPlugins={[remarkGfm]}>
                      {markdownContent}
                    </ReactMarkdown>
                  ) : (
                    <p className="text-sm text-muted-foreground italic">
                      {t("detail.noContent")}
                    </p>
                  )}
                </div>
              ) : activeTab === "raw" ? (
                <pre
                  className="p-6 text-[12px] leading-5 font-mono whitespace-pre-wrap break-words text-foreground/80"
                  role="tabpanel"
                  aria-label={t("detail.rawSource")}
                >
                  {content ?? t("detail.noContent")}
                </pre>
              ) : (
                <div
                  className="p-6 space-y-4"
                  role="tabpanel"
                  aria-label={t("detail.aiExplanation")}
                >
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <h2 className="text-sm font-semibold flex items-center gap-2">
                        <Bot className="size-4 text-primary" />
                        {t("detail.aiExplanation")}
                      </h2>
                      <p className="text-xs text-muted-foreground mt-1">
                        {t("detail.aiExplanationDesc")}
                      </p>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={explanation ? handleRefreshExplanation : handleGenerateExplanation}
                      disabled={!content || isExplanationLoading || isExplanationStreaming}
                      className="gap-1.5"
                    >
                      {isExplanationLoading || isExplanationStreaming ? (
                        <Loader2 className="size-3.5 animate-spin" />
                      ) : explanation ? (
                        <RefreshCw className="size-3.5" />
                      ) : (
                        <Bot className="size-3.5" />
                      )}
                      {explanation ? t("detail.regenerateExplanation") : t("detail.generateExplanation")}
                    </Button>
                  </div>

                  {explanationError && (
                    <div className="rounded-md border border-destructive/30 bg-destructive/5 p-3 space-y-2">
                      <p className="text-sm text-destructive">
                        {explanationErrorInfo?.message || explanationError}
                      </p>
                      {(explanationErrorInfo?.details || explanationError !== explanationErrorInfo?.message) && (
                        <div>
                          <button
                            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors cursor-pointer"
                            onClick={() => setShowErrorDetails((v) => !v)}
                          >
                            {showErrorDetails ? <ChevronDown className="size-3" /> : <ChevronRight className="size-3" />}
                            {t("detail.showDetails")}
                          </button>
                          {showErrorDetails && (
                            <pre className="mt-1.5 text-[11px] leading-4 font-mono text-muted-foreground whitespace-pre-wrap break-all bg-muted/30 rounded-md p-2 max-h-40 overflow-auto">
                              {explanationErrorInfo?.details || explanationError}
                            </pre>
                          )}
                        </div>
                      )}
                      {explanationErrorInfo?.fallbackTried && (
                        <p className="text-xs text-muted-foreground">
                          {t("detail.fallbackTried")}
                        </p>
                      )}
                    </div>
                  )}

                  {isExplanationLoading && !explanation ? (
                    <div className="flex items-center gap-2 text-sm text-muted-foreground">
                      <Loader2 className="size-4 animate-spin" />
                      {t("detail.explanationLoading")}
                    </div>
                  ) : explanation ? (
                    <div className={cn("markdown-body rounded-lg border border-border bg-card p-4", detailTypographyClassName)}>
                      <ReactMarkdown remarkPlugins={[remarkGfm]}>
                        {explanation}
                      </ReactMarkdown>
                      {isExplanationStreaming && (
                        <div className="mt-3 flex items-center gap-2 text-xs text-muted-foreground">
                          <Loader2 className="size-3.5 animate-spin" />
                          {t("detail.explanationStreaming")}
                        </div>
                      )}
                    </div>
                  ) : (
                    <div className="rounded-lg border border-dashed border-border p-8 text-center space-y-3">
                      <Bot className="size-8 mx-auto text-muted-foreground/60" />
                      <div>
                        <p className="text-sm font-medium">{t("detail.noExplanationTitle")}</p>
                        <p className="text-xs text-muted-foreground mt-1">
                          {t("detail.noExplanationDesc")}
                        </p>
                      </div>
                      <Button
                        size="sm"
                        onClick={handleGenerateExplanation}
                        disabled={!content || isExplanationLoading || isExplanationStreaming}
                        className="gap-1.5"
                      >
                        <Bot className="size-3.5" />
                        {t("detail.generateExplanation")}
                      </Button>
                    </div>
                  )}
                </div>
              )}
            </div>

            {/* ── Right: Sidebar ─────────────────────────────────────── */}
            <aside
              data-testid="skill-detail-right-sidebar"
              className="w-full shrink-0 border-t border-border overflow-y-auto p-4 space-y-5 md:w-64 md:border-t-0 md:border-l"
            >
              {/* Metadata */}
              <section aria-label={t("detail.metadataRegion")}>
                <SectionLabel>{t("detail.metadata")}</SectionLabel>
                <div className="space-y-2.5">
                  <MetadataRow label={t("detail.filePath")} value={detail.file_path} />
                  {detail.canonical_path && (
                    <MetadataRow label={t("detail.canonical")} value={detail.canonical_path} />
                  )}
                  {detail.source && (
                    <MetadataRow label={t("detail.source")} value={detail.source} />
                  )}
                  <MetadataRow
                    label={t("detail.scannedAt")}
                    value={new Date(detail.scanned_at).toLocaleString()}
                  />
                </div>
              </section>

              {/* Install Status — compact icon grid */}
              <section aria-label={t("detail.installStatusRegion")}>
                <SectionLabel>{t("detail.installStatus")}</SectionLabel>
                <div className="space-y-1.5">
                  {targetAgents.length === 0 ? (
                    <p className="text-xs text-muted-foreground">
                      {t("detail.noPlatforms")}
                    </p>
                  ) : (
                    <>
                      {lobsterAgents.length > 0 && (
                        <div className="flex items-center gap-1">
                          <span className="text-[10px] font-medium text-muted-foreground/70 uppercase tracking-wider w-12 shrink-0">
                            {t("sidebar.categoryLobster")}
                          </span>
                          <div className="flex items-center gap-0.5 flex-wrap">
                            {lobsterAgents.map((agent) => (
                              <PlatformToggleIcon
                                key={agent.id}
                                agent={agent}
                                skillName={detail.name}
                                isInstalled={installationMap.has(agent.id)}
                                isLoading={installingAgentId === agent.id}
                                onToggle={() => handleToggle(agent.id)}
                              />
                            ))}
                          </div>
                        </div>
                      )}
                      {codingAgents.length > 0 && (
                        <div className="flex items-center gap-1">
                          <span className="text-[10px] font-medium text-muted-foreground/70 uppercase tracking-wider w-12 shrink-0">
                            {t("sidebar.categoryCoding")}
                          </span>
                          <div className="flex items-center gap-0.5 flex-wrap">
                            {codingAgents.map((agent) => (
                              <PlatformToggleIcon
                                key={agent.id}
                                agent={agent}
                                skillName={detail.name}
                                isInstalled={installationMap.has(agent.id)}
                                isLoading={installingAgentId === agent.id}
                                onToggle={() => handleToggle(agent.id)}
                              />
                            ))}
                          </div>
                        </div>
                      )}
                    </>
                  )}
                </div>
              </section>

              {/* Collections */}
              <section aria-label={t("detail.collections")}>
                <SectionLabel>{t("detail.collections")}</SectionLabel>
                <div className="flex flex-wrap gap-1.5 items-center">
                  {(detail.collections ?? []).map((collectionId) => (
                    <span
                      key={collectionId}
                      className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium bg-primary/10 text-primary ring-1 ring-primary/20"
                    >
                      <Tag className="size-2.5" />
                      {collectionId}
                    </span>
                  ))}
                  <Button
                    variant="ghost"
                    size="sm"
                    className="gap-1 text-muted-foreground hover:text-foreground h-6 px-2 text-xs"
                    aria-label={t("detail.addToCollection")}
                    onClick={() => setIsCollectionPickerOpen(true)}
                  >
                    <Plus className="size-3" />
                    {t("detail.addToCollection")}
                  </Button>
                </div>
              </section>
            </aside>
          </div>
        )}
      </div>

      {/* Collection Picker Dialog */}
      {skillId && (
        <CollectionPickerDialog
          open={isCollectionPickerOpen}
          onOpenChange={setIsCollectionPickerOpen}
          skillId={skillId}
          currentCollectionIds={detail?.collections ?? []}
          onAdded={handleCollectionAdded}
        />
      )}
    </div>
  );
}
