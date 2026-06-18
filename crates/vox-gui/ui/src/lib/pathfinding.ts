export interface PathNode {
  x: number;
  y: number;
}

interface OpenNode extends PathNode {
  g: number;
  f: number;
  parent?: OpenNode;
}

export function findPath(
  start: PathNode,
  target: PathNode,
  solidBlocks: Set<string>,
  width: number,
  height: number
): PathNode[] {
  const openList: OpenNode[] = [{ ...start, g: 0, f: 0 }];
  const closedList = new Set<string>();

  const getNeighbors = (node: PathNode): PathNode[] => {
    const directions = [
      { x: 0, y: -1 }, { x: 0, y: 1 },
      { x: -1, y: 0 }, { x: 1, y: 0 }
    ];
    return directions
      .map(d => ({ x: node.x + d.x, y: node.y + d.y }))
      .filter(n => n.x >= 0 && n.x < width && n.y >= 0 && n.y < height)
      .filter(n => !solidBlocks.has(`${n.x},${n.y}`));
  };

  while (openList.length > 0) {
    // Sort open list by f cost
    openList.sort((a, b) => a.f - b.f);
    const current = openList.shift()!;
    
    const currentKey = `${current.x},${current.y}`;
    closedList.add(currentKey);

    if (current.x === target.x && current.y === target.y) {
      const path: PathNode[] = [];
      let temp: OpenNode | undefined = current;
      while (temp) {
        path.unshift({ x: temp.x, y: temp.y });
        temp = temp.parent;
      }
      return path;
    }

    const neighbors = getNeighbors(current);
    for (const neighbor of neighbors) {
      const neighborKey = `${neighbor.x},${neighbor.y}`;
      if (closedList.has(neighborKey)) continue;

      const gScore = current.g + 1;
      let existing = openList.find(n => n.x === neighbor.x && n.y === neighbor.y);

      if (!existing) {
        const h = Math.abs(neighbor.x - target.x) + Math.abs(neighbor.y - target.y);
        const nextNode: OpenNode = {
          ...neighbor,
          g: gScore,
          f: gScore + h,
          parent: current
        };
        openList.push(nextNode);
      } else if (gScore < existing.g) {
        existing.g = gScore;
        existing.f = gScore + (existing.f - existing.g);
        existing.parent = current;
      }
    }
  }

  return [start, target]; // Fallback to straight line if blocked
}
