//! Identify the object and assistant selected for an explanation.

use super::{
    jobs::Job,
    repository::{
        Pipeline,
        project::{BranchKey, CommitKey},
    },
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Agent {
    Codex,
    Claude,
    ZCode,
    DeepSeek,
}

#[derive(Debug, Clone)]
pub(crate) enum Target {
    File(PathBuf),
    Commit(CommitKey),
    Branch(BranchKey),
    Project(String, PathBuf),
    Space(usize),
    Pipeline(Pipeline),
    Job(u64, Job),
}
