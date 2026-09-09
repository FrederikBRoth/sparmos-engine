use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::Context;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

pub struct Assets {
    bytes: HashMap<String, Arc<[u8]>>,

    #[cfg(target_arch = "wasm32")]
    root: String,

    #[cfg(not(target_arch = "wasm32"))]
    root: PathBuf,
}

impl Assets {
    #[cfg(target_arch = "wasm32")]
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            bytes: HashMap::new(),
            root: root.into(),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let root = if root.is_absolute() || root.exists() {
            root
        } else {
            std::env::current_exe()
                .ok()
                .and_then(|executable| executable.parent().map(|parent| parent.join(&root)))
                .filter(|candidate| candidate.exists())
                .unwrap_or(root)
        };

        Self {
            bytes: HashMap::new(),
            root,
        }
    }

    pub async fn load(&mut self, path: &str) -> anyhow::Result<Arc<[u8]>> {
        let path = normalize_path(path)?;

        if let Some(bytes) = self.bytes.get(&path) {
            return Ok(Arc::clone(bytes));
        }

        let data = self
            .load_uncached(&path)
            .await
            .with_context(|| format!("failed to preload asset '{path}'"))?;
        let data: Arc<[u8]> = data.into();
        self.bytes.insert(path, Arc::clone(&data));
        Ok(data)
    }

    pub async fn load_all<I, S>(&mut self, paths: I) -> anyhow::Result<()>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut loaded = HashSet::new();

        for path in paths {
            let path = normalize_path(path.as_ref())?;
            if loaded.insert(path.clone()) {
                self.load(&path).await?;
            }
        }

        Ok(())
    }

    pub fn get(&self, path: &str) -> Option<Arc<[u8]>> {
        let path = normalize_path(path).ok()?;
        self.bytes.get(&path).cloned()
    }

    pub fn require(&self, path: &str) -> Arc<[u8]> {
        self.get(path)
            .unwrap_or_else(|| panic!("asset was not preloaded: {path}"))
    }

    #[cfg(target_arch = "wasm32")]
    async fn load_uncached(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        let origin = web_sys::window()
            .ok_or_else(|| anyhow::anyhow!("browser window is unavailable"))?
            .location()
            .origin()
            .map_err(|_| anyhow::anyhow!("could not read the page origin"))?;

        let root = self.root.trim_matches('/');
        let asset_url = if root.is_empty() {
            format!("{}/{path}", origin.trim_end_matches('/'))
        } else {
            format!("{}/{root}/{path}", origin.trim_end_matches('/'))
        };

        let response = reqwest::get(&asset_url)
            .await
            .with_context(|| format!("request failed for {asset_url}"))?;
        let response = response
            .error_for_status()
            .with_context(|| format!("request failed for {asset_url}"))?;

        Ok(response
            .bytes()
            .await
            .with_context(|| format!("could not read response body from {asset_url}"))?
            .to_vec())
    }

    #[cfg(not(target_arch = "wasm32"))]
    async fn load_uncached(&self, path: &str) -> anyhow::Result<Vec<u8>> {
        let full_path = self.root.join(path);
        std::fs::read(&full_path).with_context(|| format!("could not read {}", full_path.display()))
    }
}

fn normalize_path(path: &str) -> anyhow::Result<String> {
    let mut parts = Vec::new();
    let normalized = path.replace('\\', "/");

    for part in normalized.split('/') {
        match part {
            "" | "." => {}
            ".." => anyhow::bail!("asset path may not contain '..': {path}"),
            part => parts.push(part),
        }
    }

    if parts.is_empty() {
        anyhow::bail!("asset path may not be empty");
    }

    Ok(parts.join("/"))
}

pub const ENGINE_ASSETS: &[&str] = &[
    "engine/shaders/pbr_shader.wgsl",
    "engine/shaders/pbr_shader2.wgsl",
    "engine/shaders/pbr_shader_textured.wgsl",
    "engine/shaders/skybox.wgsl",
    "engine/shaders/equirectangular_to_cubemap.wgsl",
    "engine/shaders/irradiance_convolution.wgsl",
    "engine/shaders/prefilter_environment.wgsl",
    "engine/shaders/cubemap_mipmap.wgsl",
    "engine/shaders/brdf_integration.wgsl",
    "engine/shaders/chromatic_aberration.wgsl",
];

#[derive(Clone, Default)]
pub struct AssetManifest {
    paths: Vec<&'static str>,
}

impl AssetManifest {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn file(mut self, path: &'static str) -> Self {
        self.paths.push(path);
        self
    }

    pub fn extend<I>(mut self, paths: I) -> Self
    where
        I: IntoIterator<Item = &'static str>,
    {
        self.paths.extend(paths);
        self
    }

    pub fn iter(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.paths.iter().copied()
    }
}
