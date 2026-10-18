import {
  createHashHistory,
  createRootRoute,
  createRoute,
  createRouter,
  redirect,
} from "@tanstack/react-router";
import { AppShell } from "@/components/app-shell";
import { EntityPage } from "@/features/entities/entity-page";
import { currentWorldQuery } from "@/features/worlds/queries";
import { GraphPage } from "@/features/relationships/graph-view";
import { CalendarsPage } from "@/features/calendars/calendars-page";
import { TypesPage } from "@/features/types/types-page";
import { TimelinePage } from "@/features/timeline/timeline-view";
import { SettingsPage } from "@/features/settings/settings-page";
import { WelcomePage } from "@/features/worlds/welcome-page";
import { WorldEmptyState, WorldLayout } from "@/features/worlds/world-layout";
import { queryClient } from "@/lib/query-client";

// Code-based routes (no file-based routing plugin). Add new routes here.
const rootRoute = createRootRoute({ component: AppShell });

// `/` — welcome screen. If a world is already open, go straight to it.
const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  beforeLoad: async () => {
    const world = await queryClient.query({ ...currentWorldQuery, staleTime: "static" });
    if (world) throw redirect({ to: "/world" });
  },
  component: WelcomePage,
});

// `/world` — everything that needs an open world. Guarded: no world → welcome.
const worldRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/world",
  beforeLoad: async () => {
    const world = await queryClient.query({ ...currentWorldQuery, staleTime: "static" });
    if (!world) throw redirect({ to: "/" });
  },
  component: WorldLayout,
});

const worldIndexRoute = createRoute({
  getParentRoute: () => worldRoute,
  path: "/",
  component: WorldEmptyState,
});

// `/world/entity/$id` — one entity's page; `$id` is a path parameter.
const entityRoute = createRoute({
  getParentRoute: () => worldRoute,
  path: "/entity/$id",
  component: EntityPage,
});

// `/world/timeline` - the zoomable event timeline (not an entity, so no tab).
const timelineRoute = createRoute({
  getParentRoute: () => worldRoute,
  path: "/timeline",
  component: TimelinePage,
});

// `/world/calendars` - calendar list and editor.
const calendarsRoute = createRoute({
  getParentRoute: () => worldRoute,
  path: "/calendars",
  component: CalendarsPage,
});

// `/world/types` - entity types and field templates.
const typesRoute = createRoute({
  getParentRoute: () => worldRoute,
  path: "/types",
  component: TypesPage,
});

// `/world/graph` - relationship graph and family tree (focus lives in a store).
const graphRoute = createRoute({
  getParentRoute: () => worldRoute,
  path: "/graph",
  component: GraphPage,
});

// `/settings` - app settings; reachable from the welcome page and from inside a world.
const settingsRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/settings",
  component: SettingsPage,
});

const routeTree = rootRoute.addChildren([
  indexRoute,
  settingsRoute,
  worldRoute.addChildren([
    worldIndexRoute,
    entityRoute,
    timelineRoute,
    calendarsRoute,
    typesRoute,
    graphRoute,
  ]),
]);

// Hash history keeps URLs working both in `tauri dev` and in the packaged app,
// where the frontend is served from a custom protocol rather than a web server.
export const router = createRouter({ routeTree, history: createHashHistory() });

// Tells TanStack Router about our routes so <Link to="..."> is type-checked.
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
