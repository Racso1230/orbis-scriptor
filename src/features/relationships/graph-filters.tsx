import { CATEGORY_LABELS, groupByCategory } from "./kinds";
import { useGraphStore } from "./graph-store";
import type { EntityTypeDef, RelationshipKindDef } from "@/lib/bindings";

function Check({
  label,
  checked,
  onChange,
  className,
}: {
  label: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  className?: string;
}) {
  return (
    <label className={`flex items-center gap-2 text-sm ${className ?? ""}`}>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

/** Filter panel: relationship kinds by category, entity types, and weak links. */
export function GraphFilters({
  kinds,
  types,
}: {
  kinds: RelationshipKindDef[];
  types: EntityTypeDef[];
}) {
  const hiddenKinds = useGraphStore((s) => s.hiddenKinds);
  const hiddenTypes = useGraphStore((s) => s.hiddenTypes);
  const includeMentions = useGraphStore((s) => s.includeMentions);
  const includeInvolved = useGraphStore((s) => s.includeInvolved);
  const toggleKinds = useGraphStore((s) => s.toggleKinds);
  const toggleType = useGraphStore((s) => s.toggleType);
  const setIncludeMentions = useGraphStore((s) => s.setIncludeMentions);
  const setIncludeInvolved = useGraphStore((s) => s.setIncludeInvolved);

  return (
    <div
      role="group"
      aria-label="Graph filters"
      className="flex shrink-0 flex-wrap gap-x-8 gap-y-3 border-b bg-muted/30 px-4 py-3"
    >
      <div className="flex flex-col gap-1">
        <h2 className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
          Relationships
        </h2>
        <div className="flex flex-wrap gap-x-6 gap-y-2">
          {groupByCategory(kinds).map((group) => {
            const keys = group.items.map((k) => k.key);
            const allOn = keys.every((k) => !hiddenKinds.includes(k));
            return (
              <div key={group.category} className="flex flex-col gap-0.5">
                <Check
                  label={CATEGORY_LABELS[group.category]}
                  checked={allOn}
                  onChange={(on) => toggleKinds(keys, on)}
                  className="font-medium"
                />
                {group.items.map((k) => (
                  <Check
                    key={k.key}
                    label={k.forwardLabel}
                    checked={!hiddenKinds.includes(k.key)}
                    onChange={(on) => toggleKinds([k.key], on)}
                    className="pl-5 text-muted-foreground"
                  />
                ))}
              </div>
            );
          })}
        </div>
      </div>
      <div className="flex flex-col gap-1">
        <h2 className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
          Entity types
        </h2>
        {types.map((t) => (
          <Check
            key={t.key}
            label={t.plural}
            checked={!hiddenTypes.includes(t.key)}
            onChange={(on) => toggleType(t.key, on)}
          />
        ))}
      </div>
      <div className="flex flex-col gap-1">
        <h2 className="text-xs font-medium tracking-wide text-muted-foreground uppercase">
          Weak links
        </h2>
        <Check label="Mentions in lore" checked={includeMentions} onChange={setIncludeMentions} />
        <Check label="Involved in events" checked={includeInvolved} onChange={setIncludeInvolved} />
      </div>
    </div>
  );
}
