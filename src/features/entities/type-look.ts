import { useEffect } from "react";
import { create } from "zustand";
import type { EntityTypeDef } from "@/lib/bindings";
import { useEntityTypes } from "./queries";

/** Colour used when a type is unknown (matches the backend default for new types). */
export const DEFAULT_TYPE_COLOR = "#0891b2";

export interface TypeLook {
  /** `#rrggbb`. */
  color: string;
  /** One character for map pins: the first letter of the type's label. */
  glyph: string;
}

/** Colour, glyph and icon of an entity type. Pure, so it is easy to test. */
export function lookFor(type: Pick<EntityTypeDef, "color" | "label"> | undefined): TypeLook {
  if (!type) return { color: DEFAULT_TYPE_COLOR, glyph: "•" };
  const first = Array.from(type.label.trim())[0];
  return {
    color: /^#[0-9a-fA-F]{6}$/.test(type.color) ? type.color : DEFAULT_TYPE_COLOR,
    glyph: first ? first.toUpperCase() : "•",
  };
}

/**
 * Looks by type key, kept in a small store so Konva components (which render
 * in their own React root, outside the QueryClient context) can read them.
 */
interface LooksState {
  looks: Record<string, TypeLook>;
  setTypes: (types: EntityTypeDef[]) => void;
}

export const useTypeLooksStore = create<LooksState>()((set) => ({
  looks: {},
  setTypes: (types) =>
    set({ looks: Object.fromEntries(types.map((t) => [t.key, lookFor(t)] as const)) }),
}));

/** Mounted once in the world layout: keeps the looks store in step with the types query. */
export function useSyncTypeLooks() {
  const types = useEntityTypes();
  const setTypes = useTypeLooksStore((s) => s.setTypes);
  const data = types.data;
  // Runs again whenever a type is created, edited or deleted (the query is invalidated).
  useEffect(() => {
    if (data) setTypes(data);
  }, [data, setTypes]);
}

/** The look of one type, from the cached type list. */
export function useTypeLook(typeKey: string): TypeLook {
  const types = useEntityTypes();
  return lookFor(types.data?.find((t) => t.key === typeKey));
}
