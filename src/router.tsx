import {
  createHashHistory,
  createRootRoute,
  createRoute,
  createRouter,
} from "@tanstack/react-router";
import { AppShell } from "@/components/app-shell";
import { WelcomePage } from "@/features/app/welcome-page";

// Code-based routes (no file-based routing plugin). Add new routes here.
const rootRoute = createRootRoute({ component: AppShell });

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: WelcomePage,
});

const routeTree = rootRoute.addChildren([indexRoute]);

// Hash history keeps URLs working both in `tauri dev` and in the packaged app,
// where the frontend is served from a custom protocol rather than a web server.
export const router = createRouter({ routeTree, history: createHashHistory() });

// Tells TanStack Router about our routes so <Link to="..."> is type-checked.
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
