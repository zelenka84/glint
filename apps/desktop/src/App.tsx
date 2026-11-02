import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";

type AppInfo = {
  version: string;
  dataDir: string;
};

function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<AppInfo>("app_info").then(setInfo, (e) => setError(String(e)));
  }, []);

  return (
    <main className="shell">
      <h1 className="pix">glint</h1>
      <p className="muted">Phase 1 scaffold: the UI talks to the Rust core.</p>
      <section className="panel px4">
        {error && <p className="error">{error}</p>}
        {!error && !info && <p className="muted">Loading…</p>}
        {info && (
          <dl>
            <dt>Version</dt>
            <dd className="pix">{info.version}</dd>
            <dt>Data folder</dt>
            <dd>{info.dataDir}</dd>
          </dl>
        )}
      </section>
    </main>
  );
}

export default App;
