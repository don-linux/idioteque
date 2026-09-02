import { describe, expect, it } from "vitest";
import {
  clampPanelSize,
  dragSize,
  MAX_PANEL_RATIO,
  MIN_TERMINAL_BOTTOM,
  MIN_TREE_WIDTH,
  panelMaximum,
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
