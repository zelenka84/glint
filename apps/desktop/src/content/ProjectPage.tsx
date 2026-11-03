import DOMPurify from "dompurify";
import { marked } from "marked";
import { type MouseEvent, useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { openUrl } from "@tauri-apps/plugin-opener";
import { api, type ContentKind, type ModHit, type PageVersion, type ProjectPage as Page } from "../api";
import { B3D, Icon, type IconName, formatBytes } from "../ui";
import { HIDDEN_TAGS, KIND_LABEL, ProjectIcon, compact } from "./ContentBrowser";

type Tab = "description" | "gallery" | "versions";

const SIDES: Record<string, string> = { required: "Required", optional: "Optional", unsupported: "Not needed", unknown: "Unknown" };

/** Markdown from Modrinth, cleaned of scripts and inline handlers. */
function renderMarkdown(source: string): string {
  const html = marked.parse(source, { async: false, gfm: true, breaks: false });
  return DOMPurify.sanitize(html, { ADD_ATTR: ["target"], FORBID_TAGS: ["style", "form", "input", "button"] });
}

/** Links in rendered Markdown open in the system browser instead of the launcher window. */
function openLinksOutside(e: MouseEvent) {
  const link = (e.target as HTMLElement).closest("a");
  if (!link) return;
  e.preventDefault();
  const href = link.getAttribute("href") ?? "";
  if (/^https?:\/\//.test(href)) openUrl(href);
}

function ago(iso: string): string {
  const then = Date.parse(iso);
  if (!then) return "";
  const days = Math.floor((Date.now() - then) / 86_400_000);
  if (days < 1) return "today";
  if (days === 1) return "yesterday";
  if (days < 30) return `${days} days ago`;
  if (days < 365) return `${Math.floor(days / 30)} mo ago`;
  return `${Math.floor(days / 365)} yr ago`;
}

const date = (iso: string) =>
  Date.parse(iso) ? new Date(iso).toLocaleDateString("en", { year: "numeric", month: "short", day: "numeric" }) : "—";

/** A Modrinth project in full: description, gallery, versions and what installing it brings along. */
export function ProjectPage({ instance, kind, hit, onBack, onInstalled, onError }: {
  instance: string;
  kind: ContentKind;
  hit: ModHit;
  onBack: () => void;
  onInstalled: (titles: string[]) => void;
  onError: (message: string) => void;
}) {
  const [page, setPage] = useState<Page | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("description");
  const [busy, setBusy] = useState<string | null>(null);
  const [viewing, setViewing] = useState<number | null>(null);

  function load() {
    return api.contentProject(instance, kind, hit.projectId).then(setPage, (e) => setFailed(String(e)));
  }

  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [instance, kind, hit.projectId]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && viewing === null) onBack();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [viewing, onBack]);

  const body = useMemo(() => (page?.project.body ? renderMarkdown(page.project.body) : ""), [page?.project.body]);

  async function install(versionId?: string) {
    setBusy(versionId ?? "main");
    try {
      onInstalled(await api.installContent(instance, kind, hit.projectId, versionId));
      await load();
    } catch (e) {
      onError(String(e));
    } finally {
      setBusy(null);
    }
  }

  const project = page?.project;
  const banner = project?.gallery.find((g) => g.featured) ?? project?.gallery[0];
  const tags = [...(project?.categories ?? hit.categories), ...(project?.additionalCategories ?? [])].filter((c) => !HIDDEN_TAGS.has(c));
  const required = page?.dependencies.filter((d) => d.required) ?? [];
  const optional = page?.dependencies.filter((d) => !d.required) ?? [];
  const installed = page?.installedVersion ?? null;
  const outdated = !!installed && !!page?.recommended && installed !== page.recommended;
  const noVersion = !!page && page.versions.length === 0;
  const pageUrl = `https://modrinth.com/${kind === "resourcepack" ? "resourcepack" : kind}/${project?.slug ?? hit.slug ?? hit.projectId}`;

  const links: { icon: IconName; label: string; url: string | null | undefined }[] = [
    { icon: "globe", label: "Modrinth page", url: pageUrl },
    { icon: "code", label: "Source code", url: project?.sourceUrl },
    { icon: "bug", label: "Issues", url: project?.issuesUrl },
    { icon: "book", label: "Wiki", url: project?.wikiUrl },
    { icon: "chat", label: "Discord", url: project?.discordUrl },
  ];

  return (
    <div className="project-page">
      <button className="pp-back link" onClick={onBack}>
        <Icon name="chevronLeft" size={12} /> Back to {KIND_LABEL[kind].many.toLowerCase()}
      </button>

      <header className="pp-head px4">
        {banner && <img key={banner.url} className="pp-banner" src={banner.url} alt="" aria-hidden="true" />}
        <div className="pp-head-main">
          <ProjectIcon url={project?.iconUrl ?? hit.iconUrl} size={88} />
          <div className="pp-title">
            <h1 className="pix">{project?.title ?? hit.title}</h1>
            <span className="muted">by {hit.author}</span>
            <p>{project?.description ?? hit.description}</p>
            <div className="pp-stats">
              <span title="Downloads">
                <Icon name="download" size={12} /> {compact.format(project?.downloads ?? hit.downloads)}
              </span>
              {project && (
                <span title="Followers">
                  <Icon name="heart" size={12} /> {compact.format(project.followers)}
                </span>
              )}
              {project?.updated && (
                <span title={`Updated ${date(project.updated)}`}>
                  <Icon name="clock" size={12} /> Updated {ago(project.updated)}
                </span>
              )}
            </div>
          </div>
          <div className="pp-cta">
            {installed && !outdated ? (
              <span className="installed-chip px2">
                <Icon name="check" size={12} /> Installed
              </span>
            ) : (
              <B3D variant={outdated ? "warn" : "accent"} size="lg" disabled={!page || noVersion || !!busy} onClick={() => install()}>
                {busy === "main" ? <span className="spinner small" /> : <Icon name={outdated ? "arrowUp" : "download"} />}
                <span className="pix">{busy === "main" ? "Installing" : outdated ? "Update" : "Install"}</span>
              </B3D>
            )}
            {noVersion && <span className="muted small">No version for this instance</span>}
          </div>
        </div>
        {tags.length > 0 && (
          <div className="pp-tags">
            {tags.map((t) => (
              <span key={t} className="tag-muted px2">
                {t}
              </span>
            ))}
          </div>
        )}
      </header>

      {required.length > 0 && !installed && (
        <section className="pp-deps px4">
          <span className="label pix">INSTALLS TOGETHER WITH</span>
          <div className="pp-dep-list">
            {required.map((d) => (
              <span key={d.projectId} className={`pp-dep px2 ${d.installed ? "have" : ""}`}>
                <ProjectIcon url={d.iconUrl} size={24} />
                {d.title}
                {d.installed && <Icon name="check" size={10} />}
              </span>
            ))}
          </div>
        </section>
      )}

      <div className="tabs" role="tablist">
        {(
          [
            ["description", "notes", "Description"],
            ["gallery", "image", `Gallery${project ? ` · ${project.gallery.length}` : ""}`],
            ["versions", "archive", `Versions${page ? ` · ${page.versions.length}` : ""}`],
          ] as [Tab, IconName, string][]
        ).map(([id, icon, label]) => (
          <button key={id} role="tab" aria-selected={tab === id} className={`tab px2 ${tab === id ? "active" : ""}`} onClick={() => setTab(id)}>
            <Icon name={icon} />
            <span className="pix">{label}</span>
          </button>
        ))}
      </div>

      <div className="pp-body">
        <div className="pp-main px4" key={tab}>
          {failed && <p className="error">{failed}</p>}
          {!page && !failed && (
            <div className="pp-loading">
              <span className="spinner" /> Loading from Modrinth…
            </div>
          )}
          {page && tab === "description" &&
            (body ? (
              <div className="markdown" onClick={openLinksOutside} dangerouslySetInnerHTML={{ __html: body }} />
            ) : (
              <p className="muted">{project?.description}</p>
            ))}
          {page && tab === "gallery" &&
            (project!.gallery.length ? (
              <div className="pp-gallery">
                {project!.gallery.map((g, i) => (
                  <button key={g.url} className="pp-shot px2" onClick={() => setViewing(i)}>
                    <img src={g.url} alt={g.title ?? ""} loading="lazy" />
                    {g.title && <span className="pp-shot-title">{g.title}</span>}
                  </button>
                ))}
              </div>
            ) : (
              <p className="muted">No pictures yet.</p>
            ))}
          {page && tab === "versions" && (
            <VersionList
              versions={page.versions}
              installed={installed}
              recommended={page.recommended}
              busy={busy}
              onInstall={(id) => install(id)}
            />
          )}
        </div>

        <aside className="pp-side">
          <section className="pp-card px4">
            <span className="label pix">LINKS</span>
            {links
              .filter((l) => l.url)
              .map((l) => (
                <button key={l.label} className="pp-link px2" onClick={() => openUrl(l.url!)}>
                  <Icon name={l.icon} size={14} />
                  <span className="grow">{l.label}</span>
                  <Icon name="external" size={10} />
                </button>
              ))}
          </section>
          {project && (
            <section className="pp-card px4">
              <span className="label pix">DETAILS</span>
              <dl className="pp-facts">
                {project.license && (
                  <>
                    <dt>License</dt>
                    <dd>{project.license.name || project.license.id}</dd>
                  </>
                )}
                {kind === "mod" && (
                  <>
                    <dt>Client</dt>
                    <dd>{SIDES[project.clientSide] ?? project.clientSide}</dd>
                    <dt>Server</dt>
                    <dd>{SIDES[project.serverSide] ?? project.serverSide}</dd>
                  </>
                )}
                <dt>Published</dt>
                <dd>{date(project.published)}</dd>
                <dt>Updated</dt>
                <dd>{date(project.updated)}</dd>
              </dl>
            </section>
          )}
          {optional.length > 0 && (
            <section className="pp-card px4">
              <span className="label pix">WORKS WELL WITH</span>
              {optional.map((d) => (
                <span key={d.projectId} className={`pp-dep px2 ${d.installed ? "have" : ""}`}>
                  <ProjectIcon url={d.iconUrl} size={24} />
                  <span className="grow">{d.title}</span>
                  {d.installed && <Icon name="check" size={10} />}
                </span>
              ))}
            </section>
          )}
        </aside>
      </div>

      {viewing !== null && project && (
        <Lightbox images={project.gallery} index={viewing} onIndex={setViewing} onClose={() => setViewing(null)} />
      )}
    </div>
  );
}

function VersionList({ versions, installed, recommended, busy, onInstall }: {
  versions: PageVersion[];
  installed: string | null;
  recommended: string | null;
  busy: string | null;
  onInstall: (id: string) => void;
}) {
  const [open, setOpen] = useState<string | null>(null);
  if (!versions.length) return <p className="muted">No version of this project works with this instance.</p>;
  return (
    <div className="pp-versions">
      {versions.map((v) => {
        const type = v.versionType || "release";
        const expanded = open === v.id;
        return (
          <div key={v.id} className={`pp-version px2 ${expanded ? "open" : ""}`}>
            <div className="pp-version-row">
              <span className={`pp-type pix px2 ${type}`} title={type}>
                {type[0].toUpperCase()}
              </span>
              <button className="pp-version-name" onClick={() => setOpen(expanded ? null : v.id)} aria-expanded={expanded}>
                <b>{v.name || v.versionNumber}</b>
                <span className="muted small">
                  {v.versionNumber} · {v.loaders.join(", ")} · {date(v.datePublished)} · {compact.format(v.downloads)} downloads
                  {v.size ? ` · ${formatBytes(v.size)}` : ""}
                </span>
              </button>
              {v.id === recommended && <span className="tag px2">Recommended</span>}
              {v.id === installed ? (
                <span className="installed-chip px2">
                  <Icon name="check" size={12} /> Installed
                </span>
              ) : (
                <B3D size="sm" disabled={!!busy} onClick={() => onInstall(v.id)}>
                  {busy === v.id ? <span className="spinner small" /> : <Icon name="download" size={12} />}
                  <span className="pix">{installed ? "Switch" : "Install"}</span>
                </B3D>
              )}
            </div>
            {expanded && (
              <div
                className="markdown small pp-changelog"
                onClick={openLinksOutside}
                dangerouslySetInnerHTML={{ __html: v.changelog ? renderMarkdown(v.changelog) : "<p>No changelog for this version.</p>" }}
              />
            )}
          </div>
        );
      })}
    </div>
  );
}

function Lightbox({ images, index, onIndex, onClose }: {
  images: Page["project"]["gallery"];
  index: number;
  onIndex: (i: number) => void;
  onClose: () => void;
}) {
  const current = images[index];
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      if (e.key === "ArrowRight") onIndex(Math.min(index + 1, images.length - 1));
      if (e.key === "ArrowLeft") onIndex(Math.max(index - 1, 0));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [index, images.length, onIndex, onClose]);

  return createPortal(
    <div className="viewer" role="dialog" aria-modal="true" aria-label={current.title ?? "Picture"} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <button className="viewer-nav prev" aria-label="Previous" disabled={index === 0} onClick={() => onIndex(index - 1)}>
        <Icon name="chevronLeft" size={24} />
      </button>
      <figure className="viewer-figure">
        <img key={current.url} src={current.rawUrl ?? current.url} alt={current.title ?? ""} />
        <figcaption className="viewer-bar px4">
          <span className="viewer-title grow">
            <b>{current.title ?? "Picture"}</b>
            <span className="muted small">
              {current.description ? `${current.description} · ` : ""}
              {index + 1}/{images.length}
            </span>
          </span>
          <button className="icon-btn" aria-label="Close" onClick={onClose}>
            <Icon name="close" />
          </button>
        </figcaption>
      </figure>
      <button className="viewer-nav next" aria-label="Next" disabled={index === images.length - 1} onClick={() => onIndex(index + 1)}>
        <Icon name="chevronRight" size={24} />
      </button>
    </div>,
    document.body,
  );
}
