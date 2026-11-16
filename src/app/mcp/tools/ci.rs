//! Read CI data through the existing provider adapters.

use super::{failure_message, optional_count, optional_string, resolve_project};
use crate::{
    domain::{
        Failure,
        jobs::Status,
        repository::{Provider, Remote, Repository},
    },
    infrastructure::{auth::Service, repository},
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const RUN_LIMIT: usize = 30;

pub(super) fn repository_parameter() -> Value {
    json!({
        "type": "string",
        "description": "Repository ID from list_ci_repositories. Omit only when one repository supports CI."
    })
}

pub(super) async fn list_repositories(arguments: &Value) -> Result<Value, String> {
    let (project_id, repositories, incomplete) = discover(arguments).await?;
    let repositories: Vec<_> = repositories
        .iter()
        .map(|(id, remote)| {
            json!({
                "id": id,
                "provider": remote.provider.map(Provider::name),
                "host": remote.host,
                "project": remote.project,
            })
        })
        .collect();
    Ok(json!({ "projectId": project_id, "repositories": repositories, "incomplete": incomplete }))
}

pub(super) async fn list_runs(arguments: &Value) -> Result<Value, String> {
    let limit = optional_count(arguments, "limit")?.unwrap_or(RUN_LIMIT);
    if !(1..=RUN_LIMIT).contains(&limit) {
        return Err("limit must be between 1 and 30.".to_owned());
    }
    let (project_id, repository, remote) = select_repository(arguments).await?;
    let runs = Service::new()
        .read_ci_runs(&remote)
        .await
        .map_err(ci_error)?;
    let runs: Vec<_> = runs
        .into_iter()
        .take(limit)
        .map(|run| {
            json!({
                "id": run.id,
                "number": run.number,
                "title": run.title,
                "branch": run.branch,
                "commit": run.commit,
                "status": run.status,
                "date": run.date,
                "url": run.url,
            })
        })
        .collect();
    Ok(json!({ "projectId": project_id, "repository": repository, "runs": runs }))
}

pub(super) async fn run_jobs(arguments: &Value) -> Result<Value, String> {
    let run_id = arguments
        .get("runId")
        .and_then(Value::as_u64)
        .filter(|id| *id != 0)
        .ok_or("runId must be a positive integer.")?;
    let (project_id, repository, remote) = select_repository(arguments).await?;
    let details = Service::new()
        .read_ci_jobs(&remote, run_id)
        .await
        .map_err(ci_error)?;
    let stages: Vec<_> = details
        .stages
        .into_iter()
        .map(|stage| {
            let status = stage_status(stage.status());
            let jobs: Vec<_> = stage
                .jobs
                .into_iter()
                .map(|job| {
                    json!({
                        "id": job.id, "name": job.name, "status": job.status, "url": job.url,
                    })
                })
                .collect();
            json!({ "name": stage.name, "status": status, "jobs": jobs })
        })
        .collect();
    Ok(
        json!({ "projectId": project_id, "repository": repository, "runId": run_id, "stages": stages }),
    )
}

async fn discover(arguments: &Value) -> Result<(String, Vec<(String, Remote)>, bool), String> {
    let project = resolve_project(arguments).await?;
    let id = project.id();
    let found = repository::load_project_remotes(project.directory)
        .await
        .map_err(failure_message)?;
    Ok((
        id,
        found
            .repositories
            .into_iter()
            .filter_map(ci_remote)
            .collect(),
        !found.failures.is_empty(),
    ))
}

fn ci_remote(repo: Repository) -> Option<(String, Remote)> {
    let remote = repo.remote.filter(|remote| remote.provider.is_some())?;
    let hash = Sha256::digest(repo.root.as_os_str().as_encoded_bytes());
    Some((format!("{hash:x}"), remote))
}

async fn select_repository(arguments: &Value) -> Result<(String, String, Remote), String> {
    let requested = optional_string(arguments, "repository")?;
    let (id, repositories, incomplete) = discover(arguments).await?;
    let found = match requested {
        Some(requested) => repositories
            .into_iter()
            .find(|(key, _)| *key == requested)
            .ok_or("Unknown CI repository. Call list_ci_repositories."),
        None => match repositories.len() {
            1 if !incomplete => repositories
                .into_iter()
                .next()
                .ok_or("No CI repository found."),
            0 if !incomplete => Err("No GitHub or GitLab repository was found in this project."),
            _ => Err("Specify a repository ID from list_ci_repositories."),
        },
    }?;
    Ok((id, found.0, found.1))
}

fn stage_status(status: Status) -> &'static str {
    match status {
        Status::Passed => "passed",
        Status::Failed => "failed",
        Status::Running => "running",
        Status::Canceling => "canceling",
        Status::Pending => "pending",
        Status::Manual => "manual",
        Status::Cancelled => "cancelled",
        Status::Skipped => "skipped",
        Status::Unknown => "unknown",
    }
}

fn ci_error(error: Failure) -> String {
    match error {
        Failure::AuthExpired | Failure::InvalidToken => "The CI account has expired. Reconnect it in IWE Code.",
        Failure::Credentials => "IWE Code could not read the saved CI account.",
        Failure::Access | Failure::PermissionDenied => "The CI provider denied access. Check the account and repository permissions in IWE Code.",
        Failure::RateLimited => "The CI provider is limiting requests. Try again later.",
        Failure::TooLarge => "The CI response exceeds the supported size limit.",
        _ => "IWE Code could not read the CI data from the provider.",
    }.to_owned()
}
