# Glint architecture

Glint is an open-source Minecraft launcher (planned license GPL-3.0). It has instances like Prism, a built-in Modrinth and CurseForge catalogue, support for the Horizon Rust engine, tools for making mods (Glint Studio), and joining a friend's world (Glint Link). There are no paid features.

Platforms: Windows first, then Linux.

## 1. Overview

```
┌───────────────────────────── Glint Desktop (Tauri 2) ─────────────────────────────┐
│  Frontend: React + TS                                                              │
│  screens · design system · task centre · i18n                                     │
│                │  commands (request → response)   ▲  events (progress, logs)       │
│                ▼                                  │                                │
│  Backend: Rust (tokio)                                                             │
│  auth · meta · java · content · instance · engine · launch · link · studio · cloud│
└──────┬──────────────┬───────────────┬───────────────┬──────────────┬──────────────┘
       ▼              ▼               ▼               ▼              ▼
   Microsoft /    Mojang meta,    Modrinth /      Glint API      Another launcher
   Xbox / MC      Fabric, Forge…  CurseForge      (accounts,     (P2P over iroh)
                                                  friends)
```

## 2. Monorepo

| Path | Contents |
|---|---|
| `apps/desktop` | Tauri app: `src-tauri` (thin command layer) and the frontend |
| `crates/glint-core` | Shared models (instance, version, account), paths, config, error types |
| `crates/glint-net` | HTTP client and download manager |
| `crates/glint-auth` | Microsoft → Xbox → XSTS → Minecraft, token storage |
| `crates/glint-meta` | Mojang and loader manifests, normalised to one format |
| `crates/glint-java` | Finding and installing Java |
| `crates/glint-content` | Modrinth and CurseForge providers, dependency resolver, modpacks |
| `crates/glint-instance` | Instances, lock file, snapshots, import from other launchers |
| `crates/glint-engine` | Engine abstraction: Java and (later) Horizon |
| `crates/glint-launch` | Process launch, log stream, crash analyser |
| `crates/glint-link` | P2P tunnel, LAN announcement capture, Quick Play |
| `crates/glint-studio` | Mod templates, builds, publishing |
| `crates/glint-cloud` | Glint API client |
| `crates/glint-proto` | Types shared by the launcher and the backend |
| `services/glint-api` | Accounts and friends backend |

The main rule: **the Tauri layer is thin.** All logic lives in crates that know nothing about Tauri. They can be tested on their own and later assembled into a CLI.

## 3. Key decisions

### Frontend and backend
- **Tauri commands** for requests ("list instances", "find a mod").
- **Events** for anything long: download progress, game logs, friend statuses.
- **Types are generated** with `tauri-specta`: Rust structs become TypeScript types, so the frontend and backend never drift apart.

### Tasks
Every long operation (install, update, import) is a **task** with an id, progress, cancellation and a clear error. The frontend has a task centre, which also drives the XP-bar progress on the Play button.

### Download manager
- Concurrency limits, overall and per host (Modrinth rate-limits requests).
- SHA-1 and SHA-512 checks, retries with backoff, resumed downloads.
- **Cache by hash:** the same file is never downloaded twice.

### Data storage

```
%APPDATA%\Glint\
├─ meta\          libraries, assets, versions: shared by all instances
├─ java\          installed runtimes
├─ cache\         mod files by hash, API responses
├─ instances\
│  └─ <name>\
│     ├─ instance.toml   what this instance is
│     ├─ glint.lock      exactly what is installed in it
│     ├─ snapshots\      restore points
│     └─ game\           the game folder (mods, saves, config…)
└─ glint.db       SQLite: indexes, search cache, account metadata
```

- **Files are the source of truth, not the database.** An instance can be copied, archived or put in git. SQLite only speeds things up and is rebuilt from the files if lost.
- **`instance.toml`** describes the instance: engine, game version, loader, Java, memory, arguments.
- **`glint.lock`** records every mod: source (Modrinth, CurseForge, local file), project and version id, hash. It is what **instance sharing**, **updates**, **rollback** and **"install my friend's setup"** in Glint Link rely on.
- Mods go into an instance as **hard links from the cache**: ten instances with Sodium take the space of one.

### Secrets
- Microsoft refresh tokens go in the **Windows Credential Manager** via the `keyring` crate, not in files.
- The CurseForge key is injected **at build time** from CI secrets and never enters the repository. The Azure client ID is not a secret.

### Errors and logs
- Crates use typed errors (`thiserror`). The UI gets an **error code and a human-readable message**, not a stack trace.
- `tracing` writes rotating log files. A "Copy debug report" button helps with bug reports.

## 4. Engine abstraction

Each engine has four responsibilities:

| Step | Java | Horizon (experimental, Dev only) |
|---|---|---|
| **Prepare** | Version, libraries, assets, loader, Java | Horizon nightly build (`glint-horizon`), Minecraft 26.3 files, the server jar and Java 25 for `horizon-prepare` |
| **Content** | jar mods from Modrinth and CurseForge | Resource packs; `.hmod` mods later (Horizon has no WASM) |
| **Launch arguments** | JVM arguments, classpath, game arguments | Launch API 0.1: only the options from `--launch-api`, session over stdin |
| **Run** | `java` with arguments | Native process |

The rest of the launcher (UI, instances, logs, Link) works only through this interface.

## 5. The Play flow

```
Play
 → read instance.toml and glint.lock
 → check Java                         (glint-java)
 → check version files                (glint-meta + glint-net)
 → check the loader                   (glint-meta)
 → match mods against the lock file   (glint-content)
 → refresh the Microsoft token        (glint-auth)
 → build arguments                    (glint-engine)
 → launch and stream logs             (glint-launch)
 → on a crash: analyse it
```

Every check is **idempotent**: when everything is in place a step takes milliseconds, so launching again is almost instant.

## 6. Content: Modrinth and CurseForge

- One provider interface: search, project, versions, files, dependencies. Modrinth and CurseForge are two implementations.
- **Merged results:** duplicates are joined by file hash or slug, and the result shows both source badges.
- **Dependency resolver:** pulls in required dependencies, warns about incompatible mods, respects the game version and loader.
- **CurseForge mods that forbid redistribution:** a "download it yourself" flow that watches the downloads folder.
- **Modpacks:** import and export of `.mrpack` and CurseForge zips; import of instances from Prism, MultiMC, ATLauncher and the official launcher.

## 7. Glint Link

```
Host                                           Guest
game: "Open to LAN"
  │ multicast announcement (port)
  ▼
glint-link catches it ──► Glint API: "Vasya opened a world" ──► status for friends
                                                             │
                           ◄── ticket request ───────────────┤ "Join"
                           ──► ticket + host iroh address ──►│
                                                             ▼
iroh connection ◄══════════ QUIC (P2P or relay) ═══════════► iroh connection
  │                                                          │
127.0.0.1:<LAN port>                              127.0.0.1:<local port>
                                                             │
                                      game --quickPlayMultiplayer 127.0.0.1:<port>
```

- The host accepts a connection only with a **valid one-time ticket** from the server.
- Before connecting, the guest gets the host's instance installed from its `glint.lock`.
- The tunnel copies bytes between TCP and QUIC and does not parse the Minecraft protocol, so game updates don't break it.
- A licensed host's LAN world checks the licence of everyone who joins.

## 8. Glint API backend

**Stack for a zero budget:** Cloudflare Workers in Rust (`workers-rs`) + D1 (SQLite) + Durable Objects for online status over WebSocket + R2 for instance sharing. When it grows: Axum + Postgres on a VPS; the types from `glint-proto` stay the same.

| Area | What it does |
|---|---|
| **Sign-in** | The launcher sends a Minecraft token once; the server checks the profile with Mojang, **discards the token** and issues its own |
| **Friends** | Requests, list, blocking |
| **Status** | Online, "playing X", "opened a world" |
| **Link tickets** | Issues one-time join tickets |
| **Sharing** | Uploads `instance.toml` + `glint.lock`, a short code for a friend |

The server never stores mods, only links to Modrinth and CurseForge.

## 9. Frontend

- **React + TS + Vite**, TanStack Router and Query, Zustand, Tailwind, Framer Motion.
- **Design system as its own package:** tokens (colours, spacing, pixel step), 9-slice components, pixel icons. The current design system is documented in `docs/DESIGN.md`.
- **i18n from day one** (ru and en).
- **Works offline:** installed instances launch without internet, from the cache.

## 10. Testing

- **Crate unit tests:** manifest parsing, the resolver, argument building.
- **HTTP mocks** (`wiremock`): tests never hit real APIs.
- **Snapshot tests of launch arguments** for a set of versions (1.8, 1.12, 1.16, 1.20, 26.x).
- **CI on GitHub Actions:** Windows build, tests, release to GitHub Releases on a tag.

## 11. Stage 1 build order

1. `glint-core` + `glint-net`: models, paths, file downloader
2. `glint-meta` + `glint-java`: vanilla versions and Java
3. `glint-engine` (Java) + `glint-launch`: **first vanilla launch**
4. `glint-auth`: Microsoft sign-in (after Mojang approval)
5. `glint-instance`: several instances, `instance.toml` / `glint.lock`
6. Tauri shell and the first screens

Until Mojang approves the app, vanilla launches are debugged with an offline name, which is acceptable for our own development.

## Roadmap

| Stage | What it includes |
|---|---|
| **0. Groundwork** | Mojang application, CurseForge key, license, design mock-up |
| **1. Core** | Microsoft sign-in, instances, vanilla, automatic Java, engine abstraction |
| **2. Mods** | Fabric/Quilt, Modrinth catalogue, dependencies, updates |
| **3. Ecosystem** | CurseForge, modpacks, import from Prism and others, Forge/NeoForge |
| **3.5. Social** | Glint accounts, friends, status, instance sharing, Glint Link |
| **4. Glint Studio** | Mod templates, test launches, publishing |
| **5. Horizon** | Horizon runtime, WASM mods, a Rust template in Studio |
| **6. Release** | Code signing, glnt.net, Linux |
