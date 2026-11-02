import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type Instance, type LogFile } from "../api";
import { Icon, formatBytes } from "../ui";
import { SearchBar } from "./LogTab";
import { type LogLine, LogView, Toggle, useCopied } from "./LogView";

const LEVEL = /\[[^\]]*?\/(INFO|WARN|ERROR|FATAL|DEBUG)\]/;

/** Splits raw log text into lines. Log levels come from log4j's `[thread/LEVEL]` and
 * carry over to continuation lines such as stack traces; crash reports mark exceptions. */
function parse(text: string, crash: boolean): LogLine[] {
  let level = "INFO";
  return text.split(/\r?\n/).map((message, id) => {
    if (crash) return { id, level: /exception|error|caused by|^\s+at /i.test(message) ? "ERROR" : "INFO", message };
    const m = LEVEL.exec(message);
    if (m) level = m[1];
    return { id, level, message };
  });
}

function when(seconds: number) {
  return new Date(seconds * 1000).toLocaleString(undefined, { dateStyle: "short", timeStyle: "short" });
}

export function LogsTab({ instance }: { instance: Instance }) {
  const [files, setFiles] = useState<LogFile[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [text, setText] = useState<string | null>(null);
  const [wrap, setWrap] = useState(true);
  const [colors, setColors] = useState(true);
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [copied, copy] = useCopied();

  const refresh = useCallback(() => {
    api.listLogFiles(instance.name).then(
      (list) => {
        setFiles(list);
        setSelected((s) => (s && list.some((f) => f.id === s) ? s : list[0]?.id ?? null));
      },
      (e) => setError(String(e)),
    );
  }, [instance.name]);
  useEffect(refresh, [refresh]);

  useEffect(() => {
    setText(null);
    if (!selected) return;
    let cancelled = false;
    api.readLogFile(instance.name, selected).then(
      (t) => !cancelled && setText(t),
      (e) => !cancelled && setError(String(e)),
    );
    return () => {
      cancelled = true;
    };
  }, [instance.name, selected]);

  const file = files?.find((f) => f.id === selected) ?? null;
  const lines = useMemo(() => (text === null ? [] : parse(text, file?.kind === "crash")), [text, file?.kind]);

  async function remove(id: string) {
    try {
      await api.deleteLogFile(instance.name, id);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function removeOld() {
    try {
      await api.deleteOldLogs(instance.name);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="tab-pane">
      <div className="pane-head">
        <h2 className="pix">Other logs</h2>
        {files && <span className="count px2">{files.length}</span>}
        <div className="grow" />
        <button className="tool-btn px2" onClick={refresh}>
          <Icon name="refresh" /> Refresh
        </button>
        <button className="tool-btn danger px2" onClick={removeOld} disabled={!files || files.length < 2}>
          <Icon name="trash" /> Delete old
        </button>
      </div>
      {error && (
        <div className="notice px2" role="alert">
          <span className="grow">{error}</span>
          <button className="icon-btn" aria-label="Dismiss" onClick={() => setError(null)}>
            <Icon name="close" size={12} />
          </button>
        </div>
      )}

      {files?.length === 0 ? (
        <div className="empty px4">
          <Icon name="file" size={28} />
          <p className="pix">No logs yet</p>
          <p className="muted">The game writes logs and crash reports here after it runs.</p>
        </div>
      ) : (
        <div className="logs-split">
          <div className="log-files">
            {files?.map((f) => (
              <button key={f.id} className={`log-file px2 ${f.id === selected ? "active" : ""} ${f.kind}`} onClick={() => setSelected(f.id)}>
                <Icon name={f.kind === "crash" ? "warn" : "file"} />
                <span className="grow">
                  <span className="log-file-name">{f.name}</span>
                  <span className="muted small">
                    {when(f.modified)} · {formatBytes(f.size)}
                  </span>
                </span>
              </button>
            ))}
          </div>
          <div className="log-reader">
            <div className="log-toolbar">
              <b className="grow log-file-title">{file?.kind === "crash" ? "Crash report" : file?.name}</b>
              <Toggle on={wrap} onChange={setWrap} icon="wrap" label="Wrap" />
              <Toggle on={colors} onChange={setColors} icon="brush" label="Colors" />
              <button className="tool-btn px2" onClick={() => copy(text ?? "")} disabled={!text}>
                <Icon name={copied ? "check" : "copy"} /> {copied ? "Copied" : "Copy"}
              </button>
              <button className="tool-btn danger px2" onClick={() => selected && remove(selected)} disabled={!selected} aria-label="Delete this log">
                <Icon name="trash" />
              </button>
            </div>
            {text === null ? (
              <div className="skeleton log-skeleton px4" />
            ) : (
              <LogView lines={lines} wrap={wrap} colors={colors} follow={false} query={query} empty="This log is empty." />
            )}
            <SearchBar query={query} onQuery={setQuery} />
          </div>
        </div>
      )}
    </div>
  );
}
