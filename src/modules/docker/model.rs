//! Describe Docker resources and the operations available for each state.

use crate::i18n::Key;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Kind {
    #[default]
    Containers,
    Images,
    Networks,
    Volumes,
    Compose,
}

impl Kind {
    pub(super) const ALL: [Self; 5] = [
        Self::Containers,
        Self::Images,
        Self::Networks,
        Self::Volumes,
        Self::Compose,
    ];
    pub(super) fn key(self) -> Key {
        match self {
            Self::Containers => Key::DockerContainers,
            Self::Images => Key::DockerImages,
            Self::Networks => Key::DockerNetworks,
            Self::Volumes => Key::DockerVolumes,
            Self::Compose => Key::DockerCompose,
        }
    }

    pub(super) fn create(self) -> Action {
        match self {
            Self::Images => Action::Pull,
            Self::Compose => Action::Up,
            _ => Action::Create,
        }
    }
    fn noun(self) -> &'static str {
        match self {
            Self::Containers => "container",
            Self::Images => "image",
            Self::Networks => "network",
            Self::Volumes => "volume",
            Self::Compose => "compose",
        }
    }
    pub(super) fn list(self) -> Vec<String> {
        let mut args = vec![self.noun(), "ls"];
        if matches!(self, Self::Containers | Self::Compose) {
            args.push("--all");
        }
        if matches!(self, Self::Containers | Self::Images | Self::Networks) {
            args.push("--no-trunc");
        }
        args.extend([
            "--format",
            if self == Self::Compose {
                "json"
            } else {
                "{{json .}}"
            },
        ]);
        args.into_iter().map(String::from).collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Item {
    pub id: String,
    pub name: String,
    pub detail: String,
    pub status: String,
    pub state: String,
    pub(super) search: String,
}

impl Item {
    pub(super) fn requires_force_removal(&self) -> bool {
        matches!(self.state.as_str(), "running" | "restarting" | "paused")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Inspect,
    Logs,
    Start,
    Stop,
    Restart,
    Remove,
    Terminal,
    Create,
    Pull,
    Build,
    Pause,
    Resume,
    Tag,
    Up,
    AddContext,
    Connect,
    Disconnect,
}

impl Action {
    pub(super) fn key(self) -> Key {
        match self {
            Self::Inspect => Key::DockerInspect,
            Self::Logs => Key::DockerLogs,
            Self::Start => Key::DockerStart,
            Self::Stop => Key::DockerStop,
            Self::Restart => Key::DockerRestart,
            Self::Remove => Key::DockerRemove,
            Self::Terminal => Key::Terminal,
            Self::Create => Key::DockerCreate,
            Self::Pull => Key::DockerPull,
            Self::Build => Key::DockerBuild,
            Self::Pause => Key::DockerPause,
            Self::Resume => Key::DockerResume,
            Self::Tag => Key::DockerTag,
            Self::Up => Key::DockerUp,
            Self::AddContext => Key::DockerAddContext,
            Self::Connect => Key::DockerConnect,
            Self::Disconnect => Key::DockerDisconnect,
        }
    }
    pub(super) fn actions(kind: Kind, item: &Item, terminal: bool) -> Vec<Self> {
        use Action::*;
        let mut actions = vec![Inspect];
        match kind {
            Kind::Containers => {
                actions.push(Logs);
                match item.state.as_str() {
                    "running" => {
                        if terminal {
                            actions.push(Terminal);
                        }
                        actions.extend([Pause, Restart, Stop]);
                    }
                    "paused" => actions.push(Resume),

                    "created" | "exited" => actions.push(Start),
                    "restarting" => actions.push(Stop),
                    _ => {}
                }
                actions.push(Remove);
            }
            Kind::Compose => actions.extend([Logs, Start, Stop, Restart, Remove]),
            Kind::Images => actions.extend([Tag, Remove]),
            Kind::Networks => {
                actions.extend([Connect, Disconnect]);
                if !["bridge", "host", "none"].contains(&item.name.as_str()) {
                    actions.push(Remove);
                }
            }
            Kind::Volumes => actions.push(Remove),
        }
        actions
    }

    pub(super) fn writes(self) -> bool {
        !matches!(self, Self::Inspect | Self::Logs | Self::Terminal)
    }
    pub(super) fn confirm(self) -> bool {
        matches!(
            self,
            Self::Remove | Self::Pause | Self::Stop | Self::Restart | Self::Disconnect
        )
    }
}

pub(super) fn operand(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && !value.starts_with('-')
        && !value.chars().any(char::is_control)
}

/// Keep every value in its own process argument.
pub(super) fn arguments(
    kind: Kind,
    action: Action,
    item: Option<&Item>,
    value: &str,
    extra: &str,
) -> Option<Vec<String>> {
    let id = item.map(|item| item.id.as_str()).filter(|id| operand(id));
    let value = value.trim();
    let extra = extra.trim();
    let args = match action {
        Action::Inspect if kind == Kind::Compose => {
            vec!["compose", "--project-name", id?, "ps", "--all"]
        }
        Action::Inspect => vec![kind.noun(), "inspect", "--", id?],
        Action::Logs if kind == Kind::Compose => vec![
            "compose",
            "--project-name",
            id?,
            "logs",
            "--no-color",
            "--tail",
            "500",
        ],
        Action::Logs if kind == Kind::Containers => {
            vec!["container", "logs", "--tail", "500", "--", id?]
        }
        Action::Pause | Action::Resume if kind == Kind::Containers => vec![
            "container",
            if action == Action::Pause {
                "pause"
            } else {
                "unpause"
            },
            "--",
            id?,
        ],
        Action::Build if kind == Kind::Images && operand(value) && operand(extra) => {
            let parent = std::path::Path::new(value).parent()?;
            let directory = if parent.as_os_str().is_empty() {
                "."
            } else {
                parent.to_str()?
            };
            vec!["build", "--file", value, "--tag", extra, "--", directory]
        }
        Action::Start | Action::Stop | Action::Restart | Action::Remove => {
            let verb = match action {
                Action::Start => "start",
                Action::Stop => "stop",
                Action::Restart => "restart",
                _ => "rm",
            };
            if kind == Kind::Compose {
                vec![
                    "compose",
                    "--project-name",
                    id?,
                    if action == Action::Remove {
                        "down"
                    } else {
                        verb
                    },
                ]
            } else if action == Action::Remove
                && kind == Kind::Containers
                && item.is_some_and(Item::requires_force_removal)
            {
                vec!["container", "rm", "--force", "--", id?]
            } else if action == Action::Remove || kind == Kind::Containers {
                vec![kind.noun(), verb, "--", id?]
            } else {
                return None;
            }
        }
        Action::Create if kind == Kind::Containers => {
            return super::container::Options::default().arguments(value, extra);
        }
        Action::Create if matches!(kind, Kind::Volumes | Kind::Networks) && operand(value) => {
            if extra.is_empty() {
                vec![kind.noun(), "create", "--", value]
            } else if operand(extra) {
                vec![kind.noun(), "create", "--driver", extra, "--", value]
            } else {
                return None;
            }
        }
        Action::Tag if kind == Kind::Images && operand(value) => {
            vec!["image", "tag", "--", id?, value]
        }
        Action::Pull if kind == Kind::Images && operand(value) => {
            vec!["image", "pull", "--", value]
        }
        Action::Connect | Action::Disconnect if kind == Kind::Networks && operand(value) => vec![
            "network",
            if action == Action::Connect {
                "connect"
            } else {
                "disconnect"
            },
            "--",
            id?,
            value,
        ],
        Action::Up if kind == Kind::Compose && operand(value) => {
            vec!["compose", "--file", value, "up", "--detach"]
        }
        Action::Terminal if kind == Kind::Containers && operand(value) => {
            vec!["exec", "--interactive", "--tty", "--", id?, value]
        }
        Action::AddContext if operand(value) => {
            let url = url::Url::parse(extra).ok()?;
            if url.scheme() != "ssh"
                || url.host_str().is_none()
                || url.password().is_some()
                || extra.contains([',', '\n', '\r'])
            {
                return None;
            }
            return Some(vec![
                "context".into(),
                "create".into(),
                "--docker".into(),
                format!("host={extra}"),
                "--".into(),
                value.into(),
            ]);
        }
        _ => return None,
    };
    Some(args.into_iter().map(String::from).collect())
}
