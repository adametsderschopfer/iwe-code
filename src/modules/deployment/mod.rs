//! Expose deployment controls without depending on the application host.

mod state;
mod view;

pub(crate) use crate::infrastructure::deployment::transfer::{Operation, Output};
pub(crate) use state::{Effect, Event, NameAction, State};
pub(crate) use view::{
    activity, activity_details, connections_view, error_key, overlay, panel, profiles_view,
    settings_dialog, target_popup,
};
