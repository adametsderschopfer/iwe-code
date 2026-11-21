//! Keep the terminal runtime, interface, and shell adapter together.

mod session;
mod state;
mod style;
mod view;

pub(crate) use state::{Event, Program, State, Status};
pub(crate) use view::{tab_menu, view};
