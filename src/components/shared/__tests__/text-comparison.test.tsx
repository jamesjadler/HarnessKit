import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { TextComparison } from "../text-comparison";
const left = {
  id: "a",
  environment: "Windows",
  provider: "Codex",
  path: "C:/a/config.toml",
  revision: "r8",
  content: "old\n",
};
const right = {
  id: "b",
  environment: "Ubuntu",
  provider: "Codex",
  path: "/home/a/config.toml",
  revision: "r7",
  content: "new\n",
};
describe("shared merge", () => {
  it("retains multiline draft and original revision after failed save", async () => {
    const save = vi
      .fn()
      .mockRejectedValue(new Error("File changed externally"));
    render(<TextComparison left={left} right={right} onSave={save} />);
    fireEvent.change(screen.getByLabelText("Result block 1"), {
      target: { value: "edited\nsecond\n" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "Mark block resolved" }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Save & apply result" }),
    );
    await screen.findByRole("alert");
    expect(screen.getByLabelText("Result block 1")).toHaveValue(
      "edited\nsecond\n",
    );
    expect(save).toHaveBeenCalledWith(left, "edited\nsecond\n");
  });
  it("keeps a named source as draft, then explicitly applies to writable targets", async () => {
    const save = vi.fn(async (s, c) => ({
      ...s,
      content: c,
      revision: "saved",
    }));
    render(<TextComparison left={left} right={right} onSave={save} />);
    fireEvent.click(screen.getByRole("button", { name: "Keep Ubuntu · r7" }));
    expect(screen.getByLabelText("Result block 1")).toHaveValue("new\n");
    expect(save).not.toHaveBeenCalled();
    fireEvent.click(
      screen.getByRole("button", { name: "Save & apply result" }),
    );
    await waitFor(() => expect(save).toHaveBeenCalledWith(left, "new\n"));
  });
  it("ignores each side without deleting text and puts arrows closest to result", () => {
    render(<TextComparison left={left} right={right} onSave={vi.fn()} />);
    const ignore = screen.getByRole("button", {
      name: "Ignore Windows change 1",
    });
    expect(ignore.nextElementSibling).toHaveAccessibleName(
      "Use Windows change 1",
    );
    const accept = screen.getByRole("button", { name: "Use Ubuntu change 1" });
    expect(accept.nextElementSibling).toHaveAccessibleName(
      "Ignore Ubuntu change 1",
    );
    fireEvent.click(ignore);
    expect(
      screen.getByRole("button", { name: "Save & apply result" }),
    ).toBeDisabled();
    fireEvent.click(
      screen.getByRole("button", { name: "Ignore Ubuntu change 1" }),
    );
    expect(screen.getByLabelText("Result block 1")).toHaveValue("old\n");
    expect(
      screen.getByRole("button", { name: "Save & apply result" }),
    ).toBeEnabled();
  });
  it("preserves edits when switching among three sources and supports undo", () => {
    render(
      <TextComparison
        left={left}
        right={right}
        third={{ ...right, id: "c", environment: "macOS", content: "mac\n" }}
        onSave={vi.fn()}
      />,
    );
    fireEvent.change(screen.getByLabelText("Result block 1"), {
      target: { value: "custom\n" },
    });
    fireEvent.change(screen.getByLabelText("Right source"), {
      target: { value: "2" },
    });
    expect(screen.getByLabelText("Result block 1")).toHaveValue("custom\n");
    fireEvent.click(screen.getByRole("button", { name: "Use macOS change 1" }));
    expect(screen.getByLabelText("Result block 1")).toHaveValue("mac\n");
    fireEvent.click(screen.getByRole("button", { name: "Undo" }));
    expect(screen.getByLabelText("Result block 1")).toHaveValue("custom\n");
  });
  it("does not write historical sources", async () => {
    const save = vi.fn(async (s, c) => ({ ...s, content: c }));
    render(
      <TextComparison
        left={{ ...left, readOnly: true }}
        right={right}
        onSave={save}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Keep Windows · r8" }));
    fireEvent.click(
      screen.getByRole("button", { name: "Save & apply result" }),
    );
    await waitFor(() => expect(save).toHaveBeenCalledTimes(1));
    expect(save).toHaveBeenCalledWith(right, "old\n");
  });
  it("retains successful revisions on partial failure and retries only remaining files", async () => {
    const save = vi
      .fn()
      .mockImplementationOnce(async (s, c) => ({
        ...s,
        content: c,
        revision: "r9",
      }))
      .mockRejectedValueOnce(new Error("Offline"))
      .mockImplementation(async (s, c) => ({
        ...s,
        content: c,
        revision: "r10",
      }));
    render(<TextComparison left={left} right={right} onSave={save} />);
    fireEvent.change(screen.getByLabelText("Result block 1"), {
      target: { value: "merged" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "Mark block resolved" }),
    );
    fireEvent.click(
      screen.getByRole("button", { name: "Save & apply result" }),
    );
    await screen.findByRole("alert");
    fireEvent.click(
      screen.getByRole("button", { name: "Save & apply result" }),
    );
    await waitFor(() => expect(save).toHaveBeenCalledTimes(3));
    expect(save.mock.calls[2][0].id).toBe("b");
  });
});

it("preserves CRLF when editing a Windows result block", async () => {
  const save = vi.fn(async (s, c) => ({ ...s, content: c }));
  render(
    <TextComparison
      left={{ ...left, content: "old\r\nsecond\r\n" }}
      right={{ ...right, content: "new\r\nother\r\n" }}
      onSave={save}
    />,
  );
  fireEvent.change(screen.getByLabelText("Result block 1"), {
    target: { value: "edited\nsecond\n" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Mark block resolved" }));
  fireEvent.click(screen.getByRole("button", { name: "Save & apply result" }));
  await waitFor(() =>
    expect(save).toHaveBeenCalledWith(
      expect.objectContaining({ id: "a" }),
      "edited\r\nsecond\r\n",
    ),
  );
});

it("stops further writes if the editor is unmounted during a save", async () => {
  let finish!: (value: typeof left) => void;
  const save = vi.fn(
    () =>
      new Promise<typeof left>((resolve) => {
        finish = resolve;
      }),
  );
  const busy = vi.fn();
  const view = render(
    <TextComparison
      left={left}
      right={right}
      onSave={save}
      onBusyChange={busy}
    />,
  );
  fireEvent.change(screen.getByLabelText("Result block 1"), {
    target: { value: "merged" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Mark block resolved" }));
  fireEvent.click(screen.getByRole("button", { name: "Save & apply result" }));
  expect(busy).toHaveBeenCalledWith(true);
  view.unmount();
  finish({ ...left, content: "merged" });
  await waitFor(() => expect(busy).toHaveBeenLastCalledWith(false));
  expect(save).toHaveBeenCalledTimes(1);
});
