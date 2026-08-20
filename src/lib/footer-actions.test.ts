import { describe, expect, it } from "vitest";
import {
  DEFAULT_FOOTER_ACTION_ORDER,
  moveFooterAction,
  normalizeFooterOrder,
} from "./footer-actions";

describe("normalizeFooterOrder", () => {
  it("uses the default when the saved list is missing or empty", () => {
    expect(normalizeFooterOrder(undefined)).toEqual(DEFAULT_FOOTER_ACTION_ORDER);
    expect(normalizeFooterOrder(null)).toEqual(DEFAULT_FOOTER_ACTION_ORDER);
    expect(normalizeFooterOrder([])).toEqual(DEFAULT_FOOTER_ACTION_ORDER);
  });

  it("keeps a valid custom order", () => {
    expect(normalizeFooterOrder(["terminal", "home", "settings", "folder"])).toEqual([
      "terminal",
      "home",
      "settings",
      "folder",
    ]);
  });

  it("drops unknown ids and duplicates", () => {
    expect(
      normalizeFooterOrder(["home", "ghost", "home", "terminal", "folder", "settings"]),
    ).toEqual(["home", "terminal", "folder", "settings"]);
  });

  it("appends new default ids that are missing from the saved order", () => {
    expect(normalizeFooterOrder(["terminal", "home"])).toEqual([
      "terminal",
      "home",
      "folder",
      "settings",
    ]);
  });

  it("ignores future ids that the app does not know yet", () => {
    expect(normalizeFooterOrder(["search", "home", "folder", "settings", "terminal"])).toEqual(
      DEFAULT_FOOTER_ACTION_ORDER,
    );
  });
});

describe("moveFooterAction", () => {
  it("moves an item to another index", () => {
    expect(moveFooterAction(["home", "folder", "settings", "terminal"], 0, 2)).toEqual([
      "folder",
      "settings",
      "home",
      "terminal",
    ]);
  });

  it("leaves the order alone when the index is out of range", () => {
    const order = ["home", "folder", "settings", "terminal"] as const;
    expect(moveFooterAction(order, -1, 1)).toEqual([...order]);
    expect(moveFooterAction(order, 0, 9)).toEqual([...order]);
  });
});
