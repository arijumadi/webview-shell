#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    webview::{NewWindowResponse, PermissionKind, PermissionResponse},
    Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;

/// UA string used by `--ua chrome`. Deliberately looks like plain Chrome:
/// the `Edg/...` suffix that WebView2 appends is what naïve checks look for.
/// Bump the major version occasionally, an ancient Chrome triggers "outdated browser".
const CHROME_UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";

/// Zoom handled in the page instead of by WebView2's IsZoomControlEnabled.
/// WebView2 keeps its zoom factor internal and Tauri exposes no getter, so a
/// native zoom could never be remembered. This shim applies CSS zoom, stores it
/// per origin in localStorage, and restores it before the page scripts run.
const ZOOM_SHIM: &str = r#"
(() => {
  if (window.top !== window) return; // top frame only, never iframes

  const KEY = "__webview_shell_zoom";
  const MIN = 0.5;
  const MAX = 3;
  const STEP = 0.1;
  let current = 1;

  const clamp = (value) => Math.min(MAX, Math.max(MIN, Math.round(value * 100) / 100));

  const paint = () => {
    const root = document.documentElement;
    if (!root) return false;
    root.style.zoom = current === 1 ? "" : String(current);
    return true;
  };

  const apply = (value, persist) => {
    current = clamp(value);
    if (!paint()) {
      // documentElement may not exist yet at document-start; retry on the next frames
      // so the page never renders once at 100% before the saved zoom kicks in.
      let tries = 0;
      const wait = () => {
        if (paint() || ++tries > 120) return;
        requestAnimationFrame(wait);
      };
      requestAnimationFrame(wait);
    }
    if (persist) {
      try { localStorage.setItem(KEY, String(current)); } catch (_) {}
    }
  };

  try {
    const saved = parseFloat(localStorage.getItem(KEY) || "1");
    if (Number.isFinite(saved) && saved >= MIN && saved <= MAX) current = saved;
  } catch (_) {}

  if (current !== 1) apply(current, false);

  addEventListener(
    "keydown",
    (event) => {
      if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
      if (event.key === "+" || event.key === "=") {
        event.preventDefault();
        apply(current + STEP, true);
      } else if (event.key === "-" || event.key === "_") {
        event.preventDefault();
        apply(current - STEP, true);
      } else if (event.key === "0") {
        event.preventDefault();
        apply(1, true);
      }
    },
    true
  );

  addEventListener(
    "wheel",
    (event) => {
      if (!event.ctrlKey) return;
      event.preventDefault();
      apply(current + (event.deltaY < 0 ? STEP : -STEP), true);
    },
    { passive: false, capture: true }
  );
})();
"#;

/// Turns the page's `Notification` API into real Windows toasts.
///
/// WebView2 never surfaces them on its own, and the site cannot reach Tauri
/// unless a capability grants its origin remote IPC (see capabilities/remote.json).
/// If IPC is unavailable this shim steps aside and leaves the original API alone.
const NOTIFY_SHIM: &str = r#"
(() => {
  // No early return when __TAURI__ is missing: remote pages may not get the global,
  // so fall back to a navigation the Rust side intercepts (same trick as links).
  const api = window.__TAURI__ && window.__TAURI__.core;
  const invoke = api && typeof api.invoke === "function" ? api.invoke : null;
  const HOP = "https://webview-shell-notify.invalid/?";

  const hop = (title, body) => {
    window.location.href =
      HOP + "title=" + encodeURIComponent(title) + "&body=" + encodeURIComponent(body || "");
  };

  const forward = (title, body) => {
    const safeTitle = String(title == null ? "" : title);
    const safeBody = String(body == null ? "" : body);
    if (invoke) {
      invoke("notify", { title: safeTitle, body: safeBody }).catch(() => hop(safeTitle, safeBody));
    } else {
      hop(safeTitle, safeBody);
    }
  };

  // Toast clicks arrive from Rust via eval; the page object is kept here so its
  // onclick handler can run. Only the newest one is remembered, which is what chat
  // apps need (focus the window, open the latest chat).
  const pending = [];
  window.__webviewShellNotifyClick = function () {
    const latest = pending[pending.length - 1];
    if (!latest) return;
    pending.length = 0;
    const event = new Event("click");
    try {
      if (typeof latest.onclick === "function") latest.onclick(event);
    } catch (_) {}
    latest.dispatchEvent(event);
  };

  const Original = window.Notification;

  class BridgedNotification extends EventTarget {
    constructor(title, options) {
      super();
      options = options || {};
      this.title = String(title == null ? "" : title);
      this.body = String(options.body || "");
      this.tag = String(options.tag || "");
      this.onclick = null;
      this.onshow = null;
      this.onclose = null;
      this.onerror = null;
      this._closed = false;

      const close = () => {
        if (this._closed) return;
        this._closed = true;
        const event = new Event("close");
        if (typeof this.onclose === "function") this.onclose(event);
        this.dispatchEvent(event);
      };
      this._close = close;

      pending.push(this);
      if (pending.length > 10) pending.shift();

      forward(this.title, this.body);
      const event = new Event("show");
      if (typeof this.onshow === "function") this.onshow(event);
      this.dispatchEvent(event);
    }

    close() {
      if (this._close) this._close();
    }

    static get permission() {
      return "granted";
    }

    static requestPermission(callback) {
      if (typeof callback === "function") callback("granted");
      return Promise.resolve("granted");
    }
  }

  if (Original) {
    Object.defineProperty(window, "Notification", {
      value: BridgedNotification,
      configurable: true,
      writable: true,
    });
  }

  // Sites that go through a service worker instead of `new Notification`.
  const registration = window.ServiceWorkerRegistration;
  if (registration && registration.prototype && registration.prototype.showNotification) {
    const original = registration.prototype.showNotification;
    registration.prototype.showNotification = function (title, options) {
      forward(title, options && options.body);
      return Promise.resolve();
    };
    registration.prototype.__webviewShellOriginalShowNotification = original;
  }
})();
"#;

/// Routes link clicks that would open a popup to the system browser.
///
/// WebView2 popup handling proved unreliable for this case, so this works in layers:
///   1. IPC command `open_external` (same channel as notifications)
///   2. fallback: navigate to an https hop host that the Rust side intercepts
///      in `on_navigation` and hands to the system browser (no OS dialog)
///   3. `window.open` is also wrapped, for sites that skip the anchor element
const LINKS_SHIM: &str = r#"
(() => {
  if (window.top !== window) return;

  const api = window.__TAURI__ && window.__TAURI__.core;
  const invoke = api && typeof api.invoke === "function" ? api.invoke : null;

  // A custom scheme makes WebView2 show an "open link?" dialog that cannot succeed.
  // Navigate to an https host we intercept in on_navigation instead.
  const HOP = "https://webview-shell-open.invalid/?url=";

  const handoff = (url) => {
    window.location.href = HOP + encodeURIComponent(url);
  };

  const openExternal = (url) => {
    if (invoke) {
      invoke("open_external", { url }).catch(() => handoff(url));
    } else {
      handoff(url);
    }
  };

  const isHttp = (value) => /^https?:\/\//i.test(String(value || ""));

  document.addEventListener(
    "click",
    (event) => {
      const target = event.target;
      if (!target || typeof target.closest !== "function") return;
      const anchor = target.closest("a[href]");
      if (!anchor) return;

      const href = anchor.href;
      if (!isHttp(href)) return;

      // Only links that ask for a new window/tab; same-tab navigation stays in-app.
      if ((anchor.getAttribute("target") || "").toLowerCase() !== "_blank") return;

      event.preventDefault();
      event.stopPropagation();
      openExternal(href);
    },
    true
  );

  const nativeOpen = window.open;
  window.open = function (url) {
    const target = String(url == null ? "" : url);

    if (isHttp(target)) {
      openExternal(target);
      return null;
    }

    // Sites often do `const w = window.open(); w.location = url;`. Returning null
    // makes that throw, so hand back a stub that forwards the assignment.
    if (!target) {
      return {
        closed: false,
        focus() {},
        blur() {},
        close() {},
        postMessage() {},
        location: {
          set href(value) {
            if (isHttp(value)) openExternal(String(value));
          },
        },
      };
    }

    return nativeOpen.apply(window, arguments);
  };
})();
"#;

/// Everything the shell needs to know, resolved from CLI flags on top of the
/// JSON config that sits next to the executable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct AppConfig {
    url: String,
    title: String,
    width: f64,
    height: f64,
    tray: bool,
    autostart: bool,
    shortcut: String,
    inject: Vec<String>,
    #[serde(rename = "userAgent", alias = "user_agent")]
    user_agent: String,
    #[serde(rename = "zoomHotkeys", alias = "zoom_hotkeys")]
    zoom_hotkeys: bool,
    notifications: bool,
    #[serde(rename = "closeToTray", alias = "close_to_tray")]
    close_to_tray: bool,
    #[serde(rename = "openExternalInBrowser", alias = "open_external_in_browser")]
    open_external_in_browser: bool,
    #[serde(rename = "allowedHosts", alias = "allowed_hosts")]
    allowed_hosts: Vec<String>,
    camera: bool,
    microphone: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            url: String::new(),
            title: "Webview".to_string(),
            width: 1200.0,
            height: 800.0,
            tray: true,
            autostart: false,
            shortcut: String::new(),
            inject: Vec::new(),
            user_agent: String::new(),
            zoom_hotkeys: true,
            notifications: true,
            close_to_tray: false,
            open_external_in_browser: false,
            allowed_hosts: Vec::new(),
            camera: false,
            microphone: false,
        }
    }
}

struct AppState {
    config_path: PathBuf,
    config: Mutex<AppConfig>,
}

/// Last window geometry. Read *before* the window is created so it opens at the
/// right size immediately, with no visible resize after startup.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WindowState {
    #[serde(default)]
    width: f64,
    #[serde(default)]
    height: f64,
    #[serde(default)]
    x: Option<i32>,
    #[serde(default)]
    y: Option<i32>,
    #[serde(default)]
    maximized: bool,
}

struct WindowStateStore {
    path: PathBuf,
    state: Mutex<WindowState>,
}

fn load_window_state(path: &PathBuf) -> Option<WindowState> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
}

/// Keep the saved position only if it still lands on a connected monitor,
/// otherwise a resolution change would reopen the window off-screen.
fn position_is_reachable(app: &tauri::AppHandle, x: i32, y: i32) -> bool {
    let Ok(monitors) = app.available_monitors() else {
        return false;
    };
    monitors.iter().any(|monitor| {
        let origin = monitor.position();
        let size = monitor.size();
        let right = origin.x + size.width as i32;
        let bottom = origin.y + size.height as i32;
        x + 80 > origin.x && x < right - 40 && y + 40 > origin.y && y < bottom - 40
    })
}

fn save_geometry(
    store: &WindowStateStore,
    maximized: bool,
    size: (u32, u32),
    position: Option<(i32, i32)>,
) {
    let mut snapshot = match store.state.lock() {
        Ok(state) => state.clone(),
        Err(_) => return,
    };

    snapshot.maximized = maximized;
    // While maximized the reported size is the screen, not the restored one.
    if !maximized {
        snapshot.width = size.0 as f64;
        snapshot.height = size.1 as f64;
        if let Some((x, y)) = position {
            snapshot.x = Some(x);
            snapshot.y = Some(y);
        }
    }

    if let Some(dir) = store.path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string_pretty(&snapshot) {
        let _ = std::fs::write(&store.path, json);
    }
    if let Ok(mut state) = store.state.lock() {
        *state = snapshot;
    }
}

fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|dir| dir.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Pull `--url <value>` out of a command line, used when a second launch should
/// retarget the already running window.
fn url_from_args(args: &[String]) -> Option<&String> {
    args.iter()
        .position(|arg| arg == "--url")
        .and_then(|index| args.get(index + 1))
}

fn find_config_arg(args: &[String]) -> Option<PathBuf> {
    let mut index = 0;
    while index < args.len() {
        if args[index] == "--config" {
            if let Some(value) = args.get(index + 1) {
                return Some(PathBuf::from(value));
            }
        }
        index += 1;
    }
    None
}

fn load_config(path: &PathBuf) -> AppConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Defaults baked in at build time. `make-app.ps1` writes this env var through
/// `.cargo/config.toml`, so a generated build already knows its URL and features
/// without shipping a sidecar JSON file.
fn build_defaults() -> AppConfig {
    option_env!("WEBVIEW_SHELL_DEFAULT_CONFIG")
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_default()
}

/// CLI flags win over the config file, so a single exe can be retargeted
/// without editing JSON.
fn apply_args(config: &mut AppConfig, args: &[String]) {
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].as_str();
        let value = args.get(index + 1).cloned();

        match flag {
            "--url" => {
                if let Some(value) = value {
                    config.url = value;
                    index += 1;
                }
            }
            "--title" => {
                if let Some(value) = value {
                    config.title = value;
                    index += 1;
                }
            }
            "--width" => {
                if let Some(value) = value {
                    if let Ok(parsed) = value.parse::<f64>() {
                        config.width = parsed;
                    }
                    index += 1;
                }
            }
            "--height" => {
                if let Some(value) = value {
                    if let Ok(parsed) = value.parse::<f64>() {
                        config.height = parsed;
                    }
                    index += 1;
                }
            }
            "--shortcut" => {
                if let Some(value) = value {
                    config.shortcut = value;
                    index += 1;
                }
            }
            "--inject" => {
                if let Some(value) = value {
                    config.inject.push(value);
                    index += 1;
                }
            }
            "--ua" | "--user-agent" => {
                if let Some(value) = value {
                    config.user_agent = match value.to_ascii_lowercase().as_str() {
                        "chrome" => CHROME_UA.to_string(),
                        "default" | "webview" => String::new(),
                        _ => value,
                    };
                    index += 1;
                }
            }
            "--config" => index += 1,
            "--tray" => config.tray = true,
            "--no-tray" => config.tray = false,
            "--autostart" => config.autostart = true,
            "--no-autostart" => config.autostart = false,
            "--zoom" => config.zoom_hotkeys = true,
            "--no-zoom" => config.zoom_hotkeys = false,
            "--notify" => config.notifications = true,
            "--no-notify" => config.notifications = false,
            "--close-to-tray" => config.close_to_tray = true,
            "--no-close-to-tray" => config.close_to_tray = false,
            "--external-in-browser" => config.open_external_in_browser = true,
            "--no-external-in-browser" => config.open_external_in_browser = false,
            "--allow-host" => {
                if let Some(value) = value {
                    config.allowed_hosts.push(value);
                    index += 1;
                }
            }
            "--camera" => config.camera = true,
            "--no-camera" => config.camera = false,
            "--microphone" => config.microphone = true,
            "--no-microphone" => config.microphone = false,
            _ => {}
        }

        index += 1;
    }
}

fn toggle_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            _ => {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }
}

/// Local pages only. Remote pages are granted IPC exclusively for notifications,
/// so the config commands have to reject them explicitly.
fn caller_is_local(window: &tauri::WebviewWindow) -> bool {
    match window.url() {
        Ok(url) => url.scheme() == "tauri" || url.host_str() == Some("tauri.localhost"),
        Err(_) => false,
    }
}

#[tauri::command]
fn get_config(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
) -> Result<AppConfig, String> {
    if !caller_is_local(&window) {
        return Err("not available to remote pages".into());
    }
    Ok(state
        .config
        .lock()
        .map(|config| config.clone())
        .unwrap_or_default())
}

#[tauri::command]
fn open_external(app: tauri::AppHandle, url: String) -> Result<(), String> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err("only http(s) URLs can be opened".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())
}

/// Shared toast path for both transports (IPC command and navigation hop).
fn show_toast(app: &tauri::AppHandle, title: &str, body: &str) -> Result<(), String> {
    // Cap the payload so a page cannot flood the toast queue with huge strings.
    let title: String = title.chars().take(120).collect();
    let body: String = body.chars().take(400).collect();

    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn notify(app: tauri::AppHandle, title: String, body: String) -> Result<(), String> {
    show_toast(&app, &title, &body)
}

#[tauri::command]
fn save_config(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, AppState>,
    config: AppConfig,
) -> Result<String, String> {
    if !caller_is_local(&window) {
        return Err("not available to remote pages".into());
    }

    let json = serde_json::to_string_pretty(&config).map_err(|error| error.to_string())?;

    if let Some(dir) = state.config_path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    }
    std::fs::write(&state.config_path, json).map_err(|error| error.to_string())?;

    if let Ok(mut current) = state.config.lock() {
        *current = config;
    }

    Ok(state.config_path.display().to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let config_path =
        find_config_arg(&args).unwrap_or_else(|| exe_dir().join("webview-shell.json"));

    let mut config = if config_path.exists() {
        load_config(&config_path)
    } else {
        build_defaults()
    };
    apply_args(&mut config, &args);

    tauri::Builder::default()
        // Must be registered first: a second launch has to exit before any other
        // plugin initialises, and instead focus the window that is already running.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
                if let Some(raw) = url_from_args(&argv) {
                    if let Ok(url) = raw.parse::<tauri::Url>() {
                        let _ = window.navigate(url);
                    }
                }
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            let Some(store) = window.app_handle().try_state::<WindowStateStore>() else {
                return;
            };
            match event {
                tauri::WindowEvent::Resized(size) => {
                    if let Ok(false) = window.is_maximized() {
                        if let Ok(mut state) = store.state.lock() {
                            state.width = size.width as f64;
                            state.height = size.height as f64;
                        }
                    }
                }
                tauri::WindowEvent::Moved(position) => {
                    if let Ok(mut state) = store.state.lock() {
                        state.x = Some(position.x);
                        state.y = Some(position.y);
                    }
                }
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // Close-to-tray: only meaningful when there is a tray to come back from.
                    if let Some(app_state) = window.app_handle().try_state::<AppState>() {
                        let hide = app_state
                            .config
                            .lock()
                            .map(|config| config.close_to_tray && config.tray)
                            .unwrap_or(false);
                        if hide {
                            api.prevent_close();
                            let _ = window.hide();
                        }
                    }
                    if let (Ok(maximized), Ok(size)) =
                        (window.is_maximized(), window.inner_size())
                    {
                        let position = window.outer_position().ok().map(|p| (p.x, p.y));
                        save_geometry(&store, maximized, (size.width, size.height), position);
                    }
                }
                tauri::WindowEvent::Destroyed | tauri::WindowEvent::Focused(false) => {
                    if let (Ok(maximized), Ok(size)) =
                        (window.is_maximized(), window.inner_size())
                    {
                        let position = window.outer_position().ok().map(|p| (p.x, p.y));
                        save_geometry(&store, maximized, (size.width, size.height), position);
                    }
                }
                _ => {}
            }
        })
        .manage(AppState {
            config_path: config_path.clone(),
            config: Mutex::new(config.clone()),
        })
        .setup(move |app| {
            let handle = app.handle().clone();

            // Toast click -> bring the window back and let the page run its own onclick.
            // Windows only reports this while the toast is still on screen.
            let notification_handle = handle.clone();
            let _ = app.notification().on_action(move |action| {
                if action.action_id() != "tap" {
                    return;
                }
                if let Some(window) = notification_handle.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.unminimize();
                    let _ = window.set_focus();
                    let _ = window.eval(
                        "window.__webviewShellNotifyClick && window.__webviewShellNotifyClick();",
                    );
                }
            });

            let state_dir = app.path().app_config_dir().unwrap_or_else(|_| exe_dir());
            let window_state_path = state_dir.join("window-state.json");
            let saved_state = load_window_state(&window_state_path);
            // Seed with the configured size so a first-run maximize never records 0x0.
            let initial_state = saved_state.clone().unwrap_or_else(|| WindowState {
                width: config.width,
                height: config.height,
                x: None,
                y: None,
                maximized: false,
            });
            app.manage(WindowStateStore {
                path: window_state_path,
                state: Mutex::new(initial_state),
            });

            // Autostart is declarative: config on = registered, off = removed.
            let autolaunch = app.autolaunch();
            if config.autostart {
                let _ = autolaunch.enable();
            } else {
                let _ = autolaunch.disable();
            }

            // User scripts are injected before the page parses, so they can patch
            // fetch/localStorage or hide elements before the app boots.
            let mut scripts = String::new();
            for entry in &config.inject {
                let path = PathBuf::from(entry);
                let path = if path.is_absolute() {
                    path
                } else {
                    exe_dir().join(path)
                };
                match std::fs::read_to_string(&path) {
                    Ok(source) => {
                        scripts.push_str(&source);
                        scripts.push_str("\n;\n");
                    }
                    Err(error) => eprintln!("inject {}: {}", path.display(), error),
                }
            }

            if config.zoom_hotkeys {
                scripts.push_str(ZOOM_SHIM);
                scripts.push_str("\n;\n");
            }

            if config.notifications {
                scripts.push_str(NOTIFY_SHIM);
                scripts.push_str("\n;\n");
            }

            scripts.push_str(LINKS_SHIM);
            scripts.push_str("\n;\n");

            let start_url = if config.url.trim().is_empty() {
                WebviewUrl::App("index.html".into())
            } else {
                match config.url.parse() {
                    Ok(url) => WebviewUrl::External(url),
                    Err(_) => WebviewUrl::App("index.html".into()),
                }
            };

            let mut builder = WebviewWindowBuilder::new(&handle, "main", start_url)
                .title(config.title.as_str())
                .inner_size(config.width, config.height)
                .min_inner_size(480.0, 360.0)
                .center()
                // Tauri replaces WebView2's drag-drop handler by default, which hides
                // HTML5 drops from the page. The shell never uses Tauri's DragDropEvent,
                // so hand drops back to the wrapped site (file upload by dragging).
                .disable_drag_drop_handler()
                // Native zoom is off on purpose: the injected shim owns zoom so the
                // level can be persisted per site (WebView2 exposes no zoom getter).
                .zoom_hotkeys_enabled(false);

            // `window.open` has no home in a single-window shell, so send popups to the
            // system browser instead of dropping them silently.
            let link_handle = handle.clone();
            builder = builder.on_new_window(move |url, _features| {
                let _ = link_handle.opener().open_url(url.as_str(), None::<&str>);
                NewWindowResponse::Deny
            });

            // Navigation guard. Always installed so the link-shim hop handoff
            // from the click shim is caught; the cross-host rule itself is opt-in,
            // because OAuth redirects have to stay inside the webview.
            let nav_handle = handle.clone();
            let allow_external = config.open_external_in_browser;
            let internal_host = config
                .url
                .parse::<tauri::Url>()
                .ok()
                .and_then(|url| url.host_str().map(|host| host.to_string()));
            let allowed_hosts = config.allowed_hosts.clone();
            builder = builder.on_navigation(move |url| {
                if url.host_str() == Some("webview-shell-notify.invalid") {
                    let mut title = String::new();
                    let mut body = String::new();
                    for (key, value) in url.query_pairs() {
                        match key.as_ref() {
                            "title" => title = value.to_string(),
                            "body" => body = value.to_string(),
                            _ => {}
                        }
                    }
                    if !title.is_empty() {
                        let _ = show_toast(&nav_handle, &title, &body);
                    }
                    return false;
                }

                if url.host_str() == Some("webview-shell-open.invalid") {
                    let target = url
                        .query_pairs()
                        .find(|(key, _)| key == "url")
                        .map(|(_, value)| value.to_string());
                    if let Some(target) = target {
                        if target.starts_with("http://") || target.starts_with("https://") {
                            let _ = nav_handle.opener().open_url(target, None::<&str>);
                        }
                    }
                    return false;
                }
                if !allow_external || !matches!(url.scheme(), "http" | "https") {
                    return true;
                }
                let host = url.host_str().unwrap_or("");
                let matches = |candidate: &str| {
                    host == candidate || host.ends_with(&format!(".{candidate}"))
                };
                let internal = internal_host.as_deref().map(matches).unwrap_or(false)
                    || allowed_hosts.iter().any(|allowed| matches(allowed));
                if internal {
                    return true;
                }
                let _ = nav_handle.opener().open_url(url.as_str(), None::<&str>);
                false
            });

            let camera = config.camera;
            let microphone = config.microphone;
            builder = builder.on_permission_request(move |_webview, kind| match kind {
                PermissionKind::Camera if camera => PermissionResponse::Allow,
                PermissionKind::Microphone if microphone => PermissionResponse::Allow,
                _ => PermissionResponse::Default,
            });

            // Applied before the window is created, so it opens at the remembered
            // size instead of jumping there a moment later.
            if let Some(state) = &saved_state {
                if state.width >= 320.0 && state.height >= 240.0 {
                    builder = builder.inner_size(state.width, state.height);
                }
                if let (Some(x), Some(y)) = (state.x, state.y) {
                    if position_is_reachable(&handle, x, y) {
                        builder = builder.position(x as f64, y as f64);
                    }
                }
                if state.maximized {
                    builder = builder.maximized(true);
                }
            }

            if !scripts.is_empty() {
                builder = builder.initialization_script(scripts);
            }

            // Set on the webview itself so the HTTP User-Agent header changes too,
            // not just navigator.userAgent.
            if !config.user_agent.is_empty() {
                builder = builder.user_agent(config.user_agent.as_str());
            }

            let window = builder.visible(false).build()?;

            // Windows applies the maximized state only after the window exists, which
            // shows one windowed frame first. The window is created hidden, maximized
            // while invisible, then revealed - so it is never seen at the wrong size.
            if saved_state
                .as_ref()
                .map(|state| state.maximized)
                .unwrap_or(false)
            {
                let _ = window.maximize();
            }
            let _ = window.show();
            let _ = window.set_focus();

            if config.tray {
                let toggle = MenuItem::with_id(app, "toggle", "Show / Hide", true, None::<&str>)?;
                let reload = MenuItem::with_id(app, "reload", "Reload", true, None::<&str>)?;
                let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&toggle, &reload, &quit])?;

                let mut tray = TrayIconBuilder::with_id("main")
                    .tooltip(config.title.as_str())
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "toggle" => toggle_window(app),
                        "reload" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.eval("location.reload()");
                            }
                        }
                        "quit" => {
                            if let Some(window) = app.get_webview_window("main") {
                                if let Some(store) = app.try_state::<WindowStateStore>() {
                                    if let (Ok(maximized), Ok(size)) =
                                        (window.is_maximized(), window.inner_size())
                                    {
                                        let position =
                                            window.outer_position().ok().map(|p| (p.x, p.y));
                                        save_geometry(
                                            &store,
                                            maximized,
                                            (size.width, size.height),
                                            position,
                                        );
                                    }
                                }
                            }
                            app.exit(0)
                        }
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            toggle_window(tray.app_handle());
                        }
                    });

                if let Some(icon) = app.default_window_icon() {
                    tray = tray.icon(icon.clone());
                }
                tray.build(app)?;
            }

            if !config.shortcut.is_empty() {
                let shortcut_window = window.clone();
                let result = app.global_shortcut().on_shortcut(
                    config.shortcut.as_str(),
                    move |_app, _shortcut, event| {
                        if event.state == ShortcutState::Pressed {
                            match shortcut_window.is_visible() {
                                Ok(true) => {
                                    let _ = shortcut_window.hide();
                                }
                                _ => {
                                    let _ = shortcut_window.show();
                                    let _ = shortcut_window.set_focus();
                                }
                            }
                        }
                    },
                );
                if let Err(error) = result {
                    eprintln!("global shortcut {:?}: {}", config.shortcut, error);
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_config, save_config, notify, open_external])
        .run(tauri::generate_context!())
        .expect("error while running webview-shell");
}
