import { usePlatformStore } from "@/stores/platformStore";

export function CentralSkillsView() {
  const skillsByAgent = usePlatformStore((state) => state.skillsByAgent);
  const count = skillsByAgent["central"] ?? 0;

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="border-b border-border px-6 py-4">
        <h1 className="text-lg font-semibold">Central Skills</h1>
        <p className="text-sm text-muted-foreground mt-0.5">~/.agents/skills/</p>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        {count === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
            <p className="text-sm">No skills in Central Skills (~/.agents/skills/)</p>
          </div>
        ) : (
          <div className="text-sm text-muted-foreground">
            {count} skill{count !== 1 ? "s" : ""} — skill list coming in next milestone
          </div>
        )}
      </div>
    </div>
  );
}
