//! ambientCG for Bevy: a drop-in plugin.
//!
//! Put this file in `src/ambientcg.rs`. It reads a palette (roles -> ambientCG assets),
//! downloads any missing textures into `assets/ambientcg/` when the game starts
//! (dev convenience, needs internet the first time), and gives you ready-made
//! `StandardMaterial`s by role name.
//!
//! Materials: `<id>_<res>-<format>.zip`. HDRIs: `<id>_<res>.zip` (up to 16K; use 1K-4K for games).
//! Written for Bevy 0.19. For Bevy 0.18 or older see the note in `load_tex`.
//! Not compiled by its author, so expect to fix small things. Paste any compiler error
//! into your chat and it can be corrected quickly.
//!
//! Set the environment variable AMBIENTCG_OFFLINE=1 to skip all downloading.
//! ambientCG assets are CC0. This is a community tool, not affiliated with ambientCG.

use bevy::image::{
    ImageAddressMode, ImageFilterMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor,
};
use bevy::math::Affine2;
use bevy::prelude::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

// ---------------------------------------------------------------- palette file

fn default_res() -> String {
    "2K".into()
}
fn default_fmt() -> String {
    "JPG".into()
}
fn default_scale() -> f32 {
    1.0
}

#[derive(Deserialize, Clone, Debug)]
pub struct AssetSpec {
    pub id: String,
    #[serde(default = "default_res")]
    pub resolution: String,
    #[serde(default = "default_fmt")]
    pub format: String,
    /// How many times the texture repeats across one UV unit (tiling).
    #[serde(default = "default_scale")]
    pub uv_scale: f32,
}

impl AssetSpec {
    /// HDRI downloads are named `<id>_<res>.zip` (no format suffix); materials are
    /// `<id>_<res>-<format>.zip`.
    fn folder_name(&self) -> String {
        if self.id.contains("HDRI") || self.format.is_empty() {
            format!("{}_{}", self.id, self.resolution)
        } else {
            format!("{}_{}-{}", self.id, self.resolution, self.format)
        }
    }
    fn zip_name(&self) -> String {
        format!("{}.zip", self.folder_name())
    }
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum OneOrMany {
    One(AssetSpec),
    Many(Vec<AssetSpec>),
}

#[derive(Deserialize, Default, Debug)]
struct PaletteFile {
    #[serde(default)]
    roles: HashMap<String, OneOrMany>,
}

// ---------------------------------------------------------------- plugin

/// Add with `.add_plugins(AmbientCgPlugin::new("palette.json"))`.
/// The path is relative to your `assets/` folder.
pub struct AmbientCgPlugin {
    pub palette_path: String,
}

impl AmbientCgPlugin {
    pub fn new(palette_path: impl Into<String>) -> Self {
        Self {
            palette_path: palette_path.into(),
        }
    }
}

impl Plugin for AmbientCgPlugin {
    fn build(&self, app: &mut App) {
        let root = assets_root();
        let palette = match load_palette(&root.join(&self.palette_path)) {
            Ok(p) => p,
            Err(e) => {
                error!("ambientCG: could not read palette '{}': {e}", self.palette_path);
                PaletteFile::default()
            }
        };

        let mut roles: HashMap<String, Vec<AssetSpec>> = HashMap::new();
        for (role, v) in palette.roles {
            roles.insert(
                role,
                match v {
                    OneOrMany::One(s) => vec![s],
                    OneOrMany::Many(v) => v,
                },
            );
        }

        if std::env::var("AMBIENTCG_OFFLINE").is_err() {
            let out = root.join("ambientcg");
            for spec in roles.values().flatten() {
                if let Err(e) = ensure_downloaded(spec, &out) {
                    warn!("ambientCG: could not fetch {}: {e}", spec.id);
                }
            }
        }

        app.insert_resource(AmbientCgLibrary {
            roles,
            cache: HashMap::new(),
            fallback: None,
        });
    }
}

// ---------------------------------------------------------------- library resource

#[derive(Resource)]
pub struct AmbientCgLibrary {
    roles: HashMap<String, Vec<AssetSpec>>,
    cache: HashMap<String, Handle<StandardMaterial>>,
    fallback: Option<Handle<StandardMaterial>>,
}

impl AmbientCgLibrary {
    /// Get a material for a role such as "exterior_wall_main".
    /// If the role has several variants, `seed` picks one deterministically
    /// (use e.g. a building id or room index). Never fails: a missing role or
    /// missing files gives a plain grey material and a warning in the log.
    pub fn material(
        &mut self,
        role: &str,
        seed: u64,
        asset_server: &AssetServer,
        materials: &mut Assets<StandardMaterial>,
    ) -> Handle<StandardMaterial> {
        let spec = self
            .roles
            .get(role)
            .filter(|v| !v.is_empty())
            .map(|v| v[(seed as usize) % v.len()].clone());

        let Some(spec) = spec else {
            warn!("ambientCG: role '{role}' is not in the palette");
            return self.fallback(materials);
        };

        let key = format!("{}_{}", spec.folder_name(), spec.uv_scale);
        if let Some(h) = self.cache.get(&key) {
            return h.clone();
        }

        let dir = assets_root().join("ambientcg").join(spec.folder_name());
        let maps = scan_maps(&dir);
        if maps.color.is_none() {
            warn!("ambientCG: no textures on disk for {} (offline or download failed?)", spec.id);
            return self.fallback(materials);
        }

        let handle = materials.add(build_material(asset_server, &spec, &maps));
        self.cache.insert(key, handle.clone());
        handle
    }

    /// For HDRI roles: the asset path (relative to `assets/`) of the downloaded
    /// .hdr/.exr file. Note: Bevy's skybox / environment lighting wants a cubemap, so an
    /// equirectangular HDRI still needs converting (not done here).
    pub fn hdri_file(&self, role: &str, seed: u64) -> Option<String> {
        let v = self.roles.get(role)?;
        if v.is_empty() {
            return None;
        }
        let spec = &v[(seed as usize) % v.len()];
        let dir = assets_root().join("ambientcg").join(spec.folder_name());
        for e in fs::read_dir(&dir).ok()?.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            if lower.ends_with(".hdr") || lower.ends_with(".exr") {
                return Some(format!("ambientcg/{}/{}", spec.folder_name(), name));
            }
        }
        None
    }

    fn fallback(&mut self, materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
        self.fallback
            .get_or_insert_with(|| {
                materials.add(StandardMaterial {
                    base_color: Color::srgb(0.6, 0.6, 0.6),
                    perceptual_roughness: 0.9,
                    ..default()
                })
            })
            .clone()
    }
}

// ---------------------------------------------------------------- material building

struct MapFiles {
    color: Option<String>,
    normal: Option<String>,
    normal_is_dx: bool,
    roughness: Option<String>,
    ao: Option<String>,
}

/// Find the maps by name suffix, so we do not depend on exact file names.
fn scan_maps(dir: &Path) -> MapFiles {
    let mut m = MapFiles {
        color: None,
        normal: None,
        normal_is_dx: false,
        roughness: None,
        ao: None,
    };
    let Ok(entries) = fs::read_dir(dir) else {
        return m;
    };
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let lower = name.to_lowercase();
        let is_image = lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".png");
        if !is_image {
            continue;
        }
        if lower.contains("_color") {
            m.color = Some(name);
        } else if lower.contains("_normalgl") {
            m.normal = Some(name);
            m.normal_is_dx = false;
        } else if lower.contains("_normaldx") && m.normal.is_none() {
            m.normal = Some(name);
            m.normal_is_dx = true;
        } else if lower.contains("_roughness") {
            m.roughness = Some(name);
        } else if lower.contains("_ambientocclusion") {
            m.ao = Some(name);
        }
    }
    m
}

fn build_material(asset_server: &AssetServer, spec: &AssetSpec, maps: &MapFiles) -> StandardMaterial {
    let base = format!("ambientcg/{}/", spec.folder_name());
    let tex = |name: &Option<String>, srgb: bool| {
        name.as_ref()
            .map(|n| load_tex(asset_server, format!("{base}{n}"), srgb))
    };

    StandardMaterial {
        base_color_texture: tex(&maps.color, true),
        // Meshes need UVs AND tangents for normal maps (see README).
        normal_map_texture: tex(&maps.normal, false),
        flip_normal_map_y: maps.normal_is_dx,
        // ambientCG gives a grey roughness map. Bevy reads roughness from the green
        // channel and metallic from blue, so keep metallic at 0 (non-metal surfaces).
        metallic_roughness_texture: tex(&maps.roughness, false),
        occlusion_texture: tex(&maps.ao, false),
        perceptual_roughness: 1.0,
        metallic: 0.0,
        uv_transform: Affine2::from_scale(Vec2::splat(spec.uv_scale)),
        ..default()
    }
}

/// Bevy 0.19 loading syntax. For Bevy 0.18 and older, replace the body with:
///   asset_server.load_with_settings(path, move |s: &mut ImageLoaderSettings| { ...same lines... })
fn load_tex(asset_server: &AssetServer, path: String, srgb: bool) -> Handle<Image> {
    asset_server
        .load_builder()
        .with_settings(move |s: &mut ImageLoaderSettings| {
            s.is_srgb = srgb; // colour = sRGB, data maps (normal/roughness/AO) = linear
            s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                address_mode_w: ImageAddressMode::Repeat,
                mag_filter: ImageFilterMode::Linear,
                min_filter: ImageFilterMode::Linear,
                mipmap_filter: ImageFilterMode::Linear,
                ..default()
            });
        })
        .load(path)
}

// ---------------------------------------------------------------- download + paths

/// Bevy looks for `assets/` next to Cargo.toml when run with `cargo run`.
fn assets_root() -> PathBuf {
    let base = std::env::var("BEVY_ASSET_ROOT")
        .or_else(|_| std::env::var("CARGO_MANIFEST_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("assets")
}

fn load_palette(path: &Path) -> Result<PaletteFile, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

fn dir_has_files(dir: &Path) -> bool {
    fs::read_dir(dir).map(|mut d| d.next().is_some()).unwrap_or(false)
}

fn ensure_downloaded(spec: &AssetSpec, out_root: &Path) -> Result<(), String> {
    let dir = out_root.join(spec.folder_name());
    if dir_has_files(&dir) {
        return Ok(()); // already have it
    }

    // Not every asset exists at every resolution, so fall back to smaller ones.
    let mut candidates = vec![spec.resolution.clone()];
    for r in ["2K", "1K"] {
        if !candidates.iter().any(|c| c == r) {
            candidates.push(r.to_string());
        }
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(300))
        .user_agent("bevy-ambientcg/0.1 (community tool)")
        .build();

    let mut last_err = String::from("no candidates");
    for res in candidates {
        let mut s = spec.clone();
        s.resolution = res;
        let url = format!("https://ambientcg.com/get?file={}", s.zip_name());
        info!("ambientCG: downloading {}", s.zip_name());

        let resp = match agent.get(&url).call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, _)) => {
                last_err = format!("HTTP {code} for {}", s.zip_name());
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };

        let mut bytes = Vec::new();
        resp.into_reader()
            .take(1024 * 1024 * 1024) // 1 GB safety cap
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;

        match zip::ZipArchive::new(Cursor::new(bytes)) {
            Ok(mut zip) => {
                fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                return zip.extract(&dir).map_err(|e| e.to_string());
            }
            Err(e) => {
                last_err = format!("{} was not a zip ({e})", s.zip_name());
            }
        }
    }
    Err(last_err)
}
