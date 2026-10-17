import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from "@tanstack/react-query";
import { commands, type EventSummary } from "@/lib/bindings";
import { searchKeys } from "@/features/search/queries";
import { unwrap } from "@/lib/unwrap";

export const eventKeys = {
  /** Prefix for everything event-related, so one call invalidates it all. */
  all: ["events"] as const,
  calendar: ["calendars", "default"] as const,
  window: (from: number, to: number) => ["events", "window", from, to] as const,
  everything: ["events", "everything"] as const,
  detail: (id: string) => ["events", "detail", id] as const,
  involved: (eventId: string) => ["events", "involved", eventId] as const,
  forEntity: (entityId: string) => ["events", "for-entity", entityId] as const,
};

/** The calendar used to format and edit dates (the world's default one). */
export function useDefaultCalendar() {
  return useQuery({
    queryKey: eventKeys.calendar,
    queryFn: () => unwrap(commands.getDefaultCalendar()),
    staleTime: Infinity,
  });
}

/**
 * Events overlapping `[from, to]`. Pass a window that is already padded and
 * snapped (see `queryWindow`) so panning does not change the key every frame;
 * `keepPreviousData` keeps the old events on screen while a new window loads.
 */
export function useEvents(from: number, to: number) {
  return useQuery({
    queryKey: eventKeys.window(from, to),
    queryFn: () => unwrap(commands.listEvents(from, to)),
    placeholderData: keepPreviousData,
  });
}

export function useEvent(entityId: string) {
  return useQuery({
    queryKey: eventKeys.detail(entityId),
    queryFn: () => unwrap(commands.getEvent(entityId)),
  });
}

/** Very wide window used by "Fit all events"; day numbers stay well inside f64 precision. */
const EVERYTHING = 1e12;

/** Returns a function that loads every event (for "Fit all events"). */
export function useFetchAllEvents() {
  const queryClient = useQueryClient();
  return () =>
    queryClient.fetchQuery({
      queryKey: eventKeys.everything,
      queryFn: () => unwrap(commands.listEvents(-EVERYTHING, EVERYTHING)),
      staleTime: 0,
    });
}

/** Events change entity lists, search results and the "Events" panels. */
function invalidateEventData(queryClient: QueryClient) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: eventKeys.all }),
    queryClient.invalidateQueries({ queryKey: searchKeys.all }),
    queryClient.invalidateQueries({ queryKey: ["entities"] }),
  ]);
}

export function useCreateEvent() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      name,
      startDay,
      endDay = null,
    }: {
      name: string;
      startDay: number;
      endDay?: number | null;
    }) => unwrap(commands.createEvent(name, startDay, endDay)),
    onSuccess: () => invalidateEventData(queryClient),
  });
}

export function useUpdateEventDates() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      entityId,
      startDay,
      endDay,
      importance = null,
    }: {
      entityId: string;
      startDay: number;
      endDay: number | null;
      importance?: number | null;
    }) => unwrap(commands.updateEventDates(entityId, startDay, endDay, importance)),
    onSuccess: (event: EventSummary) => {
      queryClient.setQueryData(eventKeys.detail(event.entityId), event);
      return invalidateEventData(queryClient);
    },
  });
}

export function useInvolved(eventId: string) {
  return useQuery({
    queryKey: eventKeys.involved(eventId),
    queryFn: () => unwrap(commands.listInvolved(eventId)),
  });
}

export function useAddInvolved(eventId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (entityId: string) => unwrap(commands.addInvolved(eventId, entityId)),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: eventKeys.all }),
  });
}

export function useRemoveInvolved(eventId: string) {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (entityId: string) => unwrap(commands.removeInvolved(eventId, entityId)),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: eventKeys.all }),
  });
}

/** Events an entity is involved in (the "Events" section of non-event pages). */
export function useEventsForEntity(entityId: string) {
  return useQuery({
    queryKey: eventKeys.forEntity(entityId),
    queryFn: () => unwrap(commands.listEventsForEntity(entityId)),
  });
}

/** Dev builds only: creates N random events (the backend refuses in release). */
export function useSeedEvents() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (count: number) => unwrap(commands.devSeedEvents(count)),
    onSuccess: () => invalidateEventData(queryClient),
  });
}
