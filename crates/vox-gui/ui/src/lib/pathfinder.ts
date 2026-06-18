export interface Coord {
  x: number;
  y: number;
}

interface PathNode {
  coord: Coord;
  g: number;
  f: number;
  parent: PathNode | null;
}

function getNeighbors(coord: Coord, width: number, height: number): Coord[] {
  const directions = [
    { x: 0, y: -1 },
    { x: 0, y: 1 },
    { x: -1, y: 0 },
    { x: 1, y: 0 },
  ];
  const neighbors: Coord[] = [];
  for (const dir of directions) {
    const nx = coord.x + dir.x;
    const ny = coord.y + dir.y;
    if (nx >= 0 && nx < width && ny >= 0 && ny < height) {
      neighbors.push({ x: nx, y: ny });
    }
  }
  return neighbors;
}

export function findPath(
  start: Coord,
  end: Coord,
  obstacles: Set<string>,
  width: number = 24,
  height: number = 24
): Coord[] {
  const openList: PathNode[] = [];
  const closedList = new Set<string>();

  const startNode: PathNode = {
    coord: start,
    g: 0,
    f: Math.abs(start.x - end.x) + Math.abs(start.y - end.y),
    parent: null,
  };

  openList.push(startNode);

  while (openList.length > 0) {
    // Sort by f value
    openList.sort((a, b) => a.f - b.f);
    const current = openList.shift()!;
    const curKey = `${current.coord.x},${current.coord.y}`;
    closedList.add(curKey);

    if (current.coord.x === end.x && current.coord.y === end.y) {
      const path: Coord[] = [];
      let curr: PathNode | null = current;
      while (curr !== null) {
        path.unshift(curr.coord);
        curr = curr.parent;
      }
      return path;
    }

    const neighbors = getNeighbors(current.coord, width, height);
    for (const neighbor of neighbors) {
      const nKey = `${neighbor.x},${neighbor.y}`;
      if (closedList.has(nKey) || obstacles.has(nKey)) continue;

      const gScore = current.g + 1;
      const hScore = Math.abs(neighbor.x - end.x) + Math.abs(neighbor.y - end.y);
      const fScore = gScore + hScore;

      const existingOpen = openList.find(o => o.coord.x === neighbor.x && o.coord.y === neighbor.y);
      if (existingOpen) {
        if (gScore < existingOpen.g) {
          existingOpen.g = gScore;
          existingOpen.f = fScore;
          existingOpen.parent = current;
        }
      } else {
        openList.push({
          coord: neighbor,
          g: gScore,
          f: fScore,
          parent: current,
        });
      }
    }
  }

  return []; // No path found
}
