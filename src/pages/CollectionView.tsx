import { useParams } from "react-router-dom";

export function CollectionView() {
  const { collectionId } = useParams<{ collectionId: string }>();

  return (
    <div className="flex flex-col h-full">
      {/* Header */}
      <div className="border-b border-border px-6 py-4">
        <h1 className="text-lg font-semibold">Collection</h1>
        <p className="text-sm text-muted-foreground mt-0.5">ID: {collectionId}</p>
      </div>

      {/* Content */}
      <div className="flex-1 overflow-auto p-6">
        <div className="text-sm text-muted-foreground">
          Collection view — coming in next milestone
        </div>
      </div>
    </div>
  );
}
