// Aligns the JS fingerprint with the UA string, so the wrapper does not read as
// WebView2/Edge. Use together with `"userAgent": "...Chrome/..."` (or `--ua chrome`).
//
// Limitation: this only fixes what JavaScript sees. The `Sec-CH-UA` header is still
// sent by the WebView2 network stack, so a server comparing headers against JS can
// still tell the difference.
(() => {
  const ua = navigator.userAgent.replace(/ Edg\/[\d.]+/, "");
  const majorMatch = ua.match(/Chrome\/(\d+)/);
  const major = majorMatch ? majorMatch[1] : "142";

  const override = (target, prop, value) => {
    try {
      Object.defineProperty(target, prop, { get: () => value, configurable: true });
    } catch (_) {
      /* property is locked - leave it alone */
    }
  };

  override(navigator, "userAgent", ua);
  override(navigator, "appVersion", ua.replace(/^Mozilla\//, ""));
  override(navigator, "vendor", "Google Inc.");
  override(navigator, "platform", "Win32");

  const brands = [
    { brand: "Chromium", version: major },
    { brand: "Google Chrome", version: major },
    { brand: "Not?A_Brand", version: "24" },
  ];
  const fullVersionList = [
    { brand: "Chromium", version: `${major}.0.0.0` },
    { brand: "Google Chrome", version: `${major}.0.0.0` },
    { brand: "Not?A_Brand", version: "24.0.0.0" },
  ];

  const base = {
    brands,
    mobile: false,
    platform: "Windows",
  };

  const highEntropy = {
    platformVersion: "15.0.0",
    architecture: "x86",
    bitness: "64",
    model: "",
    uaFullVersion: `${major}.0.0.0`,
    fullVersionList,
    wow64: false,
  };

  const uaData = {
    ...base,
    getHighEntropyValues: async (hints = []) => {
      const out = { ...base };
      for (const hint of hints) {
        if (Object.prototype.hasOwnProperty.call(highEntropy, hint)) {
          out[hint] = highEntropy[hint];
        }
      }
      return out;
    },
    toJSON: () => ({ ...base }),
  };

  override(navigator, "userAgentData", uaData);
})();
