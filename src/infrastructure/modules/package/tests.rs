//! Verify package signatures, extraction, manifest limits, and path rejection.

use super::{Error, extract, manifest, manifest_with, testing};
use std::{fs::File, io::Write};

#[test]
fn reads_a_signed_manifest_without_unpacking() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let archive = directory.path().join("sample.iwe-module");
    let key = testing::build_signed_package(&archive, "sample", "1.2.3", b"binary");
    let parsed =
        manifest_with(&archive, Some(&key.verifying_key().to_bytes())).expect("read manifest");
    assert_eq!(parsed.id, "sample");
    assert_eq!(parsed.version.to_string(), "1.2.3");
}

#[test]
fn unsigned_packages_are_rejected() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let archive = directory.path().join("sample.iwe-module");
    testing::build_package(&archive, "sample", "1.0.0", b"binary");
    let key = testing::test_key();
    assert_eq!(
        manifest_with(&archive, Some(&key.verifying_key().to_bytes())),
        Err(Error::Unsigned)
    );
}

#[test]
fn foreign_signatures_are_rejected() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let archive = directory.path().join("sample.iwe-module");
    testing::build_signed_package(&archive, "sample", "1.0.0", b"binary");
    let other = ed25519_dalek::SigningKey::from_bytes(&[99; 32]);
    assert_eq!(
        manifest_with(&archive, Some(&other.to_bytes())),
        Err(Error::Signature)
    );
}

#[test]
fn a_short_signature_is_rejected() {
    let key = testing::test_key();
    assert_eq!(
        super::verify_manifest(b"{}", &[0; 32], &key.verifying_key().to_bytes()),
        Err(Error::Signature)
    );
}

#[test]
fn extracts_entry_for_current_target() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let archive = directory.path().join("sample.iwe-module");
    testing::build_signed_package(&archive, "sample", "1.0.0", b"binary");
    let staging = tempfile::tempdir().expect("staging directory");
    let package = extract(&archive, staging.path()).expect("extract package");
    assert_eq!(package.manifest.id, "sample");
    assert!(staging.path().join(&package.entry).is_file());
}

#[test]
fn altered_content_fails_the_digest_check() {
    use ed25519_dalek::Signer;
    use sha2::{Digest, Sha256};

    let directory = tempfile::tempdir().expect("temporary directory");
    let archive = directory.path().join("sample.iwe-module");
    let key = testing::test_key();
    let entry_path = format!(
        "binaries/{}/module-bin",
        crate::domain::modules::current_target()
    );
    let mut manifest = testing::test_manifest("sample", "1.0.0", "module-bin");
    // The manifest claims a digest of different content than it packages.
    let claimed = format!("{:x}", Sha256::digest(b"different content"));
    manifest.digests.insert(entry_path.clone(), claimed);
    let manifest_bytes = serde_json::to_vec(&manifest).expect("serialize manifest");
    let signature = key.sign(&manifest_bytes).to_bytes();
    testing::build_signed_raw(
        &archive,
        &manifest_bytes,
        &signature,
        &[(&entry_path, b"packaged content")],
    );

    let staging = tempfile::tempdir().expect("staging directory");
    assert_eq!(extract(&archive, staging.path()), Err(Error::Signature));
}

#[test]
fn files_without_digests_fail_the_digest_check() {
    use ed25519_dalek::Signer;
    use sha2::Digest;

    let directory = tempfile::tempdir().expect("temporary directory");
    let archive = directory.path().join("sample.iwe-module");
    let key = testing::test_key();
    let entry_path = format!(
        "binaries/{}/module-bin",
        crate::domain::modules::current_target()
    );
    let mut manifest = testing::test_manifest("sample", "1.0.0", "module-bin");
    let packaged = format!("{:x}", sha2::Sha256::digest(b"binary"));
    manifest.digests.insert(entry_path.clone(), packaged);
    let manifest_bytes = serde_json::to_vec(&manifest).expect("serialize manifest");
    let signature = key.sign(&manifest_bytes).to_bytes();
    testing::build_signed_raw(
        &archive,
        &manifest_bytes,
        &signature,
        &[(&entry_path, b"binary"), ("extras/unlisted.txt", b"data")],
    );

    let staging = tempfile::tempdir().expect("staging directory");
    assert_eq!(extract(&archive, staging.path()), Err(Error::Signature));
}

#[test]
fn rejects_missing_archive_and_unsupported_target() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let missing = directory.path().join("missing.iwe-module");
    assert_eq!(manifest(&missing), Err(Error::Invalid));

    let mut only_other = testing::test_manifest("sample", "1.0.0", "module-bin");
    only_other
        .entry
        .remove(crate::domain::modules::current_target());
    only_other
        .entry
        .insert("other-platform".to_owned(), "module-bin".to_owned());
    let archive = directory.path().join("other.iwe-module");
    let key = testing::test_key();
    testing::build_signed_package_with(&archive, only_other, b"binary", &key);
    assert_eq!(
        manifest_with(&archive, Some(&key.verifying_key().to_bytes())),
        Err(Error::UnsupportedTarget)
    );
}

#[test]
fn rejects_declared_entry_missing_from_the_archive() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let archive = directory.path().join("ghost.iwe-module");
    let file = File::create(&archive).expect("create archive");
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut tar = tar::Builder::new(encoder);
    let manifest = serde_json::to_string(&testing::test_manifest("sample", "1.0.0", "ghost"))
        .expect("serialize manifest");
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "manifest.json", manifest.as_bytes())
        .expect("append manifest");
    tar.into_inner()
        .expect("finish tar")
        .finish()
        .expect("finish gzip")
        .flush()
        .expect("flush archive");

    let staging = tempfile::tempdir().expect("staging directory");
    assert_eq!(extract(&archive, staging.path()), Err(Error::Invalid));
}
