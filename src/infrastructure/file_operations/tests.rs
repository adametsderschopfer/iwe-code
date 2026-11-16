use super::*;

fn request(operation: Operation, source: Option<&Path>, destination: Option<&Path>) -> Request {
    Request {
        operation,
        source: source.map(Path::to_path_buf),
        destination: destination.map(Path::to_path_buf),
    }
}

#[test]
fn creates_and_copies_without_overwriting() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    let original = root.join("src");
    let file = original.join("example.rs");
    let copy = root.join("copy");
    run(
        root,
        &[],
        &request(Operation::CreateFolder, None, Some(&original)),
    )
    .unwrap();
    run(
        root,
        &[],
        &request(Operation::CreateFile, None, Some(&file)),
    )
    .unwrap();
    fs::write(&file, b"contents").unwrap();

    let operation = request(Operation::Copy, Some(&original), Some(&copy));
    run(root, &[], &operation).unwrap();
    assert_eq!(fs::read(copy.join("example.rs")).unwrap(), b"contents");
    fs::write(copy.join("example.rs"), b"existing").unwrap();
    assert_eq!(run(root, &[], &operation), Err(Error::Conflict));
    assert_eq!(fs::read(copy.join("example.rs")).unwrap(), b"existing");
    assert_eq!(fs::read(&file).unwrap(), b"contents");
}

#[test]
fn duplicate_suggestion_keeps_extension_and_skips_taken_names() {
    let project = tempfile::tempdir().unwrap();
    let source = project.path().join("main.rs");
    fs::write(&source, b"source").unwrap();
    fs::write(project.path().join("main (1).rs"), b"other").unwrap();
    assert_eq!(
        duplicate_name(project.path(), &source),
        Ok("main (2).rs".into())
    );
}

#[test]
fn rejects_missing_sources_and_never_moves_over_existing_files() {
    let project = tempfile::tempdir().unwrap();
    let root = project.path();
    let source = root.join("source");
    let target = root.join("target");
    let operation = request(Operation::Move, Some(&source), Some(&target));
    assert_eq!(run(root, &[], &operation), Err(Error::Missing));
    fs::write(&source, b"source").unwrap();
    fs::write(&target, b"target").unwrap();
    assert_eq!(run(root, &[], &operation), Err(Error::Conflict));
    assert_eq!(fs::read(&target).unwrap(), b"target");
    fs::remove_file(&target).unwrap();
    run(root, &[], &operation).unwrap();
    assert!(!source.exists());
    assert_eq!(fs::read(target).unwrap(), b"source");
}

#[test]
fn protects_nested_repositories_and_destination_boundaries() {
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = project.path();
    let source = root.join("nested");
    fs::create_dir(&source).unwrap();
    fs::write(source.join(".git"), b"gitdir: somewhere").unwrap();
    assert_eq!(
        run(root, &[], &request(Operation::Trash, Some(&source), None)),
        Err(Error::Protected)
    );
    assert_eq!(
        run(
            root,
            &[],
            &request(Operation::Copy, Some(root), Some(&root.join("other")))
        ),
        Err(Error::Protected)
    );
    assert_eq!(
        run(
            root,
            &[],
            &request(
                Operation::CreateFile,
                None,
                Some(&outside.path().join("file"))
            )
        ),
        Err(Error::Protected)
    );
    fs::remove_file(source.join(".git")).unwrap();
    assert_eq!(
        run(
            root,
            &[],
            &request(Operation::Move, Some(&source), Some(&source.join("inside")))
        ),
        Err(Error::Protected)
    );
    assert!(source.is_dir());
}

#[cfg(unix)]
#[test]
fn refuses_linked_sources_destinations_and_descendants() {
    use std::os::unix::fs::symlink;
    let project = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let root = project.path();
    let folder = root.join("folder");
    fs::create_dir(&folder).unwrap();
    let link = folder.join("link");
    symlink(outside.path(), &link).unwrap();
    assert_eq!(
        run(
            root,
            &[],
            &request(Operation::CreateFile, None, Some(&link.join("outside")))
        ),
        Err(Error::Protected)
    );
    assert_eq!(
        run(
            root,
            &[],
            &request(Operation::Copy, Some(&folder), Some(&root.join("copy")))
        ),
        Err(Error::Protected)
    );
    assert_eq!(
        run(root, &[], &request(Operation::Trash, Some(&link), None)),
        Err(Error::Protected)
    );
    assert!(!outside.path().join("outside").exists());
}

#[test]
fn names_and_io_errors_have_recoverable_results() {
    for name in ["", ".", "..", "a/b", "a\\b", "CON.txt", "x.", "x ", "x:y"] {
        assert_eq!(validate_name(name), Err(Error::InvalidName));
    }
    assert_eq!(validate_name(".Git"), Err(Error::Protected));
    assert_eq!(validate_name(".env"), Ok(()));
    assert_eq!(validate_name("файл.rs"), Ok(()));
    assert_eq!(
        Error::from(io::Error::from(ErrorKind::PermissionDenied)),
        Error::Access
    );
    assert_eq!(
        Error::from(io::Error::from(ErrorKind::CrossesDevices)),
        Error::CrossVolume
    );
}
