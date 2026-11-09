//! Only raster images from explicitly permitted locations. No SVG, HTML or executable assets.
use super::{
    cache::{digest, read_bounded},
    loader::{AssetSource, Settings, Transport},
    model::{Asset, Catalog},
    Error, Result,
};
use image::{ImageReader, Limits};
use std::{collections::HashSet, io::Cursor};
pub const MAX_ASSET_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_TOTAL_RGBA: usize = 64 * 1024 * 1024;
#[derive(Clone)]
pub struct Raster {
    pub size: [usize; 2],
    pub rgba: Vec<u8>,
}

pub fn references(c: &Catalog) -> Vec<&Asset> {
    let mut seen = HashSet::new();
    c.applications
        .iter()
        .flat_map(|a| a.logo.iter().chain(a.screenshots.iter().map(|s| &s.image)))
        .filter(|a| seen.insert(a.source.as_str()))
        .collect()
}
pub fn decode(bytes: &[u8]) -> Result<Raster> {
    if bytes.len() > MAX_ASSET_BYTES {
        return Err(Error::invalid("Image de plus de 4 MiB"));
    }
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| Error::invalid("Image illisible"))?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Png | image::ImageFormat::Jpeg)
    ) {
        return Err(Error::invalid(
            "Seules les images PNG et JPEG sont autorisées",
        ));
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|_| Error::invalid("Image corrompue ou dimensions excessives"))?;
    if u64::from(decoded.width()) * u64::from(decoded.height()) > 8_000_000 {
        return Err(Error::invalid("Image de plus de 8 mégapixels"));
    }
    let rgba = decoded.into_rgba8();
    Ok(Raster {
        size: [rgba.width() as usize, rgba.height() as usize],
        rgba: rgba.into_raw(),
    })
}

pub fn load(settings: &Settings, source: &str, http: &dyn Transport) -> Result<Raster> {
    let location = settings.asset_source(source)?;
    let key = format!("{}.image", digest(source.as_bytes()));
    let fresh = (|| {
        let bytes = match &location {
            AssetSource::Local(path) => {
                let root = settings.root.join("assets");
                if std::fs::symlink_metadata(&root)?.file_type().is_symlink() {
                    return Err(Error::invalid("Dossier assets symbolique interdit"));
                }
                read_bounded(
                    &super::security::confined_file(&root, path)?,
                    MAX_ASSET_BYTES,
                )?
            }
            AssetSource::Https(url) => http.get(url, MAX_ASSET_BYTES)?,
        };
        let raster = decode(&bytes)?;
        // Caching failure must not hide an otherwise usable image.
        let _ = settings.cache.write(&key, &bytes);
        Ok(raster)
    })();
    match fresh {
        Ok(raster) => Ok(raster),
        Err(error) => settings
            .cache
            .read(&key, MAX_ASSET_BYTES)
            .and_then(|b| decode(&b))
            .map_err(|_| error),
    }
}

/// Offline mode never retries the network for every image.
pub fn load_cached(settings: &Settings, source: &str) -> Result<Raster> {
    settings.asset_source(source)?;
    let key = format!("{}.image", digest(source.as_bytes()));
    decode(&settings.cache.read(&key, MAX_ASSET_BYTES)?)
}
