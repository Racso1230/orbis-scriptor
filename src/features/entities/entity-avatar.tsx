import type { EntityTypeDef, Portrait } from "@/lib/bindings";
import { portraitSrc } from "@/lib/file-src";
import { cn } from "@/lib/utils";
import { EntityIcon } from "./entity-icon";
import { lookFor } from "./type-look";

/**
 * Round portrait when the entity has one, otherwise the type's icon in the
 * type colour. `className` sets the size (for example `size-4`).
 */
export function EntityAvatar({
  type,
  portrait,
  className,
}: {
  type: Pick<EntityTypeDef, "icon" | "color" | "label"> | undefined;
  portrait: Portrait | null | undefined;
  className?: string;
}) {
  if (portrait) {
    return (
      <img
        src={portraitSrc(portrait)}
        alt=""
        data-testid="entity-avatar"
        draggable={false}
        className={cn("shrink-0 rounded-full object-cover", className)}
      />
    );
  }
  return (
    <EntityIcon
      icon={type?.icon ?? ""}
      className={cn("shrink-0", className)}
      style={{ color: lookFor(type).color }}
    />
  );
}
