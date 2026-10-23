#!/usr/bin/env python3
"""Grab the best icon available from a site, for use as an app icon.

Lookup order:
  1. <link rel="...icon..."> (SVG is preferred because it stays sharp at every size)
  2. icons declared in the web app manifest
  3. /favicon.ico

Supported formats: SVG (used as-is), PNG/ICO/JPEG/GIF/WebP (padded to a square and
upscaled to 1024px).

Usage:
    python fetch-icon.py https://example.com branding/icon

Prints the path it wrote to stdout (extension depends on the source format) and
logs to stderr.
"""
import io
import json
import re
import sys
import urllib.request
from urllib.parse import urljoin

UA = (
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 "
    "(KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36"
)
TIMEOUT = 20


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": UA, "Accept": "*/*"})
    with urllib.request.urlopen(request, timeout=TIMEOUT) as response:
        return response.read(), response.headers.get("content-type", "")


def sniff(data):
    head = data[:1024].lstrip()
    if head.startswith(b"<?xml") or b"<svg" in head:
        return "svg"
    if data[:8] == b"\x89PNG\r\n\x1a\n":
        return "png"
    if data[:4] == b"\x00\x00\x01\x00":
        return "ico"
    if data[:3] == b"\xff\xd8\xff":
        return "jpg"
    if data[:6] in (b"GIF87a", b"GIF89a"):
        return "gif"
    if data[:4] == b"RIFF" and data[8:12] == b"WEBP":
        return "webp"
    return None


def collect(base):
    """Return candidates as (score, url). Higher score means a better source."""
    try:
        html = fetch(base)[0].decode("utf-8", "ignore")
    except Exception as error:  # noqa: BLE001
        print(f"warn: could not read {base}: {error}", file=sys.stderr)
        html = ""

    found = []
    links = re.findall(r"<link\b[^>]*>", html, re.I)

    for tag in links:
        rel = re.search(r'rel\s*=\s*["\']([^"\']+)', tag, re.I)
        href = re.search(r'href\s*=\s*["\']([^"\']+)', tag, re.I)
        if not rel or not href:
            continue
        if "icon" not in rel.group(1).lower():
            continue
        target = href.group(1)
        size = re.search(r'sizes\s*=\s*["\'](\d+)', tag, re.I)
        pixels = int(size.group(1)) if size else 0
        score = 10000 if target.lower().split("?")[0].endswith(".svg") else pixels
        found.append((score, urljoin(base, target)))

    for tag in links:
        if "manifest" not in tag.lower():
            continue
        href = re.search(r'href\s*=\s*["\']([^"\']+)', tag, re.I)
        if not href:
            continue
        try:
            manifest = json.loads(
                fetch(urljoin(base, href.group(1)))[0].decode("utf-8", "ignore")
            )
        except Exception:  # noqa: BLE001
            continue
        for icon in manifest.get("icons", []):
            src = icon.get("src")
            if not src:
                continue
            size = str(icon.get("sizes", "")).split("x")[0]
            pixels = int(size) if size.isdigit() else 0
            score = 10000 if src.lower().split("?")[0].endswith(".svg") else pixels
            found.append((score, urljoin(base, src)))

    found.append((0, urljoin(base, "/favicon.ico")))
    found.sort(key=lambda item: -item[0])
    return found


def write_raster(data, path):
    from PIL import Image

    image = Image.open(io.BytesIO(data))
    image.load()
    if getattr(image, "is_animated", False):
        image.seek(0)
    image = image.convert("RGBA")

    width, height = image.size
    side = max(width, height)
    if width != height:
        canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
        canvas.paste(image, ((side - width) // 2, (side - height) // 2))
        image = canvas
    if image.size[0] != 1024:
        image = image.resize((1024, 1024), Image.LANCZOS)
    image.save(path)
    return width, height


def main():
    if len(sys.argv) < 3:
        print("usage: fetch-icon.py <site-url> <output-basename>", file=sys.stderr)
        return 1

    url, output = sys.argv[1], sys.argv[2]
    if not url.startswith(("http://", "https://")):
        url = "https://" + url

    for _, candidate in collect(url):
        try:
            data, content_type = fetch(candidate)
        except Exception as error:  # noqa: BLE001
            print(f"skip {candidate}: {error}", file=sys.stderr)
            continue

        kind = sniff(data)
        if not kind:
            print(f"skip {candidate}: not an image ({content_type})", file=sys.stderr)
            continue

        if kind == "svg":
            path = output + ".svg"
            with open(path, "wb") as handle:
                handle.write(data)
            print(f"icon: {candidate} (svg, vector)", file=sys.stderr)
        else:
            path = output + ".png"
            try:
                width, height = write_raster(data, path)
            except Exception as error:  # noqa: BLE001
                print(f"skip {candidate}: {error}", file=sys.stderr)
                continue
            print(f"icon: {candidate} ({kind} {width}x{height} -> png 1024)", file=sys.stderr)
            if max(width, height) < 128:
                print("warn: source icon is tiny, the result will look soft", file=sys.stderr)

        print(path)
        return 0

    print("error: no usable icon found", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
