import { Prec, type Extension } from "@codemirror/state";
import { keymap, type Command } from "@codemirror/view";

/**
 * Ctrl+G belongs to the Git graph panel, not CodeMirror's findNext.
 * Returning true marks the chord as handled so searchKeymap never runs.
 */
export const swallowFindNext: Command = () => true;

export function editorKeymap(): Extension {
  return Prec.highest(
    keymap.of([
      { key: "Mod-g", run: swallowFindNext },
      { key: "Ctrl-g", run: swallowFindNext },
    ]),
  );
}
