const $ = (id) => document.getElementById(id);
const statusEl = $("status");
const form = $("form");
const tauri = window.__TAURI__;
const invoke = tauri && tauri.core && tauri.core.invoke;

function showStatus(text, ok = true) {
  statusEl.hidden = false;
  statusEl.textContent = text;
  statusEl.className = `status ${ok ? "ok" : "err"}`;
}

function splitLines(value) {
  return value
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);
}

async function prefill() {
  if (!invoke) {
    showStatus("This page must be opened from the desktop app, not a browser.", false);
    return;
  }
  try {
    const cfg = await invoke("get_config");
    if (!cfg) return;
    if (cfg.url) $("url").value = cfg.url;
    if (cfg.title) $("title").value = cfg.title;
    if (cfg.width) $("width").value = cfg.width;
    if (cfg.height) $("height").value = cfg.height;
    if (cfg.shortcut) $("shortcut").value = cfg.shortcut;
    if (cfg.userAgent) $("ua").value = cfg.userAgent;
    if (Array.isArray(cfg.inject)) $("inject").value = cfg.inject.join("\n");
    $("tray").checked = Boolean(cfg.tray);
    $("autostart").checked = Boolean(cfg.autostart);
  } catch (err) {
    showStatus(`Could not read the config: ${err}`, false);
  }
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  if (!invoke) {
    showStatus("Tauri IPC is not available.", false);
    return;
  }

  const url = $("url").value.trim();
  if (!/^https?:\/\//i.test(url)) {
    showStatus("The URL must start with http:// or https://", false);
    return;
  }

  const config = {
    url,
    title: $("title").value.trim() || "Webview",
    width: Math.max(320, Number($("width").value) || 1200),
    height: Math.max(240, Number($("height").value) || 800),
    tray: $("tray").checked,
    autostart: $("autostart").checked,
    shortcut: $("shortcut").value.trim(),
    userAgent: $("ua").value.trim(),
    inject: splitLines($("inject").value),
  };

  const button = $("save");
  button.disabled = true;
  try {
    const savedTo = await invoke("save_config", { config });
    showStatus(`Saved to ${savedTo}. Opening…`);
    window.location.replace(url);
  } catch (err) {
    showStatus(`Could not save: ${err}`, false);
    button.disabled = false;
  }
});

prefill();
