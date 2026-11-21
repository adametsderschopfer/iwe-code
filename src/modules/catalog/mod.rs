//! Describe bundled modules independently of their presentation and runtime.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModuleId {
    Git,
    Pipelines,
    Terminal,
    Deployment,
    Docker,
    Kubernetes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    System,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Module {
    pub id: ModuleId,
    pub origin: Origin,
}

pub(crate) const MODULES: &[Module] = &[
    Module {
        id: ModuleId::Git,
        origin: Origin::System,
    },
    Module {
        id: ModuleId::Pipelines,
        origin: Origin::System,
    },
    Module {
        id: ModuleId::Terminal,
        origin: Origin::System,
    },
    Module {
        id: ModuleId::Deployment,
        origin: Origin::System,
    },
    Module {
        id: ModuleId::Docker,
        origin: Origin::System,
    },
];

impl ModuleId {
    pub(crate) fn listed(self) -> bool {
        MODULES.iter().any(|module| module.id == self)
    }

    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::Kubernetes => "kubernetes",
            Self::Docker => "docker",
            Self::Git => "git",
            Self::Pipelines => "pipelines",
            Self::Terminal => "terminal",
            Self::Deployment => "deployment",
        }
    }
}
