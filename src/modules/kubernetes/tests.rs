use super::*;
use serde_json::json;
fn resource() -> Resource {
    Resource {
        name: "app".into(),
        ns: "qa".into(),
        uid: "u1".into(),
        version: "8".into(),
        status: "2/2".into(),
    }
}
#[test]
fn selected_deployment_container_and_version_guard_the_patch() {
    let object = json!({"spec":{"template":{"spec":{"containers":[{"name":"worker","image":"worker:v1"},{"name":"web","image":"web:v1"}]}}}});
    let containers = model::containers(Kind::Deployment, &object);
    let mut s = State {
        selected: Some(resource()),
        details: Some(Arc::new(Details {
            version: "8".into(),
            containers,
            replicas: 2,
            yaml: String::new(),
            ports: vec![],
            events: Ok(vec![]),
        })),
        ..Default::default()
    };
    let _ = s.update(Event::Container("web".into()));
    let _ = s.update(Event::Begin(Operation::Image));
    assert!(s.choose());
    let image = ImageRef::parse("registry.example/team/web:v2").unwrap();
    let _ = s.update(Event::Picked(image.clone()));
    let _ = s.update(Event::Review);
    let d = s.draft.as_ref().unwrap();
    assert!(d.review);
    assert_eq!(d.input, image.to_string());
    let p = model::image_patch(s.selected.as_ref().unwrap(), s.container().unwrap(), &image);
    assert_eq!(p[0]["value"], "u1");
    assert_eq!(p[1]["value"], "8");
    assert_eq!(p[2]["value"], "web");
    assert_eq!(p[3]["value"], "web:v1");
    assert_eq!(p[4]["path"], "/spec/template/spec/containers/1/image");
}
#[test]
fn runtime_image_uses_full_digest_and_falls_back_to_the_spec() {
    let pin = format!("sha256:{}", "a".repeat(64));
    let v = json!({"spec":{"containers":[{"name":"web","image":"acme/web:v1"},{"name":"side","image":"side:v2"}]},"status":{"containerStatuses":[{"name":"side","imageID":pin},{"name":"web","imageID":format!("docker-pullable://acme/web@{pin}")}]}});
    let c = model::containers(Kind::Pod, &v);
    assert_eq!(c[0].running.as_ref().unwrap().version, pin);
    assert!(c[1].running.is_none());
    assert_eq!(c[1].image, "side:v2");
}
#[test]
fn switching_context_discards_old_rows_details_and_namespace_results() {
    let mut s = State {
        contexts: vec![
            ("one".into(), "default".into()),
            ("two".into(), "qa".into()),
        ],
        ..Default::default()
    };
    let _ = s.update(Event::Context("one".into()));
    let old = s.epoch;
    let scope = s.scope;
    let _ = s.update(Event::Context("two".into()));
    let _ = s.update(Event::Listed(old, Ok(vec![resource()])));
    let _ = s.update(Event::Namespaces(scope, Ok(vec!["old".into()])));
    assert!(s.rows.is_empty());
    assert!(s.namespaces.is_empty());
    assert_eq!(s.target.namespace, "qa");
    let _ = s.update(Event::Listed(s.epoch, Err(Key::KubeOffline)));
    assert_eq!(s.notice, Some(Key::KubeOffline));
}
#[test]
fn secrets_hide_values_and_annotations_in_properties_and_manifest_diffs() {
    let metadata = json!({"name":"test","namespace":"qa","annotations":{"private":"sensitive"},"labels":{"x":"sensitive"}});
    let secret = json!({"kind":"Secret","apiVersion":"v1","metadata":metadata,"data":{"x":"sensitive"},"stringData":{"x":"sensitive"}});
    let output = model::redact(json!({"kind":"List","items":[secret]})).to_string();
    assert!(!output.contains("sensitive"));
    assert!(output.contains("••••"));
}
#[test]
fn forbidden_and_offline_diagnostics_do_not_expose_raw_stderr() {
    for (stderr, key) in [
        ("Forbidden: sensitive", Key::KubeDenied),
        (
            "Unable to connect to the server: sensitive",
            Key::KubeOffline,
        ),
    ] {
        assert_eq!(api::failure(stderr.as_bytes()), key)
    }
}

#[test]
fn translated_cluster_controls_fit_and_emit_actions_without_docker() {
    use crate::i18n::{I18n, Language};
    for language in [Language::English, Language::Russian, Language::Spanish] {
        let i = I18n::new(language);
        let mut state = State {
            open: true,
            target: Target {
                context: "qa".into(),
                namespace: "qa".into(),
                ..Default::default()
            },
            rows: vec![resource()],
            ..Default::default()
        };
        for width in [560., 1050.] {
            let mut ui = iced_test::Simulator::with_size(
                iced::Settings::default(),
                (width, 440.),
                panel(&state, &i, false, false),
            );
            ui.click("app").unwrap();
            assert!(
                ui.into_messages()
                    .any(|e| matches!(e,Event::Select(r) if r.name=="app"))
            );
        }
        state.details = Some(Arc::new(Details {
            version: "8".into(),
            yaml: String::new(),
            events: Ok(vec![]),
            containers: vec![],
            replicas: 1,
            ports: vec![],
        }));
        state.selected = Some(resource());
        let _ = state.update(Event::Begin(Operation::Image));
        let mut ui = iced_test::Simulator::with_size(
            iced::Settings::default(),
            (680., 450.),
            panel(&state, &i, false, false),
        );
        assert!(ui.find(i.text(Key::KubeNewImage)).is_ok());
        assert!(ui.find(i.text(Key::KubePick)).is_err());
    }
}

#[tokio::test]
#[ignore = "requires the disposable iwe-acceptance namespace"]
async fn live_manifest_preconditions_permissions_logs_and_forward_cleanup() {
    use iced::futures::StreamExt;
    use iced_test::runtime::{Action, task};
    let file = PathBuf::from(std::env::var_os("KUBECONFIG").unwrap());
    assert!(file.is_absolute());
    assert_eq!(file.file_name().unwrap(), "test-kubeconfig");
    let (_, context) = api::contexts(Some(file.clone())).await.unwrap();
    let target = Target {
        file: Some(file.clone()),
        context,
        namespace: "iwe-acceptance".into(),
    };
    let name = format!("iwe-preview-{}", std::process::id());
    let temp = tempfile::tempdir().unwrap();
    let manifest = temp.path().join("secret.yaml");
    let input = |value: &str| {
        format!(
            "apiVersion: v1\nkind: Secret\nmetadata:\n  name: {name}\n  namespace: iwe-acceptance\nstringData:\n  test: {value}\n"
        )
    };
    std::fs::write(&manifest, input("fixture-before")).unwrap();
    let plan = manifest::preview(&target, manifest.clone()).await.unwrap();
    assert!(plan.0[0].label.contains(&name));
    assert!(!plan.0[0].diff.contains("fixture-before"));
    assert!(!format!("{plan:?}").contains("fixture-before"));
    std::fs::write(&manifest, input("fixture-after")).unwrap();
    manifest::apply(&target, &plan).await.unwrap();
    assert_eq!(
        api::json(
            &target,
            &target.namespace,
            &["get", "secret", &name, "-o", "json"]
        )
        .await
        .unwrap()["data"]["test"],
        "Zml4dHVyZS1iZWZvcmU="
    );
    let plan = manifest::preview(&target, manifest.clone()).await.unwrap();
    api::run(
        &target,
        &target.namespace,
        &words(&["annotate", "secret", &name, "fixture=changed"]),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        manifest::apply(&target, &plan).await,
        Err(Key::KubeConflict)
    );
    api::run(
        &target,
        &target.namespace,
        &words(&["delete", "secret", &name]),
        None,
    )
    .await
    .unwrap();
    let denied = api::run(
        &target,
        &target.namespace,
        &words(&[
            "get",
            "secrets",
            "--as=system:serviceaccount:iwe-acceptance:unprivileged",
        ]),
        None,
    )
    .await;
    assert_eq!(denied, Err(Key::KubeDenied));
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let offline = api::run(
        &target,
        &target.namespace,
        &words(&[
            "get",
            "pods",
            &format!("--server=https://127.0.0.1:{port}"),
            "--request-timeout=2s",
        ]),
        None,
    )
    .await;
    assert_eq!(offline, Err(Key::KubeOffline));
    let pods = api::list(&target, Kind::Pod, None).await.unwrap();
    let pod = pods
        .into_iter()
        .find(|p| p.status == "Running" && p.name.starts_with("sample-"))
        .unwrap();
    let mut s = State {
        target,
        kind: Kind::Pod,
        selected: Some(pod.clone()),
        ..Default::default()
    };
    let logs = s.start_session(
        words(&["logs", "--follow", "--tail=10", &pod.name, "-c", "web"]),
        false,
        pod.name.clone(),
    );
    let id = s.sequence;
    let mut stream = task::into_stream(logs).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Some(action) = stream.next().await {
            if let Action::Output(e) = action {
                let _ = s.update(e);
                if s.sessions[&id].text.contains("IWE fixture") {
                    return;
                }
            }
        }
        panic!("unexpected end of log stream")
    })
    .await
    .unwrap();
    let _ = s.update(Event::CloseSession(id));
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .unwrap()
            .is_none()
    );
    drop(stream);
    let forward = s.start_session(
        words(&[
            "port-forward",
            "--address=127.0.0.1",
            &pod.name,
            &format!("{port}:8080"),
        ]),
        true,
        pod.name,
    );
    let mut stream = task::into_stream(forward).unwrap();
    let id = s.sequence;
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        while let Some(action) = stream.next().await {
            if let Action::Output(e) = action {
                let _ = s.update(e);
                if s.sessions[&id].ready {
                    return;
                }
            }
        }
        panic!("forward did not start")
    })
    .await
    .unwrap();
    assert_eq!(
        reqwest::get(format!("http://127.0.0.1:{port}"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
            .trim(),
        "IWE fixture"
    );
    s.reset();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), stream.next())
            .await
            .unwrap()
            .is_none()
    );
    // The Iced executor releases a completed stream before processing the next task.
    drop(stream);
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_err()
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await
    }
    panic!("forward socket is still listening")
}
