import type { ComponentProps } from "react";
import { cn } from "@/lib/utils";

/**
 * A styled native `<select>`. Used for short fixed lists (calendars, months,
 * eras): it is keyboard-friendly, accessible out of the box and easy to test.
 */
export function NativeSelect({ className, ...props }: ComponentProps<"select">) {
  return (
    <select
      data-slot="native-select"
      className={cn(
        "h-8 rounded-lg border border-input bg-background px-2 text-sm text-foreground outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 aria-invalid:border-destructive",
        className,
      )}
      {...props}
    />
  );
}
