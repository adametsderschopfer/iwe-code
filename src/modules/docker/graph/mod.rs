//! Build a stable diagram from Docker inventories and direct container attachments.
mod view;
use super::{Item, Kind, adapter::Error, relations};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::Arc,
};
pub(super) use view::{Interaction, Viewport, panel};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Id(pub Kind, pub String);
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Node {
    pub id: Id,
    pub name: String,
    pub slot: usize,
}
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Data {
    pub nodes: Vec<Node>,
    pub edges: Vec<(usize, usize)>,
}
impl Id {
    fn lane(&self) -> usize {
        match self.0 {
            Kind::Images | Kind::Compose => 0,
            Kind::Containers => 1,
            Kind::Networks | Kind::Volumes => 2,
        }
    }
}
#[derive(Default)]
struct Resources {
    names: BTreeMap<Id, String>,
    network_ids: BTreeMap<String, String>,
    edges: BTreeSet<(Id, Id)>,
}
impl Resources {
    fn inventory(&mut self, kind: Kind, items: &[Item]) {
        for item in items {
            let key = if kind == Kind::Images {
                &item.detail
            } else {
                &item.id
            };
            if kind == Kind::Networks {
                self.network_ids.insert(item.name.clone(), key.clone());
            }
            self.names
                .entry(Id(kind, key.clone()))
                .or_insert_with(|| item.name.clone());
        }
    }
    fn attachments(&mut self, rows: &str) -> Result<(), Error> {
        for row in rows.lines() {
            let parts: Vec<_> = row.split('\t').collect();
            let [container, image, project, networks, volumes] = parts.as_slice() else {
                return Err(Error::Invalid);
            };
            let source = Id(Kind::Containers, (*container).into());
            if !project.is_empty() {
                let group = Id(Kind::Compose, (*project).into());
                self.names
                    .entry(group.clone())
                    .or_insert_with(|| (*project).into());
                self.edges.insert((group, source.clone()));
            }
            self.edges
                .insert((Id(Kind::Images, (*image).into()), source.clone()));
            for reference in networks.split_whitespace() {
                let id = reference.strip_prefix("id:").or_else(|| {
                    self.network_ids
                        .get(reference.strip_prefix("name:")?)
                        .map(String::as_str)
                });
                if let Some(id) = id {
                    self.edges
                        .insert((source.clone(), Id(Kind::Networks, id.into())));
                }
            }
            for name in volumes.split_whitespace() {
                self.edges
                    .insert((source.clone(), Id(Kind::Volumes, name.into())));
            }
        }
        Ok(())
    }
    fn layout(self, previous: &Data) -> Data {
        let mut slots = BTreeMap::new();
        let mut used: [BTreeSet<usize>; 3] = Default::default();
        for node in &previous.nodes {
            if self.names.contains_key(&node.id) {
                used[node.id.lane()].insert(node.slot);
                slots.insert(&node.id, node.slot);
            }
        }
        let mut next = [0; 3];
        let linked: BTreeSet<_> = self.edges.iter().flat_map(|(a, b)| [a, b]).collect();
        let mut resources: Vec<_> = self.names.into_iter().collect();
        resources.sort_by_key(|(id, _)| !linked.contains(id));
        let mut nodes: Vec<_> = resources
            .into_iter()
            .map(|(id, name)| {
                let lane = id.lane();
                let slot = slots.get(&id).copied().unwrap_or_else(|| {
                    while used[lane].contains(&next[lane]) {
                        next[lane] += 1;
                    }
                    let slot = next[lane];
                    next[lane] += 1;
                    slot
                });
                Node { id, name, slot }
            })
            .collect();
        nodes.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        let indices: BTreeMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(idx, node)| (&node.id, idx))
            .collect();
        let edges = self
            .edges
            .into_iter()
            .filter_map(|(a, b)| Some((*indices.get(&a)?, *indices.get(&b)?)))
            .collect();
        Data { nodes, edges }
    }
}
pub(super) async fn load(
    context: &str,
    root: &Path,
    previous: Arc<Data>,
) -> Result<Arc<Data>, Error> {
    let (containers, images, networks, volumes) = tokio::try_join!(
        relations::inventory(context, root, Kind::Containers),
        relations::inventory(context, root, Kind::Images),
        relations::inventory(context, root, Kind::Networks),
        relations::inventory(context, root, Kind::Volumes)
    )?;
    let ids: Vec<_> = containers.iter().map(|c| c.id.as_str()).collect();
    let mut data = Resources::default();
    for (kind, items) in [
        (Kind::Containers, &containers),
        (Kind::Images, &images),
        (Kind::Networks, &networks),
        (Kind::Volumes, &volumes),
    ] {
        data.inventory(kind, items);
    }
    // Request only identifiers and attachment names. Full inspection can include environment secrets.
    let format = concat!(
        "{{.Id}}\t{{.Image}}\t",
        r#"{{with index .Config.Labels "com.docker.compose.project"}}{{.}}{{end}}"#,
        "\t{{range $name,$network := .NetworkSettings.Networks}}{{if $network.NetworkID}}id:{{$network.NetworkID}}{{else}}name:{{$name}}{{end}} {{end}}",
        "\t{{range .Mounts}}{{if eq .Type \"volume\"}}{{.Name}} {{end}}{{end}}"
    );
    relations::inspect_batches(context, root, &ids, format, |rows| data.attachments(rows)).await?;
    tokio::task::spawn_blocking(move || {
        let graph = data.layout(&previous);
        if graph == *previous {
            previous
        } else {
            Arc::new(graph)
        }
    })
    .await
    .map_err(|_| Error::Unavailable)
}
#[cfg(test)]
mod tests;
