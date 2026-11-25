import { diffText } from "./text-diff";
export interface MergeBlock {
  texts: string[];
  changed: boolean;
}
/** Union overlapping edits in base coordinates, then consume every source once.
 * Text (including CRLF and missing final newline) is never normalized. */
export function mergeBlocks(texts: string[]): MergeBlock[] {
  if (!texts.length) return [];
  const edits = texts.map((text) => diffText(texts[0], text));
  const regions: { start: number; end: number }[] = [];
  for (const h of edits
    .flat()
    .sort((a, b) => a.leftStart - b.leftStart || a.leftEnd - b.leftEnd)) {
    const last = regions[regions.length - 1];
    if (last && h.leftStart <= last.end)
      last.end = Math.max(last.end, h.leftEnd);
    else regions.push({ start: h.leftStart, end: h.leftEnd });
  }
  const result: MergeBlock[] = [];
  const cursors = texts.map(() => 0);
  let base = 0;
  const consume = (lengths: number[], changed: boolean) => {
    const slices = texts.map((text, i) => {
      const start = cursors[i];
      cursors[i] += lengths[i];
      return text.slice(start, cursors[i]);
    });
    result.push({ texts: slices, changed });
  };
  for (const region of regions) {
    if (region.start > base)
      consume(
        texts.map(() => region.start - base),
        false,
      );
    consume(
      edits.map(
        (list) =>
          region.end -
          region.start +
          list
            .filter(
              (h) => h.leftStart >= region.start && h.leftEnd <= region.end,
            )
            .reduce(
              (n, h) =>
                n + (h.rightEnd - h.rightStart) - (h.leftEnd - h.leftStart),
              0,
            ),
      ),
      true,
    );
    base = region.end;
  }
  if (base < texts[0].length)
    consume(
      texts.map((text, i) => text.length - cursors[i]),
      false,
    );
  return result.length ? result : [{ texts: [...texts], changed: false }];
}
export const lineCount = (text: string) => (text.match(/\n/g) ?? []).length;
/** Small word-level prefix/suffix emphasis without changing source text. */
export function changedSpan(text: string, other: string): [number, number] {
  let start = 0,
    end = 0;
  while (
    start < text.length &&
    start < other.length &&
    text[start] === other[start]
  )
    start++;
  while (
    end < text.length - start &&
    end < other.length - start &&
    text[text.length - 1 - end] === other[other.length - 1 - end]
  )
    end++;
  return [start, text.length - end];
}
