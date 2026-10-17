import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";

interface Props {
  open: boolean;
  /** Human-readable date the event will be created on. */
  dateLabel: string;
  busy: boolean;
  error: string | null;
  onCancel: () => void;
  onCreate: (name: string) => void;
}

/** Small dialog opened by double-clicking the timeline: name the event, it starts on that day. */
export function CreateEventDialog({ open, dateLabel, busy, error, onCancel, onCreate }: Props) {
  return (
    <Dialog open={open} onOpenChange={(next) => !next && onCancel()}>
      <DialogContent className="sm:max-w-sm">
        {/* Remounted per open so the name field starts empty. */}
        {open && (
          <CreateForm
            dateLabel={dateLabel}
            busy={busy}
            error={error}
            onCancel={onCancel}
            onCreate={onCreate}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

function CreateForm({ dateLabel, busy, error, onCancel, onCreate }: Omit<Props, "open">) {
  const [name, setName] = useState("");
  const trimmed = name.trim();

  return (
    <form
      onSubmit={(e) => {
        e.preventDefault();
        if (trimmed) onCreate(trimmed);
      }}
      className="flex flex-col gap-4"
    >
      <DialogHeader>
        <DialogTitle>New event</DialogTitle>
        <DialogDescription>Starts on {dateLabel}. Change the dates on its page.</DialogDescription>
      </DialogHeader>
      <Input
        autoFocus
        value={name}
        onChange={(e) => setName(e.target.value)}
        placeholder="Event name"
        aria-label="Event name"
      />
      {error && <p className="text-sm text-destructive">{error}</p>}
      <DialogFooter>
        <Button type="button" variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Button type="submit" disabled={!trimmed || busy}>
          Create
        </Button>
      </DialogFooter>
    </form>
  );
}
