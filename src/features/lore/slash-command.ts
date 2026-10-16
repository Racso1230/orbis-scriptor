import { Extension, type Editor, type Range } from "@tiptap/core";
import { ReactRenderer } from "@tiptap/react";
import { PluginKey } from "@tiptap/pm/state";
import Suggestion, { type SuggestionProps } from "@tiptap/suggestion";
import { filterSlashItems, type SlashItem } from "./slash-items";
import { SlashMenu, type SlashMenuHandle, type SlashMenuProps } from "./slash-menu";

/**
 * Typing `/` at the start of a line opens a menu of block types.
 * Built on TipTap's Suggestion utility; the menu itself is a React component
 * mounted through ReactRenderer.
 */
export const SlashCommand = Extension.create({
  name: "slashCommand",

  addProseMirrorPlugins() {
    return [
      Suggestion<SlashItem>({
        // Each Suggestion instance needs its own key; Mention has one too.
        pluginKey: new PluginKey("slashCommand"),
        editor: this.editor,
        char: "/",
        startOfLine: false,
        items: ({ query }) => filterSlashItems(query),
        command: ({ editor, range, props }: { editor: Editor; range: Range; props: SlashItem }) =>
          props.command(editor, range),
        render: () => {
          let renderer: ReactRenderer<SlashMenuHandle, SlashMenuProps> | null = null;

          const toProps = (props: SuggestionProps<SlashItem>): SlashMenuProps => ({
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
