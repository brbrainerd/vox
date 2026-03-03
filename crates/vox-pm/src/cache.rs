use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Manages a central, cross-project cache for downloaded packages.
/// Enables UV-style fast installations by hard-linking files
/// from the cache into project-local `.vox_modules` directories.
#[derive(Debug, Clone)]
pub struct PackageCache {
    /// The root directory of the cache, e.g. `~/.vox/cache`.
    pub cache_dir: PathBuf,
}

impl PackageCache {
    /// Initialize the package cache, creating the directory if it doesn't exist.
    pub fn new(cache_dir: Option<PathBuf>) -> io::Result<Self> {
        let dir = cache_dir.unwrap_or_else(|| {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            home.join(".vox").join("cache")
        });

        if !dir.exists() {
            fs::create_dir_all(&dir)?;
        }

        Ok(Self { cache_dir: dir })
    }

    /// The directory where raw package tarballs/zips are stored.
    pub fn archives_dir(&self) -> PathBuf {
        let path = self.cache_dir.join("archives");
        if !path.exists() {
            let _ = fs::create_dir_all(&path);
        }
        path
    }

    /// The directory where extracted packages are stored.
    pub fn extracted_dir(&self) -> PathBuf {
        let path = self.cache_dir.join("extracted");
        if !path.exists() {
            let _ = fs::create_dir_all(&path);
        }
        path
    }

    /// Check if a package archive is already cached.
    pub fn is_cached(&self, name: &str, version: &str) -> bool {
        self.archive_path(name, version).exists()
    }

    /// Get the path to a package archive in the cache.
    pub fn archive_path(&self, name: &str, version: &str) -> PathBuf {
        self.archives_dir().join(format!("{name}-{version}.voxpkg"))
    }

    /// Get the path to an extracted package directory in the cache.
    pub fn extracted_path(&self, name: &str, version: &str) -> PathBuf {
        self.extracted_dir().join(format!("{name}-{version}"))
    }

    /// Store raw package data into the cache.
    pub fn store_archive(&self, name: &str, version: &str, data: &[u8]) -> io::Result<()> {
        let path = self.archive_path(name, version);
        fs::write(path, data)
    }

    /// Extract a package archive (tar.gz format expected) within the cache.
    pub fn extract(&self, name: &str, version: &str) -> io::Result<()> {
        let path = self.extracted_path(name, version);
        if !path.exists() {
            fs::create_dir_all(&path)?;

            let archive_path = self.archive_path(name, version);
            if archive_path.exists() {
                let tar_gz = fs::File::open(&archive_path)?;
                let tar = flate2::read::GzDecoder::new(tar_gz);
                let mut archive = tar::Archive::new(tar);
                archive.unpack(&path)?;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("Archive file not found: {}", archive_path.display()),
                ));
            }
        }
        Ok(())
    }

    /// Link an extracted package from the cache into a target directory (e.g. `.vox_modules/name`).
    /// Uses hard links for performance, falling back to a full copy if cross-device.
    pub fn link_to_project(&self, name: &str, version: &str, target_dir: &Path) -> io::Result<()> {
        let source_dir = self.extracted_path(name, version);
        if !source_dir.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Package {name}@{version} not found in extracted cache"),
            ));
        }

        if !target_dir.exists() {
            fs::create_dir_all(target_dir)?;
        }

        Self::link_or_copy_dir(&source_dir, target_dir)
    }

    /// Recursively hard-link a directory, modifying it to copy if hard-linking fails.
    fn link_or_copy_dir(src: &Path, dst: &Path) -> io::Result<()> {
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if file_type.is_dir() {
                if !dst_path.exists() {
                    fs::create_dir_all(&dst_path)?;
                }
                Self::link_or_copy_dir(&src_path, &dst_path)?;
            } else {
                // Try to hard link.
                if fs::hard_link(&src_path, &dst_path).is_err() {
                    // Fall back to copying if hard linking fails (e.g. cross-device).
                    fs::copy(&src_path, &dst_path)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_cache_initialization() {
        let dir = tempdir().unwrap();
        let cache = PackageCache::new(Some(dir.path().to_path_buf())).unwrap();
        assert!(cache.archives_dir().exists());
        assert!(cache.extracted_dir().exists());
    }

    #[test]
    fn test_store_and_link() {
        let dir = tempdir().unwrap();
        let cache = PackageCache::new(Some(dir.path().to_path_buf())).unwrap();

        let name = "test-pkg";
        let version = "1.0.0";

        // Create a real tar.gz archive in memory
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        {
            let mut tar = tar::Builder::new(&mut enc);
            let mut header = tar::Header::new_gnu();
            header.set_size(10);
            header.set_cksum();
            tar.append_data(&mut header, "test.txt", "dummy data".as_bytes())
                .unwrap();
        }
        let data = enc.finish().unwrap();

        // Store
        cache.store_archive(name, version, &data).unwrap();
        assert!(cache.is_cached(name, version));

        // Extract
        cache.extract(name, version).unwrap();

        // Link
        let target_dir = dir.path().join(".vox_modules").join(name);
        cache.link_to_project(name, version, &target_dir).unwrap();

        assert!(target_dir.join("test.txt").exists());
        let read_data = fs::read(target_dir.join("test.txt")).unwrap();
        assert_eq!(read_data, b"dummy data");
    }
}
