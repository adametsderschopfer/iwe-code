//! Build the Docker arguments for container creation from individual form fields.

use super::model::operand;
use crate::i18n::Key;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Field {
    Ports,
    Mounts,
    Environment,
    Command,
}

impl Field {
    pub(super) const ALL: [Self; 4] = [Self::Ports, Self::Mounts, Self::Environment, Self::Command];

    pub(super) fn key(self) -> Key {
        match self {
            Self::Ports => Key::DockerPorts,
            Self::Mounts => Key::DockerMounts,
            Self::Environment => Key::DockerEnvironment,
            Self::Command => Key::DockerCommand,
        }
    }
}

#[derive(Default)]
pub(super) struct Options([String; 4]);

impl Options {
    pub(super) fn get(&self, field: Field) -> &str {
        &self.0[field as usize]
    }
    pub(super) fn set(&mut self, field: Field, value: String) {
        self.0[field as usize] = value;
    }

    fn values(&self, field: Field) -> Option<Vec<String>> {
        let text = self.get(field);
        if text.len() > 16_384 || text.chars().any(char::is_control) {
            return None;
        }
        let values = shell_words::split(text).ok()?;
        (values.len() <= 256).then_some(values)
    }

    pub(super) fn arguments(&self, image: &str, name: &str) -> Option<Vec<String>> {
        if !operand(image) || (!name.is_empty() && !operand(name)) {
            return None;
        }
        let mut args = vec!["container".into(), "create".into()];
        if !name.is_empty() {
            args.extend(["--name".into(), name.into()]);
        }
        for (field, flag) in [
            (Field::Ports, "--publish"),
            (Field::Mounts, "--mount"),
            (Field::Environment, "--env"),
        ] {
            for value in self.values(field)? {
                if !operand(&value) {
                    return None;
                }
                args.extend([flag.into(), value]);
            }
        }
        args.extend(["--".into(), image.into()]);
        args.extend(self.values(Field::Command)?);
        Some(args)
    }
}
