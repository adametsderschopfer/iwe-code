//! Share neutral dark and light surfaces above the native window material.

mod buttons;
mod catalogs;
mod fields;
pub(crate) mod iced;
mod palette;
mod runtime;
mod selection;
pub(crate) mod syntax;
pub(crate) use buttons::{compact_button, text_button};
pub(crate) use fields::{
    code_editor, compact_field, labeled_field, multiline_field, text_field, validated_input,
};
pub(crate) use palette::*;
pub(crate) use runtime::{
    InstalledTheme, Theme, code_font, font, selected_code_font, selected_terminal_font, semibold,
};
pub(crate) use selection::{Select, checkbox, switch};

use ::iced::{
    Border, Color, theme,
    widget::{
        button, checkbox as iced_checkbox, container, overlay::menu, pane_grid, pick_list,
        scrollable, slider, toggler,
    },
};

pub(crate) const CONTEXT_MENU_ENTRANCE: std::time::Duration = std::time::Duration::from_millis(100);
pub(crate) const CONTEXT_SUBMENU_OVERLAP: f32 = 8.;

pub(crate) fn file_status(status: crate::domain::changes::FileStatus) -> Tone {
    use crate::domain::changes::FileStatus;
    match status {
        FileStatus::Ignored => ICON_ORANGE,
        FileStatus::Untracked | FileStatus::Added => FILE_ADDED,
        FileStatus::Modified | FileStatus::Renamed => FILE_MODIFIED,
        FileStatus::Deleted | FileStatus::Conflict => GIT_REMOVED,
    }
}

pub(crate) fn repository_color(index: usize) -> Tone {
    const COLORS: [Tone; 6] = [BLUE, PURPLE, GREEN, YELLOW, PINK, ICON_CYAN];
    COLORS[index % COLORS.len()]
}

pub(crate) fn line_change(change: crate::ui::code::LineChange) -> Tone {
    use crate::ui::code::LineChange;
    match change {
        LineChange::Added => GIT_ADDED,
        LineChange::Modified => DIFF_MODIFIED,
        LineChange::Removed => GIT_REMOVED,
    }
}

pub(crate) fn unified_color(change: crate::ui::code::LineChange) -> Tone {
    match change {
        crate::ui::code::LineChange::Modified => UNIFIED_MODIFIED,
        _ => line_change(change),
    }
}

pub(crate) fn change_fill(color: Tone, theme: &Theme, opacity: f32) -> Color {
    color
        .resolve(theme)
        .scale_alpha(opacity * theme.diff_opacity())
}

pub(crate) fn source_row(
    theme: &Theme,
    mut style: container::Style,
    highlighted: bool,
) -> container::Style {
    if highlighted {
        style.background = Some(CODE_SELECTION.resolve(theme).into());
    }
    style
}

pub(crate) fn unified_row(
    change: Option<crate::ui::code::LineChange>,
) -> impl Fn(&Theme) -> container::Style {
    move |theme| container::Style {
        background: change.map(|change| change_fill(unified_color(change), theme, 0.20).into()),
        ..Default::default()
    }
}

pub(crate) fn unified_gutter(
    change: Option<crate::ui::code::LineChange>,
) -> impl Fn(&Theme) -> container::Style {
    move |theme| container::Style {
        background: Some(
            change
                .map_or_else(
                    || SIDEBAR.resolve(theme),
                    |change| change_fill(unified_color(change), theme, 0.12),
                )
                .into(),
        ),
        ..Default::default()
    }
}

pub(crate) fn diff_row(
    change: Option<crate::ui::code::LineChange>,
) -> impl Fn(&Theme) -> container::Style {
    move |theme| container::Style {
        background: change.map(|change| change_fill(line_change(change), theme, 0.11).into()),
        ..Default::default()
    }
}

pub(crate) const COMMIT_GRAPH_PADDING: f32 = 12.;
pub(crate) const COMMIT_REPOSITORY_GRAPH_PADDING: f32 = 4.;
pub(crate) const COMMIT_REPOSITORY_MAX_WIDTH: f32 = 160.;

#[cfg(test)]
pub(crate) const SIDEBAR_WIDTH: f32 =
    crate::domain::workspace::Preferences::DEFAULT_SPACES_SIDEBAR_WIDTH;
pub(crate) const SIDEBAR_SCROLL_GAP: f32 = 6.0;
pub(crate) const SETTINGS_SIDEBAR_WIDTH: f32 = 220.0;
pub(crate) const SETTINGS_CONTENT_INSET: f32 = 24.0;
pub(crate) const SETTINGS_CONTENT_WIDTH: f32 = 860.0;
pub(crate) const SETTINGS_CONTROL_WIDTH: f32 = 320.0;
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) const SETTINGS_HEADER_HEIGHT: f32 = 40.0;
pub(crate) const NOTIFICATION_WIDTH: f32 = 380.0;
pub(crate) const NOTIFICATION_INSET: f32 = 16.0;
pub(crate) const NOTIFICATION_GAP: f32 = 8.0;
pub(crate) const NOTIFICATION_SLIDE_DURATION: std::time::Duration =
    std::time::Duration::from_millis(200);
pub(crate) const NOTIFICATION_SETTLE_DURATION: std::time::Duration =
    std::time::Duration::from_millis(200);
pub(crate) const NOTIFICATION_PADDING: f32 = 14.0;
pub(crate) const FILE_PANEL_TOOL_SIZE: f32 = 40.0;
pub(crate) const FILE_PANEL_TOOL_INSET: f32 = 4.0;
pub(crate) const PANEL_HEADER_HEIGHT: f32 = 34.0;
pub(crate) const FILE_PANEL_HEADER_HEIGHT: f32 = PANEL_HEADER_HEIGHT;
pub(crate) const FILE_TABS_HEIGHT: f32 = FILE_PANEL_HEADER_HEIGHT + 1.0;
pub(crate) const SOURCE_MIN_WIDTH: f32 = 320.0;
pub(crate) const FILE_PANEL_RESIZE_GRAB: f32 = 5.0;
pub(crate) const FILE_PANEL_CONTENT_INSET: f32 = 8.0;
#[cfg(target_os = "windows")]
pub(crate) const WINDOW_CONTROL_WIDTH: f32 = 46.0;
#[cfg(target_os = "windows")]
pub(crate) const WINDOW_CAPTION_WIDTH: f32 = 3. * WINDOW_CONTROL_WIDTH;
#[cfg(target_os = "windows")]
pub(crate) const WINDOW_CAPTION_INSET: f32 = 24.;
#[cfg(target_os = "windows")]
pub(crate) const WINDOW_CONTROL_HEIGHT: f32 = 40.0;
#[cfg(target_os = "windows")]
pub(crate) const WINDOW_RESIZE_GRIP: f32 = 5.0;
pub(crate) const COMMIT_AUTHOR_WIDTH: f32 = 144.0;
pub(crate) const COMMIT_DATE_WIDTH: f32 = 132.0;
pub(crate) const COMMIT_METADATA_SIZE: f32 = 13.0;
pub(crate) const COMMIT_HASH_WIDTH: f32 = 64.0;
pub(crate) const HEADER_REPOSITORY_WIDTH: f32 = 220.0;
pub(crate) const HEADER_REPOSITORY_MENU_WIDTH: f32 = 360.0;
pub(crate) const HEADER_REPOSITORY_ROW_HEIGHT: f32 = 48.0;
pub(crate) const HEADER_REPOSITORY_MENU_HEIGHT: f32 = 336.0;
pub(crate) const CODE_WHEEL_SCROLL_SCALE: f32 = 0.75;
pub(crate) const CODE_SCROLLBAR_WIDTH: f32 = 20.0;
pub(crate) const CODE_SCROLLBAR_THUMB_WIDTH: f32 = 10.0;
pub(crate) const CODE_CHANGE_MARK_WIDTH: f32 = 4.0;
pub(crate) const CODE_CHANGE_MARK_MIN_HEIGHT: f32 = 3.0;
pub(crate) const UNIFIED_NUMBER_MIN_WIDTH: f32 = 32.0;
pub(crate) const UNIFIED_NUMBER_GAP: f32 = 6.0;
pub(crate) const UNIFIED_GUTTER_PADDING: f32 = 8.0;
pub(crate) const SOURCE_GUTTER_WIDTH: f32 = 91.0;
pub(crate) const SOURCE_MARKERS_WIDTH: f32 = 35.0;
pub(crate) const DIFF_GUTTER_WIDTH: f32 = 65.0;
pub(crate) const DIFF_MARKERS_WIDTH: f32 = 11.0;
pub(crate) const GIT_PANE_SPACING: f32 = 1.0;
pub(crate) const GIT_PANE_MIN_SIZE: f32 = 120.0;
pub(crate) const BRANCH_HEADER_INSET: f32 = 8.0;
pub(crate) const MACOS_WINDOW_CONTROLS_WIDTH: f32 = 80.;
pub(crate) const WORKSPACE_HEADER_HEIGHT: f32 = 56.;
pub(crate) const WORKSPACE_HEADER_BUTTON_SIZE: f32 = 40.;
pub(crate) const SPACES_SIDEBAR_TRANSITION: std::time::Duration =
    std::time::Duration::from_millis(133);
pub(crate) const SPACE_ACTION_TRANSITION: std::time::Duration =
    std::time::Duration::from_millis(150);
pub(crate) const FILE_TAB_MIN_WIDTH: f32 = 200.0;
pub(crate) const FILE_TAB_TEXT_SIZE: f32 = 12.0;
pub(crate) const FILE_TAB_NAME_LIMIT: usize = 35;
pub(crate) const FILE_TAB_ICON_WIDTH: f32 = 16.0;
pub(crate) const FILE_TAB_LABEL_GAP: f32 = 6.0;
pub(crate) const FILE_TAB_PADDING: f32 = 8.0;
pub(crate) const FILE_TAB_CLOSE_WIDTH: f32 = 28.0;
pub(crate) const FILE_TAB_END_GAP: f32 = 4.0;
pub(crate) const FILE_TAB_TOOL_SIZE: f32 = 36.0;
pub(crate) const FILE_TAB_TOOL_HEIGHT: f32 = 28.0;
pub(crate) const FILE_TAB_TOOL_GAP: f32 = 6.0;
pub(crate) const TAB_MENU_WIDTH: f32 = 380.0;
pub(crate) const TAB_MENU_ROW_HEIGHT: f32 = 36.0;
pub(crate) const TAB_MENU_MAX_HEIGHT: f32 = 324.0;
pub(crate) const SEARCH_OVERLAY_WIDTH: f32 = 640.0;
pub(crate) const SEARCH_OVERLAY_TOP_INSET: f32 = 96.0;
pub(crate) const SEARCH_RESULTS_HEIGHT: f32 = 408.0;
pub(crate) const SEARCH_ROW_HEIGHT: f32 = 40.0;
pub(crate) const SEARCH_ROW_SPACING: f32 = 2.0;
pub(crate) const SEARCH_GROUP_HEIGHT: f32 = 32.0;
pub(crate) const SEARCH_WINDOW_ROWS: usize = 8;
pub(crate) const SEARCH_WINDOW_CENTER: usize = 3;
pub(crate) const FILE_TREE_ROW_HEIGHT: f32 = 24.0;
pub(crate) const FILE_TREE_TEXT_SIZE: f32 = 14.0;
pub(crate) const FILE_TREE_INDENT: f32 = 20.0;
pub(crate) const SPACE_PROJECT_HEIGHT: f32 = 56.0;
pub(crate) const PROJECT_STATUS_ICON_SIZE: f32 = 16.0;
pub(crate) const SPACE_PROJECT_GAP: f32 = 4.0;
pub(crate) const PIPELINE_ROW_HEIGHT: f32 = 72.0;
pub(crate) const PIPELINE_BADGE_SIZE: f32 = 30.0;
pub(crate) const PIPELINE_CONNECTOR_WIDTH: f32 = 8.0;
pub(crate) const PIPELINE_POPUP_WIDTH: f32 = 380.0;
pub(crate) const PIPELINE_POPUP_JOBS_HEIGHT: f32 = 240.0;
pub(crate) const PIPELINE_JOB_HEIGHT: f32 = 40.0;

fn toggle_style(theme: &Theme, status: toggler::Status) -> toggler::Style {
    use toggler::Status::*;
    let (track, knob) = match status {
        Active { is_toggled: true } => (TEXT, PRIMARY_TEXT),
        Active { is_toggled: false } => (LINE, MUTED),
        Hovered { is_toggled: true } => (PRIMARY_HOVER, PRIMARY_TEXT),
        Hovered { is_toggled: false } => (INPUT_BORDER, TEXT),
        Disabled { .. } => (PRIMARY_DISABLED, FAINT),
    };
    toggler::Style {
        background: track.resolve(theme).into(),
        foreground: knob.resolve(theme).into(),
        text_color: Some(
            if matches!(status, Disabled { .. }) {
                FAINT
            } else {
                TEXT
            }
            .resolve(theme),
        ),
        ..toggler::default(theme.native(), status)
    }
}

fn checkbox_style(theme: &Theme, status: iced_checkbox::Status) -> iced_checkbox::Style {
    use iced_checkbox::Status::*;
    let (background, outline, icon) = match status {
        Active { is_checked: true } => (PRIMARY, PRIMARY, PRIMARY_TEXT),
        Hovered { is_checked: true } => (PRIMARY_HOVER, PRIMARY_HOVER, PRIMARY_TEXT),
        Active { is_checked: false } => (ELEVATED, INPUT_BORDER, TEXT),
        Hovered { is_checked: false } => (ELEVATED, MUTED, TEXT),
        Disabled { .. } => (PRIMARY_DISABLED, LINE, FAINT),
    };
    iced_checkbox::Style {
        background: background.resolve(theme).into(),
        icon_color: icon.resolve(theme),
        border: Border {
            color: outline.resolve(theme),
            width: 1.,
            radius: theme.control_radius().into(),
        },
        text_color: Some(
            if matches!(status, Disabled { .. }) {
                FAINT
            } else {
                TEXT
            }
            .resolve(theme),
        ),
    }
}

pub(crate) fn font_slider(theme: &Theme, status: slider::Status) -> slider::Style {
    let mut style = slider::default(theme.native(), status);
    style.rail.backgrounds = (TEXT.resolve(theme).into(), LINE.resolve(theme).into());
    style.handle.background = match status {
        slider::Status::Active => TEXT,
        slider::Status::Hovered => MUTED,
        slider::Status::Dragged => BLUE,
    }
    .resolve(theme)
    .into();
    style
}

fn select(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    pick_list::Style {
        text_color: TEXT.resolve(theme),
        placeholder_color: FAINT.resolve(theme),
        handle_color: MUTED.resolve(theme),
        background: ELEVATED.resolve(theme).into(),
        border: Border {
            color: match status {
                pick_list::Status::Active => INPUT_BORDER,
                pick_list::Status::Hovered => MUTED,
                pick_list::Status::Opened { .. } => BLUE,
            }
            .resolve(theme),
            width: 1.,
            radius: theme.control_radius().into(),
        },
    }
}

fn select_menu(theme: &Theme) -> menu::Style {
    menu::Style {
        background: ELEVATED.resolve(theme).into(),
        border: Border {
            color: LINE.resolve(theme),
            width: 1.,
            radius: theme.control_radius().into(),
        },
        text_color: TEXT.resolve(theme),
        selected_text_color: TEXT.resolve(theme),
        selected_background: LINE.resolve(theme).into(),
        shadow: Default::default(),
    }
}

pub(crate) fn surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(BACKGROUND.resolve(theme).into()),
        text_color: Some(TEXT.resolve(theme)),
        ..Default::default()
    }
}

pub(crate) fn code_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(code_background(theme).into()),
        ..surface(theme)
    }
}

pub(crate) fn code_background(theme: &Theme) -> Color {
    CODE_BACKGROUND.resolve(theme)
}

pub(crate) fn sidebar(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(SIDEBAR.resolve(theme).into()),
        text_color: Some(TEXT.resolve(theme)),
        ..Default::default()
    }
}

pub(crate) fn panel(theme: &Theme) -> container::Style {
    surface(theme)
}

pub(crate) fn file_tab(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| button::Style {
        background: Some(
            match status {
                button::Status::Hovered | button::Status::Pressed => ELEVATED,
                _ if active => BACKGROUND,
                _ => SIDEBAR,
            }
            .resolve(theme)
            .into(),
        ),
        text_color: if active { TEXT } else { MUTED }.resolve(theme),
        ..Default::default()
    }
}

pub(crate) fn tab_indicator(active: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| container::Style {
        background: Some(
            if active {
                BLUE.resolve(theme)
            } else {
                LINE.resolve(theme)
            }
            .into(),
        ),
        ..Default::default()
    }
}

pub(crate) fn separator(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(LINE.resolve(theme).into()),
        ..Default::default()
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn menu_title(open: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| container::Style {
        background: open.then_some(SELECTED.resolve(theme).into()),
        text_color: Some(TEXT.resolve(theme)),
        border: Border {
            radius: theme.control_radius().into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

#[cfg(target_os = "windows")]
pub(crate) fn window_frame(theme: &Theme) -> container::Style {
    container::Style {
        border: Border {
            color: LINE.resolve(theme),
            width: 1.,
            ..Default::default()
        },
        ..surface(theme)
    }
}

#[cfg(not(target_os = "windows"))]
pub(crate) use surface as secondary_window;
#[cfg(target_os = "windows")]
pub(crate) use window_frame as secondary_window;

pub(crate) fn notification_progress(
    theme: &Theme,
    tone: Tone,
) -> iced::widget::progress_bar::Style {
    iced::widget::progress_bar::Style {
        background: ELEVATED.resolve(theme).into(),
        bar: tone.resolve(theme).into(),
        border: Border {
            radius: 2.0.into(),
            ..Default::default()
        },
    }
}

pub(crate) fn source_progress(theme: &Theme) -> iced::widget::progress_bar::Style {
    iced::widget::progress_bar::Style {
        background: Color::TRANSPARENT.into(),
        bar: BLUE.resolve(theme).into(),
        border: Border::default(),
    }
}

pub(crate) fn panes(theme: &Theme) -> pane_grid::Style {
    pane_grid::Style {
        hovered_region: pane_grid::Highlight {
            background: TEXT.resolve(theme).scale_alpha(0.06).into(),
            border: Border {
                color: FAINT.resolve(theme),
                width: 1.0,
                ..Default::default()
            },
        },
        hovered_split: pane_grid::Line {
            color: FAINT.resolve(theme),
            width: 1.0,
        },
        picked_split: pane_grid::Line {
            color: MUTED.resolve(theme),
            width: 2.0,
        },
    }
}

pub(crate) fn tooltip(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(ELEVATED.resolve(theme).into()),
        text_color: Some(TEXT.resolve(theme)),
        border: Border {
            color: LINE.resolve(theme),
            width: 1.0,
            radius: theme.control_radius().into(),
        },
        ..Default::default()
    }
}

pub(crate) fn context_menu(theme: &Theme) -> container::Style {
    notification(theme)
}

pub(crate) fn context_submenu(theme: &Theme) -> container::Style {
    container::Style {
        shadow: iced::Shadow {
            color: Color::BLACK.scale_alpha(0.32),
            offset: iced::Vector::new(0., 4.),
            blur_radius: 16.,
        },
        ..tooltip(theme)
    }
}

pub(crate) fn notification(theme: &Theme) -> container::Style {
    container::Style {
        shadow: iced::Shadow {
            color: Color::BLACK.scale_alpha(0.18),
            offset: iced::Vector::new(0., 3.),
            blur_radius: 12.,
        },
        ..tooltip(theme)
    }
}

pub(crate) fn badge(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(ELEVATED.resolve(theme).into()),
        border: Border {
            radius: theme.control_radius().into(),
            ..Default::default()
        },
        text_color: Some(MUTED.resolve(theme)),
        ..Default::default()
    }
}

pub(crate) fn branch_badge(theme: &Theme, status: button::Status) -> button::Style {
    let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: Some(
            if hovered { SELECTED } else { ELEVATED }
                .resolve(theme)
                .into(),
        ),
        text_color: if hovered { TEXT } else { MUTED }.resolve(theme),
        border: Border {
            radius: theme.control_radius().into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(crate) fn branch_attention(active: bool) -> impl Fn(&Theme) -> container::Style {
    move |theme| container::Style {
        border: Border {
            width: if active { 2.0 } else { 0.0 },
            color: BLUE.resolve(theme),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(crate) fn backdrop(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Color::BLACK.scale_alpha(theme.backdrop_opacity()).into()),
        ..Default::default()
    }
}

pub(crate) fn dialog(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(DIALOG.resolve(theme).into()),
        border: Border {
            color: LINE.resolve(theme),
            width: 1.0,
            radius: theme.dialog_radius().into(),
        },
        shadow: iced::Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.35),
            offset: iced::Vector::new(0.0, 12.0),
            blur_radius: 30.0,
        },
        ..surface(theme)
    }
}

pub(crate) fn action(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: (selected || hovered).then_some(
                (if selected { SELECTED } else { HOVER })
                    .resolve(theme)
                    .into(),
            ),
            text_color: if matches!(status, button::Status::Disabled) {
                FAINT.resolve(theme)
            } else {
                TEXT.resolve(theme)
            },
            border: Border {
                radius: theme.control_radius().into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

pub(crate) fn completion_row(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    selectable_row(selected, COMPLETION_SELECTED)
}

pub(crate) fn fold_row(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = action(false)(theme, status);
    style.background.get_or_insert(FOLD.resolve(theme).into());
    style.text_color = MUTED.resolve(theme);
    style
}

pub(crate) use action as icon_action;

#[cfg(target_os = "windows")]
pub(crate) fn window_control(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = action(false)(theme, status);
    style.border.radius = 0.0.into();
    style
}

pub(crate) fn tree_item(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    selectable_row(selected, TREE_SELECTED)
}

fn selectable_row(selected: bool, fill: Tone) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let mut style = action(selected)(theme, status);
        if selected {
            style.background = Some(fill.resolve(theme).into());
        }
        style
    }
}

pub(crate) fn primary(theme: &Theme, status: button::Status) -> button::Style {
    filled_button(theme, status, PRIMARY, PRIMARY_HOVER)
}

pub(crate) fn danger(theme: &Theme, status: button::Status) -> button::Style {
    filled_button(theme, status, RED, DANGER_HOVER)
}

fn filled_button(theme: &Theme, status: button::Status, fill: Tone, hover: Tone) -> button::Style {
    let (background, text) = match status {
        button::Status::Active | button::Status::Pressed => (fill, PRIMARY_TEXT),
        button::Status::Hovered => (hover, PRIMARY_TEXT),
        button::Status::Disabled => (PRIMARY_DISABLED, MUTED),
    };
    button::Style {
        background: Some(background.resolve(theme).into()),
        text_color: text.resolve(theme),
        border: Border {
            radius: theme.control_radius().into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

pub(crate) fn secondary(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = action(false)(theme, status);
    style.border = Border {
        color: match status {
            button::Status::Active => OUTLINE,
            button::Status::Hovered | button::Status::Pressed => MUTED,
            button::Status::Disabled => LINE,
        }
        .resolve(theme),
        width: 1.0,
        radius: theme.control_radius().into(),
    };
    style
}

pub(crate) fn code_scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut style = scroll(theme, status);
    style.vertical_rail.background = Some(SIDEBAR.resolve(theme).into());
    style
}

pub(crate) fn scrollbar_thumb(theme: &Theme, highlighted: bool) -> Color {
    if highlighted { THUMB_HOVER } else { THUMB }.resolve(theme)
}

pub(crate) fn scroll(theme: &Theme, status: scrollable::Status) -> scrollable::Style {
    let mut style = scrollable::default(theme.native(), status);
    let (horizontal_disabled, vertical_disabled) = match status {
        scrollable::Status::Active {
            is_horizontal_scrollbar_disabled,
            is_vertical_scrollbar_disabled,
        }
        | scrollable::Status::Hovered {
            is_horizontal_scrollbar_disabled,
            is_vertical_scrollbar_disabled,
            ..
        }
        | scrollable::Status::Dragged {
            is_horizontal_scrollbar_disabled,
            is_vertical_scrollbar_disabled,
            ..
        } => (
            is_horizontal_scrollbar_disabled,
            is_vertical_scrollbar_disabled,
        ),
    };
    let thumb = scrollbar_thumb(theme, !matches!(status, scrollable::Status::Active { .. }));
    for (rail, disabled) in [
        (&mut style.horizontal_rail, horizontal_disabled),
        (&mut style.vertical_rail, vertical_disabled),
    ] {
        rail.background = None;
        rail.border = Border::default();
        rail.scroller.background = if disabled { Color::TRANSPARENT } else { thumb }.into();
        rail.scroller.border = Border {
            radius: theme.control_radius().into(),
            ..Default::default()
        };
    }
    style.gap = None;
    style.auto_scroll.background = ELEVATED.resolve(theme).into();
    style.auto_scroll.border = Border {
        color: LINE.resolve(theme),
        width: 1.0,
        radius: 999.0.into(),
    };
    style.auto_scroll.icon = TEXT.resolve(theme);
    style
}

pub(crate) fn application_style(theme: &Theme) -> theme::Style {
    theme::Style {
        background_color: BACKGROUND.resolve(theme),
        text_color: TEXT.resolve(theme),
    }
}

#[cfg(test)]
mod tests;

pub(crate) fn success_tooltip(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(GREEN.resolve(theme).into()),
        text_color: Some(theme.color(crate::domain::theme::ColorRole::TooltipSuccessText)),
        border: Border {
            color: GREEN.resolve(theme),
            ..tooltip(theme).border
        },
        ..tooltip(theme)
    }
}

/// Source search uses these panels for its toolbar, results, and preview.
pub(crate) fn search_panel(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(ELEVATED.resolve(theme).into()),
        border: Border {
            color: LINE.resolve(theme),
            width: 1.,
            radius: theme.control_radius().into(),
        },
        ..Default::default()
    }
}
pub(crate) fn search_result(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    selectable_row(selected, CODE_SELECTION)
}

pub(crate) fn search_preview(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(code_background(theme).into()),
        ..search_panel(theme)
    }
}
pub(crate) const COMPLETION_ROW_HEIGHT: f32 = 32.;
pub(crate) const COMPLETION_WIDTH: f32 = 560.;
pub(crate) const COMPLETION_APPEAR: std::time::Duration = std::time::Duration::from_millis(120);
