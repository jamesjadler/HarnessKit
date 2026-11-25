import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConnectionGuide, connectionIssue } from "../connection-guide";

describe("connection guidance", () => {
  it("routes authentication and service failures to different next steps", () => {
    expect(connectionIssue("Permission denied (publickey)")?.title).toBe(
      "SSH sign-in needs setup",
    );
    expect(connectionIssue("connect: Connection refused")?.title).toBe(
      "SSH service is not accepting connections",
    );
    expect(connectionIssue("python3: command not found")?.title).toBe(
      "Python 3 is unavailable",
    );
    expect(connectionIssue("Unrecognized failure")?.title).toBe(
      "Connection needs attention",
    );
  });
  it("offers a real retry without claiming successful checks", () => {
    const retry = vi.fn();
    render(
      <ConnectionGuide
        kind="tailscale"
        context="MacBook Air"
        error="Permission denied (publickey)"
        onRetry={retry}
      />,
    );
    expect(screen.getByText("SSH sign-in needs setup")).toBeInTheDocument();
    expect(screen.queryByText("Network passed")).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Check again" }));
    expect(retry).toHaveBeenCalledOnce();
  });
  it("does not ask for SSH setup for a mounted folder", () => {
    render(<ConnectionGuide kind="local" context="Mounted folder" />);
    expect(screen.queryByText("Need an SSH key?")).not.toBeInTheDocument();
    expect(
      screen.getByText(/No SSH or Python installation/),
    ).toBeInTheDocument();
  });
});
