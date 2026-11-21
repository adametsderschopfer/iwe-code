use super::*;
#[test]
fn image_references_keep_the_registry_port_and_prefer_the_runtime_digest() {
    let pin = format!("sha256:{}", "1".repeat(64));
    let image = ImageRef::parse(&format!(
        "docker-pullable://localhost:5555/team/api:v2@{pin}"
    ))
    .unwrap();
    assert_eq!(image.to_string(), format!("localhost:5555/team/api@{pin}"));
    assert_eq!(
        ImageRef::parse("alpine").unwrap().to_string(),
        "docker.io/library/alpine:latest"
    );
    assert!(ImageRef::parse("bad/../path").is_none());
    assert!(Registry::new("repo", "https://user:password@host", true).is_none());
    assert!(Registry::new("repo", "http://localhost:5555", true).is_none());
}

#[test]
fn internal_dns_registries_are_not_treated_as_docker_hub_namespaces() {
    let r = Registry::new("Internal", "https://images", true).unwrap();
    let img = r.image("team/app", "latest").unwrap();
    assert_eq!(img.host, "images:443");
    assert_eq!(img.repository, "team/app");
}
