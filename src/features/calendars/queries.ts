import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from "@tanstack/react-query";
import { searchKeys } from "@/features/search/queries";
import { eventKeys } from "@/features/timeline/queries";
import { commands, type Calendar, type CalendarDefinition, type DateStyle } from "@/lib/bindings";
import { unwrap } from "@/lib/unwrap";

export const calendarKeys = {
  /** Prefix for everything calendar-related, so one call invalidates it all. */
  all: ["calendars"] as const,
  list: ["calendars", "list"] as const,
  default: ["calendars", "default"] as const,
  display: ["calendars", "display"] as const,
  preview: (definition: CalendarDefinition, days: number[], style: DateStyle) =>
    ["calendars", "preview", definition, days, style] as const,
};

/** Every calendar of the world: the default first, then by name. */
export function useCalendars() {
  return useQuery({
    queryKey: calendarKeys.list,
    queryFn: () => unwrap(commands.listCalendars()),
  });
}

/** The calendar dates are shown in: the saved choice, or the default one. */
export function useDisplayCalendar() {
  return useQuery({
    queryKey: calendarKeys.display,
    queryFn: () => unwrap(commands.getDisplayCalendar()),
  });
}

/**
 * Anything that depends on a calendar definition: the calendar queries, and
 * the event lists and search results whose dates are drawn from it. Events
 * themselves never move (day numbers are absolute), but their labels do.
 */
function invalidateCalendarData(queryClient: QueryClient) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: calendarKeys.all }),
    queryClient.invalidateQueries({ queryKey: eventKeys.all }),
    queryClient.invalidateQueries({ queryKey: searchKeys.all }),
    queryClient.invalidateQueries({ queryKey: ["entities"] }),
  ]);
}

/** Chooses the display calendar (`null` = follow the default) and stores it in the world. */
export function useSetDisplayCalendar() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (calendarId: string | null) => unwrap(commands.setDisplayCalendar(calendarId)),
    onSuccess: (calendar: Calendar) => {
      // Only labels change, not data, so updating the cached choice is enough.
      queryClient.setQueryData(calendarKeys.display, calendar);
    },
  });
}

export function useCreateCalendar() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: ({ name, definition }: { name: string; definition: CalendarDefinition }) =>
      unwrap(commands.createCalendar(name, definition)),
    onSuccess: () => invalidateCalendarData(queryClient),
  });
}

export function useUpdateCalendar() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: ({
      id,
      name,
      definition,
    }: {
      id: string;
      name: string | null;
      definition: CalendarDefinition | null;
    }) => unwrap(commands.updateCalendar(id, name, definition)),
    onSuccess: () => invalidateCalendarData(queryClient),
  });
}

export function useDeleteCalendar() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (id: string) => unwrap(commands.deleteCalendar(id)),
    onSuccess: () => invalidateCalendarData(queryClient),
  });
}

export function useSetDefaultCalendar() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: (id: string) => unwrap(commands.setDefaultCalendar(id)),
    onSuccess: () => invalidateCalendarData(queryClient),
  });
}

export function useDuplicateCalendar() {
  const queryClient = useQueryClient();
  return useMutation({
    meta: { inlineError: true },
    mutationFn: ({ id, newName }: { id: string; newName: string }) =>
      unwrap(commands.duplicateCalendar(id, newName)),
    onSuccess: () => invalidateCalendarData(queryClient),
  });
}

/**
 * Validates an unsaved definition and formats sample days with it. The backend
 * call is not a `Result` (problems come back as `issues`), so no `unwrap`.
 * Pass `null` to skip (e.g. while the draft has unparsable numbers).
 */
export function usePreviewCalendar(
  definition: CalendarDefinition | null,
  days: number[],
  style: DateStyle = "full",
) {
  return useQuery({
    queryKey: definition ? calendarKeys.preview(definition, days, style) : ["calendars", "preview"],
    queryFn: () => (definition ? commands.previewCalendar(definition, days, style) : null),
    enabled: definition !== null,
    placeholderData: keepPreviousData,
  });
}
