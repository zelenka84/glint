import { useEffect, useMemo, useRef, useState } from "react";
import type { Instance } from "../api";
import type { LogEntry, Session } from "../session";
import { Icon } from "../ui";
import { LogView, NEXT_MATCH_EVENT, Toggle, useCopied } from "./LogView";

type Level = "all" | "warn" | "error";

const isError = (e: LogEntry) => e.level === "ERROR" || e.level === "FATAL";

export function LogTab({ instance, session, onClear }: { instance: Instance; session: Session | undefined; onClear: () => void }) {
  const log = session?.log ?? [];
  const [level, setLevel] = useState<Level>("all");
  const [follow, setFollow] = useState(true);
  const [wrap, setWrap] = useState(true);
  const [colors, setColors] = useState(true);
  const [query, setQuery] = useState("");
  const [copied, copy] = useCopied();

  const counts = useMemo(
    () => ({ warn: log.filter((e) => e.level === "WARN").length, error: log.filter(isError).length }),
    [log],
  );
  const shown = useMemo(
    () => log.filter((e) => level === "all" || (level === "warn" ? e.level === "WARN" : isError(e))),
    [log, level],
  );

  const phase = session?.phase.kind ?? "idle";
  const status =
    phase === "running" || instance.running
      ? { text: "Running", tone: "ok" }
      : phase === "busy"
        ? { text: "Starting", tone: "warn" }
        : phase === "failed"
          ? { text: "Crashed or failed", tone: "bad" }
          : { text: log.length ? "Stopped" : "Not started yet", tone: "idle" };

  return (
    <div className="tab-pane log-tab">
      <div className="pane-head">
        <h2 className="pix">Game log</h2>
        <span className={`status-dot ${status.tone}`}>{status.text}</span>
        <div className="grow" />
        <button className="tool-btn px2" onClick={() => copy(shown.map((e) => `[${e.level}] ${e.message}`).join("\n"))} disabled={!shown.length}>
          <Icon name={copied ? "check" : "copy"} /> {copied ? "Copied" : "Copy"}
        </button>
        <button className="tool-btn px2" onClick={onClear} disabled={!log.length}>
          <Icon name="trash" /> Clear
        </button>
      </div>

      <div className="log-toolbar">
        <div className="segmented px2" role="radiogroup" aria-label="Show">
          <button role="radio" aria-checked={level === "all"} className={level === "all" ? "on" : ""} onClick={() => setLevel("all")}>
            All <span className="count">{log.length}</span>
          </button>
          <button role="radio" aria-checked={level === "warn"} className={level === "warn" ? "on" : ""} onClick={() => setLevel("warn")}>
            Warnings <span className="count warn">{counts.warn}</span>
          </button>
          <button role="radio" aria-checked={level === "error"} className={level === "error" ? "on" : ""} onClick={() => setLevel("error")}>
            Errors <span className="count bad">{counts.error}</span>
          </button>
        </div>
        <div className="grow" />
        <Toggle on={follow} onChange={setFollow} icon="follow" label="Follow" />
        <Toggle on={wrap} onChange={setWrap} icon="wrap" label="Wrap" />
        <Toggle on={colors} onChange={setColors} icon="brush" label="Colors" />
      </div>

      <LogView
        lines={shown}
        wrap={wrap}
        colors={colors}
        follow={follow}
        query={query}
        empty={log.length ? "No lines at this level." : "The game's output appears here when it starts."}
      />
      <SearchBar query={query} onQuery={setQuery} />
    </div>
  );
}

export function SearchBar({ query, onQuery }: { query: string; onQuery: (q: string) => void }) {
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.key.toLowerCase() === "f") {
        e.preventDefault();
        input.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  return (
    <label className="search slim px2">
      <Icon name="search" />
      <input
        ref={input}
        value={query}
        placeholder="Search the log  (Ctrl+F, Enter for next)"
        onChange={(e) => onQuery(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && window.dispatchEvent(new Event(NEXT_MATCH_EVENT))}
      />
      {query && (
        <button className="icon-btn" aria-label="Clear search" onClick={() => onQuery("")}>
          <Icon name="close" size={12} />
        </button>
      )}
    </label>
  );
}
