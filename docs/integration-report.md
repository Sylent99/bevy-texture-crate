# Integration report: the ambientCG texture crate in Ironclad

For the Claude that built the texture crate (`Sylent99/bevy-texture-crate`, `HANDOVER.md`, `ambientcg.rs`).
Written by another Claude session that wired it into a real game. **It worked on the user's Windows PC**
(carpets, roofs and sidewalks now show ambientCG textures). Below: what we did, what tripped us up
(feedback for the crate), and what we want next.

Caveat: that session had no Rust compiler. All Rust was written blind and compiled/run by the user. The
user reported it works; they did not report compiler errors back to us.

---

## 1. The game, briefly

- **Ironclad**: a Bevy 0.19 (Rust) skating/driving game, run on Windows with `PLAY.bat`, which builds with
  `cargo build --release -p skate_app` and starts `target\release\ironclad.exe` **from the project folder**.
- Cargo workspace. Two crates matter:
  - `skate_sim`: pure simulation, no Bevy. Units are **centimetres, Z up**. Holds the collision `World`.
  - `skate_app`: the Bevy front end (binary `ironclad`). Bevy units are **metres, Y up** (`to_bevy()` converts).
- The city is **procedural**: blocks come from `skate_sim::world::drive_city`; `skate_app/src/city.rs` draws it.
- Today we also added **procedural interiors** for the city's blocks (`skate_sim/src/interior.rs`, drawn by
  `skate_app/src/interiors.rs`): six settings (office, apartments, clinic, storage, warehouse, factory),
  rooms from a seeded BSP, doors, props. Until now every interior surface was a flat vertex colour.
- The existing procedural look uses generated textures with generated normal and roughness maps
  (`city::materials`), and a cel/shade-tint component (`ShadeTint`) on most meshes.
- The game also has a retro/PS1-style low-res render mode (from the user's project notes), so 1K textures
  are probably enough; we used 2K as the HANDOVER default.

## 2. What the user asked for, and the result

Asked Claude for a palette ("90s suburb; roles `exterior_wall_main`, `exterior_roof`, `ground_path`,
`interior_floor`") using `HANDOVER.md`. Saved the JSON as `assets/palette.json`. Dropped `ambientcg.rs` into
the game. First run downloaded all 9 asset folders into `assets/ambientcg/`:

```
WoodSiding009, WoodSiding010           (exterior_wall_main)
RoofingTiles006, RoofingTiles007       (exterior_roof)
Concrete047A, Concrete025              (ground_path)
Carpet016, Carpet011, Carpet008        (interior_floor)
```
all `2K` / `JPG`. Then we wired three of the four roles into the game (see section 4). The user confirmed
the textures show in-game.

## 3. Everything we changed to make the plugin work (the exact steps)

All files are under `crates/skate_app/`.

1. **`src/ambientcg.rs`**: the crate's file, added unchanged except one new method (section 4).
2. **`Cargo.toml`**: added the crates the plugin needs:
   ```toml
   serde = { version = "1", features = ["derive"] }
   serde_json = "1"
   ureq = "2"      # the plugin uses the ureq 2 API (AgentBuilder, Error::Status)
   zip = "2"
   ```
   (`bevy` already had the `jpeg` feature on.)
3. **`src/main.rs`**:
   - `mod ambientcg;`
   - Registered the plugin right after `DefaultPlugins`:
     `.add_plugins(ambientcg::AmbientCgPlugin::new("palette.json"))`
   - **Pointed Bevy's default asset folder at the project's `assets/`** by chaining
     `.set(AssetPlugin { file_path: cwd.join("assets").to_string_lossy().into_owned(), ..default() })`
     onto `DefaultPlugins` (`cwd` is `std::env::current_dir()`, already computed in `main`). **This was
     needed, see gotcha A.**
   - In the `spawn_scene` startup system: added a parameter
     `mut palette: Option<ResMut<ambientcg::AmbientCgLibrary>>`, built the city materials, then called
     `city_mats.apply_palette(lib, &assets, &mut materials)` before storing them.
4. **`assets/palette.json`**: the palette from the chat (roles with variant lists; `style` is ignored by serde).

## 4. How the textures get onto surfaces (so far)

The city's materials live in one struct, `city::CityMaterials`. We added two fields and one method:

```rust
path: Handle<StandardMaterial>,                 // sidewalks / park surfaces (was: the procedural concrete)
pub(crate) floors: Vec<Handle<StandardMaterial>>, // interior carpets; empty without a palette

impl CityMaterials {
    pub fn apply_palette(&mut self, lib: &mut AmbientCgLibrary, assets: &AssetServer,
                         materials: &mut Assets<StandardMaterial>) {
        if let Some(m) = lib.try_material("exterior_roof", 0, assets, materials) { self.roof = m; }
        if let Some(m) = lib.try_material("ground_path", 0, assets, materials) { self.path = m; }
        for seed in 0..3u64 {                    // up to three interior_floor variants
            if let Some(m) = lib.try_material("interior_floor", seed, assets, materials)
                && !self.floors.contains(&m) { self.floors.push(m); }
        }
    }
}
```

- **Roles the palette lacks (or whose files aren't on disk) keep the old procedural look.**
- **Roofs** and **sidewalks** simply swap the material handle on meshes that already used one.
- **Interior floors**: a mesh has one material, so `interiors.rs` now builds the carpeted rooms' floor quads
  into one extra mesh per carpet variant (rooms choose a variant by `(room index + building index) %
  variant count`), with a **white vertex colour** so the carpet's own colours show. Rooms that shouldn't be
  carpet (bathrooms, kitchens, workshops, storage, halls) stay on the old coloured-plaster mesh. A small
  function `carpeted(setting, room_kind)` decides.
- **`exterior_wall_main` is not used yet.** The city's facade texture is brick with the **windows painted
  into it** (and a `LitWindows` system lights them at night), so swapping in siding would delete the windows.

**The one change to `ambientcg.rs`**: added `try_material(...) -> Option<Handle<StandardMaterial>>`, which is
`material()` minus the grey fallback (returns `None`, with the same warning, when the role is missing or no
colour texture is on disk). `material()` now calls it and falls back to grey. We needed this so a missing
role doesn't paint the whole city grey.

## 5. Gotchas and feedback for the crate (please fix or document)

**A. Asset-root mismatch (the big one).** `assets_root()` (used for downloading and scanning) resolves to
`BEVY_ASSET_ROOT`, else `CARGO_MANIFEST_DIR`, else `.`. When the game is launched as an `.exe` (not
`cargo run`) neither env var is set, so downloads go to `./assets/ambientcg` relative to the working
directory. But Bevy's *default asset reader* resolves `assets/` relative to the **executable's folder**
(`target/release/assets`), so `asset_server.load("ambientcg/...")` would look in the wrong place and every
texture would silently fail. Our fix was to set `AssetPlugin.file_path`. Suggestion: have the plugin (or the
README) say this plainly, or let `AmbientCgPlugin` take the assets folder and use the same path for both
downloading and loading.

**B. A missing role gives grey.** Great for a demo, wrong for a game that already has fallback materials.
`try_material` (above) is the fix; suggest adding it to the crate.

**C. `uv_scale` semantics.** It scales UVs ("repeats per UV unit"). Our meshes have metre-based UVs
(`uv = world position / metres per repeat`), so the right value depends on each mesh's convention. A
per-asset real-world size in metres (the physical size of one tile) would make the same palette work on any
mesh. Even a documented default would help.

**D. First-run download blocks startup.** The downloads happen synchronously in `Plugin::build`, before the
window opens. Fine for dev; a note (or a background download with progress) would stop users thinking the
game hung. 2K JPG sets are several MB each.

**E. Materials are single-sided with fixed settings.** `build_material` sets no `double_sided` or `cull_mode`
and no per-role overrides. Our city's procedural concrete was two-sided; the ambientCG one isn't. A per-role
options block (`double_sided`, `roughness`, `uv_scale`, `tint`) in the palette would cover it.

**F. `HANDOVER.md` doesn't match the live catalog.** It describes a top-level index with columns
`type,categories,assets,index_url` and then per-type files with `category,count,file_url`. The live
`catalog/index.csv` is **one flat file** with `type,category,count,file_url`. A reader following the
instructions literally gets confused; update the doc.

**G. Catalog descriptions are mostly empty.** In the category files we read (`WoodSiding`, `RoofingTiles`,
`Concrete`, `Carpet`, `Pathway`) the `description` column is empty for nearly every asset, so picks rest on
tags only. Colour/tone and era tags ("beige", "brown", "1990s-looking") would let Claude choose far better.
Also useful: a note on which assets are tileable and a typical real-world size.

**H. Role naming vs what a procedural game needs.** The handover's roles (`exterior_wall_main`, ...) assume
one house with a few surfaces. A procedural generator needs many roles and variant lists per theme. See next
section.

**I. Untested file.** The header says "Not compiled by its author". It compiled for us after the edits above
(as far as the user reported), but a CI build would remove the doubt.

## 6. What we want next (the ask)

The user wants the textures **wired into the procedural generation**, not just a one-off palette. Today the
interior generator picks a *setting* per building and a *kind* per room, and colours everything from small
tables. Please help by producing palettes for each setting, using roles the code can look up.

**Settings** (each building gets one): `office`, `apartments`, `clinic`, `storage`, `warehouse`, `factory`.
**Room kinds** (floors can differ per kind): corridor, hall, open office, office, meeting, restroom, storage,
workshop, living, kitchen, bedroom, bath, ward, exam, waiting, unit (storage unit), locker, control.
**Props** (boxes today): desk, shelf, crate, pallet, machine, table, counter, bed, locker, pillar, bench, sofa,
cabinet.

**Proposed role scheme** (the code will look up `<setting>_<surface>` and fall back to a generic role, then to
the old colours). Each role may have several variants (the code picks one by seed):

- per setting: `<setting>_floor`, `<setting>_wall`, `<setting>_ceiling`
  (for example `office_floor`, `warehouse_wall`, `factory_ceiling`)
- wet rooms: `bathroom_floor`, `bathroom_wall` (tiles)
- shared: `interior_trim` (baseboards), `door_leaf`, `prop_wood`, `prop_metal`, `prop_fabric`, `prop_cardboard`
- exterior: `exterior_wall_main`, `exterior_roof`, `ground_path` (these three exist already; wall waiting on windows)

Tone: these are mostly run-down, liminal, slightly sinister places (office parks, clinics, factories),
**not** cosy. Prefer worn, institutional, slightly dirty materials; coherent per setting.

Please return the palette JSON in the existing format, plus one line per pick. If it helps, a naming scheme
the crate itself could enforce (so a missing role falls back cleanly) would be welcome.

## 7. Where things are, for reference

| What | Where |
|---|---|
| Plugin | `crates/skate_app/src/ambientcg.rs` |
| Palette | `assets/palette.json` |
| Downloaded textures | `assets/ambientcg/<id>_<res>-<fmt>/` |
| Material swap | `city.rs`: `CityMaterials::apply_palette`; called from `main.rs` `spawn_scene` |
| Interior floors | `interiors.rs`: `carpeted()`, `build_meshes()` |
| Interior generator (sim) | `crates/skate_sim/src/interior.rs` |
| Interior colour tables (to be replaced by roles) | `interiors.rs`: `palette()`, `floor_colour()`, `prop_colour()` |
