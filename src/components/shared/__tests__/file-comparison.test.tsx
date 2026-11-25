import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { useEnvironmentStore } from "@/stores/environment-store";
import { transport } from "@/lib/transport";
import { FileComparison } from "../file-comparison";

vi.mock("@/lib/transport", () => ({
  transport: vi.fn().mockResolvedValue({
    path: "/local/config.toml",
    content: "original",
    revision: "one",
    captured_at: "now",
  }),
}));
beforeEach(() => {
  useEnvironmentStore.setState({ data: null });
  vi.mocked(transport)
    .mockReset()
    .mockImplementation(
      (_command, args) =>
        Promise.resolve({
          path: args?.path,
          content: String(args?.path),
          revision: "one",
          captured_at: "now",
        }) as never,
    );
});
it("keeps dirty panes when the environment catalog changes", async () => {
  const source = { path: "/local/config.toml", provider: "codex" };
  const choices = [
    source,
    { path: "/peer/config.toml", provider: "codex", environment: "Windows" },
  ];
  render(
    <FileComparison source={source} choices={choices} onClose={() => {}} />,
  );
  const input = await screen.findByLabelText("Result block 1");
  fireEvent.change(input, { target: { value: "unsaved draft" } });
  act(() =>
    useEnvironmentStore.setState({
      data: {
        registry: {
          environments: [
            {
              id: "mac",
              name: "macOS",
              machine: "Mac",
              providers: [
                { id: "codex", name: "codex", root: "/remote/skills" },
              ],
            },
          ],
        },
      },
    } as never),
  );
  await waitFor(() =>
    expect(screen.getByLabelText("Result block 1")).toHaveValue(
      "unsaved draft",
    ),
  );
});

it("loads a third environment and retains it on explicit reload", async () => {
  const source = {
    path: "/a/config.toml",
    provider: "codex",
    environment: "Ubuntu",
  };
  const choices = [
    source,
    { path: "/b/config.toml", provider: "codex", environment: "Windows" },
    { path: "/c/config.toml", provider: "codex", environment: "macOS" },
  ];
  render(
    <FileComparison source={source} choices={choices} onClose={() => {}} />,
  );
  await screen.findByLabelText("Result block 1");
  fireEvent.change(screen.getByLabelText("Third environment"), {
    target: { value: "local:codex:/c/config.toml" },
  });
  await screen.findByRole("button", { name: "Keep macOS · one" });
  fireEvent.click(
    screen.getByRole("button", { name: "Reload snapshots (discard edits)" }),
  );
  await screen.findByRole("button", { name: "Keep macOS · one" });
  expect(screen.getByLabelText("Third environment")).toHaveValue(
    "local:codex:/c/config.toml",
  );
});

it("locks selection, reload and close while applying a result", async () => {
  const { transport } = await import("@/lib/transport");
  let finish!: (value: unknown) => void;
  vi.mocked(transport).mockImplementation((command, args) =>
    command === "save_comparison_file"
      ? new Promise((resolve) => {
          finish = resolve;
        })
      : (Promise.resolve({
          path: "/local/config.toml",
          content: String(args?.path),
          revision: "one",
          captured_at: "now",
        }) as never),
  );
  const source = { path: "/local/config.toml", provider: "codex" };
  render(
    <FileComparison
      source={source}
      choices={[
        source,
        {
          path: "/peer/config.toml",
          provider: "codex",
          environment: "Windows",
        },
      ]}
      onClose={vi.fn()}
    />,
  );
  fireEvent.change(await screen.findByLabelText("Result block 1"), {
    target: { value: "/peer/config.toml" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Mark block resolved" }));
  fireEvent.click(screen.getByRole("button", { name: "Save & apply result" }));
  expect(
    screen.getByRole("button", { name: "Close comparison" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Reload snapshots (discard edits)" }),
  ).toBeDisabled();
  expect(screen.getByLabelText("Third environment")).toBeDisabled();
  await act(async () =>
    finish({
      path: "/local/config.toml",
      content: "updated",
      revision: "two",
      captured_at: "now",
    }),
  );
  expect(
    screen.getByRole("button", { name: "Close comparison" }),
  ).toBeEnabled();
});

it("offers only the same agent, file and scope, including remote candidates", async () => {
  const source = {
    path: "/home/me/.codex/config.toml",
    provider: "codex",
    scope: "global",
    category: "settings",
  };
  useEnvironmentStore.setState({
    data: {
      registry: {
        environments: [
          {
            id: "windows",
            machine: "Desktop",
            name: "Windows",
            providers: [
              {
                id: "codex",
                name: "codex",
                root: "/mnt/c/Users/me/.codex/skills",
                scope: "global",
              },
              {
                id: "copilot",
                name: "copilot",
                root: "/mnt/c/Users/me/.copilot/skills",
                scope: "global",
              },
              {
                id: "project",
                name: "codex",
                root: "/project/.codex/skills",
                scope: "project",
              },
            ],
          },
        ],
      },
    },
  } as never);
  render(
    <FileComparison
      source={source}
      choices={[
        { ...source, path: "/claude/config.toml", provider: "claude" },
        { ...source, path: "/home/me/.codex/AGENTS.md" },
        {
          ...source,
          path: "/project/.codex/config.toml",
          scope: "project:/project",
        },
      ]}
      onClose={() => {}}
    />,
  );
  await screen.findByLabelText("Result block 1");
  const selector = screen.getByLabelText("Compare with");
  expect(selector).toHaveTextContent("Windows");
  expect(selector).not.toHaveTextContent("claude");
  expect(selector).not.toHaveTextContent("copilot");
  expect(selector).not.toHaveTextContent("AGENTS.md");
  expect(selector).not.toHaveTextContent("/project/");
  expect(transport).not.toHaveBeenCalledWith(
    "read_comparison_file",
    expect.objectContaining({ path: "/claude/config.toml" }),
  );
});
it("shows matching contents without opening a diff editor", async () => {
  vi.mocked(transport).mockResolvedValue({
    path: "/config.toml",
    content: "same",
    revision: "one",
    captured_at: "now",
  });
  const source = { path: "/a/config.toml", provider: "codex" };
  render(
    <FileComparison
      source={source}
      choices={[{ ...source, path: "/b/config.toml", environment: "Windows" }]}
      onClose={() => {}}
    />,
  );
  expect(await screen.findByText(/Contents match/)).toBeInTheDocument();
  expect(screen.queryByLabelText("Result block 1")).not.toBeInTheDocument();
});
