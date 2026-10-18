import { useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useDisplayCalendar } from "@/features/calendars/queries";
import { useEntities, useEntity, useEntityTypes } from "@/features/entities/queries";
import { useDocument } from "@/features/lore/queries";
import { getBackgroundCanvas } from "@/features/maps/map-background";
import { useMap } from "@/features/maps/queries";
import { useRelationships } from "@/features/relationships/queries";
import { formatDate } from "@/features/timeline/calendar";
import { useEvent, useEventsForEntity } from "@/features/timeline/queries";
import type { CalendarDefinition, MapData } from "@/lib/bindings";
import { formatFieldValue } from "./format-field";
import { isLoreEmpty, renderLore } from "./lore-render";

interface Props {
  entityId: string;
  /** Called once when everything this section needs has loaded (images included). */
  onReady?: () => void;
}

/** One entity as a printable section: header, summary, fields, lore, relationships, events. */
export function EntityPrintSection({ entityId, onReady }: Props) {
  const entity = useEntity(entityId);
  const doc = useDocument(entityId);
  const relationships = useRelationships(entityId);
  const entities = useEntities();
  const types = useEntityTypes();
  const calendar = useDisplayCalendar();
  const isEvent = entity.data?.type === "event";
  const isMap = entity.data?.type === "map";
  const events = useEventsForEntity(entityId);
  const [mapReady, setMapReady] = useState(false);

  const nameOf = useMemo(() => {
    const byId = new Map((entities.data ?? []).map((e) => [e.id, e.name] as const));
    return (id: string) => byId.get(id);
  }, [entities.data]);

  // A query that failed is "settled" too: the section prints what it has.
  const settled =
    !entity.isPending &&
    !doc.isPending &&
    !relationships.isPending &&
    !entities.isPending &&
    !types.isPending &&
    !calendar.isPending &&
    (isEvent || !events.isPending);
  const ready = settled && (!isMap || mapReady);
  const reported = useRef(false);
  useEffect(() => {
    if (ready && !reported.current) {
      reported.current = true;
      onReady?.();
    }
  }, [ready, onReady]);

  if (entity.isPending) return <section className="print-entity">Loading…</section>;
  if (!entity.data) {
    return (
      <section className="print-entity">
        <p>This entity no longer exists.</p>
      </section>
    );
  }

  const e = entity.data;
  const type = types.data?.find((t) => t.key === e.type);
  const def: CalendarDefinition | undefined = calendar.data?.definition;
  const props = e.properties;
  const fieldRows = (type?.fields ?? [])
    .map((f) => ({
      label: f.label,
      text: formatFieldValue(f, props[f.key], { calendar: def, nameOf }),
    }))
    .filter((r): r is { label: string; text: string } => r.text !== null);

  return (
    <section className="print-entity" id={`e-${e.id}`} aria-label={e.name}>
      <header>
        <h1>{e.name}</h1>
        <p className="print-type">{type?.label ?? e.type}</p>
      </header>
      {e.summary.trim() !== "" && <p className="print-summary">{e.summary}</p>}

      {isEvent && <EventDatesPrint entityId={e.id} calendar={def} />}
      {isMap && <MapPrint entityId={e.id} onReady={() => setMapReady(true)} />}

      {fieldRows.length > 0 && (
        <dl className="print-fields">
          {fieldRows.map((r) => (
            <div key={r.label}>
              <dt>{r.label}</dt>
              <dd>{r.text}</dd>
            </div>
          ))}
        </dl>
      )}

      {doc.data && !isLoreEmpty(doc.data.content) && (
        <div className="print-lore prose max-w-none">{renderLore(doc.data.content, nameOf)}</div>
      )}

      {relationships.data && relationships.data.length > 0 && (
        <div>
          <h2>Relationships</h2>
          <ul>
            {relationships.data.map((r) => (
              <li key={r.linkId}>
                {r.label} <strong>{r.otherName}</strong>{" "}
                <span className="print-muted">
                  ({types.data?.find((t) => t.key === r.otherType)?.label ?? r.otherType})
                </span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {!isEvent && events.data && events.data.length > 0 && (
        <div>
          <h2>Events</h2>
          <ul>
            {events.data.map((ev) => (
              <li key={ev.entityId}>
                <strong>{ev.name}</strong>
                {def && (
                  <span className="print-muted">
                    {" "}
                    &ndash; {formatDate(def, ev.startDay, "full")}
                    {ev.endDay !== null && ` to ${formatDate(def, ev.endDay, "full")}`}
                  </span>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}

function EventDatesPrint({
  entityId,
  calendar,
}: {
  entityId: string;
  calendar: CalendarDefinition | undefined;
}) {
  const event = useEvent(entityId);
  if (!event.data || !calendar) return null;
  const { startDay, endDay } = event.data;
  return (
    <p className="print-dates">
      <strong>When:</strong> {formatDate(calendar, startDay, "full")}
      {endDay !== null && ` to ${formatDate(calendar, endDay, "full")}`}
    </p>
  );
}

/** The map's background image (or generated texture) and a list of what is pinned on it. */
function MapPrint({ entityId, onReady }: { entityId: string; onReady: () => void }) {
  const map = useMap(entityId);
  const data: MapData | undefined = map.data;
  const imageUrl = useMemo(() => (data ? backgroundUrl(data) : null), [data]);
  const reported = useRef(false);

  // No image to wait for: ready as soon as the map data has settled.
  useEffect(() => {
    if (!map.isPending && !imageUrl && !reported.current) {
      reported.current = true;
      onReady();
    }
  }, [map.isPending, imageUrl, onReady]);

  const linked = (data?.objects ?? []).filter((o) => o.entityName);
  return (
    <div className="print-map">
      {imageUrl && (
        <img
          src={imageUrl}
          alt={`Map of ${data?.map.name ?? ""}`}
          onLoad={() => {
            if (!reported.current) {
              reported.current = true;
              onReady();
            }
          }}
          onError={() => {
            if (!reported.current) {
              reported.current = true;
              onReady();
            }
          }}
        />
      )}
      {linked.length > 0 && (
        <>
          <h2>On this map</h2>
          <ul>
            {linked.map((o) => (
              <li key={o.id}>{o.entityName}</li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
}

/** A URL for the map's picture: the imported image, or the texture of a blank map. */
function backgroundUrl(data: MapData): string | null {
  const path = data.map.background?.path;
  if (path) return convertFileSrc(path);
  const style = data.map.backgroundStyle;
  if (!style) return null;
  try {
    return (
      getBackgroundCanvas(style, data.map.width, data.map.height)?.toDataURL("image/png") ?? null
    );
  } catch {
    return null;
  }
}
