//! Locate configured language servers without invoking a command shell.

use crate::domain::language::Language;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

pub(crate) fn check(
    configured: &std::collections::BTreeMap<String, String>,
) -> std::collections::BTreeMap<String, bool> {
    let Some(base) = directories::BaseDirs::new() else {
        return Default::default();
    };
    let managed = base.data_dir().join("iwe/language-servers");
    [
        Language::Python,
        Language::Go,
        Language::TypeScript,
        Language::Php,
        Language::Rust,
        Language::Markdown,
        Language::Json,
        Language::Yaml,
        Language::Toml,
        Language::Shell,
        Language::Html,
        Language::Css,
    ]
    .into_iter()
    .map(|language| {
        let id = language.id();
        let found = discover_configured(language, &managed, configured.get(id).map(String::as_str))
            .is_some_and(|server| server.program.is_file());
        (id.to_owned(), found)
    })
    .collect()
}

#[derive(Debug, Clone)]
pub(crate) struct Server {
    pub program: PathBuf,
    pub arguments: Vec<String>,
}

pub(crate) fn discover_configured(
    language: Language,
    managed: &Path,
    configured: Option<&str>,
) -> Option<Server> {
    let (name, arguments): (&str, &[&str]) = match language {
        Language::Python => ("pyright-langserver", &["--stdio"]),
        Language::Go => ("gopls", &["serve"]),
        Language::JavaScript
        | Language::JavaScriptReact
        | Language::TypeScript
        | Language::TypeScriptReact => ("typescript-language-server", &["--stdio"]),
        Language::Php => ("intelephense", &["--stdio"]),
        Language::Rust => ("rust-analyzer", &[]),
        Language::Markdown => ("marksman", &["server"]),
        Language::Json | Language::JsonComments => ("vscode-json-language-server", &["--stdio"]),
        Language::Yaml => ("yaml-language-server", &["--stdio"]),
        Language::Toml => ("taplo", &["lsp", "stdio"]),
        Language::Shell => ("bash-language-server", &["start"]),
        Language::Html => ("vscode-html-language-server", &["--stdio"]),
        Language::Css => ("vscode-css-language-server", &["--stdio"]),
        Language::Env => return None,
    };
    let mut directories: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    directories.push(managed.join("node_modules/.bin"));
    directories.push(managed.join("bin"));
    if let Some(base) = directories::BaseDirs::new() {
        directories.push(base.home_dir().join(".cargo/bin"));
        directories.push(base.home_dir().join("go/bin"));
    }
    if let Some(program) = configured {
        let program = PathBuf::from(program);
        if cfg!(windows)
            && program
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("cmd"))
        {
            return npm_server(program.parent()?, name, arguments, &directories);
        }
        return Some(Server {
            program,
            arguments: arguments.iter().map(|value| (*value).to_owned()).collect(),
        });
    }
    for directory in &directories {
        let program = directory.join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_owned()
        });
        if program.is_file() {
            return Some(Server {
                program,
                arguments: arguments.iter().map(|value| (*value).to_owned()).collect(),
            });
        }
        if cfg!(windows)
            && let Some(server) = npm_server(directory, name, arguments, &directories)
        {
            return Some(server);
        }
    }
    None
}

fn npm_server(
    directory: &Path,
    name: &str,
    arguments: &[&str],
    search: &[PathBuf],
) -> Option<Server> {
    use std::io::Read;
    let package = match name {
        "pyright-langserver" => "pyright",
        "typescript-language-server"
        | "intelephense"
        | "yaml-language-server"
        | "bash-language-server" => name,
        "vscode-json-language-server"
        | "vscode-html-language-server"
        | "vscode-css-language-server" => "vscode-langservers-extracted",
        _ => return None,
    };
    let modules = if directory.file_name().is_some_and(|name| name == ".bin") {
        directory.parent()?.to_owned()
    } else {
        directory.join("node_modules")
    };
    let package_root = modules.join(package);
    let mut bytes = Vec::new();
    std::fs::File::open(package_root.join("package.json"))
        .ok()?
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 256 * 1024 {
        return None;
    }
    let metadata: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let entry = metadata["bin"]
        .as_str()
        .or_else(|| metadata["bin"][name].as_str())?;
    let relative = Path::new(entry);
    if relative.components().any(|part| {
        !matches!(
            part,
            std::path::Component::Normal(_) | std::path::Component::CurDir
        )
    }) {
        return None;
    }
    let script = package_root.join(relative);
    if !script.is_file() {
        return None;
    }
    let node = std::iter::once(directory.to_owned())
        .chain(search.iter().cloned())
        .map(|directory| directory.join("node.exe"))
        .find(|node| node.is_file())?;
    let mut parameters = vec![script.to_str()?.to_owned()];
    parameters.extend(arguments.iter().map(|argument| (*argument).to_owned()));
    Some(Server {
        program: node,
        arguments: parameters,
    })
}
