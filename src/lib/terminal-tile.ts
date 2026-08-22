export interface TileDimensions {
  cols: number;
  rows: number;
}

export interface TilePlan<T> {
  cols: number;
  rows: number;
  rowsOfIds: T[][];
}

export function tileDimensions(n: number, width: number, height: number): TileDimensions {
  if (n <= 0) return { cols: 0, rows: 0 };
  if (n === 1) return { cols: 1, rows: 1 };

  const wide = width >= height;
  let rows = 1;
  let cols = 1;

  while (rows * cols < n) {
    if (wide) {
      cols += 1;
      if (rows * cols < n) rows += 1;
    } else {
      rows += 1;
      if (rows * cols < n) cols += 1;
    }
  }

  return { cols, rows };
}

export function tileRows<T>(ids: readonly T[], cols: number): T[][] {
  if (cols <= 0) return [];

  const rows: T[][] = [];
  for (let index = 0; index < ids.length; index += cols) {
    rows.push(ids.slice(index, index + cols));
  }
  return rows;
}

export function tilePlan<T>(ids: readonly T[], width: number, height: number): TilePlan<T> {
  const { cols, rows } = tileDimensions(ids.length, width, height);
  return { cols, rows, rowsOfIds: tileRows(ids, cols) };
}
