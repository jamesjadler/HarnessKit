import { describe, expect, it } from "vitest";
import { mergeBlocks } from "../merge-blocks";
describe("merge alignment", () => {
  it.each([
    ["a\nb\nc\n", "a\nnew\nb\nc\n", "a\nc\n"],
    ["", "one\n", "two"],
    ["a\r\nb\r\n", "a\r\nc", "prefix\na\r\nb\r\n"],
    ["a\nb\nc\nd\n", "a\nB\nc\nd\n", "a\nB\nC\nd\n"],
    ["same", "same", "same"],
  ])("reconstructs every source exactly", (...texts) => {
    const blocks = mergeBlocks(texts);
    texts.forEach((text, i) =>
      expect(blocks.map((b) => b.texts[i]).join("")).toBe(text),
    );
  });
  it("leaves matching context outside changed blocks", () => {
    const b = mergeBlocks(["a\nb\nc\n", "a\nB\nc\n"]);
    expect(b.map((x) => x.changed)).toEqual([false, true, false]);
    expect(b[1].texts).toEqual(["b\n", "B\n"]);
  });
});

it("reconstructs varied three-source insertions and removals", () => {
  let seed = 7;
  const random = () => {
    seed = (seed * 1664525 + 1013904223) >>> 0;
    return seed / 4294967296;
  };
  for (let run = 0; run < 300; run++) {
    const texts = Array.from({ length: 3 }, () =>
      Array.from(
        { length: Math.floor(random() * 12) },
        () => ["a\n", "b\n", "c\r\n", "\n"][Math.floor(random() * 4)],
      ).join(""),
    );
    const blocks = mergeBlocks(texts);
    texts.forEach((text, i) =>
      expect(blocks.map((b) => b.texts[i]).join("")).toBe(text),
    );
  }
});
