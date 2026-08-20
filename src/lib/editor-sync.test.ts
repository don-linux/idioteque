import { history, redo, undo, undoDepth } from "@codemirror/commands";
import {
  EditorState,
  Transaction,
  type StateCommand,
  type TransactionSpec,
} from "@codemirror/state";
import { describe, expect, it } from "vitest";
import { externalDocumentSpec } from "./editor-sync";

const FILE = "# hello\n\nloaded from disk";

function createEditor(doc = "") {
  return EditorState.create({
    doc,
    extensions: [history({ newGroupDelay: 0 })],
  });
}

function apply(state: EditorState, spec: TransactionSpec): EditorState {
  return state.update(spec).state;
}

function load(state: EditorState, next: string): EditorState {
  return apply(state, externalDocumentSpec(state, next));
}

function run(state: EditorState, command: StateCommand): { state: EditorState; applied: boolean } {
  let next = state;
  const applied = command({
    state,
    dispatch(tr) {
      next = tr.state;
    },
  });
  return { state: next, applied };
}

let userClock = 1;

function userEdit(
  state: EditorState,
  changes: { from: number; to?: number; insert: string },
): EditorState {
  userClock += 10_000;
  return apply(state, {
    changes,
    annotations: Transaction.time.of(userClock),
  });
}

function text(state: EditorState): string {
  return state.doc.toString();
}

describe("externalDocumentSpec", () => {
  it("does not record a programmatic load from empty, so extra undo keeps the file", () => {
    let state = createEditor("");
    expect(undoDepth(state)).toBe(0);

    state = load(state, FILE);

    expect(text(state)).toBe(FILE);
    expect(undoDepth(state)).toBe(0);

    const extra = run(state, undo);
    expect(extra.applied).toBe(false);
    expect(text(extra.state)).toBe(FILE);
    expect(text(extra.state)).not.toBe("");
    expect(undoDepth(extra.state)).toBe(0);
  });

  it("undoes user edits in order and does not clear the loaded file after history is exhausted", () => {
    let state = load(createEditor(""), FILE);
    expect(undoDepth(state)).toBe(0);

    state = userEdit(state, { from: FILE.length, insert: "\nfirst" });
    const afterFirst = text(state);
    state = userEdit(state, { from: afterFirst.length, insert: "\nsecond" });

    expect(text(state)).toBe(`${FILE}\nfirst\nsecond`);
    expect(undoDepth(state)).toBe(2);

    const undoSecond = run(state, undo);
    expect(undoSecond.applied).toBe(true);
    expect(text(undoSecond.state)).toBe(afterFirst);
    expect(undoDepth(undoSecond.state)).toBe(1);

    const undoFirst = run(undoSecond.state, undo);
    expect(undoFirst.applied).toBe(true);
    expect(text(undoFirst.state)).toBe(FILE);
    expect(undoDepth(undoFirst.state)).toBe(0);

    const extra = run(undoFirst.state, undo);
    expect(extra.applied).toBe(false);
    expect(text(extra.state)).toBe(FILE);
    expect(text(extra.state)).not.toBe("");
    expect(undoDepth(extra.state)).toBe(0);
  });

  it("redoes a user edit after undo", () => {
    let state = load(createEditor(""), FILE);
    const edited = `${FILE}\nedit`;
    state = userEdit(state, { from: FILE.length, insert: "\nedit" });

    const undone = run(state, undo);
    expect(undone.applied).toBe(true);
    expect(text(undone.state)).toBe(FILE);

    const redone = run(undone.state, redo);
    expect(redone.applied).toBe(true);
    expect(text(redone.state)).toBe(edited);
    expect(undoDepth(redone.state)).toBe(1);
  });

  it("does not invent or delete content when the file is genuinely empty", () => {
    let state = load(createEditor(""), "");
    expect(text(state)).toBe("");
    expect(undoDepth(state)).toBe(0);

    const extraFromEmpty = run(state, undo);
    expect(extraFromEmpty.applied).toBe(false);
    expect(text(extraFromEmpty.state)).toBe("");
    expect(undoDepth(extraFromEmpty.state)).toBe(0);

    state = load(createEditor("stale leftover"), "");
    expect(text(state)).toBe("");
    expect(undoDepth(state)).toBe(0);

    const extraFromStale = run(state, undo);
    expect(extraFromStale.applied).toBe(false);
    expect(text(extraFromStale.state)).toBe("");
    expect(text(extraFromStale.state)).not.toBe("stale leftover");
    expect(undoDepth(extraFromStale.state)).toBe(0);
  });

  it("treats several extra undos in a row as no-ops", () => {
    let state = load(createEditor(""), FILE);
    expect(undoDepth(state)).toBe(0);

    for (let i = 0; i < 5; i++) {
      const extra = run(state, undo);
      expect(extra.applied).toBe(false);
      expect(text(extra.state)).toBe(FILE);
      expect(undoDepth(extra.state)).toBe(0);
      state = extra.state;
    }
  });

  it("undoes a user clear of a loaded file and does not clear again on extra undo", () => {
    let state = load(createEditor(""), FILE);
    expect(undoDepth(state)).toBe(0);

    state = userEdit(state, { from: 0, to: FILE.length, insert: "" });
    expect(text(state)).toBe("");
    expect(undoDepth(state)).toBe(1);

    const restored = run(state, undo);
    expect(restored.applied).toBe(true);
    expect(text(restored.state)).toBe(FILE);
    expect(undoDepth(restored.state)).toBe(0);

    const extra = run(restored.state, undo);
    expect(extra.applied).toBe(false);
    expect(text(extra.state)).toBe(FILE);
    expect(text(extra.state)).not.toBe("");
    expect(undoDepth(extra.state)).toBe(0);
  });
});
