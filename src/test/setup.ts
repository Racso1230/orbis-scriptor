// Adds matchers like `toBeInTheDocument()` to Vitest's `expect`.
import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// Unmount rendered components after every test.
afterEach(() => {
  cleanup();
});
