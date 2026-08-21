import { describe, expect, it } from "vitest";
import { editorHighlight } from "./editor-theme";

describe("editorHighlight", () => {
  it("returns a CodeMirror extension", () => {
    expect(editorHighlight()).toBeTruthy();
  });
});
