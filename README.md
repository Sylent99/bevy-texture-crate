# ambientCG Toolkit for Procedural Worlds

A small, dependency-free toolkit that lets **Claude choose textures from the entire
ambientCG library** and lets **your computer download only what you actually use**.
No full-library download, no MCP server, no sandbox network access needed.

ambientCG assets are CC0 (public domain). This toolkit is a community tool and is
not affiliated with ambientCG. See https://docs.ambientcg.com/license/ for the
current terms.

---

## The idea in one picture

```
 ambientCG API ──(your PC, once)──> ambientcg_catalog.csv ──(upload)──> Claude
                                                                          │
                          palette.json / manifest.json  <──(Claude writes)┘
                                       │
                 ambientcg_fetch.py ───┘──(your PC)──> textures/<AssetId>/ ──> your game
```

1. **Catalog**: `ambientcg_catalog.py` saves the *metadata* of every asset (id, type,
   title, tags, description) to a CSV. It is small compared with the textures.
2. **Claude as art director**: you give Claude the CSV plus a description of what your
   generator needs. Claude searches it and writes a palette or manifest.
3. **Fetch**: `ambientcg_fetch.py` downloads and unzips exactly those assets, skipping
   anything already on disk.
4. **Game**: your game reads the palette (roles to asset folders) and loads the maps.

---

## Files

| File | Purpose |
|---|---|
| `bevy/ambientcg.rs` | Bevy plugin: reads a palette, downloads missing textures at startup, returns `StandardMaterial`s by role. |
| `bevy/example_main.rs`, `bevy/Cargo.toml.snippet` | Minimal usage example and dependencies. |
| `crate.html` | Open in a browser. Browse the catalog, load Claude's JSON, download selected assets. No install. |
| `HANDOVER.md` | Paste into a chat so Claude knows the catalog rules and the output format. |
| `ambientcg_catalog.py` | Builds `ambientcg_catalog.jsonl` / `.csv` from the API. Run once, re-run to refresh. |
| `ambientcg_fetch.py` | Downloads assets listed in a manifest **or palette**. Skips existing, extracts safely. |
| `manifest.example.json` | Flat list of assets to download. |
| `palette.example.json` | Role-based palette (the format your game should consume). |

Requirements: Python 3.9+, internet on the machine that runs the scripts. Nothing to install.

---

## Bevy: chat-only workflow (the game downloads its own textures)

For Bevy projects the game itself is the downloader. Nothing to run besides `cargo run`.

1. Copy `bevy/ambientcg.rs` to `src/ambientcg.rs` and add `mod ambientcg;` to `main.rs`.
2. Add the dependencies from `bevy/Cargo.toml.snippet`.
3. Add `.add_plugins(AmbientCgPlugin::new("palette.json"))` after `DefaultPlugins`.
4. Chat with Claude (paste `HANDOVER.md`, attach `ambientcg_catalog.csv`) and ask for a
   palette. Save the JSON it gives you as `assets/palette.json`.
5. `cargo run`. Missing textures are downloaded into `assets/ambientcg/` on startup.
6. In your systems, ask for materials by role. See `bevy/example_main.rs`.

Things to know:
- **Meshes need UVs and tangents.** Procedural meshes without tangents will ignore the
  normal map. Call `.with_generated_tangents()` (see the example) or generate your own.
- **Tiling.** `uv_scale` in a palette entry repeats the texture. Size your UVs in
  world units for consistent texel density across walls of different sizes.
- **Metal surfaces.** The plugin treats everything as non-metal. Metals would need
  channel-packed metallic/roughness textures (not implemented).
- **Dev convenience, not shipping.** Downloading on startup suits development. For a
  release, run once so `assets/ambientcg/` is filled, then ship those files (CC0 allows
  it) and set `AMBIENTCG_OFFLINE=1` or remove the plugin's download step.
- **Bevy versions.** Written for 0.19. In 0.18 and earlier the texture-loading call differs;
  the note in `load_tex` shows the older form.
- **Untested.** The plugin was written without being compiled. Paste any error into
  your chat for a fix.

## No-install path (non-Bevy users)

You do not need Python or a command line:

1. Get `ambientcg_catalog.csv` (included, or ask the project owner for the latest copy).
2. Start a chat with Claude. Paste `HANDOVER.md` and attach the CSV.
3. Describe what you need. Claude replies with a JSON block.
4. Open `crate.html` in your browser (double-click it). Paste the JSON, press **Load**,
   then **Download all**. You can also load the CSV into the page and browse the
   catalog yourself, without Claude.
5. Unzip the downloaded files into your project.

The Python scripts below remain for people who want automation (build steps, CI).

## Quick start (command line)

```bash
python ambientcg_catalog.py                      # one-time: build the catalog
# upload ambientcg_catalog.csv to Claude, ask for a palette, save it as palette.json
python ambientcg_fetch.py palette.json --out Assets/Textures/ambientcg
```

---

## How this helps procedural exteriors and interiors

Procedural generators are great at layout (footprints, rooms, streets, roof shapes) but
they need a **material vocabulary**: *what* does a wall, floor, or roof look like in
this particular building, district, or biome? That is the part this toolkit supplies.

### 1. Think in *roles*, not asset IDs
Your generator should say `exterior_wall_main`, `interior_floor`, `ground_path`,
never `Bricks075A`. The palette file maps roles to assets, so you can restyle an
entire town by swapping one JSON file, with no generator code changes.

### 2. Style coherence from a single prompt
Ask Claude for a palette per style: "weathered coastal village", "brutalist research
base", "overgrown ruins", "cozy cabin interiors". Claude picks materials that belong
together (tone, wear level, scale) from thousands of options, which is the hard part
of making procedural output look designed rather than random.

### 3. Controlled variation
For each role you can ask for **a pool**, not a single asset: five brick variants,
four plaster variants. Your generator then picks per building or per room using a
seed. Same layout logic, noticeably different results, and still deterministic.

### 4. Exterior-specific wins
- **Walls, roofs, ground**: tileable PBR materials in matching resolutions.
- **Sky and lighting**: HDRIs set the mood (overcast, golden hour, indoor studio),
  giving consistent ambient lighting and reflections per biome.
- **Terrain and decals**: terrain assets and decals (stains, cracks, grime) break up
  repetition on large surfaces.
- **Palette per district**: wealthy, industrial, and slum districts share one
  generator but use three palettes.

### 5. Interior-specific wins
- **Room-type palettes**: `kitchen`, `bedroom`, `corridor`, `vault` can each define
  floor, wall, and ceiling roles.
- **Age and condition**: tag-driven choices (clean vs worn vs damaged) let
  you scale "grime level" with a single parameter.
- **Consistent trim**: one trim material shared across rooms ties a procedurally
  generated building together.

### 6. Iteration speed
Because only the metadata is local, trying a new look costs: one Claude prompt, one
palette file, one fetch of a few dozen megabytes. You can audition ten art
directions in an afternoon.

### 7. What this does *not* do
It does not generate textures. It selects and retrieves authored PBR ones. Pair
it with your own procedural logic (triplanar mapping, tint variation, macro noise,
vertex-colour blending) to hide tiling and add uniqueness.

---

## Adding it to your game

**Recommendation: run the downloader at editor/build time, not in the shipped game.**
Runtime downloading adds failure modes (offline players, API changes, load
hitches, store policies). Treat ambientCG as a *content source for your project*.

### Project layout (suggested)

```
YourGame/
  Tools/ambientcg/            <- this toolkit
  Content/Palettes/*.json     <- committed to version control
  Content/Textures/ambientcg/ <- downloaded output (gitignore it, or commit if licences allow and size is ok)
```

Commit the **palettes** (tiny, diffable). Regenerate textures on demand with the fetch
script. Add it as a pre-build or "sync content" step.

### Engine-agnostic integration steps

1. **Call the downloader from your tooling.** It is a plain CLI:
   `python Tools/ambientcg/ambientcg_fetch.py Content/Palettes/coastal.json --out Content/Textures/ambientcg`
   Hook it to an editor menu item, a pre-build script, or a CI step. Most engines can
   launch an external process from an editor script (a menu command, an editor
   plugin, or a build step).
2. **Read the palette in your generator.** Load the JSON, look up the role, and get
   the folder `Content/Textures/ambientcg/<id>/`.
3. **Load maps by name.** ambientCG ships maps with suffixes such as
   `_Color`, `_NormalGL` / `_NormalDX`, `_Roughness`, `_AmbientOcclusion`,
   `_Displacement`, `_Metalness`, `_Opacity`. Check a downloaded folder to confirm
   the exact names, then write one loader that builds a material from a folder.
   **Pick the normal map that matches your engine's convention** (GL vs DX
   green-channel direction).
4. **Cache materials by asset id** so every building that uses the same role shares one
   material instance.
5. **Seed variation**: store pools per role in the palette and choose with your
   world seed so results are reproducible.

### Suggested palette extension for pools

```json
"exterior_wall_main": [
  {"id": "ExampleBrickA", "resolution": "2K", "format": "JPG"},
  {"id": "ExampleBrickB", "resolution": "2K", "format": "JPG"}
]
```

(The current fetch script reads one asset per role; ask Claude to extend it for
pools, or list the extra assets in a manifest. Engine-specific loader code is not
included yet, because it depends on your engine.)

---

## Working with Claude: prompts that work

- "Here is the ambientCG catalog. Build a palette for *<style>* with roles
  `exterior_wall_main`, `exterior_roof`, `ground_path`, `interior_floor`,
  `interior_wall`. Prefer 2K JPG. Explain each choice in one line."
- "Give me 5 variants for `exterior_wall_main`, all weathered stone, similar colour."
- "Find HDRIs suited to an overcast coastal exterior and a dim interior."
- "Refresh my palette for *<new district>* and reuse the trim material."

Re-run `ambientcg_catalog.py` now and then so Claude can see new releases.

---

## Known limitations

- Written against ambientCG's **v3 API** (https://docs.ambientcg.com/api/v3/). The
  response format is not fully documented, so the scripts parse it defensively. If a
  script reports an unexpected shape, share the printed output and update the parser.
- Download file names for HDRIs and 3D models can differ from materials. Use
  `{"file": "<exact name>"}` in a manifest when the id/resolution/format lookup
  fails.
- Be polite to the API: the scripts already pause between requests. Do not run them in
  tight loops.
