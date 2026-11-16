//! Configure native window resizing.

mod macos;

pub(crate) use macos::{Zoom, configure_resize};
