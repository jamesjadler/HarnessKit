import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { useEnvironmentStore } from "@/stores/environment-store";
import { PathEntry } from "../path-entry";

it("preserves the original expected path when a conflicted draft is remounted", async () => {
  useEnvironmentStore.setState({ busy: false });
  const onSave = vi
    .fn()
    .mockRejectedValue(new Error("Path changed; reload settings"));
  const props = {
    entryKey: "cas-regression",
    label: "Skills",
    path: "/original",
    enabled: true,
    onEnable: vi.fn(),
    onSave,
  };
  const view = render(<PathEntry {...props} />);
  fireEvent.change(screen.getByLabelText("Skills path"), {
    target: { value: "/draft" },
  });
  fireEvent.blur(screen.getByLabelText("Skills path"));
  await screen.findByRole("alert");
  view.unmount();
  render(<PathEntry {...props} path="/someone-elses-change" />);
  fireEvent.blur(screen.getByLabelText("Skills path"));
  await waitFor(() =>
    expect(onSave).toHaveBeenLastCalledWith("/draft", "/original"),
  );
  expect(screen.getByLabelText("Skills path")).toHaveValue("/draft");
});
it("uses the saved canonical path for the next edit", async () => {
  const onSave = vi.fn().mockResolvedValue("/canonical");
  render(
    <PathEntry
      entryKey="canonical-regression"
      label="Canonical skills"
      path="/initial"
      enabled
      onEnable={vi.fn()}
      onSave={onSave}
    />,
  );
  const field = screen.getByLabelText("Canonical skills path");
  fireEvent.change(field, { target: { value: "/alias" } });
  fireEvent.blur(field);
  await waitFor(() => expect(field).toHaveValue("/canonical"));
  fireEvent.change(field, { target: { value: "/second" } });
  fireEvent.blur(field);
  await waitFor(() =>
    expect(onSave).toHaveBeenLastCalledWith("/second", "/canonical"),
  );
});
it("clears only the draft and does not save a blank path", async () => {
  const onSave = vi.fn();
  render(
    <PathEntry
      entryKey="blank-clear"
      label="Clear skills"
      path="/skills"
      enabled
      onEnable={vi.fn()}
      onSave={onSave}
    />,
  );
  fireEvent.click(screen.getByLabelText("Clear Clear skills path"));
  expect(screen.getByLabelText("Clear skills path")).toHaveValue("");
  fireEvent.blur(screen.getByLabelText("Clear skills path"));
  await waitFor(() =>
    expect(screen.getByRole("alert")).toHaveTextContent("Enter a path"),
  );
  expect(onSave).not.toHaveBeenCalled();
});
it("retains the draft when removal fails and the entry remounts", async () => {
  const props = {
    entryKey: "failed-removal",
    label: "Failed removal",
    path: "/original",
    enabled: true,
    onEnable: vi.fn(),
    onSave: vi.fn().mockRejectedValue(new Error("CAS conflict")),
    onRemove: vi.fn().mockRejectedValue(new Error("Offline")),
  };
  const view = render(<PathEntry {...props} />);
  fireEvent.change(screen.getByLabelText("Failed removal path"), {
    target: { value: "/draft-removal" },
  });
  fireEvent.click(screen.getByLabelText("Remove Failed removal"));
  await screen.findByRole("alert");
  view.unmount();
  render(<PathEntry {...props} />);
  expect(screen.getByLabelText("Failed removal path")).toHaveValue(
    "/draft-removal",
  );
});
