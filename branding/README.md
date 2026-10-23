# branding/

Any logo used for a branded build must be **yours** or freely licensed.

This repo only ships the repository owner's own artwork (`idxedge.svg`,
`opencode.png`). Third-party logos — WhatsApp, Telegram, and other brands — are
**not** included, because each is protected by its own trademark and must not be
redistributed here.

For apps wrapping a third-party site you have two options at build time:

```bash
# 1. let the runner grab the site favicon (automatic when icon_path/icon_url are empty)
gh workflow run build-branded.yml --ref main \
  -f name="Telegram Web" -f url="https://web.telegram.org/"

# 2. point at a logo you host yourself
gh workflow run build-branded.yml --ref main \
  -f name="App" -f url="https://app.example.com" \
  -f icon_url="https://cdn.example.com/logo-512.png"
```

`make-app.ps1` also accepts `-Icon <path>` for local builds.
