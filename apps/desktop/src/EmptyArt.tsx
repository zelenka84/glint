import type { ReactNode } from "react";

/** Small pixel scenes for empty states, drawn in the theme's colours. */
const ARTS = {
  // An open, empty chest with a spark above it.
  chest: (
    <>
      <rect x="6" y="12" width="20" height="10" fill="var(--raise)" />
      <rect x="6" y="12" width="20" height="2" fill="var(--line)" />
      <rect x="6" y="20" width="20" height="2" fill="var(--edge)" />
      <rect x="15" y="14" width="2" height="3" fill="var(--accent)" />
      <rect x="4" y="6" width="24" height="5" fill="var(--card)" />
      <rect x="4" y="6" width="24" height="1" fill="var(--line)" />
      <rect x="8" y="12" width="16" height="1" fill="var(--edge)" />
      <g className="art-spark" fill="var(--accent)">
        <rect x="15" y="1" width="2" height="1" />
        <rect x="15" y="3" width="2" height="1" />
        <rect x="14" y="2" width="1" height="1" />
        <rect x="17" y="2" width="1" height="1" />
      </g>
    </>
  ),
  // A map with a question mark: nothing found.
  map: (
    <>
      <rect x="5" y="4" width="22" height="16" fill="var(--raise)" />
      <rect x="5" y="4" width="22" height="1" fill="var(--line)" />
      <rect x="12" y="4" width="1" height="16" fill="var(--line)" />
      <rect x="19" y="4" width="1" height="16" fill="var(--line)" />
      <rect x="7" y="8" width="3" height="1" fill="var(--muted)" />
      <rect x="21" y="14" width="4" height="1" fill="var(--muted)" />
      <g className="art-spark" fill="var(--accent)">
        <rect x="14" y="7" width="4" height="1" />
        <rect x="17" y="8" width="1" height="2" />
        <rect x="15" y="10" width="2" height="1" />
        <rect x="15" y="11" width="1" height="2" />
        <rect x="15" y="14" width="1" height="1" />
      </g>
      <rect x="5" y="20" width="22" height="1" fill="var(--edge)" />
    </>
  ),
  // A picture frame with hills and a sun: no screenshots.
  picture: (
    <>
      <rect x="4" y="3" width="24" height="18" fill="var(--line)" />
      <rect x="6" y="5" width="20" height="14" fill="var(--card)" />
      <rect x="6" y="14" width="20" height="5" fill="var(--raise)" />
      <rect x="9" y="12" width="5" height="2" fill="var(--raise)" />
      <rect x="17" y="11" width="6" height="3" fill="var(--raise)" />
      <rect className="art-spark" x="20" y="7" width="3" height="3" fill="var(--accent)" />
      <rect x="4" y="21" width="24" height="1" fill="var(--edge)" />
    </>
  ),
  // A moon and stars over a quiet block: nothing happening yet.
  night: (
    <>
      <rect x="18" y="3" width="6" height="6" fill="var(--accent)" />
      <rect x="21" y="3" width="3" height="3" fill="var(--panel)" />
      <rect className="art-spark" x="7" y="5" width="1" height="1" fill="var(--text2)" />
      <rect className="art-spark late" x="12" y="2" width="1" height="1" fill="var(--text2)" />
      <rect className="art-spark" x="27" y="10" width="1" height="1" fill="var(--text2)" />
      <rect x="10" y="13" width="12" height="9" fill="var(--raise)" />
      <rect x="10" y="13" width="12" height="2" fill="var(--line)" />
      <rect x="14" y="17" width="4" height="5" fill="var(--edge)" />
    </>
  ),
};

export type ArtName = keyof typeof ARTS;

export function EmptyArt({ art, size = 112 }: { art: ArtName; size?: number }) {
  return (
    <svg className="empty-art" width={size} height={(size * 24) / 32} viewBox="0 0 32 24" shapeRendering="crispEdges" aria-hidden="true">
      {ARTS[art]}
    </svg>
  );
}

/** The standard empty panel: a pixel scene, a title, a line of help and optional actions. */
export function EmptyState({ art, title, children, compact }: { art: ArtName; title: string; children?: ReactNode; compact?: boolean }) {
  return (
    <div className={`empty px4 ${compact ? "compact" : ""}`}>
      <EmptyArt art={art} size={compact ? 72 : 112} />
      <p className="pix">{title}</p>
      {children}
    </div>
  );
}
