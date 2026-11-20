use super::*;

#[tokio::test]
async fn decodes_project_images_and_rejects_invalid_or_external_files() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("icon.PNG");
    std::fs::write(&path, include_bytes!("../../../../assets/icon-256.png")).unwrap();
    assert!(supported(&path));
    let image = read(root.path().into(), path.clone()).await.unwrap();
    assert_eq!(image.dimensions(), (256, 256));
    let other = tempfile::tempdir().unwrap();
    assert_eq!(
        read(other.path().into(), path.clone()).await,
        Err(Failure::Read)
    );
    std::fs::write(&path, b"broken").unwrap();
    assert_eq!(read(root.path().into(), path).await, Err(Failure::Read));
}

#[test]
fn svg_preserves_aspect_ratio_and_straight_alpha() {
    let image = svg(br#"<svg xmlns="http://www.w3.org/2000/svg" width="4096" height="2"><rect width="4096" height="2" fill="red" opacity=".5"/></svg>"#).unwrap();
    assert_eq!(image.dimensions(), (2048, 1));
    assert_eq!(image.get_pixel(1, 0)[0], 255);
    assert!((127..=128).contains(&image.get_pixel(1, 0)[3]));
    assert!(svg(b"broken").is_err());
}
