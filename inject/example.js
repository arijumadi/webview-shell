// Example inject script. Loaded when the config contains
// "inject": ["inject/example.js"].
// Runs before the HTML is parsed, so it is safe for patching APIs or adding styles.
(() => {
  console.log("[webview-shell] inject loaded");

  window.addEventListener("DOMContentLoaded", () => {
    const style = document.createElement("style");
    style.textContent = `
      /* example: hide promo elements on the target site */
      .ads, .promo-banner { display: none !important; }
    `;
    document.head.appendChild(style);

    const badge = document.createElement("div");
    badge.textContent = "desktop";
    badge.style.cssText =
      "position:fixed;right:8px;bottom:8px;z-index:2147483647;" +
      "font:11px/1.4 system-ui,sans-serif;padding:2px 7px;border-radius:999px;" +
      "background:rgba(0,0,0,.55);color:#fff;pointer-events:none;opacity:.7";
    document.body.appendChild(badge);
  });
})();
