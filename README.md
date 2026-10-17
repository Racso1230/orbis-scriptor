# Orbis Scriptor

A local-first desktop app for fantasy worldbuilding: maps, lore, timelines, characters and notes. Everything lives in a folder you choose on your own machine; there is no server and no account.

Built with [Tauri 2](https://tauri.app) (Rust backend) and React + TypeScript (frontend). Targets Windows and Linux.

The roadmap is in [docs/DEVELOPMENT_PLAN.md](docs/DEVELOPMENT_PLAN.md).

## Prerequisites

- **Rust** (stable) via [rustup](https://rustup.rs). Open a new terminal after installing so `cargo` is on your PATH.
- **Node.js 24** and npm.
- **Windows:** Visual Studio Build Tools with the "Desktop development with C++" workload, and the WebView2 runtime (already present on Windows 10/11).
- **Linux (Debian/Ubuntu):**
  ```sh
  sudo apt-get install -y libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf xdg-utils build-essential libssl-dev
  ```
  See the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for other distributions.

## Run

```sh
npm install
npm run tauri dev
```

The first Rust build takes a few minutes; later builds are incremental.

## Test and check

```sh
npm run lint          # ESLint
npm run format:check  # Prettier
npm run typecheck     # TypeScript
npm test              # Vitest

cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

CI runs all of these on Windows and Linux for every push and pull request.

## Build

```sh
npm run tauri build
```

Installers are written to `src-tauri/target/release/bundle/`.

## Releasing

Releases are built by [`.github/workflows/release.yml`](.github/workflows/release.yml) when a tag named `v*` is pushed. It builds the Windows installers (`.msi` and NSIS `.exe`) and the Linux packages (`.deb` and `.AppImage`), signs the updater artifacts, and creates a **draft** GitHub Release with everything attached plus `latest.json` (the file the in-app updater reads). Review the draft, then publish it.

### Steps

1. Pick the new version `X.Y.Z` and set it in all three places: `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`. Then run `cargo check` in `src-tauri` and `npm install --package-lock-only` so the lock files follow.
2. Move the `Unreleased` entries in `CHANGELOG.md` under a new `[X.Y.Z] - date` heading.
3. Commit and push to `main`, and wait for CI to pass.
4. Tag and push the tag:
   ```sh
   git tag vX.Y.Z
   git push origin vX.Y.Z
   ```
5. When the workflow finishes, open the draft release on GitHub, check the assets, and publish it.

### Signing key and GitHub secrets

Updates are verified with a minisign key pair. The **public** key is committed in `src-tauri/tauri.conf.json` (`plugins.updater.pubkey`). The **private** key is never committed:

- It lives on the maintainer's machine at `C:\Users\oscar\.tauri\orbis-scriptor.key` (public half: `orbis-scriptor.key.pub`). It was generated with `npx tauri signer generate --ci -p "" -w <path>`, which means it has an **empty password**.
- **Back it up** somewhere safe (a password manager is ideal). If it is lost, installed copies of the app can never verify another update, and users would have to reinstall by hand. Do not commit it or paste it anywhere public.
- Add two repository secrets (Settings > Secrets and variables > Actions):
  - `TAURI_SIGNING_PRIVATE_KEY`: the full contents of the `.key` file.
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: the key's password. For the current key this is empty; create the secret with an empty value (or remove that line from the workflow).

To build installers locally with updater signing (PowerShell):

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw "$HOME\.tauri\orbis-scriptor.key"
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
npm run tauri build
```

### The updater and a private repository

The app checks `https://github.com/Racso1230/orbis-scriptor/releases/latest/download/latest.json`. GitHub only serves release downloads to anonymous clients when the repository is **public**. While this repository is private, the in-app update check will fail (quietly at startup, with a message when you click "Check for updates") and installers can only be downloaded by signed-in collaborators. To ship updates you must either make the repository public, or publish releases to a separate public repository and change the endpoint in `src-tauri/tauri.conf.json` (and the repository used by the release workflow).

The update check only runs automatically in release builds, not in `tauri dev`. If it fails (for example, offline), the app carries on normally.

## End-to-end tests

The Playwright tests in `e2e/` drive the real desktop app on Windows. They launch the debug build with WebView2's remote debugging port enabled, then attach Playwright over the Chrome DevTools Protocol:

```sh
npm run e2e
```

`npm run e2e` first builds the debug app (`npm run tauri build -- --debug --no-bundle`), then runs `playwright test`. To skip the build while iterating on the tests, run `npx playwright test` (the debug executable must already exist at `src-tauri/target/debug/orbis-scriptor.exe`).

Native dialogs (the folder and image pickers) cannot be driven by Playwright. In **debug builds only**, if the environment variable `ORBIS_E2E_DIR` is set, the app skips those dialogs and uses a folder and a map image inside that directory instead. Release builds ignore the variable completely. The test creates a fresh temporary directory for every run and sets the variable for you.

Linux uses WebKitGTK, which has no CDP support, so Linux end-to-end testing is manual for now (a `tauri-driver` setup could replace this later). CI runs the e2e tests on Windows only, as a separate job.

## Project layout

```
src/                 React frontend
  features/<name>/   one folder per feature (components, hooks, queries)
  components/        shared UI (shadcn/ui components live in components/ui)
  lib/               bindings.ts (generated), query client, helpers
  stores/            Zustand stores
src-tauri/src/       Rust backend
  commands/          #[tauri::command] functions: validate, call a service, return AppResult
  services/          business logic
  db/                SQL and row mapping
  models/            structs shared across layers
  error.rs           the single AppError type
  state.rs           Tauri managed state
docs/                plan and design notes
```

## World folder and backups

A world is a folder:

```
MyWorld/
  world.json   name and format version
  world.db     the SQLite database
  assets/      imported images
  backups/     automatic daily copies of world.db
```

Each time a world is opened, a snapshot of `world.db` is written to `backups/world-YYYY-MM-DD.db` (at most one per UTC day, made in the background with SQLite's `VACUUM INTO`, so it is safe while the world is in use). The newest 5 are kept. To restore, close the app and replace `world.db` with a backup (rename it); delete any `world.db-wal` / `world.db-shm` files next to it.

## Generated bindings

`src/lib/bindings.ts` is generated by [tauri-specta](https://github.com/specta-rs/tauri-specta) from the Rust commands and models. **Never edit it by hand.** It is regenerated whenever the app starts in debug mode (`npm run tauri dev`) and by `cargo test`; commit the result alongside the Rust change that produced it.
