import { parseSnippet } from "./snippet";

/** Renders a backend snippet with matches in <mark>. Plain React nodes only, no raw HTML. */
export function HighlightedSnippet({ snippet }: { snippet: string }) {
  return (
    <>
      {parseSnippet(snippet).map((seg, i) =>
        seg.highlight ? (
          <mark key={i} className="rounded-sm bg-primary/20 px-0.5 text-foreground">
            {seg.text}
          </mark>
        ) : (
          <span key={i}>{seg.text}</span>
        ),
      )}
    </>
  );
}
