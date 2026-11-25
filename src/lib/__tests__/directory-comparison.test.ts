import { expect, it } from "vitest";
import { directoryRows, fileStates } from "../directory-comparison";
import type { ManagedSkill } from "../environments";

it("names conflicts across three environments and preserves unrelated file states", () => {
  const skill = {
    members: ["a", "b", "c"].map((id) => ({
      id,
      fingerprint: id,
      previous: "base",
      excluded: false,
      error: null,
    })),
    revisions: [
      {
        snapshot: {
          fingerprint: "base",
          files: { "SKILL.md": [1], same: [0] },
        },
      },
      ...["a", "b", "c"].map((id, i) => ({
        snapshot: {
          fingerprint: id,
          files: { "SKILL.md": [i + 1], same: [0] },
        },
      })),
    ],
  } as unknown as ManagedSkill;
  expect(fileStates(skill, "SKILL.md").map((v) => v.state)).toEqual([
    "Conflict",
    "Conflict",
    "Conflict",
  ]);
  expect(fileStates(skill, "same").map((v) => v.state)).toEqual([
    "Unchanged",
    "Unchanged",
    "Unchanged",
  ]);
});
it("includes nested directory nodes", () => {
  expect(
    directoryRows(["SKILL.md", "scripts/check.py"]).map((v) => [
      v.path,
      v.directory,
    ]),
  ).toEqual([
    ["scripts", true],
    ["scripts/check.py", false],
    ["SKILL.md", false],
  ]);
});
