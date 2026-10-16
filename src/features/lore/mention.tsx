import { mergeAttributes, Node, type Editor, type Range } from "@tiptap/core";
import { ReactNodeViewRenderer, ReactRenderer } from "@tiptap/react";
import { PluginKey } from "@tiptap/pm/state";
import Suggestion, { type SuggestionProps } from "@tiptap/suggestion";
import type { Entity, EntitySummary, EntityTypeDef } from "@/lib/bindings";
import { buildMentionItems, type MentionItem } from "./mention-items";
import { MentionView } from "./mention-view";
import { SlashMenu, type SlashMenuHandle, type SlashMenuProps } from "./slash-menu";

export interface MentionOptions {
  /** Current entities, for the picker and for plain-text export. */
  getEntities: () => EntitySummary[];
  getTypes: () => EntityTypeDef[];
  /** Creates an entity from the picker's "Create …" option. */
  createEntity: (type: string, name: string) => Promise<Entity>;
}

/**
 * An inline, atomic node that stores only the entity **id**. The name is
 * looked up live, so renaming an entity updates every mention.
 */
export const Mention = Node.create<MentionOptions>({
  name: "mention",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,

  addOptions() {
    return { getEntities: () => [], getTypes: () => [], createEntity: () => Promise.reject() };
  },

  addAttributes() {
    return {
      id: {
        default: null,
        parseHTML: (el) => el.getAttribute("data-mention-id"),
        renderHTML: (attrs) => ({ "data-mention-id": attrs.id as string }),
      },
    };
  },

  parseHTML() {
    return [{ tag: "span[data-mention-id]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["span", mergeAttributes({ class: "mention" }, HTMLAttributes), 0];
  },

  /** What `editor.getText()` emits for a mention — the name, for search. */
  renderText({ node }) {
    const id = node.attrs.id as string;
    return this.options.getEntities().find((e) => e.id === id)?.name ?? "[missing entity]";
  },

  addNodeView() {
    return ReactNodeViewRenderer(MentionView);
  },

  addProseMirrorPlugins() {
    const options = this.options;
    return [
      Suggestion<MentionItem>({
        pluginKey: new PluginKey("mentionSuggestion"),
        // Entity names contain spaces, so keep the picker open across them.
        allowSpaces: true,
        editor: this.editor,
        char: "[[",
        allowedPrefixes: null,
        items: ({ query }) => buildMentionItems(options.getEntities(), options.getTypes(), query),
        command: ({ editor, range, props }) => {
          void insertMention(editor, range, props, options);
        },
        render: () => {
          let renderer: ReactRenderer<SlashMenuHandle, SlashMenuProps> | null = null;
          const toProps = (props: SuggestionProps<MentionItem>): SlashMenuProps => ({
            items: props.items,
            command: props.command,
            rect: props.clientRect?.() ?? null,
          });
          return {
            onStart: (props) => {
              renderer = new ReactRenderer(SlashMenu, {
                props: toProps(props),
                editor: props.editor,
              });
              document.body.appendChild(renderer.element);
            },
            onUpdate: (props) => renderer?.updateProps(toProps(props)),
            onKeyDown: ({ event }) => {
              if (event.key === "Escape") {
                renderer?.destroy();
                renderer = null;
                return true;
              }
              return renderer?.ref?.onKeyDown(event) ?? false;
            },
            onExit: () => {
              renderer?.element.remove();
              renderer?.destroy();
              renderer = null;
            },
          };
        },
      }),
    ];
  },
});

async function insertMention(
  editor: Editor,
  range: Range,
  item: MentionItem,
  options: MentionOptions,
) {
  // Remove the typed `[[query` first so the editor looks responsive even if
  // creating the entity takes a moment.
  editor.chain().focus().deleteRange(range).run();
  const id =
    item.kind === "entity" ? item.id : (await options.createEntity(item.type, item.name)).id;
  editor
    .chain()
    .focus()
    .insertContent([
      { type: "mention", attrs: { id } },
      { type: "text", text: " " },
    ])
    .run();
}
