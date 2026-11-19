//! Reuse language sessions for requests in one project.

use super::{client::Client, discovery};
use crate::domain::language::{Language, Position, embedded};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Default)]
pub(crate) struct Service {
    configured: std::collections::BTreeMap<String, String>,
    root: PathBuf,
    clients: HashMap<&'static str, Client>,
    origins: HashMap<String, PathBuf>,
    buffers: Vec<(PathBuf, Arc<String>)>,
}

impl Service {
    pub(crate) fn needs_maintenance(
        &self,
        root: Option<&Path>,
        retains: impl Fn(&Path) -> bool,
    ) -> bool {
        (!self.clients.is_empty() && root != Some(self.root.as_path()))
            || self.buffers.iter().any(|(path, _)| !retains(path))
            || self.origins.values().any(|path| !retains(path))
    }

    pub(crate) async fn maintain(&mut self, root: &Path, buffers: Vec<(PathBuf, Arc<String>)>) {
        if self.root != root || buffers.is_empty() {
            self.clients.clear();
            self.origins.clear();
            self.root = root.to_owned();
        }
        self.buffers = buffers;
        let mut failed = Vec::new();
        for (key, client) in &mut self.clients {
            for uri in client.document_uris() {
                let retained = self
                    .origins
                    .get(&uri)
                    .is_some_and(|origin| self.buffers.iter().any(|(path, _)| path == origin));
                if !retained && client.close(&uri).await.is_err() {
                    failed.push(*key);
                }
            }
        }
        for key in failed {
            self.clients.remove(key);
        }
        self.origins
            .retain(|_, origin| self.buffers.iter().any(|(path, _)| path == origin));
    }
    pub(crate) fn configure(&mut self, configured: &std::collections::BTreeMap<String, String>) {
        if &self.configured != configured {
            self.clients.clear();
            self.configured = configured.clone();
        }
    }
    pub(crate) async fn request(
        &mut self,
        root: &Path,
        path: &Path,
        text: &str,
        position: Position,
        method: &str,
    ) -> io::Result<Value> {
        if self.root != root {
            self.clients.clear();
            self.root = root.to_owned();
        }
        let language = Language::for_path(path).ok_or(io::ErrorKind::Unsupported)?;
        if language == Language::Env {
            return Ok(super::environment::request(root, path, text, position, method).await);
        }
        let block = (language == Language::Markdown)
            .then(|| embedded::at(text, position))
            .flatten();
        let projected_path = block.as_ref().map(|block| {
            path.with_file_name(format!(
                "{}.iwe-block-{}.{}",
                path.file_name().unwrap_or_default().to_string_lossy(),
                block.start,
                block.extension
            ))
        });
        let server_path = projected_path.as_deref().unwrap_or(path);
        let language = block.as_ref().map_or(language, |block| block.language);
        let server_text = block.as_ref().map_or(text, |block| block.text.as_str());
        let key = match language {
            Language::JavaScript | Language::JavaScriptReact | Language::TypeScriptReact => {
                "typescript"
            }
            Language::JsonComments => "json",
            _ => language.id(),
        };
        if !self.clients.contains_key(key) {
            let base = directories::BaseDirs::new().ok_or(io::ErrorKind::NotFound)?;
            let managed = base.data_dir().join("iwe/language-servers");
            let server = discovery::discover_configured(
                language,
                &managed,
                self.configured.get(key).map(String::as_str),
            )
            .ok_or(io::ErrorKind::NotFound)?;
            let client = Client::connect(&server.program, &server.arguments, root).await?;
            self.clients.insert(key, client);
        }
        let uri = url::Url::from_file_path(server_path)
            .map_err(|_| io::ErrorKind::InvalidInput)?
            .to_string();
        let client = self
            .clients
            .get_mut(key)
            .ok_or(io::ErrorKind::NotConnected)?;
        if method == "textDocument/formatting"
            && (block.is_some()
                || !client.capabilities["documentFormattingProvider"]
                    .as_bool()
                    .unwrap_or_else(|| {
                        client.capabilities["documentFormattingProvider"].is_object()
                    }))
        {
            return Err(io::ErrorKind::Unsupported.into());
        }
        self.origins.insert(uri.clone(), path.to_owned());
        let result = async {
            for (buffer_path, buffer_text) in &self.buffers {
                let Some(buffer_language) = Language::for_path(buffer_path) else {
                    continue;
                };
                let buffer_key = match buffer_language {
                    Language::JavaScript
                    | Language::JavaScriptReact
                    | Language::TypeScriptReact => "typescript",
                    Language::JsonComments => "json",
                    _ => buffer_language.id(),
                };
                if buffer_key == key && buffer_path != server_path {
                    let buffer_uri = url::Url::from_file_path(buffer_path)
                        .map_err(|_| io::ErrorKind::InvalidInput)?
                        .to_string();
                    client
                        .synchronize(&buffer_uri, buffer_language.id(), buffer_text)
                        .await?;
                    self.origins.insert(buffer_uri, buffer_path.clone());
                }
            }
            client.synchronize(&uri, language.id(), server_text).await?;
            if method == "textDocument/diagnostic" {
                return client.document_diagnostics(&uri).await;
            }
            if method == "textDocument/formatting" {
                return client
                    .call(
                        method,
                        json!({"textDocument": {"uri": uri}, "options": {"tabSize": 4, "insertSpaces": true}}),
                    )
                    .await;
            }
            client
                .call(
                    method,
                    json!({"textDocument": {"uri": uri}, "position": position}),
                )
                .await
        }
        .await;
        if result.is_err() {
            self.clients.remove(key);
        }
        result.map(|mut value| {
            if let Some(block) = &block {
                if method == "textDocument/completion" {
                    if let Some(items) = value.as_array_mut() {
                        items.retain(|item| edits_stay_in_block(item, block));
                    } else if let Some(items) = value["items"].as_array_mut() {
                        items.retain(|item| edits_stay_in_block(item, block));
                    }
                }
                if let Ok(original) = url::Url::from_file_path(path) {
                    restore_uri(&mut value, &uri, original.as_str());
                }
            }
            value
        })
    }
}

fn edits_stay_in_block(item: &Value, block: &embedded::Block) -> bool {
    let within = |edit: &Value| {
        let range = edit.get("range").or_else(|| edit.get("replace"));
        range.is_some_and(|range| {
            let start = range["start"]["line"].as_u64();
            let end = range["end"]["line"].as_u64();
            start.is_some_and(|line| line >= block.start as u64 && line < block.end as u64)
                && end.is_some_and(|line| line >= block.start as u64 && line < block.end as u64)
        })
    };
    item.get("textEdit").is_none_or(within)
        && item.get("additionalTextEdits").is_none_or(|edits| {
            edits
                .as_array()
                .is_some_and(|edits| edits.iter().all(within))
        })
}

fn restore_uri(value: &mut Value, projected: &str, original: &str) {
    match value {
        Value::Array(values) => {
            for value in values {
                restore_uri(value, projected, original);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                if matches!(key.as_str(), "uri" | "targetUri") && value.as_str() == Some(projected)
                {
                    *value = Value::String(original.to_owned());
                } else {
                    restore_uri(value, projected, original);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
