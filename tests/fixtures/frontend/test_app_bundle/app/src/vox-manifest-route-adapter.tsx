import { createRoute } from "@tanstack/react-router";
import type { VoxRoute } from "./generated/routes.manifest";

/** TanStack uses `$param` segments; Vox manifest uses `:param`. */
function voxPathToChildPath(path: string): string {
  const p = path.trim();
  if (p === "/" || p === "") return "/";
  const rest = p.startsWith("/") ? p.slice(1) : p;
  const segs = rest.split("/").filter(Boolean);
  const mapped = segs.map((s) =>
    s.startsWith(":") ? `$${s.slice(1).replace(/\?$/, "")}` : s,
  );
  return mapped.join("/");
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function buildChildRoutes(parent: any, nodes: VoxRoute[]): any[] {
  return nodes.map((r) => {
    const path = r.path.trim() === "/" ? "/" : voxPathToChildPath(r.path);
    const route = createRoute({
      getParentRoute: () => parent,
      path,
      component: r.component,
      loader: r.loader,
      pendingComponent: r.pendingComponent,
      errorComponent: r.errorComponent,
      ...(r.index ? { index: true } : {}),
    } as never);
    if (r.children?.length) {
      return route.addChildren(buildChildRoutes(route, r.children));
    }
    return route;
  });
}
