import { useEffect, useRef, useState } from "react";
import { api, type Instance } from "../api";

/** Free text kept in the instance folder, saved as you type. */
export function NotesTab({ instance }: { instance: Instance }) {
  const [text, setText] = useState<string | null>(null);
  const [state, setState] = useState<"saved" | "saving" | "error">("saved");
  const timer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const latest = useRef("");

  useEffect(() => {
    setText(null);
    api.readNotes(instance.name).then(
      (t) => {
        latest.current = t;
        setText(t);
      },
      () => setText(""),
    );
  }, [instance.name]);

  // Flush a pending save when leaving the tab.
  useEffect(
    () => () => {
      if (timer.current) {
        clearTimeout(timer.current);
        api.saveNotes(instance.name, latest.current);
      }
    },
    [instance.name],
  );

  function change(value: string) {
    setText(value);
    latest.current = value;
    setState("saving");
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      timer.current = undefined;
      api.saveNotes(instance.name, value).then(
        () => setState("saved"),
        () => setState("error"),
      );
    }, 600);
  }

  const words = text?.trim() ? text.trim().split(/\s+/).length : 0;

  return (
    <div className="tab-pane">
      <div className="pane-head">
        <h2 className="pix">Notes</h2>
        <div className="grow" />
        <span className={`status-dot ${state === "saved" ? "ok" : state === "saving" ? "warn" : "bad"}`}>
          {state === "saved" ? "Saved" : state === "saving" ? "Saving" : "Could not save"}
        </span>
      </div>
      {text === null ? (
        <div className="skeleton notes-skeleton px4" />
      ) : (
        <textarea
          className="notes px4"
          value={text}
          spellCheck
          placeholder={"Seeds, coordinates, server IPs, which mods do what…\n\nNotes stay with this instance."}
          onChange={(e) => change(e.target.value)}
        />
      )}
      <span className="muted small">{words} words · stored in notes.md inside the instance folder</span>
    </div>
  );
}
