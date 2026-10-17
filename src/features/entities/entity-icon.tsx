// `Map` is renamed so it does not shadow the global JavaScript Map.
import { Box, Flag, Map as MapIcon, MapPin, StickyNote, Sword, User, type LucideProps } from "lucide-react";

// Icon names come from the Rust entity type definitions (models/entity_type.rs).
const ICONS: Record<string, React.ComponentType<LucideProps>> = {
  user: User,
  "map-pin": MapPin,
  flag: Flag,
  sword: Sword,
  "sticky-note": StickyNote,
  map: MapIcon,
};

export function EntityIcon({ icon, ...props }: { icon: string } & LucideProps) {
  const Icon = ICONS[icon] ?? Box;
  return <Icon {...props} />;
}
