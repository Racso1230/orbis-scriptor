import { AlertCircle, Check, Loader2 } from "lucide-react";
import { cn } from "@/lib/utils";
import type { SaveStatus } from "./use-autosave";

const LABELS: Record<SaveStatus, string> = {
  saved: "Saved",
  dirty: "Unsaved changes",
  saving: "Saving…",
  error: "Save failed",
};

export function SaveIndicator({ status, error }: { status: SaveStatus; error: string | null }) {
  return (
    <span
      role="status"
      title={error ?? undefined}
      className={cn(
        "flex items-center gap-1 text-xs text-muted-foreground",
        status === "error" && "text-destructive",
      )}
    >
      {status === "saving" && <Loader2 className="size-3 animate-spin" />}
      {status === "saved" && <Check className="size-3" />}
      {status === "error" && <AlertCircle className="size-3" />}
      {LABELS[status]}
    </span>
  );
}
