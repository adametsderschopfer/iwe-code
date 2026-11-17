//! Initialize a server and route responses while retaining document diagnostics.

use super::Session;
use serde_json::{Value, json};
use std::{collections::HashMap, io, path::Path, time::Duration};

#[cfg(test)]
mod tests;

pub(crate) struct Client {
    session: Session,
    pub capabilities: Value,
    pub diagnostics: HashMap<String, Value>,
    documents: HashMap<String, (i32, String)>,
}

impl Client {
    pub(crate) fn document_uris(&self) -> Vec<String> {
        self.documents.keys().cloned().collect()
    }
    pub(crate) async fn connect(
        program: &Path,
        arguments: &[String],
        root: &Path,
    ) -> io::Result<Self> {
        let root_uri = url::Url::from_directory_path(root)
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        let session = Session::start(program, arguments, root)?;
        let mut client = Self {
            session,
            capabilities: Value::Null,
            diagnostics: HashMap::new(),
            documents: HashMap::new(),
        };
        let result = client.call("initialize", json!({
            "processId": std::process::id(),
            "initializationOptions": {"provideFormatter": true},
            "rootUri": root_uri.as_str(),
            "workspaceFolders": [{"uri": root_uri.as_str(), "name": root.file_name().and_then(|name| name.to_str()).unwrap_or("project")}],
            "capabilities": {
                "general": {"positionEncodings": ["utf-16"]},
                "workspace": {"configuration": true, "workspaceFolders": true},
                "textDocument": {
                    "synchronization": {"didSave": true},
                    "completion": {"completionItem": {"snippetSupport": false, "documentationFormat": ["plaintext"]}},
                    "definition": {"linkSupport": true},
                    "publishDiagnostics": {"versionSupport": true}
                }
            }
        })).await?;
        client.capabilities = result["capabilities"].clone();
        if client.capabilities["positionEncoding"]
            .as_str()
            .is_some_and(|value| value != "utf-16")
        {
            return Err(io::Error::from(io::ErrorKind::Unsupported));
        }
        client.session.notify("initialized", json!({})).await?;
        Ok(client)
    }

    pub(crate) async fn notify(&mut self, method: &str, params: Value) -> io::Result<()> {
        self.session.notify(method, params).await
    }

    pub(crate) async fn synchronize(
        &mut self,
        uri: &str,
        language: &str,
        text: &str,
    ) -> io::Result<()> {
        if let Some((version, previous)) = self.documents.get(uri) {
            if previous == text {
                return Ok(());
            }
            let version = version.checked_add(1).ok_or(io::ErrorKind::InvalidData)?;
            let synchronization = &self.capabilities["textDocumentSync"];
            let kind = synchronization
                .as_u64()
                .or_else(|| synchronization["change"].as_u64())
                .unwrap_or(0);
            let change = match kind {
                1 => json!({"text": text}),
                2 => {
                    let line = previous.bytes().filter(|byte| *byte == b'\n').count();
                    let character = previous
                        .rsplit('\n')
                        .next()
                        .unwrap_or_default()
                        .trim_end_matches('\r')
                        .encode_utf16()
                        .count();
                    json!({"range": {"start": {"line": 0, "character": 0}, "end": {"line": line, "character": character}}, "text": text})
                }
                _ => return Err(io::Error::from(io::ErrorKind::Unsupported)),
            };
            self.notify("textDocument/didChange", json!({"textDocument": {"uri": uri, "version": version}, "contentChanges": [change]})).await?;
            self.documents
                .insert(uri.to_owned(), (version, text.to_owned()));
        } else {
            self.notify("textDocument/didOpen", json!({"textDocument": {"uri": uri, "languageId": language, "version": 1, "text": text}})).await?;
            self.documents.insert(uri.to_owned(), (1, text.to_owned()));
        }
        self.diagnostics.remove(uri);
        Ok(())
    }

    pub(crate) async fn close(&mut self, uri: &str) -> io::Result<()> {
        if self.documents.contains_key(uri) {
            self.notify(
                "textDocument/didClose",
                json!({"textDocument": {"uri": uri}}),
            )
            .await?;
            self.documents.remove(uri);
            self.diagnostics.remove(uri);
        }
        Ok(())
    }

    pub(crate) async fn call(&mut self, method: &str, params: Value) -> io::Result<Value> {
        tokio::time::timeout(Duration::from_secs(30), self.call_inner(method, params))
            .await
            .map_err(|_| io::Error::from(io::ErrorKind::TimedOut))?
    }

    async fn call_inner(&mut self, method: &str, params: Value) -> io::Result<Value> {
        let request = self.session.request(method, params).await?;
        loop {
            let message = self.session.receive().await?;
            if message.get("method").is_some() {
                self.handle(message).await?;
                continue;
            }
            if message["id"].as_u64() == Some(request) {
                if message.get("error").is_some() {
                    return Err(io::Error::from(io::ErrorKind::Other));
                }
                return message
                    .get("result")
                    .cloned()
                    .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidData));
            }
        }
    }

    pub(crate) async fn poll(&mut self) -> io::Result<()> {
        let message = self.session.receive().await?;
        self.handle(message).await
    }

    pub(crate) async fn document_diagnostics(&mut self, uri: &str) -> io::Result<Value> {
        if self.capabilities.get("diagnosticProvider").is_some() {
            let result = self
                .call(
                    "textDocument/diagnostic",
                    json!({"textDocument": {"uri": uri}}),
                )
                .await?;
            return Ok(result["items"].clone());
        }
        let deadline = tokio::time::Instant::now() + Duration::from_millis(150);
        loop {
            if let Some(value) = self.diagnostics.get(uri) {
                let version = self
                    .documents
                    .get(uri)
                    .map(|(version, _)| i64::from(*version));
                if value["version"]
                    .as_i64()
                    .is_none_or(|published| Some(published) == version)
                {
                    return Ok(value["diagnostics"].clone());
                }
            }
            match tokio::time::timeout_at(deadline, self.poll()).await {
                Ok(result) => result?,
                Err(_) => return Ok(Value::Null),
            }
        }
    }

    async fn handle(&mut self, message: Value) -> io::Result<()> {
        let method = message["method"].as_str().unwrap_or_default();
        if let Some(id) = message.get("id") {
            let result = match method {
                "workspace/configuration" => Value::Array(
                    message["params"]["items"]
                        .as_array()
                        .map(|items| vec![Value::Null; items.len()])
                        .unwrap_or_default(),
                ),
                "client/registerCapability"
                | "client/unregisterCapability"
                | "window/workDoneProgress/create" => Value::Null,
                _ => return self.session.reject(id.clone()).await,
            };
            return self.session.respond(id.clone(), result).await;
        }
        if method == "textDocument/publishDiagnostics"
            && let Some(uri) = message["params"]["uri"].as_str()
        {
            self.diagnostics
                .insert(uri.to_owned(), message["params"].clone());
        }
        Ok(())
    }
}
