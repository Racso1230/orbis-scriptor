import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useParams, useRouter } from "@tanstack/react-router";
import { ArrowLeft, Printer } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useEntities, useEntityTypes } from "@/features/entities/queries";
import { useCurrentWorld } from "@/features/worlds/queries";
import { printNow } from "./print-now";
import { EntityPrintSection } from "./entity-print";
import { capEntities, sortForPrint, tocGroups, WORLD_PRINT_LIMIT } from "./world-print";

/** The print view must look the same whatever the app theme is: force the light theme. */
function useForcedLightTheme() {
  useEffect(() => {
    const root = document.documentElement;
    const wasDark = root.classList.contains("dark");
    root.classList.remove("dark");
    return () => {
      if (wasDark) root.classList.add("dark");
    };
  }, []);
}

/** Toolbar (hidden when printing), the content, and one automatic print once `ready`. */
function PrintFrame({
  ready,
  notice,
  children,
}: {
  ready: boolean;
  notice?: string;
  children: React.ReactNode;
}) {
  const router = useRouter();
  useForcedLightTheme();
  const printed = useRef(false);

  useEffect(() => {
    if (!ready || printed.current) return;
    printed.current = true;
    // Give the browser a frame to lay out images before the dialog snapshots the page.
    const id = window.setTimeout(() => void printNow(), 100);
    return () => window.clearTimeout(id);
  }, [ready]);

  return (
    <div className="print-root" data-print-ready={ready ? "true" : "false"}>
      <div className="print-toolbar print:hidden">
        <Button variant="outline" size="sm" onClick={() => router.history.back()}>
          <ArrowLeft /> Back
        </Button>
        <Button size="sm" disabled={!ready} onClick={() => void printNow()}>
          <Printer /> Print / Save as PDF
        </Button>
        <span className="text-sm text-neutral-600">
          {ready ? "Ready. Choose “Save as PDF” as the printer to make a PDF." : "Loading…"}
        </span>
        {notice && <span className="text-sm text-amber-700">{notice}</span>}
      </div>
      <main className="print-pages">{children}</main>
    </div>
  );
}

/** `/print/entity/$id`: one entity, printable. */
export function PrintEntityPage() {
  const { id } = useParams({ from: "/print/entity/$id" });
  const [ready, setReady] = useState(false);
  const onReady = useCallback(() => setReady(true), []);
  return (
    <PrintFrame ready={ready}>
      <EntityPrintSection key={id} entityId={id} onReady={onReady} />
    </PrintFrame>
  );
}

/** How many sections are mounted per step, so a big world does not freeze the window. */
const BATCH = 15;

/** `/print/world`: title page, contents by type, then every entity. */
export function PrintWorldPage() {
  const world = useCurrentWorld();
  const entities = useEntities();
  const types = useEntityTypes();

  const sorted = useMemo(
    () => sortForPrint(entities.data ?? [], types.data ?? []),
    [entities.data, types.data],
  );
  const { shown, total } = capEntities(sorted);
  const toc = useMemo(() => tocGroups(shown, types.data ?? []), [shown, types.data]);

  const [readyCount, setReadyCount] = useState(0);
  const onReady = useCallback(() => setReadyCount((n) => n + 1), []);

  // Always keep one batch ahead of the sections that finished loading.
  const visible = Math.min(shown.length, readyCount + BATCH);

  const loaded = !entities.isPending && !types.isPending;
  const ready = loaded && readyCount >= shown.length;
  const notice =
    total > WORLD_PRINT_LIMIT
      ? `Large world: only the first ${WORLD_PRINT_LIMIT} of ${total} entities are printed.`
      : loaded && !ready
        ? `Preparing ${readyCount} / ${shown.length}…`
        : undefined;

  return (
    <PrintFrame ready={ready} notice={notice}>
      <section className="print-title">
        <h1>{world.data?.name ?? "World"}</h1>
        <p className="print-muted">
          {shown.length} {shown.length === 1 ? "entity" : "entities"}
          {total > shown.length && ` (of ${total})`}
        </p>
      </section>
      <section className="print-toc" aria-label="Contents">
        <h2>Contents</h2>
        {toc.map((group) => (
          <div key={group.type.key}>
            <h3>{group.type.plural}</h3>
            <ul>
              {group.entities.map((e) => (
                <li key={e.id}>
                  <a href={`#e-${e.id}`} onClick={(ev) => ev.preventDefault()}>
                    {e.name}
                  </a>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </section>
      {shown.slice(0, visible).map((e) => (
        <EntityPrintSection key={e.id} entityId={e.id} onReady={onReady} />
      ))}
    </PrintFrame>
  );
}
