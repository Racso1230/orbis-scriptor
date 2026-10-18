import { tagColorMap } from "./tag-utils";
import { TagInput } from "./tag-input";
import { useSetEntityTags, useTags } from "./queries";

/** The tag editor on an entity page: saves immediately, like the star and gallery. */
export function EntityTags({ entityId, tags }: { entityId: string; tags: string[] }) {
  const all = useTags();
  const setTags = useSetEntityTags(entityId);
  const colors = tagColorMap(all.data ?? []);
  return (
    <TagInput
      value={tags}
      known={(all.data ?? []).map((t) => t.name)}
      colors={colors}
      onChange={(names) => setTags.mutateAsync(names)}
    />
  );
}
