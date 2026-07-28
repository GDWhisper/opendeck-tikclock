# AGENTS.md

Orientation for any LLM/agent taking over this repository.

## What this project is

**TikClock** is an [OpenAction](https://openaction.amankhanna.me/) plugin (used by [OpenDeck](https://github.com/nekename/OpenDeck)) that displays a digital clock across Stream Deck keys — **one digit per key**. The user assigns each key a position of `HH:MM:SS` (hour tens, hour ones, colon, minute tens, ... or a two-digit "pair" per key), and together the keys form a large clock face.

- Bundle ID: `com.gdwhisper.tikclock` · Action UUID: `com.gdwhisper.tikclock.digit`
- Author identity everywhere: `GDWhisper` · Repo: `https://github.com/GDWhisper/opendeck-tikclock`
- Platforms: Windows (x64) + macOS (x64/arm64) + Linux (x64/arm64) — see `OS`/`CodePaths` in the manifest; binaries are named `tikclock-<target-triple>[.exe]`
- License: MIT

## Repository layout

```
src/main.rs                          # Entire plugin logic (single file, ~430 lines, unit tests at bottom)
com.gdwhisper.tikclock.sdPlugin/     # The distributable plugin bundle
  manifest.json                      # OpenAction manifest (Name/Author/Version/Actions)
  icons/icon.svg                     # Plugin icon: 2×2 key grid digital clock (NOT an analog clock — see conventions)
  propertyInspector/inspector.html   # Settings UI, self-contained HTML with built-in zh/en i18n
  bin/                               # Compiled binaries land here (tikclock-<target-triple>[.exe])
.github/workflows/build.yml          # CI: test + build 5 targets (win x64, mac x64/arm64, linux x64/arm64), package zip, release on tags
build.ps1                            # Local dev build: cargo build --release + copy exe into bundle
assets/preview.png                   # Real-device photo used by both READMEs
README.md / README.zh-CN.md          # Bilingual docs (see conventions)
plugin-development-guide.md          # General OpenDeck plugin development notes
target/                              # gitignored; also used as scratch space (e.g. plugins-fork clone)
```

## Architecture (src/main.rs)

Data flow per second: `tick_loop()` → `text_for(settings, h, m, s)` → `render_svg()` (144×144 SVG data URI) → `instance.set_image()`.

Key mechanisms — **do not remove these when refactoring**, each guards against a real failure mode:

| Mechanism | Where | Why |
|---|---|---|
| Differential rendering | `LAST_TEXT` cache | Only re-send an image when the displayed text changed; avoids flooding the device 1×/sec per key |
| Aligned tick | top of `tick_loop` | Sleeps to the next wall-clock second boundary so all keys flip together |
| Forced refresh with phase stagger | `FORCE_REFRESH_PERIOD`, `instance_phase()` | Devices silently drop images (reset/reconnect); each instance re-sends every 15 s, hashed to different ticks |
| Per-tick circuit breaker | `MAX_IMAGES_PER_TICK` | Caps burst image sends; over-budget instances get their cache evicted and retry next tick |
| Debounced full invalidation | `invalidate_all()`, `MIN_INVALIDATE_INTERVAL` | Device connect / system wake clears caches, debounced so reconnect flapping can't cause a redraw storm |
| Color whitelist | `safe_color()` | Settings values are interpolated into SVG; only `#hex` passes, preventing SVG injection |
| No lock across await | `tick_loop` inner block | Mutex guards are dropped before `set_image().await`; keep it that way |

Other behaviors: `key_down` optionally runs a user-configured shell command silently (`cmd /C` on Windows with `CREATE_NO_WINDOW`, `sh -c` on macOS/Linux); 12-hour mode blanks the hour-tens key and drops leading zero for the hour pair.

## Conventions

- **Indentation: tabs** — in Rust source, `manifest.json`, and the upstream catalogue.
- **Code comments are in Simplified Chinese**; keep that style when editing `src/main.rs`.
- **Bilingual README**: `README.md` (English) and `README.zh-CN.md` (中文) mirror each other section-for-section with language-switch links at the top. Any doc change must be applied to both.
- **Product identity**: the icon and all imagery must depict a *digital clock split across key tiles* — never an analog clock with hands.
- **Property inspector i18n**: `inspector.html` contains a zh/en string table keyed by `data-i18n` attributes, selecting language from the host's `application.language`. New UI strings need both translations.
- `manifest.json` `Version` and `Cargo.toml` `version` are kept in sync.

## Build, test, release

```powershell
cargo test          # unit tests (pure functions: text_for, safe_color, instance_phase)
./build.ps1         # local Windows build; copies exe into the .sdPlugin bundle
```

Release flow (fully automated after tagging):
1. Bump `Version` in `manifest.json` **and** `version` in `Cargo.toml`.
2. Commit, push `main`, then `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. CI builds all 5 targets (Windows x64 on `windows-latest`, macOS x64/arm64 on `macos-latest`, Linux x64 on `ubuntu-22.04`, Linux arm64 natively on `ubuntu-22.04-arm`), assembles `com.gdwhisper.tikclock.zip` (all binaries in one bundle as `bin/tikclock-<triple>[.exe]`, `chmod +x` on the Unix ones), and publishes a GitHub Release.

CI pitfalls already learned (do not regress):
- The `package` job needs `permissions: contents: write` or release creation fails with 403.
- A tag runs the workflow *as of the tagged commit* — if you fix the workflow after tagging, re-point the tag.

## Ecosystem / distribution

Listed in the OpenDeck plugin store via [`OpenActionAPI/plugins`](https://github.com/OpenActionAPI/plugins) `catalogue.json` (entry merged via [PR #33](https://github.com/OpenActionAPI/plugins/pull/33) on 2026-07-28). The store resolves the latest GitHub Release from the `repository` field at runtime, therefore:

- **Normal version updates need no catalogue PR** — just publish a new Release here.
- A catalogue PR **is** required if any of these change: plugin `Name`, `Author`, repo URL, or the icon. Catalogue rules: entry in the *Native OpenAction plugins* section, sorted alphabetically by repository URL, `name`/`author` exactly matching `manifest.json`, description matching the GitHub repo sidebar, tab-indented JSON, icon `icons/com.gdwhisper.tikclock.png` (high-res; maintainers format it themselves).
- The GitHub repo must keep the `openaction` topic to remain listed.

## Known landmines

- `openaction` crate: `visible_instances()` takes `&str` (use `DigitAction::UUID`), not a typed ActionUuid.
- Linux/macOS binaries in the bundle must be executable (`chmod +x` happens in CI packaging; remember it if packaging manually).
- OpenDeck plugin install paths differ per platform (Windows `%APPDATA%`, macOS `~/Library/Application Support`, Linux `~/.local/share`, Flatpak variant) — documented in the READMEs; keep those paths accurate.
- `target/` is gitignored and doubles as scratch space for tooling (e.g., a clone of the catalogue fork lives at `target/plugins-fork` during submission work); never commit anything under it.
