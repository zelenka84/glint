import { Fragment, type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { Icon, type IconName } from "../ui";

export type LogLine = { id: number; level: string; message: string };

/** Fired by a search box when the player presses Enter: jump to the next match. */
export const NEXT_MATCH_EVENT = "glint-log-next";

function highlight(text: string, query: string): ReactNode {
  if (!query) return text;
  const lower = text.toLowerCase();
  const needle = query.toLowerCase();
  const parts: ReactNode[] = [];
  let at = 0;
  for (let i = lower.indexOf(needle); i !== -1; i = lower.indexOf(needle, at)) {
    parts.push(<Fragment key={at}>{text.slice(at, i)}</Fragment>);
    parts.push(<mark key={`m${i}`}>{text.slice(i, i + needle.length)}</mark>);
    at = i + needle.length;
  }
  parts.push(<Fragment key="end">{text.slice(at)}</Fragment>);
  return parts;
}

export function LogView({ lines, wrap, colors, follow, query, empty }: {
  lines: LogLine[];
  wrap: boolean;
  colors: boolean;
  follow: boolean;
  query: string;
  empty: string;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [current, setCurrent] = useState(-1);
  const q = query.trim();
  const matches = q ? lines.filter((l) => l.message.toLowerCase().includes(q.toLowerCase())).map((l) => l.id) : [];

  useEffect(() => {
    if (follow && !q && box.current) box.current.scrollTop = box.current.scrollHeight;
  }, [lines, follow, q]);

  useEffect(() => setCurrent(-1), [q]);

  const next = useCallback(() => {
    if (!matches.length) return;
    const index = (matches.indexOf(current) + 1) % matches.length;
    const id = matches[index];
    setCurrent(id);
    box.current?.querySelector(`[data-line="${id}"]`)?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [matches, current]);

  useEffect(() => {
    window.addEventListener(NEXT_MATCH_EVENT, next);
    return () => window.removeEventListener(NEXT_MATCH_EVENT, next);
  }, [next]);

  return (
    <div className={`log-view px4 ${wrap ? "wrap" : "nowrap"} ${colors ? "colors" : ""}`} ref={box}>
      {q && (
        <span className="match-count pix">
          {matches.length ? `${Math.max(matches.indexOf(current) + 1, 0)}/${matches.length}` : "no matches"}
        </span>
      )}
      {lines.length === 0 ? (
        <p className="log-empty muted">{empty}</p>
      ) : (
        lines.map((line) => (
          <pre
            key={line.id}
            data-line={line.id}
            className={`line ${line.level.toLowerCase()} ${line.id === current ? "current" : ""}`}
          >
            {highlight(line.message, q)}
          </pre>
        ))
      )}
    </div>
  );
}

export function Toggle({ on, onChange, icon, label }: { on: boolean; onChange: (on: boolean) => void; icon: IconName; label: string }) {
  return (
    <button className={`toggle px2 ${on ? "on" : ""}`} aria-pressed={on} onClick={() => onChange(!on)}>
      <Icon name={icon} />
      <span>{label}</span>
    </button>
  );
}

/** Copies text and reports success for a moment. */
export function useCopied(): [boolean, (text: string) => void] {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  return [
    copied,
    (text: string) => {
      navigator.clipboard.writeText(text).then(() => {
        setCopied(true);
        clearTimeout(timer.current);
        timer.current = setTimeout(() => setCopied(false), 1500);
      });
    },
  ];
}
