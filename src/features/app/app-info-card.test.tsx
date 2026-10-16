import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { renderWithQuery } from "@/test/render";
import { AppInfoCard } from "./app-info-card";

// Replace the generated bindings so no Tauri runtime is needed.
vi.mock("@/lib/bindings", () => ({
  commands: {
    appInfo: vi.fn().mockResolvedValue({
      status: "ok",
      data: { name: "Orbis Scriptor", version: "0.1.0" },
    }),
  },
}));

describe("AppInfoCard", () => {
  it("shows the name and version returned by Rust", async () => {
    renderWithQuery(<AppInfoCard />);

    expect(await screen.findByText("Version 0.1.0")).toBeInTheDocument();
    expect(screen.getByText("Orbis Scriptor")).toBeInTheDocument();
  });
});
