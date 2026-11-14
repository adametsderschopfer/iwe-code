//! Build module package archives for tests.

use crate::domain::modules::{Manifest, current_target};
use ed25519_dalek::Signer;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs::File, io::Write, path::Path};

pub(in crate::infrastructure::modules) fn test_manifest(
    id: &str,
    version: &str,
    entry: &str,
) -> Manifest {
    use std::collections::BTreeSet;
    let mut binaries = BTreeMap::new();
    binaries.insert(current_target().to_owned(), entry.to_owned());
    Manifest {
        id: id.to_owned(),
        name: crate::domain::modules::LocalizedText {
            en: format!("{id} module"),
            ru: None,
            es: None,
        },
        description: crate::domain::modules::LocalizedText {
            en: "A test module".to_owned(),
            ru: None,
            es: None,
        },
        version: semver::Version::parse(version).expect("valid version"),
        api: 1,
        min_app_version: semver::Version::new(1, 0, 0),
        entry: binaries,
        permissions: BTreeSet::new(),
        placement: crate::domain::modules::Placement::Bottom,
        digests: BTreeMap::new(),
    }
}

/// A deterministic fixture signing key.
///
/// The key belongs only to the fixtures and never matches the packaged
/// verification key, so tests can also prove rejection.
pub(in crate::infrastructure::modules) fn test_key() -> ed25519_dalek::SigningKey {
    ed25519_dalek::SigningKey::from_bytes(&[7; 32])
}

/// Write an unsigned package archive with one manifest and one binary.
pub(in crate::infrastructure::modules) fn build_package(
    archive: &Path,
    id: &str,
    version: &str,
    binary: &[u8],
) {
    build_package_with(archive, test_manifest(id, version, "module-bin"), binary)
}

/// Write an unsigned package archive from a custom manifest.
pub(in crate::infrastructure::modules) fn build_package_with(
    archive: &Path,
    manifest: Manifest,
    binary: &[u8],
) {
    let entry = archive_entry(&manifest);
    let file = File::create(archive).expect("create archive");
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);
    append(
        &mut tar,
        "manifest.json",
        serde_json::to_string(&manifest)
            .expect("serialize manifest")
            .as_bytes(),
    );
    append(&mut tar, &entry, binary);
    tar.into_inner()
        .expect("finish tar")
        .finish()
        .expect("finish gzip")
        .flush()
        .expect("flush archive");
}

/// Write a signed package and return the fixture signing key.
///
/// The manifest carries a digest for every packaged file, and the signature
/// covers the exact manifest bytes.
pub(in crate::infrastructure::modules) fn build_signed_package(
    archive: &Path,
    id: &str,
    version: &str,
    binary: &[u8],
) -> ed25519_dalek::SigningKey {
    let key = test_key();
    build_signed_package_with(
        archive,
        test_manifest(id, version, "module-bin"),
        binary,
        &key,
    );
    key
}

/// Write a signed package from a custom manifest and signing key.
pub(in crate::infrastructure::modules) fn build_signed_package_with(
    archive: &Path,
    mut manifest: Manifest,
    binary: &[u8],
    key: &ed25519_dalek::SigningKey,
) {
    let entry_path = archive_entry(&manifest);
    let mut digests = BTreeMap::new();
    digests.insert(entry_path.clone(), digest_hex(binary));
    manifest.digests = digests;

    let manifest_bytes = serde_json::to_vec(&manifest).expect("serialize manifest");
    let signature = key.sign(&manifest_bytes);

    let file = File::create(archive).expect("create archive");
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);
    append(&mut tar, "manifest.json", &manifest_bytes);
    append(&mut tar, &entry_path, binary);
    append(&mut tar, "manifest.sig", &signature.to_bytes());
    tar.into_inner()
        .expect("finish tar")
        .finish()
        .expect("finish gzip")
        .flush()
        .expect("flush archive");
}

/// Write an archive from exact manifest bytes, a signature, and raw entries.
///
/// Tests use this to assemble packages whose content intentionally disagrees
/// with the signed digests.
pub(in crate::infrastructure::modules) fn build_signed_raw(
    archive: &Path,
    manifest_bytes: &[u8],
    signature: &[u8],
    entries: &[(&str, &[u8])],
) {
    let file = File::create(archive).expect("create archive");
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);
    append(&mut tar, "manifest.json", manifest_bytes);
    append(&mut tar, "manifest.sig", signature);
    for (path, bytes) in entries {
        append(&mut tar, path, bytes);
    }
    tar.into_inner()
        .expect("finish tar")
        .finish()
        .expect("finish gzip")
        .flush()
        .expect("flush archive");
}

fn archive_entry(manifest: &Manifest) -> String {
    let entry = manifest
        .entry
        .get(current_target())
        .cloned()
        .unwrap_or_else(|| "module-bin".to_owned());
    format!("binaries/{}/{entry}", current_target())
}

fn digest_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn append<W: Write>(tar: &mut tar::Builder<W>, path: &str, bytes: &[u8]) {
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, path, bytes)
        .expect("append entry");
}
