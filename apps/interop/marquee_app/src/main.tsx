import './vox-globals.d.ts'
// Inject global type shims for Vox built-ins (Item, list<T>, Result<T>, ItemSchema)
import { ItemSchema } from './vox-types'
import { z } from 'zod'

// Make ItemSchema globally accessible (referenced by vox-client.ts without import)
Object.assign(globalThis, { ItemSchema, z })

import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { createRouter, RouterProvider, createRootRoute, createRoute } from '@tanstack/react-router'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { Dashboard } from '../dist/Dashboard'
import { ItemDetail } from '../dist/ItemDetail'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: { staleTime: 5_000, retry: 1 },
  },
})

// Build TanStack Router v1 route tree from Vox-generated route manifest
const rootRoute = createRootRoute()

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/',
  component: Dashboard,
})

const itemDetailRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: '/items/$id',
  component: () => {
    const { id } = itemDetailRoute.useParams()
    return <ItemDetail id={id} />
  },
})

const routeTree = rootRoute.addChildren([indexRoute, itemDetailRoute])

const router = createRouter({ routeTree })

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router
  }
}

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  </StrictMode>
)
