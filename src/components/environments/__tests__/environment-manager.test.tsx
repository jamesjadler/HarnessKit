import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { transport } from "@/lib/transport";
import { useEnvironmentStore } from "@/stores/environment-store";
import { EnvironmentManager } from "../environment-manager";

vi.mock("@/lib/transport", () => ({ transport: vi.fn() }));
const command = vi.fn();
beforeEach(() => {
  vi.clearAllMocks();
  command.mockResolvedValue(undefined);
  useEnvironmentStore.setState({
    data: null,
    error: null,
    busy: false,
    command,
  });
});
it("uses a discovered address and leaves SSH overrides optional", async () => {
  vi.mocked(transport).mockResolvedValue([
    {
      name: "MacBook",
      status: "Tailscale online · SSH not checked",
      connection: { type: "ssh", host: "mac.ts.net", user: "", port: 22 },
    },
  ]);
  render(<EnvironmentManager localPaths={null} />);
  fireEvent.click(
    screen.getByRole("button", { name: "Discover environments" }),
  );
  fireEvent.click(
    await screen.findByRole("button", { name: /MacBook.*Set up connection/ }),
  );
  expect(screen.getByLabelText("Host")).toHaveValue("mac.ts.net");
  expect(screen.getByLabelText("SSH user")).not.toBeRequired();
  expect(screen.getByLabelText("SSH port")).not.toBeRequired();
  fireEvent.click(
    screen.getByRole("button", { name: "Test connection and add" }),
  );
  await waitFor(() =>
    expect(command).toHaveBeenCalledWith(
      expect.objectContaining({
        action: "connect",
        connection: { type: "ssh", host: "mac.ts.net", user: "", port: 0 },
      }),
    ),
  );
});
it("retains entered values and displays actionable guidance after failure", async () => {
  command.mockRejectedValue(new Error("Connection refused"));
  render(<EnvironmentManager localPaths={null} />);
  fireEvent.click(screen.getByRole("button", { name: "Add environment" }));
  fireEvent.change(screen.getByLabelText("Host"), {
    target: { value: "my-mac" },
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Test connection and add" }),
  );
  expect(
    await screen.findByText("SSH service is not accepting connections"),
  ).toBeInTheDocument();
  expect(screen.getByLabelText("Host")).toHaveValue("my-mac");
});
it("connects a selected Docker container using its context without SSH fields", async () => {
  vi.mocked(transport).mockResolvedValue([
    {
      name: "app-dev",
      status: "Running · Dev Container",
      connection: {
        type: "docker",
        container: "abc123",
        context: "desktop-linux",
      },
    },
  ]);
  render(<EnvironmentManager localPaths={null} />);
  fireEvent.click(
    screen.getByRole("button", { name: "Discover environments" }),
  );
  fireEvent.click(
    await screen.findByRole("button", { name: /app-dev.*Set up connection/ }),
  );
  expect(screen.getByLabelText("Container")).toHaveValue("abc123");
  expect(screen.queryByLabelText("SSH user")).not.toBeInTheDocument();
  fireEvent.click(
    screen.getByRole("button", { name: "Test connection and add" }),
  );
  await waitFor(() =>
    expect(command).toHaveBeenCalledWith(
      expect.objectContaining({
        action: "connect",
        connection: {
          type: "docker",
          container: "abc123",
          context: "desktop-linux",
        },
      }),
    ),
  );
});
it("keeps agents within their environment and remote machines outside the desktop", async () => {
  const local = {
    id: "local",
    machine: "Desktop",
    name: "Ubuntu",
    os: "linux",
    paused: false,
    providers: [],
  };
  const windows = { ...local, id: "win", name: "Windows" };
  const remote = { ...local, id: "mac", machine: "MacBook Air", name: "macOS" };
  useEnvironmentStore.setState({
    data: {
      registry: {
        version: 1,
        generation: 1,
        environments: [local, windows, remote],
        skills: [],
        catalog: ["codex", "claude"].map((name) => ({
          id: name,
          environment: "local",
          name,
          home: `/home/${name}`,
          root: `/home/${name}/skills`,
          detected: true,
          enabled: true,
          provider: null,
          error: null,
        })),
        preferences: {
          automatic: false,
          policy: "all",
          direction: "bidirectional",
          host: null,
          removal: "ask",
        },
      },
      statuses: [],
    },
  });
  const { container } = render(<EnvironmentManager />);
  const machines = container.querySelectorAll(".quiet-machine");
  expect(machines).toHaveLength(2);
  expect(machines[0].querySelectorAll(".quiet-environment")).toHaveLength(2);
  const ubuntu = machines[0].querySelector(".quiet-environment");
  expect(ubuntu?.querySelectorAll(".quiet-agent")).toHaveLength(2);
  expect(machines[1].textContent).toContain("MacBook Air");
  expect(machines[1].textContent).not.toContain("Claude Code");
});
it("opening and canceling the picker does not flush an unsaved draft", async () => {
  vi.mocked(transport).mockResolvedValue({
    browse: { path: "/skills", parent: "/", entries: [], truncated: false },
  });
  useEnvironmentStore.setState({
    data: {
      registry: {
        version: 1,
        generation: 1,
        environments: [
          {
            id: "local",
            machine: "PC",
            name: "Ubuntu",
            os: "linux",
            paused: false,
            providers: [],
          },
        ],
        catalog: [
          {
            id: "picker-draft",
            environment: "local",
            name: "codex",
            home: "/home/codex",
            root: "/skills",
            detected: true,
            enabled: true,
            provider: null,
            error: null,
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
    },
  });
  render(<EnvironmentManager />);
  const field = screen.getByLabelText("Global skills path");
  field.focus();
  fireEvent.change(field, { target: { value: "/draft" } });
  const browse = screen.getByLabelText("Browse Global skills path");
  browse.focus();
  fireEvent.click(browse);
  await screen.findByRole("dialog");
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() =>
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
  );
  expect(command).not.toHaveBeenCalledWith(
    expect.objectContaining({ action: "catalog_path" }),
  );
});
