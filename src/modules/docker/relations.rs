//! Resolve image relationships against immutable Docker image IDs.
use super::{
    adapter::{self, Error},
    model::{Item, Kind, operand},
};
use std::{path::Path, sync::Arc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Running {
    pub id: String,
    pub name: String,
}

pub(super) async fn inventory(
    context: &str,
    root: &Path,
    kind: Kind,
) -> Result<Arc<Vec<Item>>, Error> {
    let output = adapter::run(Some(context), &kind.list(), root, false, false).await?;
    tokio::task::spawn_blocking(move || adapter::parse(kind, &output).map(Arc::new))
        .await
        .map_err(|_| Error::Unavailable)?
}

pub(super) async fn open_image(
    context: &str,
    root: &Path,
    container: &Item,
) -> Result<(Arc<Vec<Item>>, String), Error> {
    let args = [
        "container",
        "inspect",
        "--format",
        "{{.Image}}",
        "--",
        &container.id,
    ]
    .map(String::from);
    let (digest, images) = tokio::try_join!(
        adapter::run(Some(context), &args, root, false, false),
        inventory(context, root, Kind::Images)
    )?;
    let image = matching_image(&images, digest.trim(), &container.detail)
        .ok_or(Error::NotFound)?
        .id
        .clone();
    Ok((images, image))
}

pub(super) fn matching_image<'a>(
    images: &'a [Item],
    digest: &str,
    reference: &str,
) -> Option<&'a Item> {
    let mut exact = images.iter().filter(|image| image.detail == digest);
    let fallback = exact.clone().next();
    let latest = format!("{reference}:latest");
    exact
        .find(|image| image.id == reference || image.id == latest)
        .or(fallback)
}

pub(super) async fn running(
    context: &str,
    root: &Path,
    image: &str,
) -> Result<Arc<Vec<Running>>, Error> {
    if !operand(image) {
        return Err(Error::Invalid);
    }
    let mut args = [
        "container",
        "ls",
        "--no-trunc",
        "--filter",
        "status=running",
        "--filter",
    ]
    .map(String::from)
    .to_vec();
    args.extend([
        format!("ancestor={image}"),
        "--format".into(),
        "{{.ID}}".into(),
    ]);
    let output = adapter::run(Some(context), &args, root, false, false).await?;
    let ids: Vec<_> = output.split_whitespace().collect();
    if !ids.iter().all(|id| operand(id)) {
        return Err(Error::Invalid);
    }
    let mut running = Vec::new();
    inspect_batches(
        context,
        root,
        &ids,
        "{{.Id}} {{.Image}} {{.State.Status}} {{.Name}}",
        |rows| {
            running.extend(exact_running(rows, image)?);
            Ok(())
        },
    )
    .await?;
    running.sort_unstable_by(|a, b| a.name.cmp(&b.name));
    Ok(Arc::new(running))
}

pub(super) async fn inspect_batches(
    context: &str,
    root: &Path,
    ids: &[&str],
    format: &str,
    mut accept: impl FnMut(&str) -> Result<(), Error>,
) -> Result<(), Error> {
    for batch in ids.chunks(64) {
        let mut pending = vec![batch];
        while let Some(group) = pending.pop() {
            let mut args = ["container", "inspect", "--format", format, "--"]
                .map(String::from)
                .to_vec();
            args.extend(group.iter().map(|id| (*id).to_owned()));
            match adapter::run(Some(context), &args, root, false, false).await {
                Ok(rows) => accept(&rows)?,
                // Containers can disappear between the list and inspection.
                Err(Error::NotFound) if group.len() > 1 => {
                    let (first, second) = group.split_at(group.len() / 2);
                    pending.extend([second, first]);
                }
                Err(Error::NotFound) => {}
                Err(error) => return Err(error),
            }
        }
    }
    Ok(())
}

pub(super) fn exact_running(rows: &str, image: &str) -> Result<Vec<Running>, Error> {
    let mut result = Vec::new();
    for line in rows.lines() {
        let mut fields = line.split_whitespace();
        let (Some(id), Some(digest), Some(status), Some(name), None) = (
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
            fields.next(),
        ) else {
            return Err(Error::Invalid);
        };
        if status == "running" && digest == image && operand(id) {
            result.push(Running {
                id: id.into(),
                name: name.trim_start_matches('/').into(),
            });
        }
    }
    Ok(result)
}
