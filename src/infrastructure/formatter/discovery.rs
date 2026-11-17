//! Prefer the closest formatter configuration without invoking package managers.

use crate::domain::language::Language;
use std::{
    ffi::OsString,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tool {
    Prettier,
    Biome,
    Ruff,
    Black,
    Dprint,
}

pub(super) struct Plan {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
    pub directory: PathBuf,
}

pub(super) fn discover(root: &Path, path: &Path) -> io::Result<Option<Plan>> {
    let directories = ancestors(root, path)?;
    let language = Language::for_path(path);
    let python = language == Some(Language::Python);
    let web = matches!(
        language,
        Some(
            Language::JavaScript
                | Language::JavaScriptReact
                | Language::TypeScript
                | Language::TypeScriptReact
                | Language::Json
                | Language::JsonComments
                | Language::Yaml
                | Language::Markdown
                | Language::Html
                | Language::Css
                | Language::Php
                | Language::Toml
        )
    );
    if !python && !web {
        return Ok(None);
    }
    for directory in &directories {
        let selected = configured(directory, language)?;
        if let Some(tool) = selected {
            return plan(tool, directory, path, &directories).map(Some);
        }
    }
    // A local dependency can supply defaults even without a separate configuration file.
    if web {
        for directory in &directories {
            for (tool, package) in [
                (Tool::Prettier, "prettier"),
                (Tool::Biome, "@biomejs/biome"),
            ] {
                if supports_defaults(tool, language)
                    && directory
                        .join("node_modules")
                        .join(package)
                        .join("package.json")
                        .is_file()
                {
                    return plan(tool, directory, path, &directories).map(Some);
                }
            }
        }
    }
    Ok(None)
}

pub(super) fn ancestors(root: &Path, path: &Path) -> io::Result<Vec<PathBuf>> {
    if !root.is_absolute()
        || !path.starts_with(root)
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut result = Vec::new();
    let mut directory = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    loop {
        result.push(directory.to_owned());
        if directory == root {
            break;
        }
        directory = directory.parent().ok_or(io::ErrorKind::InvalidInput)?;
    }
    Ok(result)
}

pub(super) fn configured(directory: &Path, language: Option<Language>) -> io::Result<Option<Tool>> {
    if language == Some(Language::Python) {
        if ["ruff.toml", ".ruff.toml"]
            .iter()
            .any(|name| directory.join(name).is_file())
        {
            return Ok(Some(Tool::Ruff));
        }
        if let Some(text) = read(&directory.join("pyproject.toml"))? {
            let value: toml::Value =
                toml::from_str(&text).map_err(|_| io::ErrorKind::InvalidData)?;
            if let Some(table) = value.get("tool") {
                if table.get("ruff").is_some() {
                    return Ok(Some(Tool::Ruff));
                }
                if table.get("black").is_some() {
                    return Ok(Some(Tool::Black));
                }
            }
        }
        return Ok(None);
    }
    if supports_defaults(Tool::Biome, language)
        && ["biome.json", "biome.jsonc"]
            .iter()
            .any(|name| directory.join(name).is_file())
    {
        return Ok(Some(Tool::Biome));
    }
    if [
        "dprint.json",
        "dprint.jsonc",
        ".dprint.json",
        ".dprint.jsonc",
    ]
    .iter()
    .any(|name| directory.join(name).is_file())
    {
        return Ok(Some(Tool::Dprint));
    }
    if [
        ".prettierrc",
        ".prettierrc.json",
        ".prettierrc.json5",
        ".prettierrc.yml",
        ".prettierrc.yaml",
        ".prettierrc.toml",
        ".prettierrc.js",
        ".prettierrc.cjs",
        ".prettierrc.mjs",
        ".prettierrc.ts",
        ".prettierrc.cts",
        ".prettierrc.mts",
        "prettier.config.js",
        "prettier.config.cjs",
        "prettier.config.mjs",
        "prettier.config.ts",
        "prettier.config.cts",
        "prettier.config.mts",
    ]
    .iter()
    .any(|name| directory.join(name).is_file())
    {
        return Ok(Some(Tool::Prettier));
    }
    if let Some(text) = read(&directory.join("package.json"))? {
        let value: serde_json::Value =
            serde_json::from_str(&text).map_err(|_| io::ErrorKind::InvalidData)?;
        if value.get("prettier").is_some() {
            return Ok(Some(Tool::Prettier));
        }
    }
    Ok(None)
}

// PHP and TOML require Prettier plugins and are not supported by Biome.
// An explicit Prettier configuration may enable those plugins.
fn supports_defaults(tool: Tool, language: Option<Language>) -> bool {
    !matches!(tool, Tool::Prettier | Tool::Biome)
        || !matches!(language, Some(Language::Php | Language::Toml))
}

fn plan(tool: Tool, directory: &Path, path: &Path, directories: &[PathBuf]) -> io::Result<Plan> {
    let name = match tool {
        Tool::Prettier => "prettier",
        Tool::Biome => "biome",
        Tool::Ruff => "ruff",
        Tool::Black => "black",
        Tool::Dprint => "dprint",
    };
    let mut arguments: Vec<OsString> = Vec::new();
    let program = match tool {
        Tool::Prettier | Tool::Biome => {
            let package = if tool == Tool::Prettier {
                "prettier"
            } else {
                "@biomejs/biome"
            };
            let mut script = None;
            for ancestor in directories {
                if let Some(entry) =
                    package_entry(&ancestor.join("node_modules").join(package), name)?
                {
                    script = Some(entry);
                    break;
                }
            }
            if let Some(script) = script {
                arguments.push(script.into_os_string());
                crate::infrastructure::process::executable("node")
            } else {
                // Windows package-manager wrappers need a project-local package entry point.
                if cfg!(windows) {
                    return Err(io::ErrorKind::NotFound.into());
                }
                crate::infrastructure::process::executable(name)
            }
        }
        Tool::Dprint => crate::infrastructure::process::executable(name),
        Tool::Ruff | Tool::Black => {
            let suffix = if cfg!(windows) {
                format!("Scripts/{name}.exe")
            } else {
                format!("bin/{name}")
            };
            directories
                .iter()
                .flat_map(|directory| {
                    [
                        directory.join(".venv").join(&suffix),
                        directory.join("venv").join(&suffix),
                    ]
                })
                .find(|path| path.is_file())
                .unwrap_or_else(|| crate::infrastructure::process::executable(name))
        }
    };
    match tool {
        Tool::Dprint => {
            arguments.push("fmt".into());
            arguments.push("--stdin".into());
            arguments.push(path.into());
        }
        Tool::Prettier => {
            arguments.push("--stdin-filepath".into());
            arguments.push(path.into());
        }
        Tool::Biome => {
            arguments.push("format".into());
            arguments.push("--stdin-file-path".into());
            arguments.push(path.into());
        }
        Tool::Ruff => {
            arguments.push("format".into());
            arguments.push("--stdin-filename".into());
            arguments.push(path.into());
            arguments.push("-".into());
        }
        Tool::Black => {
            arguments.push("--quiet".into());
            arguments.push("--stdin-filename".into());
            arguments.push(path.into());
            arguments.push("-".into());
        }
    }
    Ok(Plan {
        program,
        arguments,
        directory: directory.to_owned(),
    })
}

fn package_entry(package: &Path, name: &str) -> io::Result<Option<PathBuf>> {
    let Some(text) = read(&package.join("package.json"))? else {
        return Ok(None);
    };
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| io::ErrorKind::InvalidData)?;
    let entry = value["bin"]
        .as_str()
        .or_else(|| value["bin"][name].as_str())
        .ok_or(io::ErrorKind::InvalidData)?;
    let relative = Path::new(entry);
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let script = package.join(relative);
    if !script.is_file() {
        return Err(io::ErrorKind::NotFound.into());
    }
    Ok(Some(script))
}

fn read(path: &Path) -> io::Result<Option<String>> {
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    let mut text = String::new();
    file.take(256 * 1024 + 1).read_to_string(&mut text)?;
    if text.len() > 256 * 1024 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(Some(text))
}
