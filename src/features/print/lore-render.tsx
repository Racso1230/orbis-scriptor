import { Fragment, type ReactNode } from "react";

// A small read-only renderer for stored TipTap JSON, used by the print view.
// It covers the nodes and marks the editor can produce (StarterKit, tables and
// mentions). Unknown nodes render their children, so nothing is ever lost or
// crashes the page.

interface Mark {
  type: string;
  attrs?: Record<string, unknown>;
}
interface Node {
  type?: string;
  attrs?: Record<string, unknown>;
  content?: Node[];
  marks?: Mark[];
  text?: string;
}

/** Parses the stored JSON text; an unreadable document is treated as empty. */
export function parseLore(content: string): Node {
  try {
    const parsed: unknown = JSON.parse(content);
    return typeof parsed === "object" && parsed !== null ? (parsed as Node) : { type: "doc" };
  } catch {
    return { type: "doc" };
  }
}

/** True when the document has no text at all (so the print view can skip the section). */
export function isLoreEmpty(content: string): boolean {
  function hasContent(node: Node): boolean {
    if (node.type === "mention" || node.type === "horizontalRule") return true;
    if (node.text && node.text.trim() !== "") return true;
    return (node.content ?? []).some(hasContent);
  }
  return !hasContent(parseLore(content));
}

/** Links must be http(s) or mailto in print too; anything else is shown as plain text. */
function safeHref(href: unknown): string | null {
  return typeof href === "string" && /^(https?:\/\/|mailto:)/i.test(href) ? href : null;
}

function renderMarks(text: ReactNode, marks: Mark[] = []): ReactNode {
  return marks.reduce<ReactNode>((inner, mark) => {
    switch (mark.type) {
      case "bold":
        return <strong>{inner}</strong>;
      case "italic":
        return <em>{inner}</em>;
      case "strike":
        return <s>{inner}</s>;
      case "code":
        return <code>{inner}</code>;
      case "underline":
        return <u>{inner}</u>;
      case "link": {
        const href = safeHref(mark.attrs?.["href"]);
        return href ? <a href={href}>{inner}</a> : inner;
      }
      default:
        return inner;
    }
  }, text);
}

/** Renders a document. `nameOf` resolves mention ids to entity names. */
export function renderLore(
  content: string,
  nameOf: (entityId: string) => string | undefined,
): ReactNode {
  function children(node: Node): ReactNode {
    return (node.content ?? []).map((child, i) => <Fragment key={i}>{render(child)}</Fragment>);
  }

  function render(node: Node): ReactNode {
    switch (node.type) {
      case "text":
        return renderMarks(node.text ?? "", node.marks);
      case "paragraph":
        return <p>{children(node)}</p>;
      case "heading": {
        const level = Math.min(6, Math.max(1, Number(node.attrs?.["level"] ?? 2)));
        // Entity names are h1 in the print view, so lore headings start at h2.
        const Tag = `h${Math.min(6, level + 1)}` as "h2";
        return <Tag>{children(node)}</Tag>;
      }
      case "bulletList":
        return <ul>{children(node)}</ul>;
      case "orderedList":
        return <ol>{children(node)}</ol>;
      case "listItem":
        return <li>{children(node)}</li>;
      case "blockquote":
        return <blockquote>{children(node)}</blockquote>;
      case "codeBlock":
        return (
          <pre>
            <code>{children(node)}</code>
          </pre>
        );
      case "horizontalRule":
        return <hr />;
      case "hardBreak":
        return <br />;
      case "table":
        return (
          <table>
            <tbody>{children(node)}</tbody>
          </table>
        );
      case "tableRow":
        return <tr>{children(node)}</tr>;
      case "tableHeader":
        return <th>{children(node)}</th>;
      case "tableCell":
        return <td>{children(node)}</td>;
      case "mention": {
        const id = String(node.attrs?.["id"] ?? "");
        // Mentions print as the entity's name; a deleted entity is marked, like in the editor.
        return <span className="mention">{nameOf(id) ?? "[missing entity]"}</span>;
      }
      default:
        return children(node);
    }
  }

  return render(parseLore(content));
}
