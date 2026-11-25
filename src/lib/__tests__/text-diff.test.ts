import { describe, expect, it } from "vitest";
import { diffText, transferHunk } from "../text-diff";

describe("text comparison", () => {
  it("transfers only the chosen change and preserves later independent edits", () => {
    const left = "one\nold\ncommon\nleft\n";
    const right = "one\nnew\ncommon\nright\n";
    const changes = diffText(left, right);
    expect(changes).toHaveLength(2);
    expect(transferHunk(left, right, changes[0], "left")).toBe(
      "one\nold\ncommon\nright\n",
    );
    expect(transferHunk(left, right, changes[1], "right")).toBe(
      "one\nold\ncommon\nright\n",
    );
  });
  it("preserves CRLF, trailing newline and empty files", () => {
    for (const [a, b] of [
      ["", "new\r\n"],
      ["a\r\nb\r\n", "a\r\n"],
      ["a\n", "a"],
      ["x\nx\ny", "x\ny"],
    ]) {
      let result = b;
      for (const hunk of diffText(a, b).reverse())
        result = transferHunk(a, result, hunk, "left");
      expect(result).toBe(a);
    }
  });
  it("reports no changes for identical contents", () =>
    expect(diffText("a\n", "a\n")).toEqual([]));
  it("handles large files without an unbounded quadratic allocation", () => {
    const left = "line\n".repeat(10000) + "left";
    const right = "line\n".repeat(10000) + "right";
    expect(diffText(left, right)).toHaveLength(1);
  });
});
