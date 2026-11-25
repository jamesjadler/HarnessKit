import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { ManagedFileList } from "../managed-file-list";
import { managedStatus, type ManagedFile } from "@/lib/managed-files";
import { useManagedFileStore } from "@/stores/managed-file-store";
import { useAgentConfigStore } from "@/stores/agent-config-store";
let seq = 0;
function fixture(): ManagedFile {
  return {
    id: `config-${++seq}`,
    agent: "codex",
    category: "config",
    scope: "global",
    scope_path: null,
    file_name: "config.toml",
    logical_path: "config.toml",
    variant_name: null,
    original_id: null,
    revisions: [
      {
        number: 1,
        content: "old",
        fingerprint: "one",
        created_at: "2026-11-24T12:00:00Z",
        reason: "initial",
      },
      {
        number: 2,
        content: "same",
        fingerprint: "two",
        created_at: "2026-11-24T13:00:00Z",
        reason: "save",
      },
    ],
    copies: ["Ubuntu", "Windows"].map((environment, i) => ({
      id: `copy-${i}`,
      environment_id: String(i),
      environment,
      machine: "Desktop",
      path: `/${environment}/config.toml`,
      provider_id: "codex",
      content: "same",
      fingerprint: "two",
      accepted_fingerprint: "two",
      error: null,
      enabled: true,
      pending: false,
    })),
  };
}
beforeEach(() => {
  useAgentConfigStore.setState({ pendingFocusFile: null });
  useManagedFileStore.setState({
    files: [],
    busy: false,
    error: null,
    loaded: true,
  });
  vi.restoreAllMocks();
});
it("coalesces copies with synced status and keeps contents editable while paths/history default closed", () => {
  const file = fixture();
  render(<ManagedFileList files={[file]} />);
  expect(screen.getAllByText("config.toml")).toHaveLength(1);
  expect(screen.getByText("Synced")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: /config.toml/ }));
  expect(screen.getByLabelText("config.toml contents")).toHaveValue("same");
  expect(
    screen.getByText("Environment paths").closest("details"),
  ).not.toHaveAttribute("open");
  expect(
    screen.getByText("Version history").closest("details"),
  ).not.toHaveAttribute("open");
  expect(
    screen.queryByRole("button", { name: "Edit" }),
  ).not.toBeInTheDocument();
});
it("creates a version using the editing baseline, retains draft when save fails", async () => {
  const file = fixture(),
    command = vi.fn().mockRejectedValue(new Error("File changed externally"));
  useManagedFileStore.setState({ command });
  render(<ManagedFileList files={[file]} />);
  fireEvent.click(screen.getByRole("button", { name: /config.toml/ }));
  fireEvent.change(screen.getByLabelText("config.toml contents"), {
    target: { value: "edited" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Save & sync" }));
  await screen.findByRole("alert");
  expect(command).toHaveBeenCalledWith({
    action: "save",
    file_id: file.id,
    content: "edited",
    expected: "two",
  });
  expect(screen.getByLabelText("config.toml contents")).toHaveValue("edited");
});
it("requires confirmation before restore", () => {
  const file = fixture(),
    command = vi.fn();
  useManagedFileStore.setState({ command });
  vi.spyOn(window, "confirm").mockReturnValue(false);
  render(<ManagedFileList files={[file]} />);
  fireEvent.click(screen.getByRole("button", { name: /config.toml/ }));
  fireEvent.click(screen.getByText("Version history"));
  fireEvent.click(screen.getByRole("button", { name: "Restore" }));
  expect(window.confirm).toHaveBeenCalledWith(
    expect.stringContaining("creates a new version"),
  );
  expect(command).not.toHaveBeenCalled();
});
it("requests a user-supplied name for an independent variant", async () => {
  const file = fixture(),
    command = vi.fn().mockResolvedValue({ files: [file] });
  useManagedFileStore.setState({ files: [file], command });
  render(<ManagedFileList files={[file]} />);
  fireEvent.click(screen.getByRole("button", { name: /config.toml/ }));
  fireEvent.click(screen.getByRole("button", { name: "Create variant" }));
  fireEvent.change(screen.getByLabelText("Variant name"), {
    target: { value: "Offline" },
  });
  const dialog = screen.getByRole("dialog");
  fireEvent.submit(dialog.querySelector("form")!);
  await waitFor(() =>
    expect(command).toHaveBeenCalledWith({
      action: "variant",
      file_id: file.id,
      expected: "two",
      name: "Offline",
    }),
  );
});
it("accepts observed background fingerprint and leaves conflicts unavailable rather than synced", async () => {
  const file = fixture();
  file.copies[0] = {
    ...file.copies[0],
    content: "background",
    fingerprint: "three",
    pending: true,
  };
  file.copies[1].error = "Permission denied";
  expect(managedStatus(file)).toBe("Partial");
  const command = vi.fn().mockResolvedValue({ files: [file] });
  useManagedFileStore.setState({ command });
  render(<ManagedFileList files={[file]} />);
  fireEvent.click(screen.getByRole("button", { name: /config.toml/ }));
  fireEvent.click(screen.getByRole("button", { name: "Accept & sync" }));
  await waitFor(() =>
    expect(command).toHaveBeenCalledWith({
      action: "accept",
      file_id: file.id,
      copy_id: "copy-0",
      expected: "three",
    }),
  );
});
it("does not include intentionally disabled missing copies in sync health", () => {
  const file = fixture();
  file.copies[1].enabled = false;
  file.copies[1].error = "No such file";
  expect(managedStatus(file)).toBe("Synced");
});

it("freezes comparison CAS at open and preserves a separate inline draft", async () => {
  const file = fixture();
  file.copies[1] = {
    ...file.copies[1],
    content: "other",
    fingerprint: "other",
    pending: true,
  };
  const command = vi.fn().mockResolvedValue({ files: [file] });
  useManagedFileStore.setState({ command });
  const view = render(<ManagedFileList files={[file]} />);
  fireEvent.click(screen.getByRole("button", { name: /^config.toml/ }));
  fireEvent.change(screen.getByLabelText("config.toml contents"), {
    target: { value: "inline unsaved" },
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Compare config.toml copies" }),
  );
  const newer = {
    ...file,
    revisions: [
      ...file.revisions,
      {
        ...file.revisions[1],
        number: 3,
        content: "new canonical",
        fingerprint: "three",
      },
    ],
  };
  view.rerender(<ManagedFileList files={[newer]} />);
  fireEvent.click(
    screen.getByRole("button", { name: "Save & sync Desktop / Ubuntu result" }),
  );
  await waitFor(() =>
    expect(command).toHaveBeenCalledWith(
      expect.objectContaining({
        action: "save",
        expected: "two",
        observed: { "copy-0": "two", "copy-1": "other" },
      }),
    ),
  );
  expect(screen.getByLabelText("config.toml contents")).toHaveValue(
    "inline unsaved",
  );
});
it("allows comparison of healthy copies when a third path is unavailable", () => {
  const file = fixture();
  file.copies[1].fingerprint = "other";
  file.copies.push({
    ...file.copies[0],
    id: "offline",
    environment: "macOS",
    error: "Unavailable",
  });
  render(<ManagedFileList files={[file]} />);
  expect(
    screen.getByRole("button", { name: "Compare config.toml copies" }),
  ).toBeEnabled();
});
