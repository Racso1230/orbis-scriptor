import { useEffect, useRef, useState } from "react";
import { EditorContent, useEditor, useEditorState } from "@tiptap/react";
import StarterKit from "@tiptap/starter-kit";
import { TableKit } from "@tiptap/extension-table";
import { Placeholder } from "@tiptap/extensions";
import { Bold, Italic, Link as LinkIcon, Table2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useCreateEntity, useEntities, useEntityTypes } from "@/features/entities/queries";
import type { SaveStatus } from "@/features/entities/use-autosave";
import { useAutosave } from "@/features/entities/use-autosave";
import { cn } from "@/lib/utils";
import { useDocument, useSaveDocument } from "./queries";
import { Mention } from "./mention";
import { SlashCommand } from "./slash-command";

interface LoreEditorProps {
  entityId: string;
  /** Reports save state so the entity page can show one combined indicator. */
  onStatusChange: (status: SaveStatus, error: string | null) => void;
}

/** Loads the document, then mounts the editor once (keyed by entity). */
export function LoreEditor({ entityId, onStatusChange }: LoreEditorProps) {
  const doc = useDocument(entityId);
  if (doc.isPending) return <p className="text-sm text-muted-foreground">Loading lore…</p>;
  if (doc.error) return <p className="text-sm text-destructive">{doc.error.message}</p>;
  return (
    <Editor
      key={entityId}
      entityId={entityId}
      initialContent={doc.data.content}
      onStatusChange={onStatusChange}
    />
  );
}

function Editor({
  entityId,
  initialContent,
  onStatusChange,
}: LoreEditorProps & { initialContent: string }) {
  const saveDocument = useSaveDocument();
  const [draft, setDraft] = useState({ content: initialContent, plainText: "" });

  // The Mention extension is created once, so it reads live data through refs.
  const entities = useEntities();
  const types = useEntityTypes();
  const createEntity = useCreateEntity();
  const live = useRef({ entities, types, createEntity });
  useEffect(() => {
    live.current = { entities, types, createEntity };
  });

  const editor = useEditor({
    extensions: [
      StarterKit.configure({ heading: { levels: [1, 2, 3] } }),
      TableKit.configure({ table: { resizable: false } }),
      Placeholder.configure({ placeholder: "Write the lore… type / for blocks" }),
      SlashCommand,
      // eslint-disable-next-line react-hooks/refs -- the ref is read lazily inside callbacks, never during render
      Mention.configure({
        getEntities: () => live.current.entities.data ?? [],
        getTypes: () => live.current.types.data ?? [],
        createEntity: (type, name) => live.current.createEntity.mutateAsync({ type, name }),
      }),
    ],
    content: parseContent(initialContent),
    editorProps: {
      attributes: {
        class:
          "prose prose-neutral dark:prose-invert max-w-none min-h-48 px-3 py-2 focus:outline-none",
      },
    },
    // Normalise the initial draft to what the editor would emit, so loading a
    // document never counts as a change.
    onCreate: ({ editor }) => {
      setDraft({ content: JSON.stringify(editor.getJSON()), plainText: editor.getText() });
    },
    onUpdate: ({ editor }) => {
      setDraft({ content: JSON.stringify(editor.getJSON()), plainText: editor.getText() });
    },
  });

  const { status, error } = useAutosave(draft, (value) =>
    saveDocument.mutateAsync({ entityId, ...value }),
  );
  useEffect(() => onStatusChange(status, error), [status, error, onStatusChange]);

  // Re-render the toolbar when marks/selection change.
  const state = useEditorState({
    editor,
    selector: ({ editor }) => ({
      bold: editor?.isActive("bold") ?? false,
      italic: editor?.isActive("italic") ?? false,
      link: editor?.isActive("link") ?? false,
      inTable: editor?.isActive("table") ?? false,
    }),
  });

  if (!editor) return null;

  function toggleLink() {
    if (editor!.isActive("link")) {
      editor!.chain().focus().unsetLink().run();
      return;
    }
    const href = window.prompt("Link URL");
    if (href) editor!.chain().focus().setLink({ href }).run();
  }

  return (
    <div className="rounded-md border">
      <div className="flex flex-wrap items-center gap-1 border-b p-1">
        <ToolbarButton
          active={state.bold}
          label="Bold"
          onClick={() => editor.chain().focus().toggleBold().run()}
        >
          <Bold />
        </ToolbarButton>
        <ToolbarButton
          active={state.italic}
          label="Italic"
          onClick={() => editor.chain().focus().toggleItalic().run()}
        >
          <Italic />
        </ToolbarButton>
        <ToolbarButton active={state.link} label="Link" onClick={toggleLink}>
          <LinkIcon />
        </ToolbarButton>
        {state.inTable && (
          <>
            <span className="mx-1 h-4 w-px bg-border" />
            <Table2 className="size-4 text-muted-foreground" />
            <Button
              variant="ghost"
              size="sm"
              onClick={() => editor.chain().focus().addRowAfter().run()}
            >
              + Row
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => editor.chain().focus().addColumnAfter().run()}
            >
              + Column
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => editor.chain().focus().deleteRow().run()}
            >
              − Row
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => editor.chain().focus().deleteColumn().run()}
            >
              − Column
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => editor.chain().focus().deleteTable().run()}
            >
              Delete table
            </Button>
          </>
        )}
      </div>
      <EditorContent editor={editor} />
    </div>
  );
}

function ToolbarButton({
  active,
  label,
  onClick,
  children,
}: {
  active: boolean;
  label: string;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <Button
      variant="ghost"
      size="icon-xs"
      aria-label={label}
      aria-pressed={active}
      onClick={onClick}
      className={cn(active && "bg-accent text-accent-foreground")}
    >
      {children}
    </Button>
  );
}

/** Stored content is JSON text; fall back to an empty doc if it is unreadable. */
function parseContent(content: string): object {
  try {
    return JSON.parse(content) as object;
  } catch {
    return { type: "doc", content: [] };
  }
}
