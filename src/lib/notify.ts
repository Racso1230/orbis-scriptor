import { toast } from "sonner";
import { CommandError } from "@/lib/unwrap";

/** Human title for each kind of failure; the message from Rust is the detail. */
const TITLES: Record<CommandError["kind"], string> = {
  io: "File problem",
  validation: "Invalid input",
  notFound: "Not found",
  database: "Database problem",
  internal: "Something went wrong",
};

/**
 * Shows an error toast. `id` dedupes: reporting the same id again updates the
 * toast that is already on screen instead of stacking a second one.
 */
export function toastError(error: unknown, id?: string) {
  if (error instanceof CommandError) {
    toast.error(TITLES[error.kind], {
      id,
      description: error.message,
    });
    return;
  }
  toast.error("Something went wrong", {
    id,
    description: error instanceof Error ? error.message : String(error),
  });
}
