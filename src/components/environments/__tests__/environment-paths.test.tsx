import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { beforeEach, expect, it, vi } from "vitest";
import type {
  AgentCatalogEntry,
  EnvironmentOverview,
  ManagedEnvironment,
} from "@/lib/environments";
import { useEnvironmentStore } from "@/stores/environment-store";
import { useUIStore } from "@/stores/ui-store";
import { EnvironmentPaths, environmentHealth } from "../environment-paths";

const command = vi.fn().mockResolvedValue(undefined);
const env: ManagedEnvironment = {
  id: "win",
  machine: "PC",
  name: "Windows",
  os: "windows",
  paused: true,
  providers: [{ id: "c", name: "codex", root: "C:/c/skills", paused: false }],
};
const catalog: AgentCatalogEntry[] = [
  {
    id: "one",
    environment: "win",
    name: "codex",
    home: "C:/c",
    root: "C:/c/skills",
    detected: true,
    enabled: true,
    home_enabled: true,
    provider: "c",
    error: null,
    scope: "global",
    access: { state: "available", count: 2, error: null },
    home_access: { state: "available", count: null, error: null },
  },
  {
    id: "two",
    environment: "win",
    name: "codex",
    home: "C:/c",
    root: "C:/shared",
    detected: true,
    enabled: true,
    provider: null,
    error: "No such file or directory",
    access: {
      state: "missing",
      count: null,
      error: "No such file or directory",
    },
  },
];
const data: EnvironmentOverview = {
  registry: {
    version: 1,
    generation: 1,
    environments: [env],
    catalog,
    skills: [],
    projects: [],
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
beforeEach(() => {
  vi.clearAllMocks();
  useUIStore.setState({ agentVisibility: "all" });
  useEnvironmentStore.setState({ data, command, busy: false, error: null });
});
function show() {
  render(
    <MemoryRouter>
      <EnvironmentPaths environment={env} />
    </MemoryRouter>,
  );
}
it("nests paths under agents with tracked open and untracked collapsed", () => {
  show();
  expect(
    screen.getByText("Tracked paths · 1").closest("details"),
  ).toHaveAttribute("open");
  expect(
    screen.getByText("Untracked paths · 1").closest("details"),
  ).not.toHaveAttribute("open");
  expect(screen.getByText("No such file or directory")).toHaveClass(
    "text-yellow-500",
  );
  expect(screen.queryByText("Sync paused")).not.toBeInTheDocument();
});
it("edits an untracked path independently without enrolling it", async () => {
  show();
  const field = screen.getAllByLabelText("Global skills path")[1];
  fireEvent.change(field, { target: { value: "C:/new-shared" } });
  fireEvent.blur(field);
  await waitFor(() =>
    expect(command).toHaveBeenCalledWith({
      action: "catalog_path",
      environment: "win",
      entry: "two",
      expected: "C:/shared",
      path: "C:/new-shared",
    }),
  );
  expect(command).not.toHaveBeenCalledWith(
    expect.objectContaining({ action: "map_provider" }),
  );
});
it("enables each path separately", async () => {
  show();
  fireEvent.click(screen.getAllByLabelText("Enable Global skills")[0]);
  await waitFor(() =>
    expect(command).toHaveBeenCalledWith({
      action: "catalog_enabled",
      environment: "win",
      entry: "one",
      enabled: false,
    }),
  );
});
it("adds projects only to the selected environment and agent", async () => {
  show();
  fireEvent.change(screen.getByLabelText("Codex project directory"), {
    target: { value: "C:/project" },
  });
  fireEvent.blur(screen.getByLabelText("Codex project directory"));
  await waitFor(() =>
    expect(command).toHaveBeenCalledWith({
      action: "add_agent_project",
      environment: "win",
      agent: "codex",
      root: "C:/project",
    }),
  );
});
it("access summary does not turn errors into pause state", () => {
  expect(
    environmentHealth(env, [
      catalog[0],
      { ...catalog[1], provider: "unavailable" },
    ]),
  ).toEqual({
    issues: 1,
    available: 1,
    label: "Partial · 1 path issue",
  });
});
it("permission denial stays red and visible while sync is paused", () => {
  useEnvironmentStore.setState({
    data: {
      ...data,
      registry: {
        ...data.registry,
        catalog: [
          {
            ...catalog[0],
            access: {
              state: "denied",
              count: null,
              error: "Permission denied",
            },
          },
        ],
      },
    },
  });
  show();
  expect(screen.getByText("Permission denied")).toHaveClass("text-destructive");
});
it("ignores inaccessible disabled and explicitly untracked entries, even when all are untracked", () => {
  const missing = { ...catalog[0], enabled: false, access: catalog[1].access };
  expect(environmentHealth(env, [missing]).issues).toBe(0);
  expect(
    environmentHealth(
      {
        ...env,
        providers: [
          {
            ...env.providers[0],
            disabled: true,
            last_error: "Permission denied",
          },
        ],
      },
      [],
    ).issues,
  ).toBe(0);
});
it("keeps enabled projects within Tracked and disabled projects within Untracked", () => {
  useEnvironmentStore.setState({
    data: {
      ...data,
      registry: {
        ...data.registry,
        projects: [
          {
            id: "p1",
            environment: "win",
            agent: "codex",
            root: "C:/one",
            enabled: true,
          },
          {
            id: "p2",
            environment: "win",
            agent: "codex",
            root: "C:/two",
            enabled: false,
          },
        ],
      },
    },
  });
  show();
  const one = screen.getByDisplayValue("C:/one");
  const two = screen.getByDisplayValue("C:/two");
  expect(one.closest("[data-path-group]")).toHaveAttribute(
    "data-path-group",
    "tracked",
  );
  expect(two.closest("[data-path-group]")).toHaveAttribute(
    "data-path-group",
    "untracked",
  );
});
it("counts tracked missing paths without a provider but ignores disabled project roots and children", () => {
  const e = { ...catalog[1], tracked: true };
  expect(environmentHealth({ ...env, providers: [] }, [e]).issues).toBe(1);
  const child = { ...catalog[0], project: "p1", access: catalog[1].access };
  const project = {
    id: "p1",
    environment: "win",
    agent: "codex",
    root: "C:/gone",
    enabled: false,
    access: catalog[1].access,
  };
  expect(environmentHealth(env, [child], [project]).issues).toBe(0);
});
it("uses only the edited catalog identity when accepting the canonical saved path", async () => {
  const other = {
    ...catalog[0],
    id: "unrelated",
    provider: undefined,
    root: "C:/unrelated",
  } as unknown as AgentCatalogEntry;
  const state = {
    ...data,
    registry: { ...data.registry, catalog: [other, ...catalog] },
  };
  command.mockImplementationOnce(async () => {
    useEnvironmentStore.setState({
      data: {
        ...state,
        registry: {
          ...state.registry,
          catalog: [
            other,
            catalog[0],
            { ...catalog[1], root: "C:/canonical-two" },
          ],
        },
      },
    });
  });
  useEnvironmentStore.setState({ data: state });
  show();
  const input = screen.getByDisplayValue("C:/shared");
  fireEvent.change(input, { target: { value: "C:/alias-two" } });
  fireEvent.blur(input);
  await waitFor(() => expect(input).toHaveValue("C:/canonical-two"));
});

it("filters undetected agents per environment without forgetting configured paths", () => {
  const absent = {
    ...catalog[1],
    id: "absent",
    name: "gemini",
    detected: false,
    home: "C:/absent",
  };
  useEnvironmentStore.setState({
    data: {
      ...data,
      registry: {
        ...data.registry,
        catalog: [
          ...catalog,
          absent,
          { ...absent, environment: "local", detected: true },
        ],
      },
    },
  });
  useUIStore.setState({ agentVisibility: "detected" });
  show();
  expect(screen.queryByLabelText("Gemini CLI path")).not.toBeInTheDocument();
  expect(screen.getByLabelText("Codex path")).toBeInTheDocument();
  expect(command).not.toHaveBeenCalled();
});
it("retains an explicitly tracked agent with unavailable home in detected-only mode", () => {
  useEnvironmentStore.setState({
    data: {
      ...data,
      registry: {
        ...data.registry,
        catalog: [
          { ...catalog[0], detected: false, access: catalog[1].access },
        ],
      },
    },
  });
  useUIStore.setState({ agentVisibility: "detected" });
  show();
  expect(screen.getByLabelText("Codex path")).toBeInTheDocument();
  expect(screen.getByText("No such file or directory")).toBeInTheDocument();
});
