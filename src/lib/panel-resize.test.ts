import { describe, expect, it } from "vitest";
import {
  clampPanelSize,
  dragSize,
  FOOTER_HEIGHT,
  MAX_PANEL_RATIO,
  MIN_EDITOR_HEIGHT,
  MIN_EDITOR_WIDTH,
  MIN_TERMINAL_BOTTOM,
  MIN_TERMINAL_RIGHT,
  MIN_TREE_WIDTH,
  panelMaximum,
  SASH_SIZE,
  terminalBottomReserve,
  terminalRightReserve,
  treeReserve,
} from "./panel-resize";

describe("panelMaximum", () => {
  it("caps a panel at 80% of the viewport", () => {
    expect(panelMaximum(MIN_TREE_WIDTH, 1000)).toBe(800);
    expect(MAX_PANEL_RATIO).toBe(0.8);
  });

  it("never returns less than the minimum", () => {
    expect(panelMaximum(MIN_TREE_WIDTH, 100)).toBe(MIN_TREE_WIDTH);
    expect(panelMaximum(MIN_TREE_WIDTH, 0)).toBe(MIN_TREE_WIDTH);
    expect(panelMaximum(MIN_TREE_WIDTH, Number.NaN)).toBe(MIN_TREE_WIDTH);
  });

  it("gives up room the other regions reserved", () => {
    expect(panelMaximum(MIN_TREE_WIDTH, 1000, 400)).toBe(600);
    expect(panelMaximum(MIN_TREE_WIDTH, 1000, 900)).toBe(MIN_TREE_WIDTH);
  });

  it("still honours the 80% share when little is reserved", () => {
    expect(panelMaximum(MIN_TREE_WIDTH, 1000, 100)).toBe(800);
  });

  it("ignores a negative reserve", () => {
    expect(panelMaximum(MIN_TREE_WIDTH, 1000, -500)).toBe(800);
  });
});

describe("clampPanelSize", () => {
  it("keeps a size that already fits, rounded", () => {
    expect(clampPanelSize(260, MIN_TREE_WIDTH, 1400)).toBe(260);
    expect(clampPanelSize(260.4, MIN_TREE_WIDTH, 1400)).toBe(260);
    expect(clampPanelSize(260.6, MIN_TREE_WIDTH, 1400)).toBe(261);
  });

  it("clamps to the minimum and to the viewport share", () => {
    expect(clampPanelSize(10, MIN_TREE_WIDTH, 1400)).toBe(MIN_TREE_WIDTH);
    expect(clampPanelSize(-500, MIN_TREE_WIDTH, 1400)).toBe(MIN_TREE_WIDTH);
    expect(clampPanelSize(5000, MIN_TREE_WIDTH, 1000)).toBe(800);
  });

  it("prefers the minimum when the window is tiny", () => {
    expect(clampPanelSize(300, MIN_TREE_WIDTH, 100)).toBe(MIN_TREE_WIDTH);
    expect(clampPanelSize(50, MIN_TERMINAL_BOTTOM, 120)).toBe(MIN_TERMINAL_BOTTOM);
  });

  it("survives garbage input", () => {
    expect(clampPanelSize(Number.NaN, MIN_TREE_WIDTH, 1400)).toBe(MIN_TREE_WIDTH);
    expect(clampPanelSize(Number.POSITIVE_INFINITY, MIN_TREE_WIDTH, 1400)).toBe(MIN_TREE_WIDTH);
  });

  it("stops where the reserved room begins", () => {
    expect(clampPanelSize(900, MIN_TREE_WIDTH, 1200, 700)).toBe(500);
  });
});

describe("reserves", () => {
  it("keeps the editor and a right docked terminal out of the tree's budget", () => {
    expect(treeReserve(0)).toBe(MIN_EDITOR_WIDTH + SASH_SIZE);
    expect(treeReserve(380)).toBe(MIN_EDITOR_WIDTH + SASH_SIZE + 380);
    expect(treeReserve(-10)).toBe(MIN_EDITOR_WIDTH + SASH_SIZE);
  });

  it("keeps the editor and the tree out of the terminal's horizontal budget", () => {
    expect(terminalRightReserve(0)).toBe(MIN_EDITOR_WIDTH);
    expect(terminalRightReserve(260)).toBe(MIN_EDITOR_WIDTH + 260 + SASH_SIZE);
  });

  it("keeps the editor and the footer out of the terminal's vertical budget", () => {
    expect(terminalBottomReserve()).toBe(MIN_EDITOR_HEIGHT + FOOTER_HEIGHT);
  });

  it("leaves the editor exactly its minimum when both panels are maxed out", () => {
    const viewport = 1200;
    const terminalRight = 675;

    const tree = clampPanelSize(900, MIN_TREE_WIDTH, viewport, treeReserve(terminalRight));
    const rightDock = clampPanelSize(
      terminalRight,
      MIN_TERMINAL_RIGHT,
      viewport,
      terminalRightReserve(tree),
    );

    expect(tree).toBe(201);
    expect(rightDock).toBe(terminalRight);
    expect(viewport - tree - SASH_SIZE - rightDock).toBe(MIN_EDITOR_WIDTH);
  });

  it("falls back to both minimums in a window too small for either", () => {
    const viewport = 500;

    const tree = clampPanelSize(900, MIN_TREE_WIDTH, viewport, treeReserve(675));
    const rightDock = clampPanelSize(
      675,
      MIN_TERMINAL_RIGHT,
      viewport,
      terminalRightReserve(tree),
    );

    expect(tree).toBe(MIN_TREE_WIDTH);
    expect(rightDock).toBe(MIN_TERMINAL_RIGHT);
  });

  it("leaves the editor room under a bottom docked terminal", () => {
    const viewport = 570;
    const size = clampPanelSize(500, MIN_TERMINAL_BOTTOM, viewport, terminalBottomReserve());

    expect(size).toBe(viewport - MIN_EDITOR_HEIGHT - FOOTER_HEIGHT);
    expect(size).toBeLessThan(Math.floor(viewport * MAX_PANEL_RATIO));
  });
});

describe("dragSize", () => {
  it("grows forward when the panel sits before its handle", () => {
    const origin = { start: 300, startSize: 260, grow: "forward" as const };

    expect(dragSize(origin, 340)).toBe(300);
    expect(dragSize(origin, 260)).toBe(220);
    expect(dragSize(origin, 300)).toBe(260);
  });

  it("grows backward when the panel sits after its handle", () => {
    const origin = { start: 700, startSize: 280, grow: "backward" as const };

    expect(dragSize(origin, 650)).toBe(330);
    expect(dragSize(origin, 750)).toBe(230);
    expect(dragSize(origin, 700)).toBe(280);
  });

  it("composes with the clamp for a full drag", () => {
    const origin = { start: 300, startSize: 260, grow: "forward" as const };
    const size = clampPanelSize(dragSize(origin, 0), MIN_TREE_WIDTH, 1400);

    expect(size).toBe(MIN_TREE_WIDTH);
  });
});
