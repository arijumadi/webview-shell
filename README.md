# webview-shell

[![build-windows](https://github.com/arijumadi/webview-shell/actions/workflows/build-windows.yml/badge.svg)](https://github.com/arijumadi/webview-shell/actions/workflows/build-windows.yml)
[![release](https://img.shields.io/github/v/release/arijumadi/webview-shell)](https://github.com/arijumadi/webview-shell/releases/latest)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

Turn **any URL** into a desktop app — without Electron. The URL and the feature set are
read at runtime, so a single binary can serve many sites without rebuilding.

Built on **Tauri v2** using the OS webview: WebView2 (Windows), WebKitGTK (Linux),
WKWebView (macOS). No bundled Chromium.

| | |
|---|---|
| portable exe | ~3.0 MB |
| NSIS installer | ~1.2 MB |
| MSI installer | ~1.6 MB |
| idle RAM | ~100 MB |
| startup | 0.3–0.7 s |

---

## Download

Latest build: **[v0.1.0](https://github.com/arijumadi/webview-shell/releases/latest)**

| File | Size | Notes |
|---|---|---|
| [`webview-shell-0.1.0-x64-setup.exe`](https://github.com/arijumadi/webview-shell/releases/latest/download/webview-shell-0.1.0-x64-setup.exe) | 1.1 MB | NSIS installer — easiest |
| [`webview-shell-0.1.0-x64.msi`](https://github.com/arijumadi/webview-shell/releases/latest/download/webview-shell-0.1.0-x64.msi) | 1.6 MB | MSI — for deployment / group policy |
| [`webview-shell-0.1.0-x64-portable.exe`](https://github.com/arijumadi/webview-shell/releases/latest/download/webview-shell-0.1.0-x64-portable.exe) | 3.0 MB | Portable — drop it anywhere |

> The binaries are unsigned. If you download them through a browser, SmartScreen may warn
> once — pick *More info → Run anyway*.

## Features

| Feature | How to enable | Notes |
|---|---|---|
| Dynamic URL | config / CLI / setup page | no rebuild per site |
| Tray icon | `--tray` | menu: Show/Hide, Reload, Quit |
| Global shortcut | `--shortcut CmdOrCtrl+Shift+Space` | show/hide the window |
| Autostart | `--autostart` | declarative: off removes the entry |
| Remembers size & position | automatic | read before the window is created, no jump |
| Per-site zoom | `Ctrl` `+` `-` `0`, `Ctrl`+wheel | remembered per origin |
| Native notifications | automatic | `new Notification(...)` → OS toast |
| Drag & drop files | automatic | drag-to-upload works |
| User agent | `--ua chrome` / `--ua "<string>"` | also changes the HTTP header |
| JS inject | `--inject file.js` | runs before the page is parsed |
| Multiple configs | `--config file.json` | one binary, several sites — but they share one profile |
| Close to tray | `--close-to-tray` | X hides the window instead of quitting |
| Camera / microphone | `--camera --microphone` | grant media permissions to the page |
| External links | `--external-in-browser` | cross-host navigation opens in the default browser |

Everything can be turned off: `--no-tray`, `--no-autostart`, `--no-zoom`, `--no-notify`.

## Quick start

### Windows

Download from the **[latest release](https://github.com/arijumadi/webview-shell/releases/latest)**
and run the installer, or use the portable build:

```powershell
# portable, straight to one site
.\webview-shell.exe --url https://app.example.com --tray

# no arguments: opens the setup form, saves to webview-shell.json
.\webview-shell.exe
```

Requires the **WebView2 Runtime** — present on Windows 11 and most Windows 10 machines.
The NSIS/MSI installers take care of fetching it when missing.

### Linux

Not part of the release yet; build it with the `build-branded-linux` workflow or
`npm run build` on a Linux machine. Output is a `.deb` (recommended) and an `.AppImage`.

```bash
sudo apt install ./webview-shell_0.1.0_amd64.deb
# or: chmod +x *.AppImage && ./*.AppImage
```

Requires `libwebkit2gtk-4.1` and `libayatana-appindicator3` (already declared by the `.deb`).

### Build from source

```bash
npm install
npm run dev      # desktop window, hot reload
npm run build    # bundle for the current OS
```

Bundling **cannot** cross-compile — Windows on Windows, Linux on Linux. To build from
another machine, use the workflows below.

---

## Usage

Three config sources, in **priority order**:

```
CLI flags  >  JSON file  >  defaults baked at build time
```

Flags always win, and `webview-shell.json` overrides build defaults.

### 1. Setup page (no config)

Run the exe with no arguments and no `webview-shell.json`. A form appears: URL, title,
size, tray, autostart, shortcut, user agent, inject. Save → the config is written to
`webview-shell.json` next to the exe and the app navigates to the URL.

Good for trying things quickly, or for handing one generic exe to several people.

### 2. CLI flags

```powershell
.\webview-shell.exe --url https://app.example.com --title "My App" `
  --width 1400 --height 900 --tray --autostart `
  --shortcut "CmdOrCtrl+Shift+Space" --ua chrome `
  --inject inject/ua-ch.js
```

Flags are not persisted — they apply to that run only. Use a config file for permanence.

### 3. Config file

`webview-shell.json` next to the exe, or any path via `--config`:

```json
{
  "url": "https://app.example.com",
  "title": "My App",
  "width": 1400,
  "height": 900,
  "tray": true,
  "autostart": false,
  "shortcut": "CmdOrCtrl+Shift+Space",
  "userAgent": "",
  "zoomHotkeys": true,
  "notifications": true,
  "inject": ["inject/hide-ads.js"]
}
```

| Field | Default | Description |
|---|---|---|
| `url` | empty | Empty opens the setup page |
| `title` | `Webview` | Window title and tray tooltip |
| `width` / `height` | 1200 / 800 | Only used on first launch |
| `tray` | `true` | Tray icon and menu |
| `autostart` | `false` | Register on login |
| `shortcut` | empty | Global accelerator, e.g. `CmdOrCtrl+Shift+Space` |
| `userAgent` | empty | Empty keeps the webview default |
| `zoomHotkeys` | `true` | `Ctrl +/-/0` and `Ctrl`+wheel |
| `notifications` | `true` | Bridge `Notification` → OS toast |
| `closeToTray` | `false` | X hides to tray instead of quitting |
| `camera` | `false` | Grant camera permission to the page |
| `microphone` | `false` | Grant microphone permission to the page |
| `openExternalInBrowser` | `false` | Cross-host navigation opens in the default browser |
| `allowedHosts` | `[]` | Extra hosts kept inside the app when the above is on |
| `inject` | `[]` | Script paths, relative to the exe folder |

## Flags

| Flag | Effect |
|---|---|
| `--url <url>` | Page to open (http/https) |
| `--title <text>` | Window title and tray tooltip |
| `--width <px>` / `--height <px>` | Initial window size |
| `--tray` / `--no-tray` | Tray icon and menu |
| `--autostart` / `--no-autostart` | Register/remove from login startup |
| `--shortcut <accelerator>` | Global show/hide shortcut |
| `--inject <path>` | Script injected before the page is parsed (repeatable) |
| `--ua chrome` | Plain Chrome UA, without the `Edg/` marker |
| `--ua "<string>"` | Custom UA, used verbatim |
| `--zoom` / `--no-zoom` | Zoom hotkeys |
| `--notify` / `--no-notify` | Notification bridge |
| `--close-to-tray` / `--no-close-to-tray` | X hides to tray (requires `--tray`) |
| `--external-in-browser` / `--no-external-in-browser` | Cross-host navigation opens outside |
| `--allow-host <host>` | Keep an extra host inside the app (repeatable) |
| `--camera` / `--no-camera` | Grant camera permission |
| `--microphone` / `--no-microphone` | Grant microphone permission |
| `--config <path>` | Use another config file |

---

## Recipes

### One exe, many sites

Create several configs and pick one with `--config`:

```powershell
.\webview-shell.exe --config D:\apps\stock.json
.\webview-shell.exe --config D:\apps\checkout.json
```

Note: configs share the same app identifier, so they also share cookies and storage.
Running two configs against the same site gives you the same session, not two accounts.
Isolated profiles are not implemented yet — that needs a per-config `dataDirectory`.

### Chat apps (WhatsApp / Telegram)

```powershell
.\webview-shell.exe --url https://web.whatsapp.com/ --tray --width 1200 --height 800
```

On **Linux** the webview is WebKitGTK with a Safari-like UA, and WhatsApp rejects it.
Use `--ua chrome`. QR pairing still persists, because the webview profile is stored per
`identifier`.

### Internal dashboards (local / VPN / tailscale)

Nothing special — the shell does not care whether the URL is public. For a branded build
the runner cannot reach the icon, so pass `-f icon_path=` with a file committed to the
repo (see the generator section).

### Hiding elements or patching a page

```js
// inject/hide-ads.js — runs before the HTML is parsed
window.addEventListener("DOMContentLoaded", () => {
  const style = document.createElement("style");
  style.textContent = ".ads, .promo-banner { display: none !important; }";
  document.head.appendChild(style);
});
```

You can also patch `fetch` or `localStorage` before the site's own scripts run. For
elements that get removed on SPA re-render, wrap the logic in a `MutationObserver`.

Ready-made examples live in `inject/`.

---

## Feature details

### Window size and position

Geometry is read **before** the window is created, so the app opens at the last size
immediately — no small window that jumps. Stored in
`%APPDATA%\<identifier>\window-state.json` on close, tray quit, or focus loss. The saved
position is only reused if it still lands on a connected monitor.

If the last state was maximized, the window is created **hidden**, maximized, then shown,
so not a single frame is painted at the windowed size.

### Per-site zoom

`Ctrl` `+`, `Ctrl` `-`, `Ctrl` `0`, and `Ctrl`+wheel. The level is remembered **per
origin** using that site's own `localStorage`.

Why not native zoom: WebView2 keeps its zoom factor internally and Tauri exposes no
getter, so the last value cannot be read back. The injected shim sets
`documentElement.style.zoom` instead, which means very complex layouts can differ slightly
from native browser zoom. At 100% there is no difference at all.

### Notifications

`new Notification("Title", { body: "..." })` from the site is forwarded to an OS toast via
the `notify` command. Remote pages are only allowed to call that one command — the
`remote-notify` capability grants IPC to `http/https` origins with `permissions: []`, and
`get_config`/`save_config` reject remote callers.

Two limits: toasts usually only appear for the **installed** build (Windows needs a Start
Menu shortcut as the AUMID), and clicking a toast is not forwarded to the page's `onclick`.

### Drag & drop

Dropped files are handed to the page, so drag-to-upload works. This is not Tauri's
default — Tauri replaces WebView2's drag-drop handler to emit its own `DragDropEvent`,
which means HTML5 drops never reach the page. The shell disables that handler because it
does not use those events.

### Close to tray, links, media permissions

**Close to tray.** With `--close-to-tray` (and a tray icon), the close button hides the
window instead of quitting. Quit through the tray menu. Ignored when the tray is disabled,
so you can never lose the window.

**Links.** `window.open` and `target="_blank"` always go to the system browser — a
single-window shell has nowhere to put a popup. Cross-host navigation in the same tab is
opt-in via `--external-in-browser`, because OAuth redirects must stay inside the webview to
complete; add provider hosts with `--allow-host` when enabling it.

**Media.** Camera and microphone are denied by default. Grant them with `--camera` /
`--microphone`; the handler answers the webview's permission request instead of relying on
a browser prompt that never appears.

### Login & OAuth

Google blocks OAuth in *embedded webviews* based on the User-Agent. On Windows, WebView2
reports a plain Edge UA, so the **redirect** flow (full navigation to
`accounts.google.com` and back) generally works — that is the pattern used by an internal
app and it is proven to work.

What breaks is **popups**: Tauri v2 blocks `window.open`, so OAuth flows that open a
separate login window fail with `popup_blocked_by_browser`. If the target site uses that
pattern, the options are to switch it to a redirect flow, or intercept `window.open` via
inject → open the system browser and return through a deep link (not included here).

On Linux the webview is WebKitGTK with a Safari-like UA. If a provider rejects it,
`--ua chrome` usually solves it.

### User agent

Default: the webview's own UA (Edge on Windows, WebKit on Linux). `--ua chrome` replaces
the UA **at the webview level**, so the HTTP header changes too, not just
`navigator.userAgent`.

`inject/ua-ch.js` aligns the other JS signals (`navigator.userAgentData`, `vendor`,
`platform`) with the UA string. Limit: the `Sec-CH-UA` header is still sent by the network
stack, so serious fingerprinting can still tell the difference. This is not an
anti-detection tool.

---

## Generator: branded app per site

`scripts/make-app.ps1` copies the project, rebrands name/identifier/icon, and **bakes the
config into the binary** — the result is a single-site app with no sidecar JSON.

### Locally (needs a Tauri toolchain on Windows/Linux)

```powershell
./scripts/make-app.ps1 -Name "Checkout" -Url https://checkout.example.com `
  -Icon ./logo.png -Tray -Shortcut "CmdOrCtrl+Shift+K" -UserAgent chrome
```

Output in `out/checkout/`:

```
src-tauri/target/release/checkout.exe
src-tauri/target/release/bundle/nsis/Checkout_0.1.0_x64-setup.exe
src-tauri/target/release/bundle/msi/Checkout_0.1.0_x64_en-US.msi
```

| Parameter | Default | Description |
|---|---|---|
| `-Name` | required | App name, also the output folder slug |
| `-Url` | empty | Empty starts from the setup page |
| `-Identifier` | `com.arijumadi.<slug>` | Change if needed |
| `-Icon` | shell icon | Square PNG/SVG, at least 512px |
| `-Tray` / `-Autostart` | off | Default feature state |
| `-Shortcut` | empty | Default global shortcut |
| `-UserAgent` | empty | `chrome` or a full string |
| `-Inject` | empty | Script paths, repeatable |
| `-Targets` | per OS | `nsis,msi` (Windows) / `deb,appimage` (Linux) |
| `-SkipBuild` | off | Generate the folder only |

How it works: the config is written to `.cargo/config.toml` as
`WEBVIEW_SHELL_DEFAULT_CONFIG` and read by the binary through `option_env!` at compile
time.

### Via GitHub Actions

No local toolchain needed:

```bash
gh workflow run build-branded.yml --ref main \
  -f name="Checkout" -f url="https://checkout.example.com" -f tray=true
```

| Workflow | Runner | Output |
|---|---|---|
| `build-windows.yml` | `windows-latest` | generic shell: exe + NSIS + MSI |
| `build-branded.yml` | `windows-latest` | branded Windows app |
| `build-branded-linux.yml` | `ubuntu-22.04` | `.deb` + binary (+ AppImage opt-in) |

### Icon

Three sources, in order:

1. `icon_path` — a file in the repo, e.g. `branding/logo.svg`
2. `icon_url` — an image URL
3. both empty → `scripts/fetch-icon.py` grabs the site favicon (SVG preferred, PNG/ICO
   upscaled to 1024px)

If none of them work, the default webview-shell icon is used. For sites the runner cannot
reach (local/VPN), fetch the icon on your machine and commit it under `branding/`.

---

## Limitations

| Area | Status |
|---|---|
| Google OAuth | **redirect** login usually works (WebView2 UA is Edge); flows that need a **popup** fail because Tauri blocks `window.open` |
| Notification clicks | not forwarded to the page's `onclick` |
| Zoom | CSS zoom, not native browser zoom |
| Cross-compilation | unsupported; build per OS |
| Profile isolation | `--config` shares cookies; two accounts on the same site need two builds |

## License & logos

MIT — see [LICENSE](./LICENSE).

Third-party logos (WhatsApp, Telegram, other brands) are not included because each is
protected by its own trademark. Let the runner fetch the site favicon, or use `icon_url`
with a logo you own. See [branding/README.md](./branding/README.md).
