export interface SnippetSegment {
  text: string;
  highlight: boolean;
}

const START = "\u0002";
const END = "\u0003";

/**
 * Splits a snippet from the backend into plain and highlighted segments.
 * Matched terms are wrapped in U+0002 ... U+0003. Unbalanced markers are
 * tolerated: a missing end highlights to the end of the text, and stray
 * end markers are dropped. Empty segments are never returned.
 */
export function parseSnippet(snippet: string): SnippetSegment[] {
  const segments: SnippetSegment[] = [];
  let highlight = false;
  let current = "";

  const flush = () => {
    if (current) segments.push({ text: current, highlight });
    current = "";
  };

  for (const ch of snippet) {
    if (ch === START) {
      flush();
      highlight = true;
    } else if (ch === END) {
      flush();
      highlight = false;
    } else {
      current += ch;
    }
  }
  flush();
  return segments;
}
