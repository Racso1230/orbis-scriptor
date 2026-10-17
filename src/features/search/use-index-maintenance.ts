import { useState } from "react";
import { useSeedEvents } from "@/features/timeline/queries";
import { useRebuildSearchIndex, useSeedEntities } from "./queries";

export const SEED_COUNT = 1000;
export const SEED_EVENT_COUNT = 200;

export type MaintenanceStatus = { kind: "success" | "error"; message: string } | null;

function errorText(e: unknown): string {
  return e instanceof Error ? e.message : "Something went wrong.";
}

/** Rebuild / seed actions plus a status message, shared by the palette and the debug menu. */
export function useIndexMaintenance() {
  const rebuild = useRebuildSearchIndex();
  const seed = useSeedEntities();
  const seedEventsMutation = useSeedEvents();
  const [status, setStatus] = useState<MaintenanceStatus>(null);

  async function rebuildIndex() {
    setStatus(null);
    try {
      await rebuild.mutateAsync();
      setStatus({ kind: "success", message: "Search index rebuilt." });
    } catch (e) {
      setStatus({ kind: "error", message: `Rebuild failed: ${errorText(e)}` });
    }
  }

  async function seedEntities() {
    setStatus(null);
    try {
      const created = await seed.mutateAsync(SEED_COUNT);
      setStatus({ kind: "success", message: `Created ${created} test entities.` });
    } catch (e) {
      setStatus({ kind: "error", message: `Seeding failed: ${errorText(e)}` });
    }
  }

  async function seedEvents() {
    setStatus(null);
    try {
      const created = await seedEventsMutation.mutateAsync(SEED_EVENT_COUNT);
      setStatus({ kind: "success", message: `Created ${created} test events.` });
    } catch (e) {
      setStatus({ kind: "error", message: `Seeding failed: ${errorText(e)}` });
    }
  }

  return {
    rebuildIndex,
    seedEntities,
    seedEvents,
    status,
    busy: rebuild.isPending || seed.isPending || seedEventsMutation.isPending,
  };
}
