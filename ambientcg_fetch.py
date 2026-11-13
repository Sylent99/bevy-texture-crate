#!/usr/bin/env python3
"""
Download ambientCG assets listed in a manifest, straight to your machine.

    python ambientcg_fetch.py manifest.json [--out textures] [--no-extract] [--force]
(also accepts a material palette file, see palette.example.json)

manifest.json (Claude can write this for you):
{
  "assets": [
    {"id": "PavingStones137", "resolution": "2K", "format": "JPG"},
    {"id": "Rock030",         "resolution": "4K", "format": "PNG"},
    {"id": "PoolHDRI001",     "resolution": "2K", "format": "HDR"},
    {"file": "SomeAsset_1K-JPG.zip"}          # or name an exact file
  ]
}
Files land in <out>/<id>/ . Standard library only. ambientCG assets are CC0.
"""
import argparse, io, json, re, sys, time, urllib.parse, urllib.request, zipfile
from pathlib import Path

API = "https://ambientcg.com/api/v3/assets"
GET = "https://ambientcg.com/get?file="
UA = {"User-Agent": "ambientcg-fetch/0.1 (community tool)"}


def http(url):
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def walk_strings(o):
    if isinstance(o, str):
        yield o
    elif isinstance(o, dict):
        for v in o.values():
            yield from walk_strings(v)
    elif isinstance(o, list):
        for v in o:
            yield from walk_strings(v)


def resolve_url(item):
    """Find the download URL for an item. Uses the API's own download list when
    possible (schema-agnostic), else falls back to the known get?file= pattern."""
    if item.get("file"):
        return GET + item["file"], item["file"]
    aid, res, fmt = item["id"], item.get("resolution", "1K"), item.get("format", "JPG")
    token = f"_{res}-{fmt}".lower()
    try:
        q = urllib.parse.urlencode({"id": aid, "include": "downloads"})
        data = json.loads(http(f"{API}?{q}"))
        urls = [s for s in walk_strings(data) if s.startswith("http") and "file=" in s]
        match = [u for u in urls if token in u.lower() and u.lower().endswith(".zip")] or \
                [u for u in urls if token in u.lower()]
        if match:
            fname = urllib.parse.parse_qs(urllib.parse.urlparse(match[0]).query).get("file", [f"{aid}{token}.zip"])[0]
            return match[0], fname
    except Exception as e:
        print(f"  (API lookup failed for {aid}: {e}; trying direct pattern)")
    fname = f"{aid}_{res}-{fmt}.zip"
    return GET + fname, fname


def safe_extract(data, dest):
    dest.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(io.BytesIO(data)) as z:
        for m in z.namelist():
            if not (dest / m).resolve().is_relative_to(dest.resolve()):
                raise ValueError(f"unsafe path in zip: {m}")
        z.extractall(dest)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("manifest")
    ap.add_argument("--out", default="textures")
    ap.add_argument("--no-extract", action="store_true")
    ap.add_argument("--force", action="store_true", help="re-download even if present")
    a = ap.parse_args()

    doc = json.load(open(a.manifest))
    if "roles" in doc:  # a material palette: {"roles": {"role_name": {id, resolution, format}}}
        items = [dict(v, role=k) for k, v in doc["roles"].items()]
    else:
        items = doc["assets"]
    out = Path(a.out)
    ok = fail = 0
    for it in items:
        aid = it.get("id") or re.split(r"_\d+K-", it.get("file", "asset"))[0]
        folder = out / aid
        if folder.exists() and any(folder.iterdir()) and not a.force:
            print(f"{aid}: already present, skipping (use --force to re-download)")
            ok += 1
            continue
        try:
            url, fname = resolve_url(it)
            print(f"{aid}: downloading {fname}")
            data = http(url)
            folder = out / aid
            folder.mkdir(parents=True, exist_ok=True)
            if a.no_extract or not zipfile.is_zipfile(io.BytesIO(data)):
                (folder / fname).write_bytes(data)
            else:
                safe_extract(data, folder)
            ok += 1
        except Exception as e:
            fail += 1
            print(f"  FAILED {aid}: {e}", file=sys.stderr)
        time.sleep(0.5)
    print(f"Done. {ok} ok, {fail} failed. Files in {out.resolve()}")


if __name__ == "__main__":
    main()
