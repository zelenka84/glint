import { useCallback, useEffect, useMemo, useState } from "react";
import { api, type ContentKind, type Instance } from "../api";
import { B3D, Icon, flavourOf } from "../ui";
import { ContentBrowser, KIND_LABEL } from "./ContentBrowser";

const KINDS: ContentKind[] = ["mod", "resourcepack", "shader"];

/** Modrinth catalog for mods, resource packs and shaders, installing into a chosen instance. */
export function Discover({ instances, selected, onSelect, onCreate }: {
  instances: Instance[];
  selected: string | null;
  onSelect: (name: string) => void;
  onCreate: () => void;
}) {
  const [kind, setKind] = useState<ContentKind>("mod");
  const [installed, setInstalled] = useState<Set<string>>(new Set());
  const [notice, setNotice] = useState<{ tone: "ok" | "bad"; text: string } | null>(null);

  // Mods need a loader; packs work in any instance. Horizon takes resource packs only.
  const eligible = useMemo(
    () => instances.filter((i) => (kind !== "mod" || i.loader) && (i.engine !== "horizon" || kind === "resourcepack")),
    [instances, kind],
  );
  const instance = eligible.find((i) => i.name === selected) ?? eligible[0] ?? null;

  const refresh = useCallback(() => {
    if (!instance) return;
    api.installedContent(instance.name, kind).then(
      (items) => setInstalled(new Set(items.flatMap((m) => (m.projectId ? [m.projectId] : [])))),
      (e) => setNotice({ tone: "bad", text: String(e) }),
    );
  }, [instance, kind]);

  useEffect(refresh, [refresh]);

  return (
    <div className="screen discover">
      <div className="screen-head">
        <h1 className="pix">Discover</h1>
        {instance && (
          <label className="field inline">
            <span>Install into</span>
            <select className="select px2" value={instance.name} onChange={(e) => onSelect(e.target.value)}>
              {eligible.map((i) => (
                <option key={i.name} value={i.name}>
                  {i.name} · {flavourOf(i, false)} {i.gameVersion}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      <div className="tabs" role="tablist">
        {KINDS.map((k) => (
          <button key={k} role="tab" aria-selected={kind === k} className={`tab px2 ${kind === k ? "active" : ""}`} onClick={() => setKind(k)}>
            <Icon name={k === "mod" ? "mods" : k === "resourcepack" ? "brush" : "sun"} />
            <span className="pix">{KIND_LABEL[k].many}</span>
          </button>
        ))}
      </div>

      {notice && (
        <div className={`notice px2 ${notice.tone === "ok" ? "ok" : ""}`} role="status">
          <Icon name={notice.tone === "ok" ? "check" : "warn"} />
          <span className="grow">{notice.text}</span>
          <button className="icon-btn" aria-label="Dismiss" onClick={() => setNotice(null)}>
            <Icon name="close" size={12} />
          </button>
        </div>
      )}

      {instance ? (
        <ContentBrowser
          key={`${instance.name}-${kind}`}
          instance={instance.name}
          kind={kind}
          installed={installed}
          onError={(text) => setNotice({ tone: "bad", text })}
          onInstalled={(titles, hit) => {
            setNotice({
              tone: "ok",
              text: titles.length ? `Installed ${titles.join(" + ")} into ${instance.name}` : `${hit.title} is already up to date`,
            });
            refresh();
          }}
        />
      ) : (
        <div className="empty px4">
          <p className="pix">{kind === "mod" ? "Mods need a modded instance" : "Create an instance first"}</p>
          <p className="muted">
            {kind === "mod"
              ? "Create an instance with Fabric, Quilt, Forge or NeoForge, then add mods from Modrinth here."
              : "Resource packs and shaders install into an instance."}
          </p>
          <div className="empty-action">
            <B3D variant="accent" onClick={onCreate}>
              <Icon name="plus" />
              <span className="pix">New instance</span>
            </B3D>
          </div>
        </div>
      )}
    </div>
  );
}
