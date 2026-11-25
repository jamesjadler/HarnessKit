/** Character offsets preserve original line endings and missing final newlines. */
export interface TextHunk {
  leftStart: number;
  leftEnd: number;
  rightStart: number;
  rightEnd: number;
}
const lines = (text: string) => text.match(/[^\n]*\n|[^\n]+$/g) ?? [];
export function diffText(left: string, right: string): TextHunk[] {
  if (left === right) return [];
  const a = lines(left),
    b = lines(right);
  let prefix = 0,
    suffix = 0;
  while (prefix < a.length && prefix < b.length && a[prefix] === b[prefix])
    prefix++;
  while (
    suffix < a.length - prefix &&
    suffix < b.length - prefix &&
    a[a.length - 1 - suffix] === b[b.length - 1 - suffix]
  )
    suffix++;
  const x = a.slice(prefix, a.length - suffix),
    y = b.slice(prefix, b.length - suffix);
  let ai = a.slice(0, prefix).join("").length,
    bi = ai;
  if ((x.length + 1) * (y.length + 1) > 2_000_000)
    return [
      {
        leftStart: ai,
        leftEnd: left.length - a.slice(a.length - suffix).join("").length,
        rightStart: bi,
        rightEnd: right.length - b.slice(b.length - suffix).join("").length,
      },
    ];
  const width = y.length + 1,
    dp = new Uint32Array((x.length + 1) * width);
  for (let i = x.length - 1; i >= 0; i--)
    for (let j = y.length - 1; j >= 0; j--)
      dp[i * width + j] =
        x[i] === y[j]
          ? 1 + dp[(i + 1) * width + j + 1]
          : Math.max(dp[(i + 1) * width + j], dp[i * width + j + 1]);
  const hunks: TextHunk[] = [];
  let pending: TextHunk | undefined;
  let i = 0,
    j = 0;
  const flush = () => {
    if (pending) {
      pending.leftEnd = ai;
      pending.rightEnd = bi;
      hunks.push(pending);
      pending = undefined;
    }
  };
  while (i < x.length || j < y.length) {
    if (i < x.length && j < y.length && x[i] === y[j]) {
      flush();
      ai += x[i++].length;
      bi += y[j++].length;
      continue;
    }
    pending ??= { leftStart: ai, leftEnd: ai, rightStart: bi, rightEnd: bi };
    if (
      i < x.length &&
      (j === y.length || dp[(i + 1) * width + j] >= dp[i * width + j + 1])
    )
      ai += x[i++].length;
    else bi += y[j++].length;
  }
  flush();
  return hunks;
}
export function transferHunk(
  left: string,
  right: string,
  hunk: TextHunk,
  source: "left" | "right",
): string {
  return source === "left"
    ? right.slice(0, hunk.rightStart) +
        left.slice(hunk.leftStart, hunk.leftEnd) +
        right.slice(hunk.rightEnd)
    : left.slice(0, hunk.leftStart) +
        right.slice(hunk.rightStart, hunk.rightEnd) +
        left.slice(hunk.leftEnd);
}
