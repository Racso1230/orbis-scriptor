import { useEffect, useRef, useState } from "react";
import type { MapInfo, MapObject } from "@/lib/bindings";
import { focusPoint, type Point } from "./geometry";
import { cleanLabelText, defaultLabelStyle, labelLook } from "./label-style";
import { useMapActions } from "./map-actions-context";
import { useMapToolStore, type LabelInput } from "./map-tool-store";
import { mapToScreen, type Transform } from "./viewport";

const IDENTITY: Transform = { scale: 1, x: 0, y: 0 };

interface Props {
  input: LabelInput;
  map: MapInfo;
  objects: MapObject[];
}

/**
 * The small text box that floats over the map while you type a label:
 * Enter or clicking away saves it, Esc cancels. New labels are one undo step;
 * editing the text of an existing one is another.
 */
export function LabelInputBox({ input, map, objects }: Props) {
  // The saved viewport follows every pan and zoom, so the box stays on its spot.
  const transform = useMapToolStore((s) => s.viewports[map.entityId]) ?? IDENTITY;
  const actions = useMapActions();
  const ref = useRef<HTMLInputElement>(null);
  const done = useRef(false);

  const editing = input.mode === "edit" ? objects.find((o) => o.id === input.objectId) : undefined;
  const editGeometry = editing?.geometry.kind === "label" ? editing.geometry : undefined;
  const [text, setText] = useState(editGeometry?.text ?? "");

  // Where to show the box, in map pixels: the click, or the label's anchor.
  let at: Point | null = null;
  if (input.mode === "create") at = { x: input.x, y: input.y };
  else if (editGeometry) {
    const followed = editGeometry.pathId ? objects.find((o) => o.id === editGeometry.pathId) : null;
    at = (followed ? focusPoint(followed.geometry) : null) ?? {
      x: editGeometry.x,
      y: editGeometry.y,
    };
  }

  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);

  if (!at || (input.mode === "edit" && !editGeometry)) return null;
  const screen = mapToScreen(transform, at);
  const look = labelLook(editing?.style ?? defaultLabelStyle(map.width, map.backgroundStyle));
  const fontPx = Math.min(36, Math.max(14, look.fontSize * transform.scale));

  function close() {
    useMapToolStore.getState().setLabelInput(null);
  }

  function commit() {
    if (done.current) return;
    done.current = true;
    const value = cleanLabelText(text);
    if (input.mode === "create") {
      if (value !== "") {
        const [id] = actions.create("Add label", [
          {
            layerId: input.layerId,
            geometry: {
              kind: "label",
              x: input.x,
              y: input.y,
              text: value,
              rotation: 0,
              pathId: input.pathId,
            },
            style: defaultLabelStyle(map.width, map.backgroundStyle),
          },
        ]);
        if (id) useMapToolStore.getState().select(id);
      }
    } else if (editGeometry && value !== "" && value !== editGeometry.text) {
      actions.update("Edit label text", [
        { id: input.objectId, patch: { geometry: { ...editGeometry, text: value } } },
      ]);
    }
    close();
  }

  function cancel() {
    done.current = true;
    close();
  }

  return (
    <input
      ref={ref}
      type="text"
      aria-label="Label text"
      maxLength={200}
      value={text}
      placeholder="Label text"
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") {
          e.preventDefault();
          commit();
        } else if (e.key === "Escape") {
          e.preventDefault();
          cancel();
        }
      }}
      className="absolute z-20 w-56 -translate-x-1/2 -translate-y-1/2 rounded-md border border-ring bg-background/95 px-2 py-1 text-center shadow-lg outline-none"
      style={{
        left: screen.x,
        top: screen.y,
        fontFamily: look.fontFamily,
        fontSize: fontPx,
        color: "inherit",
      }}
    />
  );
}
