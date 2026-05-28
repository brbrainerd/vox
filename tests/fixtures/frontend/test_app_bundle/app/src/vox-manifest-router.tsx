import {
  RouterProvider,
  Outlet,
  createRootRoute,
  createRouter,
} from "@tanstack/react-router";
import { voxRoutes } from "./generated/routes.manifest";
import { VoxQueryProvider } from "./generated/vox-tanstack-query";
import { buildChildRoutes } from "./vox-manifest-route-adapter";

const rootRoute = createRootRoute({
  component: () => (
    <VoxQueryProvider>
      <Outlet />
    </VoxQueryProvider>
  ),
});

const routeTree = rootRoute.addChildren(buildChildRoutes(rootRoute, voxRoutes));

const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}

export function VoxManifestApp() {
  return <RouterProvider router={router} />;
}
