import type { SVGProps } from "react";

/** Split document with a dotted divider, shared by all comparison entry points. */
export function DiffIcon({
  size = 16,
  strokeWidth = 1.7,
  ...props
}: SVGProps<SVGSVGElement> & { size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...props}
    >
      <path d="M9 3H4a1 1 0 0 0-1 1v16a1 1 0 0 0 1 1h5M15 3h5a1 1 0 0 1 1 1v16a1 1 0 0 1-1 1h-5" />
      <path d="M12 3v18" strokeDasharray="1 3" />
      <path d="M6 8h2M6 12h2M6 16h2M16 8h2M16 12h2M16 16h2" />
    </svg>
  );
}
