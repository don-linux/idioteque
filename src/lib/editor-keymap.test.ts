import { findNext } from "@codemirror/search";
import { EditorState, type Extension } from "@codemirror/state";
import { keymap, type KeyBinding } from "@codemirror/view";
import { basicSetup } from "codemirror";
import { describe, expect, it } from "vitest";
import { editorKeymap, swallowFindNext } from "./editor-keymap";

function bindingsFor(key: string, extensions: Extension[]): KeyBinding[] {
  const state = EditorState.create({ doc: "hola", extensions });
  return state.facet(keymap).flat().filter((binding) => binding.key === key);
}

describe("swallowFindNext", () => {
  it("consumes the chord so CodeMirror treats it as handled", () => {
    expect(swallowFindNext({} as never)).toBe(true);
    expect(swallowFindNext).not.toBe(findNext);
  });
});

describe("editorKeymap", () => {
  it("binds Mod-g and Ctrl-g ahead of findNext from basicSetup", () => {
    const onlySearch = bindingsFor("Mod-g", [basicSetup]);
    expect(onlySearch.some((binding) => binding.run === findNext)).toBe(true);
    expect(onlySearch[0]?.run).toBe(findNext);

    const withOurs = bindingsFor("Mod-g", [basicSetup, editorKeymap()]);
    expect(withOurs[0]?.run).toBe(swallowFindNext);
    expect(withOurs.some((binding) => binding.run === findNext)).toBe(true);
    expect(withOurs.findIndex((binding) => binding.run === swallowFindNext)).toBeLessThan(
      withOurs.findIndex((binding) => binding.run === findNext),
    );

    const ctrl = bindingsFor("Ctrl-g", [basicSetup, editorKeymap()]);
    expect(ctrl[0]?.run).toBe(swallowFindNext);
  });

  it("does not steal find previous or go-to-line", () => {
    const state = EditorState.create({
      doc: "hola",
      extensions: [basicSetup, editorKeymap()],
    });
    const keys = state.facet(keymap).flat().map((binding) => binding.key);

    expect(keys).toContain("Mod-g");
    expect(keys).toContain("Mod-Alt-g");
    expect(keys).toContain("Mod-f");
  });
});
