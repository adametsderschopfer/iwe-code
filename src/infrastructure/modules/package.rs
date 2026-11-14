//! Read, verify, and unpack external module packages before installation.

use super::Error;
use crate::domain::modules::{Manifest, SYSTEM_MODULE_IDS, current_target};
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Component, Path, PathBuf},
};

/// Largest accepted module package.
pub(crate) const MAX_PACKAGE: u64 = 128 * 1024 * 1024;
/// Largest accepted manifest file.
const MAX_MANIFEST: u64 = 256 * 1024;
/// Largest accepted unpacked package content.
const MAX_UNPACKED: u64 = MAX_PACKAGE * 3;
/// Largest accepted file count inside a package.
const MAX_ENTRIES: usize = 1024;
/// Exact byte length of an Ed25519 signature.
const SIGNATURE_BYTES: usize = 64;

const MANIFEST_NAME: &str = "manifest.json";
const SIGNATURE_NAME: &str = "manifest.sig";

/// Public key that verifies every distributed module package.
const PACKAGE_KEY: &[u8; 32] = include_bytes!("../../../assets/module-public-key.bin");

/// Validated package content staged for installation.
#[derive(Debug, PartialEq)]
pub(crate) struct Package {
    pub manifest: Manifest,
    /// Staging directory that holds the unpacked package.
    pub directory: PathBuf,
    /// Executable path inside the staging directory.
    pub entry: PathBuf,
}

/// Read and verify only the manifest from a package archive.
pub(crate) fn manifest(archive: &Path) -> Result<Manifest, Error> {
    manifest_with(archive, verification_key())
}

/// Read a package manifest, verifying its signature with the given key.
///
/// Verification is skipped only when the caller passes `None`, which the
/// `IWE_ALLOW_UNSIGNED_MODULES` override and tests use.
pub(crate) fn manifest_with(archive: &Path, key: Option<&[u8; 32]>) -> Result<Manifest, Error> {
    let size = fs::metadata(archive).map_err(|_| Error::Invalid)?.len();
    if size > MAX_PACKAGE {
        return Err(Error::TooLarge);
    }
    let file = File::open(archive).map_err(|_| Error::Invalid)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut bytes: Option<Vec<u8>> = None;
    let mut signature: Option<Vec<u8>> = None;
    for entry in archive.entries().map_err(|_| Error::Invalid)? {
        let mut entry = entry.map_err(|_| Error::Invalid)?;
        let path = entry.path().map_err(|_| Error::Invalid)?.into_owned();
        if path == Path::new(MANIFEST_NAME) {
            if bytes.is_some() || entry.size() > MAX_MANIFEST {
                return Err(Error::Invalid);
            }
            bytes = Some(read_bounded(&mut entry, MAX_MANIFEST)?);
        } else if path == Path::new(SIGNATURE_NAME) {
            if signature.is_some() {
                return Err(Error::Invalid);
            }
            let mut content = Vec::new();
            entry
                .take(SIGNATURE_BYTES as u64 + 1)
                .read_to_end(&mut content)
                .map_err(|_| Error::Invalid)?;
            if content.len() != SIGNATURE_BYTES {
                return Err(Error::Signature);
            }
            signature = Some(content);
        }
    }
    let bytes = bytes.ok_or(Error::Invalid)?;
    if let Some(key) = key {
        let signature = signature.as_deref().ok_or(Error::Unsigned)?;
        verify_manifest(&bytes, signature, key).map_err(|_| Error::Signature)?;
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?;
    if key.is_some() && manifest.digests.is_empty() {
        // A signature over a manifest without digests leaves the packaged
        // binaries unprotected, so signed packages must carry digests.
        return Err(Error::Signature);
    }
    check(&manifest)?;
    Ok(manifest)
}

/// Verify a manifest signature over its exact bytes.
pub(crate) fn verify_manifest(
    bytes: &[u8],
    signature: &[u8],
    public: &[u8; 32],
) -> Result<(), Error> {
    if signature.len() != SIGNATURE_BYTES {
        return Err(Error::Signature);
    }
    VerifyingKey::from_bytes(public)
        .map_err(|_| Error::Signature)?
        .verify_strict(
            bytes,
            &Signature::from_slice(signature).map_err(|_| Error::Signature)?,
        )
        .map_err(|_| Error::Signature)
}

/// The verifying key for distributed packages, unless the development
/// override disables signature enforcement for this process.
pub(crate) fn verification_key() -> Option<&'static [u8; 32]> {
    if std::env::var_os("IWE_ALLOW_UNSIGNED_MODULES").is_some() {
        None
    } else {
        Some(PACKAGE_KEY)
    }
}

/// Unpack a package archive into `staging` and validate its manifest.
///
/// Files are written only after their manifest signature verified, and the
/// staged content is checked against the manifest digests before use.
pub(crate) fn extract(archive: &Path, staging: &Path) -> Result<Package, Error> {
    let size = fs::metadata(archive).map_err(|_| Error::Invalid)?.len();
    if size > MAX_PACKAGE {
        return Err(Error::TooLarge);
    }
    let file = File::open(archive).map_err(|_| Error::Invalid)?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let mut total = 0_u64;
    let mut seen = BTreeSet::new();
    for entry in archive.entries().map_err(|_| Error::Invalid)? {
        let mut entry = entry.map_err(|_| Error::Invalid)?;
        let path = entry.path().map_err(|_| Error::Invalid)?.into_owned();
        if !valid_path(&path) {
            return Err(Error::Invalid);
        }
        if !seen.insert(path.clone()) || seen.len() > MAX_ENTRIES {
            return Err(Error::Invalid);
        }
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err(Error::Invalid);
        }
        total = total
            .checked_add(entry.size())
            .filter(|total| *total <= MAX_UNPACKED)
            .ok_or(Error::TooLarge)?;
        let output = staging.join(&path);
        if kind.is_dir() {
            fs::create_dir_all(&output).map_err(|_| Error::Storage)?;
            continue;
        }
        let parent = output.parent().ok_or(Error::Invalid)?;
        fs::create_dir_all(parent).map_err(|_| Error::Storage)?;
        let mut target = File::options()
            .create_new(true)
            .write(true)
            .open(&output)
            .map_err(|_| Error::Storage)?;
        std::io::copy(&mut entry, &mut target).map_err(|_| Error::Storage)?;
    }
    validate(staging)
}

fn validate(staging: &Path) -> Result<Package, Error> {
    let manifest_path = staging.join(MANIFEST_NAME);
    let mut bytes = Vec::new();
    File::open(&manifest_path)
        .map_err(|_| Error::Invalid)?
        .take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Invalid)?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err(Error::TooLarge);
    }
    let manifest: Manifest = serde_json::from_slice(&bytes).map_err(|_| Error::Invalid)?;
    check(&manifest)?;
    verify_digests(staging, &manifest)?;
    let target = current_target();
    let name = manifest.entry_name().ok_or(Error::UnsupportedTarget)?;
    let entry = Path::new("binaries").join(target).join(name);
    let entry_path = staging.join(&entry);
    if !entry_path.is_file() {
        return Err(Error::Invalid);
    }
    Ok(Package {
        manifest,
        directory: staging.to_owned(),
        entry,
    })
}

/// Check every staged file against the digest map of the manifest.
///
/// The manifest and signature files themselves are excluded: the manifest is
/// covered by the signature directly. An empty digest map means verification
/// was disabled for this process, so nothing is enforced.
fn verify_digests(staging: &Path, manifest: &Manifest) -> Result<(), Error> {
    if manifest.digests.is_empty() {
        return Ok(());
    }
    let mut files = BTreeSet::new();
    collect_files(staging, staging, &mut files)?;
    for (path, expected) in &manifest.digests {
        if !files.contains(path) || digest_file(&staging.join(path))? != *expected {
            return Err(Error::Signature);
        }
    }
    for path in &files {
        if path != MANIFEST_NAME && path != SIGNATURE_NAME && !manifest.digests.contains_key(path) {
            return Err(Error::Signature);
        }
    }
    Ok(())
}

fn collect_files(root: &Path, directory: &Path, files: &mut BTreeSet<String>) -> Result<(), Error> {
    let entries = fs::read_dir(directory).map_err(|_| Error::Storage)?;
    for entry in entries {
        let entry = entry.map_err(|_| Error::Storage)?;
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| Error::Invalid)?
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            files.insert(relative);
        }
    }
    Ok(())
}

fn digest_file(path: &Path) -> Result<String, Error> {
    let mut file = File::open(path).map_err(|_| Error::Invalid)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|_| Error::Invalid)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn read_bounded<R: Read>(reader: &mut R, limit: u64) -> Result<Vec<u8>, Error> {
    let mut content = Vec::new();
    reader
        .take(limit + 1)
        .read_to_end(&mut content)
        .map_err(|_| Error::Invalid)?;
    if content.len() as u64 > limit {
        return Err(Error::TooLarge);
    }
    Ok(content)
}

fn check(manifest: &Manifest) -> Result<(), Error> {
    if !manifest.is_valid(&SYSTEM_MODULE_IDS) {
        return Err(Error::Invalid);
    }
    if manifest.min_app_version > app_version() {
        return Err(Error::Incompatible);
    }
    manifest.entry_name().ok_or(Error::UnsupportedTarget)?;
    Ok(())
}

fn app_version() -> semver::Version {
    semver::Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or(semver::Version::new(1, 0, 0))
}

fn valid_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && path.components().count() <= 16
}

#[cfg(test)]
pub(in crate::infrastructure::modules) mod testing;

#[cfg(test)]
mod tests;
