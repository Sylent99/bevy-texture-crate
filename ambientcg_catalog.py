#!/usr/bin/env python3
"""
Build a hosted-friendly catalog of the whole ambientCG library (metadata only).

    python ambientcg_catalog.py

Writes into ./catalog/ :
    index.csv                      type,category,count,file_url   <- Claude starts here
    <type>/<Category>.csv          id,title,tags,description     <- small files, one per category
    full.jsonl                     raw records, for local/power use

Set CATALOG_BASE_URL to where ./catalog/ will be hosted, so index.csv contains full URLs
(e.g. https://raw.githubusercontent.com/USER/REPO/main/catalog). Claude can only fetch
URLs it has been given or has seen, so the index must carry the full links.

Standard library only. Needs internet. Be polite: it pauses between requests.
"""
import csv, json, os, re, time, urllib.parse, urllib.request
from collections import defaultdict
from pathlib import Path

BASE = "https://ambientcg.com/api/v3/assets"
INCLUDE = "title,type,technique,tags,shortDescription,releaseDate,url"
PAGE = 500
UA = {"User-Agent": "ambientcg-catalog/0.2 (community tool)"}
OUT = Path("catalog")
BASE_URL = os.environ.get("CATALOG_BASE_URL", "").rstrip("/")


def get(params):
    url = BASE + "?" + urllib.parse.urlencode(params)
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=60) as r:
        return json.load(r)


def records_from(data):
    """v3 response shape is not fully documented, so find the asset list generically."""
    if isinstance(data, list):
        return data
    if isinstance(data, dict):
        for k in ("assets", "foundAssets", "results", "data", "items"):
            if isinstance(data.get(k), list):
                return data[k]
        for v in data.values():
            if isinstance(v, list) and v and isinstance(v[0], dict):
                return v
    return []


def pick(rec, *keys, default=""):
    for k in keys:
        if k in rec and rec[k] not in (None, ""):
            return rec[k]
    return default


def tags_text(rec):
    t = pick(rec, "tags", default=[])
    if isinstance(t, list):
        return ";".join(str(x.get("name", x)) if isinstance(x, dict) else str(x) for x in t)
    return str(t)


def category_of(asset_id):
    """PavingStones137 -> PavingStones, Metal049A -> Metal, 3DApple001 -> 3DApple."""
    m = re.match(r"^(.*?)\d+[A-Za-z]?$", asset_id)
    return m.group(1) if m and m.group(1) else asset_id


def safe(s):
    return re.sub(r"[^A-Za-z0-9_-]", "_", str(s)) or "unknown"


def main():
    recs, offset = [], 0
    while True:
        data = get({"include": INCLUDE, "limit": PAGE, "offset": offset, "sort": "alphabet"})
        page = records_from(data)
        if not page:
            if offset == 0:
                print("Unexpected response shape; first 500 chars:\n", json.dumps(data)[:500])
            break
        recs.extend(page)
        print(f"fetched {len(recs)} assets...")
        if len(page) < PAGE:
            break
        offset += PAGE
        time.sleep(0.5)
    if not recs:
        raise SystemExit("No records fetched; not overwriting the catalog.")

    OUT.mkdir(exist_ok=True)
    groups = defaultdict(list)
    for r in recs:
        aid = str(pick(r, "id", "assetId", "assetID"))
        if not aid:
            continue
        typ = safe(pick(r, "type", default="unknown"))
        desc = str(pick(r, "shortDescription"))[:120].replace("\n", " ")
        groups[(typ, category_of(aid))].append([aid, pick(r, "title"), tags_text(r), desc])

    index_rows = []
    for (typ, cat), rows in sorted(groups.items()):
        d = OUT / typ
        d.mkdir(exist_ok=True)
        with open(d / f"{safe(cat)}.csv", "w", newline="", encoding="utf-8") as f:
            w = csv.writer(f)
            w.writerow(["id", "title", "tags", "description"])
            w.writerows(sorted(rows))
        rel = f"{typ}/{safe(cat)}.csv"
        index_rows.append([typ, cat, len(rows), f"{BASE_URL}/{rel}" if BASE_URL else rel])

    with open(OUT / "index.csv", "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["type", "category", "count", "file_url"])
        w.writerows(index_rows)
    with open(OUT / "full.jsonl", "w", encoding="utf-8") as f:
        for r in recs:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    print(f"Done: {len(recs)} assets, {len(index_rows)} category files in {OUT}/")
    if not BASE_URL:
        print("Tip: set CATALOG_BASE_URL so index.csv contains full URLs.")


if __name__ == "__main__":
    main()
