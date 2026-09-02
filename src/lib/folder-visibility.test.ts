import { describe, expect, it } from "vitest";
import {
  FOLDER_VISIBILITY_LABEL,
  FOLDER_VISIBILITY_TOAST,
  folderName,
  needsFolderPicker,
  pathKeysMatch,
  shouldShowFolderVisibilityToast,
  visibilityFor,
} from "./folder-visibility";

describe("pathKeysMatch", () => {
  it("treats a trailing slash as the same folder", () => {
    expect(pathKeysMatch("/tmp/idioteque", "/tmp/idioteque/")).toBe(true);
    expect(pathKeysMatch("/tmp/idioteque", "/tmp/other")).toBe(false);
  });
});

describe("folderName", () => {
  it("uses the last path segment", () => {
    expect(folderName("/home/user/idioteque")).toBe("idioteque");
    expect(folderName("/home/user/.cursor/")).toBe(".cursor");
  });
});

describe("visibilityFor", () => {
  const views = [
    { path: "/tmp/idioteque", visibleFolders: [".cursor", "src"] },
    { path: "/tmp/docs", visibleFolders: [] },
  ];

  it("returns the saved folders for a matching path", () => {
    expect(visibilityFor(views, "/tmp/idioteque/")).toEqual([".cursor", "src"]);
    expect(visibilityFor(views, "/tmp/docs")).toEqual([]);
  });

  it("is undefined when that folder was never configured", () => {
    expect(visibilityFor(views, "/tmp/other")).toBeUndefined();
    expect(visibilityFor([], "/tmp/idioteque")).toBeUndefined();
  });
});

describe("needsFolderPicker", () => {
  it("asks only when there are subfolders and no saved view", () => {
    expect(needsFolderPicker(["src", "docs"], undefined)).toBe(true);
    expect(needsFolderPicker(["src"], [])).toBe(false);
    expect(needsFolderPicker(["src"], ["src"])).toBe(false);
    expect(needsFolderPicker([], undefined)).toBe(false);
  });
});

describe("shouldShowFolderVisibilityToast", () => {
  it("hints only for unconfigured folders that have subdirs", () => {
    expect(shouldShowFolderVisibilityToast(true, undefined)).toBe(true);
    expect(shouldShowFolderVisibilityToast(true, [])).toBe(false);
    expect(shouldShowFolderVisibilityToast(true, ["src"])).toBe(false);
    expect(shouldShowFolderVisibilityToast(false, undefined)).toBe(false);
  });
});

describe("copy", () => {
  it("names the control Carpetas visibles", () => {
    expect(FOLDER_VISIBILITY_LABEL).toBe("Carpetas visibles");
    expect(FOLDER_VISIBILITY_TOAST).toContain("Carpetas visibles");
    expect(FOLDER_VISIBILITY_TOAST).toContain("carpeta con +");
  });
});
