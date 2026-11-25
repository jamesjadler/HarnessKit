import { render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, expect, it, vi } from "vitest";
import type { EnvironmentOverview } from "@/lib/environments";
import { useEnvironmentStore } from "@/stores/environment-store";
import NotificationsPage from "../notifications";

const data: EnvironmentOverview = {
  registry: {
    version: 1,
    generation: 1,
    environments: [
      {
        id: "win",
        machine: "PC",
        name: "Windows",
        os: "windows",
        paused: true,
        providers: [
          {
            id: "codex",
            name: "codex",
            root: "C:/missing",
            paused: true,
            last_error: "No such file or directory",
          },
        ],
      },
    ],
    skills: [],
    preferences: {
      automatic: false,
      policy: "all",
      direction: "bidirectional",
      host: null,
      removal: "ask",
    },
  },
  statuses: [],
};
beforeEach(() =>
  useEnvironmentStore.setState({
    data: null,
    error: null,
    busy: false,
    command: vi.fn(),
  }),
);
it("does not declare no changes before inventory has loaded", () => {
  render(
    <MemoryRouter>
      <NotificationsPage />
    </MemoryRouter>,
  );
  expect(
    screen.queryByText("No changes require your attention."),
  ).not.toBeInTheDocument();
  expect(screen.getByText(/Loading notifications/)).toBeInTheDocument();
});
it("shows failed paths even when no skill has a sync conflict", () => {
  useEnvironmentStore.setState({ data });
  render(
    <MemoryRouter>
      <NotificationsPage />
    </MemoryRouter>,
  );
  expect(screen.getByText("No such file or directory")).toBeInTheDocument();
  expect(
    screen.queryByText("No changes require your attention."),
  ).not.toBeInTheDocument();
});
it("does not raise notifications for inaccessible paths the user disabled", () => {
  useEnvironmentStore.setState({
    data: {
      ...data,
      registry: {
        ...data.registry,
        environments: data.registry.environments.map((e) => ({
          ...e,
          providers: e.providers.map((p) => ({ ...p, disabled: true })),
        })),
      },
    },
  });
  render(
    <MemoryRouter>
      <NotificationsPage />
    </MemoryRouter>,
  );
  expect(
    screen.queryByText("No such file or directory"),
  ).not.toBeInTheDocument();
  expect(
    screen.getByText("No changes require your attention."),
  ).toBeInTheDocument();
});
