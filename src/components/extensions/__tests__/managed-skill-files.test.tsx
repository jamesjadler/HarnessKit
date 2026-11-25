import { fireEvent, render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { ManagedSkillFiles } from "../managed-skill-files";
import type { ManagedFile } from "@/lib/managed-files";
it("nests support files and retains direct editing, saving, history and inline variant creation", () => {
  const root = "/home/me/.codex/skills/review";
  const files = ["SKILL.md", "references/check.md"].map(
    (path, i): ManagedFile => ({
      id: `sidebar-${i}`,
      agent: "codex",
      category: "skills",
      scope: "global",
      scope_path: null,
      logical_path: path,
      file_name: path.split("/").pop()!,
      variant_name: null,
      original_id: null,
      revisions: [
        {
          number: 1,
          content: "Example",
          fingerprint: "one",
          created_at: "2026-11-24",
          reason: "initial",
        },
      ],
      copies: [
        {
          id: `copy-${i}`,
          environment_id: "local",
          environment: "Ubuntu",
          machine: "Desktop",
          path: root + "/" + path,
          provider_id: "codex",
          content: "Example",
          fingerprint: "one",
          accepted_fingerprint: "one",
          error: null,
          enabled: true,
          pending: false,
        },
      ],
    }),
  );
  render(
    <ManagedSkillFiles
      files={files}
      roots={[{ agent: "codex", path: root }]}
    />,
  );
  expect(
    screen.getByText("check.md").closest(".managed-skill-directory"),
  ).toHaveTextContent("references");
  fireEvent.click(screen.getByRole("button", { name: /SKILL.md/ }));
  expect(
    screen.getByText("Version history").closest("details"),
  ).not.toHaveAttribute("open");
  const variant = screen.getByRole("button", { name: "Create variant" });
  expect(variant.closest("summary")).toHaveTextContent("Variants");
  fireEvent.change(screen.getByLabelText("SKILL.md contents"), {
    target: { value: "Edited" },
  });
  expect(screen.getByRole("button", { name: "Save & sync" })).toBeEnabled();
});
