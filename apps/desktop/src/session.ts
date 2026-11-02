import type { PlayEvent } from "./api";

export type LogEntry = { id: number; level: string; message: string };

export type Phase =
  | { kind: "idle" }
  | { kind: "busy"; label: string; done: number; total: number }
  | { kind: "running" }
  | { kind: "failed"; message: string };

/** What the UI knows about one instance's current or last launch. */
export type Session = { phase: Phase; log: LogEntry[] };

const MAX_LOG = 1000;
let nextId = 0;

const mb = (bytes: number) => (bytes / 1e6).toFixed(0);

function nextPhase(event: PlayEvent, previous: Phase): Phase {
  switch (event.type) {
    case "resolving":
      return { kind: "busy", label: `Checking ${event.version}`, done: 0, total: 1 };
    case "downloading": {
      const what = event.what === "java" ? "Java" : "game files";
      const label = `Downloading ${what} · ${mb(event.doneBytes)}/${mb(event.totalBytes)} MB`;
      return { kind: "busy", label, done: event.doneBytes, total: Math.max(event.totalBytes, 1) };
    }
    case "installingLoader":
      return { kind: "busy", label: `Installing ${event.name} · first time can take a minute`, done: 1, total: 2 };
    case "preparing":
      return { kind: "busy", label: "Preparing", done: 1, total: 1 };
    case "started":
      return { kind: "running" };
    case "exited":
      return event.code === 0 || event.code === null
        ? { kind: "idle" }
        : { kind: "failed", message: `The game exited with code ${event.code}. The log below shows why.` };
    case "failed":
      return { kind: "failed", message: event.message };
    case "killed":
      return { kind: "idle" };
    case "log":
      return previous;
  }
}

export function applyEvent(session: Session | undefined, event: PlayEvent): Session {
  const current = session ?? { phase: { kind: "idle" }, log: [] };
  const fresh = event.type === "resolving";
  let log = fresh ? [] : current.log;
  if (event.type === "log") {
    log = [...log.slice(-(MAX_LOG - 1)), { id: nextId++, level: event.level, message: event.message }];
  } else if (event.type === "killed") {
    log = [...log, { id: nextId++, level: "WARN", message: "Glint killed the game at your request." }];
  }
  return { phase: nextPhase(event, current.phase), log };
}
