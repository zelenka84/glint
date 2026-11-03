# Glint design system

A working reference for any new screen or block. When the task is "make it look like the rest", take the parts and rules from here. You don't need to re-read the code: everything you need is below.

Code: `apps/desktop/src/`. Styles live in a single `App.css`, split into `/* ---- name ---- */` sections. Components are in `ui.tsx`, `EmptyArt.tsx` and `Logo.tsx`. Themes are in `theme.ts`.

## 1. Character

- **Pixel premium.** Pixel corners, chunky 3D buttons, 16×16 pixel icons and pixel scenes, while the interface still looks neat and calm, not like a game from 2010.
- **Dark base, one bright accent.** Surfaces step up in shade (`bg` → `panel` → `card` → `raise`). The accent marks the selection, the main action and focus. Green `play` is only for launching the game.
- **Motion has two kinds.** Pixel effects (stars, glints, the XP bar, blinking) step with `steps()`. Everything else (entrances, hover, expanding) moves smoothly on `--ease` and `--spring`.
- **No borders.** Edges come from neighbouring surface shades, or from `box-shadow: inset 0 0 0 2px var(--accent)` for the selected item. `border` is almost never used.

## 2. Tokens

Every colour comes from a CSS variable. `theme.ts` (`paletteFor`) sets them from the three base themes `mine`, `sunset` and `day`, plus the user's accent and tint. **Never write hex colours in new styles.** The only exceptions are the red `danger` button and the colours inside pixel SVG scenes.

| Token | Use |
|---|---|
| `--bg` | window background |
| `--panel` | panels, section cards, tabs |
| `--card` | items inside a panel: rows, tiles, stat boxes |
| `--raise` | inputs, hover on `card`, `tag-muted` chips |
| `--line` | dividers, inactive bars, picture frames |
| `--edge` | the "bottom" of 3D buttons and cards (near black) |
| `--text`, `--text2`, `--muted` | primary, secondary and muted text |
| `--link` | links and `.link` buttons |
| `--accent`, `--on-accent` | the main colour and text on it |
| `--accent-ink` | accent text on dark (eyebrow, stat numbers, title hover) |
| `--accent-edge` | the bottom of an accent button |
| `--ok`, `--warn`, `--danger` | success, warning, error (`--on-warn` and `--warn-edge` for warn buttons) |
| `--play`, `--play-edge`, `--on-play` | only the Play button, RUNNING and the XP bar |
| `--plaque` | translucent plate over a scene or picture |
| `--chip`, `--chip-fg` | chips on the Home plaque |
| `--overlay` | dimming behind dialogs |
| `--display-font` | the pixel font, picked in Settings |

Tinting: `color-mix(in srgb, var(--accent) 9%, var(--panel))` plus `box-shadow: inset 4px 0 0 var(--accent)` makes an "accent" panel (hint, dependency block). With `--warn` it makes the updates panel.

**Motion**

```css
--ease: cubic-bezier(0.22, 1, 0.36, 1);    /* entrances, colour, expanding */
--spring: cubic-bezier(0.34, 1.56, 0.64, 1); /* move and scale on hover and press */
--t-fast: 140ms;  --t: 240ms;  --t-slow: 420ms;
```

## 3. Typography

- Body: Inter 14px. Small text `.small` 12px, muted `.muted`.
- Headings and all "game" labels use the **`.pix`** class with `--display-font`: h1/h2, button text, eyebrows, badges, section labels.
- Heading sizes: screen `h1` 32px (42px on the Home plaque, 30px on a project page), section `h2` 19–22px, weight 600.
- Section label: `<span className="label pix">THEME</span>`, all caps, 12px, letter-spacing 2px, `--muted`.
- Eyebrow above a title: `<span className="eyebrow pix">READY TO PLAY</span>`, in the accent colour.
- The UI language is English, in sentence case, kept short. Compact numbers with `compact` from `content/ContentBrowser.tsx` (`12.3K`).

## 4. Shape and depth

- **Pixel corners**: `px4` for panels, cards and big buttons; `px2` for small things (chips, rows, inputs, icons). They are a `clip-path`. The user can switch corners to round or square (`data-corners` on `<html>`) and the classes stay the same.
- `clip-path` clips everything outside the element, including `position: fixed` children. So **dialogs and full-screen viewers render through a portal into `document.body`** (as `Modal` does).
- **Depth** is an edge underneath: `B3D` and `.lift` cards have an `.edge` (background `--edge`) with a `.face` on top. On hover the face lifts `translateY(-2px)`; on press it sinks onto the edge.
- Inset light: a light strip at the top and a dark one at the bottom of a button face (`inset 0 3px 0 rgba(255,255,255,.28)`). No soft outer shadows.

## 5. Components (React)

| Component | Where | Use |
|---|---|---|
| `B3D` | `ui.tsx` | Every action button. `variant`: `secondary` (default), `accent` (the main action on a screen), `play` (launch only), `warn` (update), `danger` (delete, kill). `size`: `sm` / `md` / `lg`. `className="square"` for an icon-only square, `label` for aria and tooltip, `idle` for a periodic glint on the main button. Wrap text in `<span className="pix">`; the icon goes before the text. |
| `Icon` | `ui.tsx` | 16×16 pixel icons; `size` in steps of 2 (10, 12, 14, 16, 20, 24, 28). |
| `Modal` | `ui.tsx` | Dialogs. Rendered in a portal; Esc and a click on the backdrop close it; focus goes to the first field. `wide` makes it 760px (readers). Inside: `<form className="form">` and, at the bottom, `<div className="modal-actions">` (Cancel left of the main button) or `modal-actions spread`. |
| `Switch` | `ui.tsx` | Pixel on/off switch for boolean settings. |
| `XpBar` | `ui.tsx` | 32-segment progress (game download, install). |
| `Cover` | `ui.tsx` | Instance cover: a preset or the user's picture. |
| `Photo` | `ui.tsx` | The user's background picture with blur, dim and focal point. |
| `RustBadge` | `ui.tsx` | Marks a Horizon instance. |
| `EmptyState`, `EmptyArt` | `EmptyArt.tsx` | Empty states with a pixel scene: `chest` (nothing here), `map` (nothing found), `picture` (no pictures), `night` (nothing has happened yet). `compact` for inline spots. |
| `ProjectIcon` | `content/ContentBrowser.tsx` | Modrinth project icon with a placeholder. |
| `WorldIcon` | `instance/WorldsTab.tsx` | World icon. |
| `SparkleMark`, `Wordmark` | `Logo.tsx` | The logo. |

Formatters: `formatBytes`, `relativeTime` and `flavourOf` in `ui.tsx`; `formatDuration` in `home/Dashboard.tsx`.

## 6. CSS classes (ready-made bricks)

**Screen skeleton**
```tsx
<div className="screen">                      {/* column with gap 20; children rise in one after another */}
  <div className="screen-head">
    <h1 className="pix">Title</h1>
    <B3D variant="accent">…</B3D>               {/* the screen's main action, on the right */}
  </div>
  <section className="panel px4">…</section>
</div>
```

| Class | What it is |
|---|---|
| `panel px4` | Settings or content block: `panel` background, column, gap 12, with `h2` and `p` inside. |
| `dash-card px4` + `dash-head` | Widget card with an `h2.pix` title on the left and a `.link` or `.seg` on the right (Home). |
| `section-head` | Section heading outside a panel (`h2` + an "All …" link). |
| `card-grid` / `card-grid four` | Grid of instance cards. |
| `mod-row px4` | List row: 48px icon, `.mod-text` (`.mod-title`, `.mod-desc`, `.mod-meta`), action on the right. A clickable row adds `clickable`, `role="button"` and `tabIndex={0}`. |
| `jump px2` / `popular px2` / `news px2` | Compact tile on `--card`: icon + `.jump-text` (`b` and `.muted.small`). Lifts 2px on hover. |
| `tabs` + `tab px2` (`.active`) | Button tabs: icon + `span.pix`; the active one has an accent background. |
| `seg px2` | Small segmented switch inside a card (`button.on`). |
| `search px4` / `search slim px4` | Search field with an icon. |
| `field` (`inline`) + `input px2` / `select px2` | Form fields; the label `span` sits above in 12px muted text. |
| `check` | Checkbox with a label. |
| `tag px2` | Accent tag (Active, Recommended). |
| `tag-muted px2` | Neutral category tag. |
| `chip px2` | Chip on the plaque over a scene. |
| `count px2` | Counter next to a heading (`.warn`, `.bad`). |
| `installed-chip px2` | "Installed" state in place of a button. |
| `badge pix px2` | RUNNING over a cover. |
| `notice px2` (`.ok`) | Notice line at the top of a screen with a close button. |
| `hint` | Soft tip on an accent tint. |
| `warning-box px2` | Warning before a dangerous action. |
| `skeleton-list` + `skeleton px4` | Loading placeholders for a list (shimmer). |
| `spinner` / `spinner small` | Spinner in a search field or a button. |
| `link` | Text button that looks like a link. |
| `icon-btn` | Bare 28×28 icon button (close, delete in a row). |
| `tool-btn px2` | Flat toolbar button (Copy, Rename) on `--raise`. |
| `markdown` | Typography for HTML/Markdown (mod descriptions, patch notes). Always run it through `DOMPurify`. |
| `viewer` | Full-screen picture viewer with arrows (in a portal). |
| `row`, `grow`, `muted`, `small`, `error` | Utilities. |

## 7. Motion recipes

- **Entrances**: `animation: rise var(--t-slow) var(--ease) backwards;` (12px up plus fade). Tiles in a grid: `pop`. Backdrop, modal, feed: `fade`. Dialogs: `drop` on `--spring`.
- **Stagger**: each next child starts 40–70ms later, up to about 320ms. Example: `.x > :nth-child(2) { animation-delay: 40ms; }`.
- **Always `backwards`**, never `both` or `forwards`, so no transform lingers to break hover and `position: fixed`.
- **Hover**: a card or tile moves `translateY(-2px)` and its background goes one shade lighter; an icon inside does `rotate(-4deg) scale(1.06)`; an icon in a button does `scale(1.12)`. Press: `scale(0.97–0.98)` with `transition-duration: 60ms`.
- **Switching a tab or page**: give the container `key={tab}` so the entrance plays again.
- **Pixel effects** only with `steps()`: `blink`, `tw` (twinkling stars), `sweep` (button glint), `flick` (fire).
- **Accessibility**: the Motion setting (`data-motion` on `<html>`) and `prefers-reduced-motion` already silence animations globally. Add any new infinite animation to `@media (prefers-reduced-motion: reduce)`.
- Available keyframes: `rise`, `pop`, `fade`, `drop`, `slide-down`, `scene-in`, `glow`, `blink`, `tw`, `flick`, `drift`, `sweep`, `idle`, `shimmer`, `bar-grow`, `art-bob`, `ob-in`.

## 8. Icons

An icon is an SVG path string in `ICONS` (`ui.tsx`) on a 16×16 grid of rectangles `MxyhWvHhZz`, for example `"M7 3h2v10H7zM3 7h10v2H3z"` (plus). It renders with `shapeRendering="crispEdges"` in `currentColor`. Add a new icon at the end of the map with a short camelCase name. Existing ones: home, library, mods, friends, studio, settings, folder, edit, trash, plus, close, minimize, maximize, play, check, user, link, search, stop, terminal, cube, brush, sun, notes, globe, server, image, file, back, copy, refresh, download, upload, archive, arrowUp, arrowDown, chevronLeft, chevronRight, wrap, follow, external, seed, pin, warn, return, heart, code, bug, book, chat, clock.

Pixel pictures (empty states, scenes, covers) are inline SVG made of `rect`s with `fill="var(--token)"`, so they follow the theme.

## 9. Screen patterns

- **List + detail**: clicking a row opens the page in the same container, like `ContentBrowser` → `ProjectPage`. The list stays mounted (`hidden`) and the `.main` scroll position is saved and restored. There is a "Back to …" button (`.link` + `chevronLeft`), and Esc goes back too.
- **Entity page** (instance, project): a `px4` header with a picture or blurred banner, an 88px icon, `h1.pix`, a meta line with 12px icons, and the main button on the right. Below that, `tabs`, then two columns: content `1fr` and a 260–340px sidebar of `panel` cards (links, a `dl` of details). Below 1100–1180px wide it collapses into one column.
- **Dashboard widget**: `dash-card` with a heading. When it has nothing to show, use a compact `EmptyArt` or don't render the block at all.
- **Loading**: skeletons or a `spinner` with "Loading … from Modrinth…". Data for repeat visits is cached (`useCached` in `home/Dashboard.tsx`).
- **Errors**: `<p className="error">` next to the action, or a `notice` at the top of the screen.
- **External links** open in the system browser through `openUrl` from `@tauri-apps/plugin-opener`, never inside the launcher window.
- **User pictures** come from the data folder through `convertFileSrc` or `mediaUrl`. A new folder must be added to `assetProtocol.scope` in `tauri.conf.json`.

## 10. Appearance settings (already live; respect them)

Theme (`mine` / `sunset` / `day`), accent and tint recompute every token. Also: the font (`--display-font`), scale (`zoom` on `<html>`, 90–125%), corners (`data-corners`), motion (`data-motion`), compact sidebar (`data-sidebar="compact"`) and a background picture (Home or the whole window). New elements must look right in all three themes, including the light `day`. So use tokens only, never `#fff`/`#000` for text.

## 11. New UI checklist

1. Skeleton from `screen` / `panel` / `dash-card`, corners `px4` or `px2`.
2. Colours from tokens only, one accent, `play` only for launching.
3. Headings and button labels in `.pix`, everything else in Inter.
4. `B3D` buttons in the right variant; small actions as `icon-btn`, `tool-btn` or `link`.
5. `rise` / `pop` entrances with `backwards` and a stagger; hover lifts 2px; press in 60ms.
6. Empty state with `EmptyArt`, loading with a skeleton or spinner, errors with `.error`.
7. Dialogs and overlays through a portal.
8. English copy, short, in sentence case.
9. Draw new icons in `ICONS`; don't pull them from libraries.
10. Run `npx tsc --noEmit`. Add styles as a new section at the end of `App.css`.
