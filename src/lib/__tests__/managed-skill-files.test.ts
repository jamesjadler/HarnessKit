import { expect, it } from "vitest";
import { selectSkillFiles, isSkillFile } from "../managed-skill-files";
import type { ManagedFile } from "../managed-files";
const file = (
  id: string,
  path: string,
  agent = "codex",
  category = "skills",
): ManagedFile => ({
  id,
  agent,
  category,
  scope: "global",
  scope_path: null,
  logical_path: path,
  file_name: path.split("/").pop()!,
  variant_name: null,
  original_id: null,
  revisions: [],
  copies: [
    {
      id,
      environment_id: "local",
      environment: "Ubuntu",
      machine: "Desktop",
      path,
      provider_id: agent,
      content: "",
      fingerprint: "a",
      accepted_fingerprint: "a",
      error: null,
      enabled: true,
      pending: false,
    },
  ],
});
it("selects only the selected skill's nested files and independent variants", () => {
  const root = "/home/me/.codex/skills/review";
  const main = file("main", root + "/SKILL.md"),
    nested = file("nested", root + "/references/check.md");
  const variant = {
    ...main,
    id: "variant",
    original_id: "main",
    variant_name: "Strict",
    copies: [],
  };
  const selected = selectSkillFiles(
    [
      main,
      nested,
      variant,
      file("sibling", root + "-other/SKILL.md"),
      file("other-agent", root + "/SKILL.md", "claude"),
      file("config", "/home/me/.codex/config.toml", "codex", "config"),
    ],
    [{ agent: "codex", path: root + "/SKILL.md" }],
  );
  expect(selected.map((f) => f.id)).toEqual(["main", "nested", "variant"]);
});
it("recognizes skill files without excluding configs, instructions or hooks", () => {
  expect(isSkillFile(file("skill", "/skills/a/SKILL.md"))).toBe(true);
  for (const category of ["config", "instructions", "hooks"])
    expect(isSkillFile(file(category, "/agent/file", "codex", category))).toBe(
      false,
    );
});
