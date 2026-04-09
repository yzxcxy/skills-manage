import { useParams } from "react-router-dom";
import { usePlatformStore } from "@/stores/platformStore";

export function PlatformView() {
  const { agentId } = useParams<{ agentId: string }>();
  const agents = usePlatformStore((state) => state.agents);
  const skillsByAgent = usePlatformStore((state) => state.skillsByAgent);

  const agent = agents.find((a) => a.id === agentId);
  const count = skillsByAgent[agentId ?? ""] ?? 0;

  if (!agent) {
    return (
      <div className="flex items-center justify-center h-full text-muted-foreground">
        Platform not found
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="border-b border-border px-6 py-4">
        <h1 className="text-lg font-semibold">{agent.display_name}</h1>
        <p className="text-sm text-muted-foreground mt-0.5">
          {agent.global_skills_dir}
        </p>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        {count === 0 ? (
          <div className="flex flex-col items-center justify-center h-full gap-3 text-muted-foreground">
            <p className="text-sm">No skills installed for {agent.display_name}</p>
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
