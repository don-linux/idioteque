import { describe, expect, it } from "vitest";
import {
  CURRENT_BRANCH_LANE,
  branchLaneColors,
  branchPriority,
  commitLaneColors,
} from "./git-branch-colors";

/** Cuántos secundarios trae el tema en estas pruebas. Lo decide el tema real. */
const SECONDARIES = 7;

function refs(...names: string[]) {
  return names.map((name) => ({ name }));
}

describe("branchLaneColors", () => {
  it("pinta la rama actual con el acento", () => {
    const colors = branchLaneColors(refs("dev", "main", "spike"), "main", SECONDARIES);

    expect(colors.get("main")).toBe(CURRENT_BRANCH_LANE);
    expect(colors.get("dev")).not.toBe(CURRENT_BRANCH_LANE);
    expect(colors.get("spike")).not.toBe(CURRENT_BRANCH_LANE);
  });

  it("no repite color entre ramas vecinas de la lista", () => {
    const names = Array.from({ length: SECONDARIES + 3 }, (_, index) => `r${index}`);
    const colors = branchLaneColors(refs(...names), null, SECONDARIES);

    for (let index = 1; index < names.length; index += 1) {
      expect(colors.get(names[index])).not.toBe(colors.get(names[index - 1]));
    }
  });

  it("deja la paleta completa de distancia antes de repetir un color", () => {
    const names = Array.from({ length: SECONDARIES * 2 }, (_, index) => `r${index}`);
    const colors = branchLaneColors(refs(...names), null, SECONDARIES);

    for (const [index, name] of names.entries()) {
      const repeated = names.findIndex(
        (other, position) => position > index && colors.get(other) === colors.get(name),
      );
      if (repeated < 0) continue;
      expect(repeated - index).toBe(SECONDARIES);
    }
  });

  it("acorta la distancia de repetición cuando el tema trae paleta corta", () => {
    // Un tema de cuatro secundarios repite a la quinta rama, no a la octava.
    const names = Array.from({ length: 9 }, (_, index) => `r${index}`);
    const colors = branchLaneColors(refs(...names), null, 4);

    expect(names.map((name) => colors.get(name))).toEqual([1, 2, 3, 4, 1, 2, 3, 4, 1]);
  });

  it("usa solo los carriles secundarios cuando no hay rama actual", () => {
    const names = Array.from({ length: SECONDARIES * 2 }, (_, index) => `r${index}`);
    const colors = branchLaneColors(refs(...names), null, SECONDARIES);

    for (const color of colors.values()) {
      expect(color).toBeGreaterThanOrEqual(1);
      expect(color).toBeLessThanOrEqual(SECONDARIES);
    }
  });

  it("no gasta un color en la rama actual", () => {
    const withCurrent = branchLaneColors(refs("main", "a", "b"), "main", SECONDARIES);
    const withoutCurrent = branchLaneColors(refs("a", "b"), null, SECONDARIES);

    expect(withCurrent.get("a")).toBe(withoutCurrent.get("a"));
    expect(withCurrent.get("b")).toBe(withoutCurrent.get("b"));
  });

  it("ignora nombres repetidos en la lista", () => {
    const colors = branchLaneColors(refs("a", "a", "b"), null, SECONDARIES);

    expect(colors.size).toBe(2);
    expect(colors.get("b")).toBe(2);
  });
});

describe("branchPriority", () => {
  it("pone la actual, después las comparadas y al final el resto", () => {
    const order = branchPriority(refs("alpha", "beta", "gamma", "main"), "main", ["gamma"]);

    expect(order).toEqual(["main", "gamma", "alpha", "beta"]);
  });

  it("descarta nombres que no están en el repositorio", () => {
    const order = branchPriority(refs("main"), "ghost", ["fantasma"]);

    expect(order).toEqual(["main"]);
  });

  it("sirve con HEAD separado", () => {
    expect(branchPriority(refs("main", "dev"), null, ["dev"])).toEqual(["dev", "main"]);
  });
});

describe("commitLaneColors", () => {
  // main:  c ── b ── a
  // feat:  d ──┘
  const commits = [
    { hash: "c", parents: ["b"], refs: ["main"] },
    { hash: "d", parents: ["b"], refs: ["feat"] },
    { hash: "b", parents: ["a"] },
    { hash: "a", parents: [] },
  ];
  const branches = refs("feat", "main");

  it("da el tronco de la rama actual al acento", () => {
    const colors = commitLaneColors(
      commits,
      branchLaneColors(branches, "main", SECONDARIES),
      branchPriority(branches, "main", ["feat"]),
      "c",
    );

    expect(colors.get("c")).toBe(CURRENT_BRANCH_LANE);
    expect(colors.get("b")).toBe(CURRENT_BRANCH_LANE);
    expect(colors.get("a")).toBe(CURRENT_BRANCH_LANE);
  });

  it("deja los commits propios de otra rama con el color de esa rama", () => {
    const branchColors = branchLaneColors(branches, "main", SECONDARIES);
    const colors = commitLaneColors(
      commits,
      branchColors,
      branchPriority(branches, "main", ["feat"]),
      "c",
    );

    expect(colors.get("d")).toBe(branchColors.get("feat"));
    expect(colors.get("d")).not.toBe(CURRENT_BRANCH_LANE);
  });

  it("cede la historia compartida a la rama actual, no a la comparada", () => {
    // Con feat primero en la prioridad, 'b' sería de feat.
    const branchColors = branchLaneColors(branches, "main", SECONDARIES);
    const colors = commitLaneColors(commits, branchColors, ["feat", "main"], null);

    expect(colors.get("b")).toBe(branchColors.get("feat"));
    expect(colors.get("c")).toBe(CURRENT_BRANCH_LANE);
  });

  it("pinta el tronco de un HEAD separado con el acento", () => {
    const detached = [
      { hash: "z", parents: ["b"] },
      ...commits,
    ];
    const branchColors = branchLaneColors(branches, null, SECONDARIES);
    const colors = commitLaneColors(detached, branchColors, branchPriority(branches, null, []), "z");

    expect(colors.get("z")).toBe(CURRENT_BRANCH_LANE);
    expect(colors.get("b")).toBe(CURRENT_BRANCH_LANE);
    expect(colors.get("c")).toBe(branchColors.get("main"));
  });

  it("no inventa color para lo que no cuelga de ninguna punta listada", () => {
    const colors = commitLaneColors(
      [
        { hash: "b", parents: ["a"], refs: ["main"] },
        { hash: "a", parents: [] },
        { hash: "huerfano", parents: [] },
      ],
      branchLaneColors(refs("main"), "main", SECONDARIES),
      ["main"],
      "b",
    );

    expect(colors.has("huerfano")).toBe(false);
  });

  it("ignora decoraciones que no son ramas locales del selector", () => {
    const branchColors = branchLaneColors(refs("main"), "main", SECONDARIES);
    const colors = commitLaneColors(
      [
        { hash: "b", parents: ["a"], refs: ["HEAD", "origin/main", "main"] },
        { hash: "a", parents: [] },
      ],
      branchColors,
      ["main"],
      null,
    );

    expect(colors.get("b")).toBe(CURRENT_BRANCH_LANE);
    expect(colors.get("a")).toBe(CURRENT_BRANCH_LANE);
  });

  it("corta el recorrido en el primer padre, no sigue la rama fusionada", () => {
    // m es un merge: su primer padre es c, el segundo es d.
    const merged = [
      { hash: "m", parents: ["c", "d"], refs: ["main"] },
      { hash: "c", parents: ["a"] },
      { hash: "d", parents: ["a"], refs: ["feat"] },
      { hash: "a", parents: [] },
    ];
    const branchColors = branchLaneColors(refs("feat", "main"), "main", SECONDARIES);
    const colors = commitLaneColors(merged, branchColors, ["main", "feat"], "m");

    expect(colors.get("c")).toBe(CURRENT_BRANCH_LANE);
    expect(colors.get("d")).toBe(branchColors.get("feat"));
  });
});
