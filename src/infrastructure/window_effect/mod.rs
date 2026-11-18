//! Configure native window effects.

mod macos;

pub(crate) use macos::{Zoom, configure_resize, fade_for_exit};
