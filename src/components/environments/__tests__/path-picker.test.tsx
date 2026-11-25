import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { transport } from "@/lib/transport";
import { useEnvironmentStore } from "@/stores/environment-store";
import { PathPicker } from "../path-picker";
import { ProjectDiscovery } from "../project-discovery";

vi.mock("@/lib/transport", () => ({ transport: vi.fn() }));
beforeEach(() => {
  vi.clearAllMocks();
});
it("browses the selected environment and saves the canonical chosen path", async () => {
  vi.mocked(transport).mockResolvedValue({
    browse: {
      path: "/home/demo",
      parent: "/home",
      entries: [
        { name: "project", path: "/home/demo/project", is_directory: true },
        {
          name: "AGENTS.md",
          path: "/home/demo/AGENTS.md",
          is_directory: false,
        },
      ],
      truncated: false,
    },
  });
  const onSelect = vi.fn();
  render(
    <PathPicker
      environment="remote-mac"
      initialPath="~/missing"
      onClose={vi.fn()}
      onSelect={onSelect}
    />,
  );
  await screen.findByText("project");
  expect(transport).toHaveBeenCalledWith("environment_command", {
    req: {
      action: "browse_paths",
      environment: "remote-mac",
      path: "~/missing",
    },
  });
  expect(screen.queryByText("AGENTS.md")).not.toBeInTheDocument();
  fireEvent.click(
    screen.getByRole("button", { name: "Choose this directory" }),
  );
  expect(onSelect).toHaveBeenCalledWith("/home/demo");
});
it("keeps recovery navigation available after an inaccessible path", async () => {
  vi.mocked(transport)
    .mockRejectedValueOnce(new Error("Permission denied"))
    .mockResolvedValueOnce({
      browse: { path: "/safe", parent: "/", entries: [], truncated: false },
    });
  render(
    <PathPicker
      environment="win"
      initialPath="/denied"
      onClose={vi.fn()}
      onSelect={vi.fn()}
    />,
  );
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Permission denied",
  );
  fireEvent.change(screen.getByLabelText("Browse directory"), {
    target: { value: "/safe" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Go" }));
  await waitFor(() =>
    expect(
      screen.getByRole("button", { name: "Choose this directory" }),
    ).toBeEnabled(),
  );
});
it("file mode allows selecting instruction files", async () => {
  vi.mocked(transport).mockResolvedValue({
    browse: {
      path: "/project",
      parent: "/",
      entries: [
        { name: "AGENTS.md", path: "/project/AGENTS.md", is_directory: false },
      ],
      truncated: false,
    },
  });
  const choose = vi.fn();
  render(
    <PathPicker
      environment="local"
      directory={false}
      initialPath="/project"
      onClose={vi.fn()}
      onSelect={choose}
    />,
  );
  fireEvent.click(await screen.findByText("AGENTS.md"));
  expect(choose).toHaveBeenCalledWith("/project/AGENTS.md");
});
it("submits a project once on Enter followed by blur and keeps discovery agent scoped", async () => {
  const command = vi.fn().mockResolvedValue(undefined);
  useEnvironmentStore.setState({ command, busy: false });
  render(<ProjectDiscovery environment="win" agent="codex" />);
  const input = screen.getByLabelText("Codex project directory");
  fireEvent.change(input, { target: { value: "C:/projects" } });
  input.focus();
  fireEvent.keyDown(input, { key: "Enter" });
  fireEvent.blur(input);
  await waitFor(() => expect(command).toHaveBeenCalledTimes(1));
  expect(command).toHaveBeenCalledWith({
    action: "add_agent_project",
    environment: "win",
    agent: "codex",
    root: "C:/projects",
  });
});
it("standalone discovery categorizes by environment without imposing an agent", async () => {
  const command = vi.fn().mockResolvedValue(undefined);
  useEnvironmentStore.setState({ command, busy: false });
  render(<ProjectDiscovery environment="local" />);
  const input = screen.getByLabelText("Environment project directory");
  fireEvent.change(input, { target: { value: "~/code" } });
  fireEvent.blur(input);
  await waitFor(() =>
    expect(command).toHaveBeenCalledWith({
      action: "discover_projects",
      environment: "local",
      root: "~/code",
    }),
  );
});
