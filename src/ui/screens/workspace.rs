use super::{auth, diff, git, git_actions, pipelines, settings, unified};
use crate::ui::tooltip;
use crate::{
    app::{App, Message, Navigation, Panel, error_key, spaces},
    domain::modules::Placement,
    domain::workspace::{DiffView, MarkdownViewMode, Preferences},
    i18n::{FluentArgs, Key},
    modules::ModuleId,
    ui::{
        changed_tree, code, context_menu, external_module_panel, file_icons,
        icons::{Icon, icon, sized_icon},
        presentation::{IconButton, center_message, divider, icon_button, short, vertical_divider},
        reorder, resize, tab_label, theme,
    },
};
use iced::{
    Alignment, Element, Length,
    widget::{
        self, Column, Row, button, column, container, mouse_area, pane_grid, responsive, row,
        scrollable, space, text,
    },
};
use std::path::Path;

/// Height of the top and bottom external module zones.
const EXTERNAL_ZONE_HEIGHT: f32 = 220.;
/// Width of the left and right external module zones.
const EXTERNAL_ZONE_WIDTH: f32 = 320.;

pub(crate) fn view(app: &App) -> Element<'_, Message> {
    let sidebar_width =
        app.workspace.preferences.spaces_sidebar_width * app.spaces_sidebar_progress();
    let content: Element<'_, Message> = if app.navigation == Navigation::Settings {
        settings::view(app)
    } else {
        let panes = pane_grid(&app.panes, |_, panel, _| {
            let content: Element<'_, Message> = match panel {
                Panel::Workspace => {
                    responsive(move |size| deployment_content(app, size.width)).into()
                }
                Panel::Module(id) => {
                    let content = match id {
                        ModuleId::Git => git::view(app),
                        ModuleId::Pipelines => pipelines::view(app),
                        _ => crate::ui::modules::panel(app, *id),
                    };
                    row![
                        vertical_toolbar(panel_buttons(app), "bottom-toolbar-divider"),
                        container(content).width(Length::Fill).height(Length::Fill),
                    ]
                    .into()
                }
            };
            pane_grid::Content::new(
                container(content)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .style(theme::surface),
            )
        })
        .spacing(1)
        .style(theme::panes)
        .on_resize(6, Message::Resize);
        let panes = if app.module_open(ModuleId::Git) || app.module_open(ModuleId::Pipelines) {
            panes.on_click(Message::PaneClicked)
        } else {
            panes
        };
        let mut body = column![
            container(panes)
                .style(theme::separator)
                .height(Length::Fill)
        ];
        if !app.external_zone_modules(Placement::Bottom).is_empty() {
            body = body.push(divider()).push(
                container(external_module_panel::zone_panel(app, Placement::Bottom))
                    .height(EXTERNAL_ZONE_HEIGHT)
                    .width(Length::Fill)
                    .clip(true),
            );
        }
        body.into()
    };
    let main = column![content].height(Length::Fill).width(Length::Fill);
    let mut workspace = Column::new();
    if !app.external_zone_modules(Placement::Top).is_empty() {
        workspace = workspace
            .push(
                container(external_module_panel::zone_panel(app, Placement::Top))
                    .height(EXTERNAL_ZONE_HEIGHT)
                    .width(Length::Fill)
                    .clip(true),
            )
            .push(divider());
    }
    workspace = workspace.push(main);
    let mut center = row![workspace.width(Length::Fill)];
    let has_left_zone = !app.external_zone_modules(Placement::Left).is_empty();
    if has_left_zone {
        center = row![
            container(external_module_panel::zone_panel(app, Placement::Left))
                .width(EXTERNAL_ZONE_WIDTH)
                .height(Length::Fill)
                .clip(true),
            vertical_divider(),
            center,
        ]
        .width(Length::Fill);
    }
    let spaces: Element<'_, Message> = if sidebar_width > 0. {
        row![
            // Keep the sidebar's layout at its saved width while its visible edge moves.
            scrollable(
                container(sidebar(app)).width(app.workspace.preferences.spaces_sidebar_width)
            )
            .id("spaces-slide-viewport")
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::hidden()
            ))
            .anchor_right()
            .wheel_scroll_scale(0.)
            .width(sidebar_width)
            .height(Length::Fill),
            vertical_divider(),
        ]
        .into()
    } else {
        space().width(0).into()
    };
    let body = resize::horizontal(
        row![spaces, center.height(Length::Fill)]
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
        app.can_resize_spaces_sidebar().then_some(sidebar_width),
        sidebar_width,
        crate::domain::workspace::Preferences::MIN_SPACES_SIDEBAR_WIDTH
            ..=crate::domain::workspace::Preferences::MAX_SPACES_SIDEBAR_WIDTH,
        Message::ResizeSpacesSidebar,
    );
    let sidebar_separator = if sidebar_width > 0. { 1. } else { 0. };
    let left_zone_span = if has_left_zone {
        EXTERNAL_ZONE_WIDTH + 1.
    } else {
        0.
    };
    let header = container(row![
        header_controls(app, sidebar_width + sidebar_separator + left_zone_span),
        header(app),
    ])
    .style(theme::surface);
    let mut layout = column![header, divider(), body];
    if app.navigation != Navigation::Settings && app.workspace.directory().is_some() {
        layout = layout.push(divider()).push(footer(app));
    }
    let base = container(layout)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::surface)
        .into();
    let base = crate::modules::deployment::overlay(
        &app.modules.deployment,
        &app.i18n,
        base,
        app.deployment_profiles(),
        app.workspace.directory().map(|path| path.as_path()),
        crate::app::modules::deployment_message,
    );
    let base = crate::ui::activity::overlay(
        base,
        app.modules.deployment.activity_expanded.then(|| {
            crate::modules::deployment::activity_details(&app.modules.deployment, &app.i18n)
                .map(crate::app::modules::deployment_message)
        }),
    );
    let base = crate::modules::docker::overlay(
        &app.modules.docker,
        &app.i18n,
        app.module_enabled(ModuleId::Terminal),
        base,
        crate::app::modules::docker_message,
    );
    let base = super::header_repositories::overlay(app, base);
    let base = super::tab_menu::overlay(app, base);
    let base =
        crate::modules::terminal::tab_menu(&app.modules.terminal, &app.i18n, base, |event| {
            Message::Module(crate::app::modules::Event::Terminal(event))
        });
    let base = if let Some(point) = app.files.behavior_menu {
        use crate::app::files::Event;
        context_menu::overlay(
            base,
            point,
            iced::Size::new(230., 80.),
            || {
                column![
                    container(
                        text(app.i18n.text(Key::TreeBehavior))
                            .size(12)
                            .style(theme::MUTED.text())
                    )
                    .padding([4, 8]),
                    context_menu::row(
                        row![
                            text(app.i18n.text(Key::PreviewTab)).size(13),
                            space().width(Length::Fill)
                        ]
                        .extend(
                            app.workspace
                                .preferences
                                .preview_tab
                                .then(|| icon(Icon::Check, theme::BLUE))
                        )
                    )
                    .on_press(Message::Files(Event::TogglePreview)),
                ]
                .into()
            },
            Message::Files(Event::BehaviorMenu(None)),
        )
    } else {
        base
    };
    let base = crate::ui::branch_selection::watch(
        base,
        !app.modules.git.branch_selection.selected.is_empty(),
        |message| {
            matches!(
                message,
                Message::Module(crate::app::modules::Event::Git(
                    crate::modules::git::Event::BranchSelection(_)
                        | crate::modules::git::Event::Git(
                            crate::modules::git::actions::Message::Menu(..)
                        )
                )) | Message::BranchSelection(
                    crate::modules::git::selection::Event::Toggle(_)
                        | crate::modules::git::selection::Event::Menu(..)
                ) | Message::Git(crate::modules::git::actions::Message::Menu(..))
            )
        },
        || Message::BranchSelection(crate::modules::git::selection::Event::Clear),
    );
    let base = super::space_form::overlay(
        app,
        auth::overlay(
            app,
            git_actions::overlay(
                app,
                project_menu(
                    app,
                    super::file_menu::overlay(app, pipelines::overlay(app, base)),
                ),
            ),
        ),
    );
    let base = super::project_add::overlay(app, base);
    let base = crate::ui::explain::overlay(app, base);
    let base = super::search_everywhere::overlay(app, base);
    let base = super::text_search::overlay(app, base);
    let base = super::line_navigation::overlay(app, base);
    let base = crate::ui::notifications::overlay(
        &app.notifications,
        &app.i18n,
        base,
        Message::Notification,
    );
    #[cfg(target_os = "windows")]
    let base = {
        let base = super::menu_bar::overlay(app, base);
        crate::ui::window_chrome::overlay(app, base)
    };
    let base = if app.navigation == Navigation::Settings {
        settings::shortcut_recording_overlay(app, base)
    } else {
        base
    };
    let base = crate::ui::editor::overlay(app, super::file_menu::dialog(app, base));
    crate::ui::updates::overlay(app, base)
}

fn header(app: &App) -> Element<'_, Message> {
    let is_settings = app.navigation == Navigation::Settings;
    let selected_space = app
        .workspace
        .selected_space
        .and_then(|index| app.workspace.spaces.get(index));
    let title = if is_settings {
        app.i18n.text(Key::Settings)
    } else if let Some(space) = selected_space {
        app.workspace
            .project()
            .map(|project| format!("{} • {}", short(&space.name, 24), short(&project.name, 28)))
            .unwrap_or_else(|| short(&space.name, 42))
    } else {
        app.i18n.text(Key::Workspace)
    };
    let mut title_row = Row::new().spacing(10).align_y(Alignment::Center);
    if !is_settings {
        title_row = title_row
            .push(container(icon(Icon::Folder, theme::MUTED)).id("workspace-heading-folder"));
    }
    let title = title_row.push(
        container(
            text(title)
                .size(18)
                .wrapping(text::Wrapping::None)
                .font(theme::semibold()),
        )
        .id("workspace-heading")
        .clip(true)
        .width(Length::Fill),
    );
    #[cfg(target_os = "windows")]
    let title = container(
        row![super::menu_bar::header_prefix(app), title]
            .spacing(12)
            .align_y(Alignment::Center),
    )
    .clip(true)
    .width(Length::Fill);
    let mut heading = row![title].spacing(12).align_y(Alignment::Center);
    if !is_settings && app.repository.is_some() {
        heading = heading.push(super::header_repositories::controls(app));
    }
    if !is_settings
        && !app
            .external_zone_modules(crate::domain::modules::Placement::Header)
            .is_empty()
    {
        heading = heading.push(external_module_panel::header_panel(app));
    }
    let action = if is_settings {
        icon_button(
            &app.i18n,
            Icon::Close,
            Key::BackToWorkspace,
            Message::Navigate(Navigation::Projects),
            false,
        )
    } else {
        icon_button(
            &app.i18n,
            Icon::Refresh,
            Key::RefreshGitRepositories,
            Message::RefreshGitRepositories,
            false,
        )
    };
    heading = heading.push(container(action).id("window-header-action"));
    if !is_settings {
        heading = heading.push(
            container(icon_button(
                &app.i18n,
                Icon::Settings,
                Key::Settings,
                Message::Navigate(Navigation::Settings),
                false,
            ))
            .id("nav-settings"),
        );
        if let Some(update) = crate::ui::updates::header_button(app) {
            heading = heading.push(update);
        }
    }
    #[cfg(target_os = "windows")]
    let right_inset = theme::WINDOW_CAPTION_INSET + theme::WINDOW_CAPTION_WIDTH + 12.;
    #[cfg(not(target_os = "windows"))]
    let right_inset = if !is_settings && app.module_enabled(ModuleId::Deployment) {
        theme::FILE_PANEL_TOOL_INSET
    } else {
        24.
    };
    mouse_area(
        container(heading)
            .center_y(theme::WORKSPACE_HEADER_HEIGHT)
            .width(Length::Fill)
            .padding(iced::Padding {
                left: 16.,
                right: right_inset,
                ..iced::Padding::ZERO
            })
            .style(theme::surface),
    )
    .on_press(Message::DragWindow)
    .on_double_click(Message::ToggleWindowMaximized)
    .into()
}

fn header_controls(app: &App, panel_span: f32) -> Element<'_, Message> {
    let show_spaces = app.workspace.preferences.show_spaces_sidebar;
    let inset = if cfg!(target_os = "macos") {
        theme::MACOS_WINDOW_CONTROLS_WIDTH * 100. / f32::from(app.workspace.preferences.window_zoom)
    } else {
        theme::FILE_PANEL_TOOL_INSET
    };
    let minimum = inset + theme::WORKSPACE_HEADER_BUTTON_SIZE + 8.;
    let controls = row![
        container(icon_button(
            &app.i18n,
            Icon::Sidebar,
            if show_spaces {
                Key::HideSpacesSidebar
            } else {
                Key::ShowSpacesSidebar
            },
            Message::ToggleSpacesSidebar,
            show_spaces,
        ))
        .id("toggle-spaces-sidebar"),
        space().width(Length::Fill),
        vertical_divider()
            .id("workspace-header-separator")
            .height(22),
    ]
    .align_y(Alignment::Center);
    mouse_area(
        container(controls)
            .id("window-header-drag-area")
            .width(panel_span.max(minimum))
            .center_y(theme::WORKSPACE_HEADER_HEIGHT)
            .padding(iced::Padding::ZERO.left(inset)),
    )
    .on_press(Message::DragWindow)
    .on_double_click(Message::ToggleWindowMaximized)
    .into()
}

fn panel_buttons(app: &App) -> Element<'_, Message> {
    let buttons = [
        (
            ModuleId::Terminal,
            Icon::Terminal,
            Key::Terminal,
            "nav-terminal",
        ),
        (
            ModuleId::Kubernetes,
            Icon::Kubernetes,
            Key::Kubernetes,
            "nav-kubernetes",
        ),
        (ModuleId::Docker, Icon::Docker, Key::Docker, "nav-docker"),
        (ModuleId::Git, Icon::Branch, Key::Git, "nav-git"),
        (
            ModuleId::Pipelines,
            Icon::Play,
            Key::Pipelines,
            "nav-pipelines",
        ),
    ]
    .into_iter()
    .filter(|(id, ..)| id.listed() && app.panel_available(*id))
    .map(move |(id, symbol, label, widget_id)| {
        let active = app.module_open(id);
        tooltip::view(
            container(IconButton::Inline(theme::FILE_PANEL_TOOL_SIZE).button(
                symbol,
                Some(Message::Module(crate::app::modules::Event::Toggle(id))),
                active,
            ))
            .id(widget_id),
            text(app.i18n.text(label)).size(12),
            tooltip::Position::Right,
        )
        .into()
    });
    container(
        scrollable(column(buttons).spacing(theme::FILE_PANEL_TOOL_INSET))
            .anchor_bottom()
            .style(theme::scroll),
    )
    .align_bottom(Length::Fill)
    .into()
}

fn footer(app: &App) -> Element<'_, Message> {
    let path = app.footer_path().map(|path| path.to_string_lossy());
    let label = path
        .as_deref()
        .map(|value| short_path(value, 100))
        .unwrap_or_else(|| app.i18n.text(Key::LocalWorkspace));
    let control = container(
        button(
            text(label)
                .size(12)
                .style(theme::FAINT.text())
                .wrapping(text::Wrapping::None),
        )
        .on_press_maybe(path.as_ref().map(|_| Message::CopyPath))
        .padding([4, 8])
        .style(theme::action(false)),
    )
    .id("footer-path")
    .clip(true);
    let path: Element<'_, Message> = match path {
        Some(value) => tooltip::copy(
            control,
            &app.i18n,
            Key::CopyPath,
            value,
            app.footer_path_was_copied(),
            tooltip::Position::Top,
        )
        .into(),
        None => control.into(),
    };
    let mut content = row![container(path).width(Length::Fill)].spacing(16);
    if let Some(activity) = git::activity(app) {
        content = content.push(activity);
    }
    if app.modules.deployment.activity_visible {
        content = content.push(
            crate::modules::deployment::activity(&app.modules.deployment, &app.i18n)
                .map(crate::app::modules::deployment_message),
        );
    }
    container(content.align_y(Alignment::Center))
        .padding([6, 16])
        .into()
}

fn selection_count(app: &App, count: usize) -> String {
    let mut args = FluentArgs::new();
    args.set("count", count as i64);
    app.i18n.format(Key::ProjectsSelected, &args)
}

fn sidebar(app: &App) -> Element<'_, Message> {
    let navigation: Element<'_, Message> = {
        container(
            row![
                container(
                    text(
                        app.project_selection
                            .as_ref()
                            .map(|set| selection_count(app, set.len()))
                            .unwrap_or_else(|| app.i18n.text(Key::Spaces))
                    )
                    .size(14)
                    .font(theme::semibold())
                    .style(theme::MUTED.text())
                )
                .id("spaces-heading")
                .width(Length::Fill),
                tooltip::view(
                    container(
                        IconButton::Inline(28.).button(
                            Icon::Plus,
                            (app.loaded && !app.picking)
                                .then_some(Message::Space(crate::app::spaces::Message::New)),
                            false
                        )
                    )
                    .id("new-space"),
                    text(app.i18n.text(Key::NewSpace)).size(12),
                    tooltip::Position::Right,
                ),
            ]
            .align_y(Alignment::Center)
            .spacing(8),
        )
        .padding([4, 8])
        .into()
    };
    let mut body = Column::new().spacing(4).width(Length::Fill);
    let pinned: Vec<_> = app
        .workspace
        .spaces
        .iter()
        .enumerate()
        .flat_map(|(space, group)| {
            group
                .projects
                .iter()
                .enumerate()
                .filter_map(move |(project, item)| item.pinned.then_some((space, project)))
        })
        .collect();
    if !pinned.is_empty() {
        body = body.push(
            container(
                row![
                    icon(Icon::Pin, theme::FAINT),
                    text(app.i18n.text(Key::PinnedProjects))
                        .size(13)
                        .style(theme::FAINT.text())
                ]
                .spacing(8),
            )
            .padding([4, 9]),
        );
        for (space, project) in pinned {
            body = body.push(project_entry(app, space, project));
        }
        body = body.push(space().height(5));
    }
    {
        let mut groups = Column::new().spacing(3).width(Length::Fill);
        for (space_index, group) in app.workspace.spaces.iter().enumerate() {
            let mut group_body = Column::new().spacing(2).width(Length::Fill);
            group_body = group_body.push(context_menu::trigger(
                container(
                    row![
                        button(
                            row![
                                file_icons::chevron(!app.collapsed_spaces.contains(&space_index)),
                                text(short(&group.name, 22))
                                    .size(13)
                                    .font(theme::semibold())
                                    .wrapping(text::Wrapping::None)
                                    .width(Length::Fill),
                                text(
                                    group
                                        .projects
                                        .iter()
                                        .filter(|project| !project.pinned)
                                        .count()
                                )
                                .size(12)
                                .style(theme::FAINT.text()),
                            ]
                            .spacing(6)
                            .align_y(Alignment::Center)
                            .height(24)
                        )
                        .padding([0, 8])
                        .width(Length::Fill)
                        .on_press(Message::ToggleSpace(space_index))
                        .style(theme::action(false)),
                        tooltip::view(
                            container(IconButton::Inline(26.).button(
                                Icon::Settings,
                                (!app.picking).then_some(Message::Space(
                                    crate::app::spaces::Message::Edit(space_index)
                                )),
                                false
                            ))
                            .id(format!("edit-space-{space_index}")),
                            text(app.i18n.text(Key::EditSpace)).size(12),
                            tooltip::Position::Right
                        ),
                    ]
                    .spacing(4)
                    .align_y(Alignment::Center),
                )
                .id(format!("space-group-{space_index}"))
                .padding([1, 0])
                .width(Length::Fill),
                move |point| Message::SpaceMenu(space_index, point),
            ));
            if app.collapsed_spaces.contains(&space_index) {
                groups = groups.push(group_body);
                continue;
            }
            let mut projects = Column::new().spacing(2).width(Length::Fill);
            let visible: Vec<_> = group
                .projects
                .iter()
                .enumerate()
                .filter_map(|(index, project)| (!project.pinned).then_some(index))
                .collect();
            for &project_index in &visible {
                projects = projects.push(project_entry(app, space_index, project_index));
            }
            if !visible.is_empty() {
                let projects: Element<'_, Message> =
                    if app.picking || app.space_draft.is_some() || app.auth_dialog.is_some() {
                        projects.into()
                    } else {
                        reorder::column(
                            projects,
                            36.,
                            move |from, to| {
                                Message::Space(crate::app::spaces::Message::MoveProject(
                                    Some(space_index),
                                    visible[from],
                                    visible[to],
                                ))
                            },
                            move |offset| {
                                Message::Space(crate::app::spaces::Message::ScrollProjects(
                                    Some(space_index),
                                    offset,
                                ))
                            },
                        )
                    };
                group_body = group_body.push(projects);
            }
            if group.projects.is_empty() {
                group_body = group_body.push(
                    container(
                        text(app.i18n.text(Key::NoProjects))
                            .size(12)
                            .style(theme::FAINT.text()),
                    )
                    .padding([3, 9]),
                );
            }
            let selected = matches!(app.space_menu, Some((index, spaces::Menu::AddProject { parent: None, .. })) if index == space_index);
            let open =
                Message::ProjectAdd(crate::app::project_add::Message::Open(Some(space_index)));
            let control = button(
                row![
                    icon(Icon::Plus, theme::MUTED),
                    text(app.i18n.text(Key::AddProject))
                        .size(12)
                        .width(Length::Fill),
                    sized_icon(Icon::Right, theme::MUTED, 12.),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .on_press_maybe((!app.picking).then_some(open))
            .style(theme::action(selected))
            .padding([4, 9])
            .width(Length::Fill);
            let add = mouse_area(container(control).id(spaces::add_project_id(space_index, false)));
            let add = if !app.picking
                && app
                    .space_menu
                    .is_none_or(|(_, menu)| menu.parent().is_none())
                && app.project_menu.is_none()
                && app.files.menu.is_none()
            {
                add.on_enter(Message::OpenSpaceAddMenu(space_index))
            } else {
                add
            };
            group_body = group_body.push(add);
            groups = groups.push(group_body);
        }
        let groups: Element<'_, Message> = if app.picking
            || app.space_draft.is_some()
            || app.project_add.is_some()
            || app.auth_dialog.is_some()
            || app.modules.git.git_actions.has_dialog()
        {
            groups.into()
        } else {
            reorder::groups(
                groups,
                28.,
                |from, to| Message::Space(crate::app::spaces::Message::MoveSpace(from, to)),
                |offset| Message::Space(crate::app::spaces::Message::ScrollSpaces(offset)),
            )
        };
        body = body.push(groups);
        if app.workspace.spaces.is_empty() {
            body = body.push(
                container(
                    text(app.i18n.text(Key::NoSpaces))
                        .size(13)
                        .style(theme::MUTED.text()),
                )
                .id("spaces-empty-message")
                .padding([12, 8]),
            );
        }
    }
    let sidebar = column![
        navigation,
        space().height(8),
        scrollable(body)
            .id("project-browser")
            .spacing(theme::SIDEBAR_SCROLL_GAP)
            .style(theme::scroll)
            .height(Length::Fill),
    ];
    container(sidebar)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(iced::Padding {
            top: 8.,
            right: 12.,
            bottom: 12.,
            left: 12.,
        })
        .style(theme::sidebar)
        .into()
}

fn deployment_content(app: &App, width: f32) -> Element<'_, Message> {
    use crate::{app::modules::deployment_message, modules::deployment::Event};
    if !app.module_enabled(crate::modules::ModuleId::Deployment) {
        return project_content(app, width);
    }
    let toolbar_width = theme::FILE_PANEL_TOOL_SIZE + 2. * theme::FILE_PANEL_TOOL_INSET;
    let maximum = (width - toolbar_width - 400.).clamp(220., 640.);
    let panel_width = app
        .workspace
        .preferences
        .deployment_panel_width
        .clamp(220., maximum);
    let visible = app.modules.deployment.open;
    let reserved = toolbar_width + 1. + if visible { panel_width } else { 0. };
    let tools = container(tooltip::view(
        container(IconButton::Inline(theme::FILE_PANEL_TOOL_SIZE).button(
            Icon::Server,
            Some(deployment_message(Event::Toggle)),
            visible,
        ))
        .id("deployment-toggle"),
        text(app.i18n.text(Key::Deployment)).size(12),
        tooltip::Position::Left,
    ))
    .padding(theme::FILE_PANEL_TOOL_INSET)
    .height(Length::Fill)
    .style(theme::sidebar);
    let mut children: Vec<Element<'_, Message>> = vec![
        container(project_content(app, (width - reserved).max(0.)))
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
        vertical_divider().into(),
        container(
            crate::modules::deployment::panel(
                &app.modules.deployment,
                &app.workspace.deployment,
                app.deployment_profiles(),
                app.workspace.directory().map(std::path::PathBuf::as_path),
                &app.i18n,
            )
            .map(deployment_message),
        )
        .id("deployment-panel")
        .width(if visible { panel_width } else { 0. })
        .height(Length::Fill)
        .clip(true)
        .into(),
    ];
    if !app.external_zone_modules(Placement::Right).is_empty() {
        children.push(vertical_divider().into());
        children.push(
            container(external_module_panel::zone_panel(app, Placement::Right))
                .width(EXTERNAL_ZONE_WIDTH)
                .height(Length::Fill)
                .clip(true)
                .into(),
        );
    }
    children.push(tools.into());
    let content = Row::with_children(children)
        .width(Length::Fill)
        .height(Length::Fill);
    resize::right(
        content.into(),
        visible.then_some(panel_width),
        width - reserved,
        220. ..=maximum,
        |width, finished| deployment_message(Event::Resize(width, finished)),
    )
}

fn project_content(app: &App, width: f32) -> Element<'_, Message> {
    use crate::domain::workspace::FilePanel;
    if app.workspace.directory().is_none() {
        return preview(app);
    }
    let toolbar_width = theme::FILE_PANEL_TOOL_SIZE + 2. * theme::FILE_PANEL_TOOL_INSET;
    let maximum = (width - toolbar_width - 1. - theme::SOURCE_MIN_WIDTH)
        .clamp(1., Preferences::MAX_FILE_PANEL_WIDTH);
    let minimum = Preferences::MIN_FILE_PANEL_WIDTH.min(maximum);
    let panel_width = app
        .workspace
        .preferences
        .file_panel_width
        .clamp(minimum, maximum);
    let mut tools = Column::new()
        .spacing(theme::FILE_PANEL_TOOL_INSET)
        .width(theme::FILE_PANEL_TOOL_SIZE);
    for panel in &app.workspace.preferences.file_panels {
        if *panel == crate::domain::workspace::FilePanel::Changes
            && !app.module_enabled(crate::modules::ModuleId::Git)
        {
            continue;
        }
        let (symbol, label, id) = match panel {
            FilePanel::Project => (Icon::Folder, Key::ProjectFiles, "file-panel-project"),
            FilePanel::Changes => (Icon::Changes, Key::WorkingChanges, "file-panel-changes"),
        };
        tools = tools.push(tooltip::view(
            container(IconButton::Inline(theme::FILE_PANEL_TOOL_SIZE).button(
                symbol,
                Some(Message::ToggleFilePanel(*panel)),
                app.file_panel_visible && app.file_panel == *panel,
            ))
            .id(id),
            text(
                app.i18n
                    .text(if app.file_panel_visible && app.file_panel == *panel {
                        Key::HidePanel
                    } else {
                        label
                    }),
            )
            .size(12),
            tooltip::Position::Right,
        ));
    }
    let tools = reorder::fixed_column(tools, Message::MoveFilePanel);
    let tools = if app.panes.len() == 1 {
        column![tools, panel_buttons(app)].into()
    } else {
        tools
    };
    let tools = vertical_toolbar(tools, "file-toolbar-divider");
    let content: Element<'_, Message> = match app.file_panel {
        FilePanel::Project => {
            let root = app.workspace.directory().unwrap();
            let width = app
                .tree
                .get(root)
                .into_iter()
                .flatten()
                .map(|entry| tree_width(app, &entry.path, 0))
                .fold(panel_width - 2. * theme::FILE_PANEL_CONTENT_INSET, f32::max);
            file_tree(app, root, 0, Column::new()).width(width).into()
        }
        FilePanel::Changes => {
            super::working_changes::view(app, panel_width - 2. * theme::FILE_PANEL_CONTENT_INSET)
        }
    };
    let title = app.i18n.text(match app.file_panel {
        FilePanel::Project => Key::ProjectFiles,
        FilePanel::Changes => Key::WorkingChanges,
    });
    use crate::app::files::Event as Files;
    let mut heading = row![
        container(
            text(title)
                .size(13)
                .font(theme::semibold())
                .wrapping(text::Wrapping::None)
        )
        .id("file-panel-title")
        .width(Length::Fill)
    ]
    .align_y(Alignment::Center);
    if app.file_panel == FilePanel::Project {
        let select = IconButton::Inline(26.).button(
            Icon::Locate,
            app.tree_target()
                .map(|_| Message::Files(Files::RevealOpened)),
            false,
        );
        let more = context_menu::anchored(
            container(icon(Icon::More, theme::MUTED)).padding(3),
            |point| Message::Files(Files::BehaviorMenu(Some(point))),
        );
        heading = heading
            .push(tooltip::view(
                select,
                text(app.i18n.text(Key::SelectOpenedFile)).size(12),
                tooltip::Position::Bottom,
            ))
            .push(tooltip::view(
                more,
                text(app.i18n.text(Key::TreeBehavior)).size(12),
                tooltip::Position::Bottom,
            ));
    }
    let mut files = column![
        container(heading)
            .id("file-panel-header")
            .height(theme::FILE_PANEL_HEADER_HEIGHT)
            .width(Length::Fill)
            .padding([0., theme::FILE_PANEL_CONTENT_INSET])
            .clip(true)
            .align_y(Alignment::Center),
        divider(),
    ];
    if app.file_panel == FilePanel::Changes {
        files = files
            .push(
                container(row![icon_button(
                    &app.i18n,
                    Icon::Refresh,
                    Key::RefreshWorkingChanges,
                    Message::Worktree(crate::app::changes::Event::RefreshManually),
                    false,
                )])
                .id("working-changes-toolbar")
                .padding([
                    theme::FILE_PANEL_TOOL_INSET,
                    theme::FILE_PANEL_CONTENT_INSET,
                ])
                .width(Length::Fill),
            )
            .push(divider());
    }
    let browser = scrollable(container(content).padding(theme::FILE_PANEL_CONTENT_INSET))
        .id("project-files-browser")
        .width(Length::Fill)
        .direction(scrollable::Direction::Both {
            vertical: scrollable::Scrollbar::default(),
            horizontal: scrollable::Scrollbar::default(),
        })
        .style(theme::scroll)
        .height(Length::Fill);
    let browser = if app.file_panel == FilePanel::Project
        && let Some(root) = app.workspace.directory()
    {
        context_menu::fallback(browser, |point| {
            Message::Files(crate::app::files::Event::OpenMenu(root.clone(), point))
        })
    } else {
        browser.into()
    };
    let files = container(files.push(browser))
        .id("project-files-panel")
        .width(Length::Fill)
        .height(Length::Fill)
        .style(theme::surface);
    // Keep the preview in the same child slot when the file panel is hidden.
    let file_divider: Element<'_, Message> = if app.file_panel_visible {
        vertical_divider().into()
    } else {
        space().width(1).into()
    };
    resize::horizontal(
        Row::from_vec(vec![
            tools,
            container(files)
                .width(if app.file_panel_visible {
                    panel_width
                } else {
                    0.
                })
                .clip(true)
                .into(),
            container(file_divider).id("file-panel-divider").into(),
            container(preview(app))
                .id("source-preview")
                .width(Length::Fill)
                .height(Length::Fill)
                .into(),
        ])
        .width(Length::Fill)
        .height(Length::Fill)
        .into(),
        app.file_panel_visible.then_some(panel_width),
        toolbar_width + panel_width,
        minimum..=maximum,
        Message::ResizeFilePanel,
    )
}

fn project_entry(app: &App, space_index: usize, project_index: usize) -> Element<'_, Message> {
    let group = &app.workspace.spaces[space_index];
    let project = &group.projects[project_index];
    let active = app.workspace.selected_space == Some(space_index)
        && app.workspace.selected_project == Some(project_index);
    let marked = app
        .project_selection
        .as_ref()
        .map(|set| set.contains(space_index, &project.directory));
    let mut title = column![text(short(&project.name, 20)).size(13)];
    if project.pinned {
        title = title.push(
            text(short(&group.name, 24))
                .size(11)
                .style(theme::FAINT.text()),
        );
    }
    let mut contents = row![
        icon(
            if marked == Some(true) {
                Icon::Check
            } else {
                Icon::Folder
            },
            if marked == Some(true) {
                theme::BLUE
            } else if active {
                theme::TEXT
            } else {
                theme::MUTED
            }
        ),
        title.width(Length::Fill),
    ]
    .spacing(7)
    .align_y(Alignment::Center);
    if app.project_has_changes(&project.directory) {
        contents = contents.push(tooltip::view(
            container(sized_icon(
                Icon::Changes,
                theme::GREEN,
                theme::PROJECT_STATUS_ICON_SIZE,
            ))
            .id(format!("project-changes-{space_index}-{project_index}")),
            text(app.i18n.text(Key::WorkingChanges)).size(12),
            tooltip::Position::Right,
        ));
    }
    let entry = tooltip::view(
        container(
            button(contents)
                .padding([5, 9])
                .width(Length::Fill)
                .style(theme::action(marked.unwrap_or(active)))
                .on_press(if app.project_modifiers.command() {
                    Message::ToggleProjectSelection(space_index, project.directory.clone())
                } else {
                    Message::SelectProject(space_index, project_index)
                }),
        )
        .id(format!("space-project-{space_index}-{project_index}"))
        .width(Length::Fill),
        text(project.directory.to_string_lossy().into_owned()).size(12),
        tooltip::Position::Right,
    );
    let path = project.directory.clone();
    context_menu::trigger(entry, move |point| {
        Message::ProjectMenu(space_index, path.clone(), point)
    })
}

fn project_menu<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let base = space_menu(app, base);
    let Some((source, path, point)) = &app.project_menu else {
        return base;
    };
    let Some(project) = app.workspace.spaces.get(*source).and_then(|space| {
        space
            .projects
            .iter()
            .find(|project| &project.directory == path)
    }) else {
        return base;
    };
    use spaces::ProjectMenuPage;
    let Some(selection) = app.project_menu_selection() else {
        return base;
    };
    let count = selection.len();
    let destinations = selection.destinations(&app.workspace);
    let can_edit = !app.modules.git.git_actions.pending && !app.picking;
    let has_destinations = !destinations.is_empty();
    let anchor = match app.project_menu_page {
        ProjectMenuPage::Destinations(anchor) => anchor,
        _ => None,
    };
    let parent = context_menu::overlay(
        base,
        *point,
        iced::Size::new(252. + context_menu::ICON_COLUMN_WIDTH, 185.),
        move || {
            let item =
                |key, message| context_menu::action(key, app.i18n.text(key)).on_press(message);
            let transfer = || -> Element<'_, Message> {
                let open = (can_edit && has_destinations).then_some(Message::ProjectMenuPage(
                    ProjectMenuPage::Destinations(None),
                ));
                let control = context_menu::branch(
                    "project-move-spaces".into(),
                    Key::ProjectMoveToSpace,
                    app.i18n.text(Key::ProjectMoveToSpace),
                    anchor.is_some(),
                    open.clone(),
                    open,
                );
                if !has_destinations {
                    tooltip::view(
                        control,
                        text(app.i18n.text(Key::NoProjectDestination)).size(12),
                        tooltip::Position::Right,
                    )
                    .into()
                } else {
                    control
                }
            };
            if count > 1 {
                let mut items = column![
                    container(
                        text(selection_count(app, count))
                            .size(12)
                            .style(theme::MUTED.text())
                    )
                    .padding([4, 8]),
                    context_menu::separator()
                ];
                let confirm = app.project_menu_page == ProjectMenuPage::ConfirmRemoval;
                if confirm {
                    items = items
                        .push(
                            container(text(app.i18n.text(Key::RemoveProjectsHint)).size(13))
                                .padding([6, 8]),
                        )
                        .push(item(
                            Key::Cancel,
                            Message::ProjectMenuPage(ProjectMenuPage::Actions),
                        ));
                } else {
                    items = items.push(transfer());
                }
                let remove = if confirm {
                    Message::RemoveProjectFromSpace
                } else {
                    Message::ProjectMenuPage(ProjectMenuPage::ConfirmRemoval)
                };
                return items
                    .push(
                        container(
                            context_menu::action(
                                Key::RemoveSelectedProjects,
                                app.i18n.text(Key::RemoveSelectedProjects),
                            )
                            .on_press_maybe(can_edit.then_some(remove)),
                        )
                        .id("remove-selected-projects"),
                    )
                    .into();
            }
            let items = column![
                container(item(
                    if project.pinned {
                        Key::UnpinProject
                    } else {
                        Key::PinProject
                    },
                    Message::TogglePinned,
                ))
                .id("toggle-project-pin"),
                item(Key::ProjectOpenFolder, Message::ProjectFolder(false)),
                item(Key::CopyPath, Message::ProjectFolder(true)),
            ];
            items
                .push(transfer())
                .push(crate::ui::explain::submenu(
                    &app.i18n,
                    crate::domain::explain::Target::Project(project.name.clone(), path.clone()),
                    crate::ui::explain::choose,
                ))
                .push(context_menu::separator())
                .push(
                    container(
                        context_menu::action(
                            Key::RemoveProjectFromSpace,
                            app.i18n.text(Key::RemoveProjectFromSpace),
                        )
                        .on_press_maybe(can_edit.then_some(Message::RemoveProjectFromSpace)),
                    )
                    .id("remove-project-from-space"),
                )
                .into()
        },
        Message::CloseProjectMenu,
    );
    context_menu::beside(
        parent,
        anchor.map(|row| (row, Message::CloseProjectDestinations)),
        252.,
        move || {
            let choices = Column::with_children(destinations.iter().map(|&index| {
                container(
                    context_menu::row(context_menu::icon_label(
                        None,
                        app.workspace.spaces[index].name.clone(),
                    ))
                    .on_press_maybe(can_edit.then_some(Message::TransferProject(index))),
                )
                .id(format!("project-destination-{index}"))
                .into()
            }));
            container(
                scrollable(choices)
                    .height(Length::Shrink)
                    .style(theme::scroll),
            )
            .max_height(196.)
            .into()
        },
        true,
    )
}

fn space_menu<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let layers = widget::stack![base];
    let Some((index, menu)) = app.space_menu else {
        return layers.into();
    };
    let Some(group) = app.workspace.spaces.get(index) else {
        return layers.into();
    };
    // Keep the scrolling sidebar in the same widget tree while either menu is open.
    let background = space().width(Length::Fill).height(Length::Fill).into();
    let parent = menu.parent();
    let menu_layer = if let Some(point) = parent {
        context_menu::overlay(
            background,
            point,
            iced::Size::new(252. + context_menu::ICON_COLUMN_WIDTH, 185.),
            move || {
                use crate::app::spaces::Message as Space;
                let item =
                    |key, message| context_menu::action(key, app.i18n.text(key)).on_press(message);
                let mut items = column![
                    context_menu::branch(
                        spaces::add_project_id(index, true),
                        Key::AddProject,
                        app.i18n.text(Key::AddProject),
                        matches!(menu, spaces::Menu::AddProject { .. }),
                        Some(Message::OpenSpaceAddMenu(index)),
                        Some(Message::ProjectAdd(crate::app::project_add::Message::Open(
                            Some(index)
                        )))
                    ),
                    container(item(Key::EditSpace, Message::Space(Space::Edit(index))))
                        .id("space-menu-edit"),
                ];
                if !group.projects.is_empty() {
                    let key = if app.collapsed_spaces.contains(&index) {
                        Key::SpaceExpandProjects
                    } else {
                        Key::SpaceCollapseProjects
                    };
                    items = items.push(item(key, Message::ToggleSpace(index)));
                }
                items
                    .push(
                        container(item(
                            Key::DeploymentSettings,
                            crate::app::modules::deployment_message(
                                crate::modules::deployment::Event::SpaceSettings(index),
                            ),
                        ))
                        .id("space-menu-deployment"),
                    )
                    .push(crate::ui::explain::submenu(
                        &app.i18n,
                        crate::domain::explain::Target::Space(index),
                        crate::ui::explain::choose,
                    ))
                    .push(context_menu::separator())
                    .push(
                        container(
                            context_menu::action(Key::DeleteSpace, app.i18n.text(Key::DeleteSpace))
                                .on_press_maybe(
                                    (!app.modules.git.git_actions.pending && !app.picking)
                                        .then_some(Message::Space(Space::Delete(index))),
                                ),
                        )
                        .id("space-menu-delete"),
                    )
                    .into()
            },
            Message::CloseSpaceMenu,
        )
    } else {
        background
    };
    let anchor = match menu {
        spaces::Menu::AddProject { anchor, .. } => anchor,
        spaces::Menu::Context(_) => None,
    };
    layers
        .push(context_menu::beside(
            menu_layer,
            anchor.map(|row| (row, Message::CloseSpaceAddMenu(index))),
            228.,
            move || {
                let action = |symbol, key, clone, id| {
                    container(
                        context_menu::row(context_menu::icon_label(
                            Some(symbol),
                            app.i18n.text(key),
                        ))
                        .on_press(Message::AddProjectFromMenu(index, clone)),
                    )
                    .id(id)
                };
                column![
                    action(
                        Icon::Folder,
                        Key::SpaceAddLocalProject,
                        false,
                        "space-add-local"
                    ),
                    action(Icon::Branch, Key::CloneRepository, true, "space-add-clone"),
                ]
                .into()
            },
            parent.is_some(),
        ))
        .into()
}

fn tree_width(app: &App, path: &Path, depth: usize) -> f32 {
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy();
    let width = 46.
        + depth as f32 * theme::FILE_TREE_INDENT
        + crate::ui::ellipsis::measured_width(&name, theme::FILE_TREE_TEXT_SIZE).ceil();
    if app.expanded.contains(path) {
        app.tree
            .get(path)
            .into_iter()
            .flatten()
            .map(|entry| tree_width(app, &entry.path, depth + 1))
            .fold(width, f32::max)
    } else {
        width
    }
}

fn file_tree<'a>(
    app: &'a App,
    directory: &Path,
    depth: usize,
    mut body: Column<'a, Message>,
) -> Column<'a, Message> {
    if let Some(entries) = app.tree.get(directory) {
        for entry in entries {
            let expanded = app.expanded.contains(&entry.path);
            let event = if entry.directory {
                Message::ToggleFolder(entry.path.clone())
            } else {
                Message::OpenFile(entry.path.clone())
            };
            let active = if entry.directory {
                app.files.selected_path.as_ref() == Some(&entry.path)
            } else {
                app.preview().revision.is_none() && app.preview().path.as_ref() == Some(&entry.path)
            };
            let marker: Element<'_, Message> = if entry.directory {
                file_icons::chevron(expanded)
            } else {
                space().width(10).into()
            };
            let repository_index = app
                .changes
                .status
                .roots
                .iter()
                .position(|root| root == &entry.path);
            let symbol = if entry.directory
                && let Some(index) = repository_index
            {
                icon(Icon::Branch, theme::repository_color(index))
            } else if entry.directory {
                let package = app
                    .tree
                    .get(&entry.path)
                    .is_some_and(|entries| entries.iter().any(|item| item.name == "__init__.py"));
                file_icons::folder(expanded, package)
            } else {
                file_icons::file(&entry.path)
            };
            let status = app.changes.status.status(&entry.path, entry.directory);
            let item = row![
                marker,
                symbol,
                text(&entry.name)
                    .size(theme::FILE_TREE_TEXT_SIZE)
                    .style(
                        (if let Some(index) = repository_index {
                            theme::repository_color(index)
                        } else if status == Some(crate::domain::changes::FileStatus::Ignored) {
                            theme::ICON_ORANGE
                        } else if entry.directory {
                            theme::MUTED
                        } else {
                            status.map(theme::file_status).unwrap_or(theme::TEXT)
                        })
                        .text()
                    )
                    .wrapping(text::Wrapping::None),
                space().width(Length::Fill),
            ]
            .spacing(4)
            .padding(iced::Padding {
                left: depth as f32 * theme::FILE_TREE_INDENT,
                right: 0.,
                top: 0.,
                bottom: 0.,
            })
            .height(theme::FILE_TREE_ROW_HEIGHT)
            .align_y(Alignment::Center);
            let mut hint = entry.path.to_string_lossy().into_owned();
            if !entry.directory
                && let Some(status) = status
            {
                hint.push('\n');
                hint.push_str(&app.i18n.text(changed_tree::status_key(status)));
            }
            let preview_mode = !entry.directory && app.workspace.preferences.preview_tab;
            let item: Element<'_, Message> = if preview_mode {
                widget::mouse_area(container(item).padding([0, 6]).width(Length::Fill).style(
                    move |theme| {
                        let style = crate::ui::theme::tree_item(active)(
                            theme,
                            iced::widget::button::Status::Active,
                        );
                        container::Style {
                            background: style.background,
                            text_color: Some(style.text_color),
                            ..Default::default()
                        }
                    },
                ))
                .on_press(Message::Files(crate::app::files::Event::Preview(
                    entry.path.clone(),
                )))
                .on_double_click(event)
                .interaction(iced::mouse::Interaction::Pointer)
                .into()
            } else {
                button(item)
                    .on_press(event)
                    .height(theme::FILE_TREE_ROW_HEIGHT)
                    .padding([0, 6])
                    .width(Length::Fill)
                    .style(theme::tree_item(active))
                    .into()
            };
            body = body.push(context_menu::trigger(
                tooltip::view(
                    container(item)
                        .id(format!("file-tree-{}", entry.path.to_string_lossy()))
                        .width(Length::Fill),
                    text(hint).size(12),
                    tooltip::Position::FollowCursor,
                ),
                |point| {
                    Message::Files(crate::app::files::Event::OpenMenu(
                        entry.path.clone(),
                        point,
                    ))
                },
            ));
            if entry.directory && expanded {
                body = file_tree(app, &entry.path, depth + 1, body);
            }
        }
    } else if app.files.loading.contains(directory) {
        body = body.push(
            text(app.i18n.text(Key::Loading))
                .size(12)
                .style(theme::MUTED.text()),
        );
    }
    if let Some(error) = app.files.errors.get(directory) {
        body = body.push(
            column![
                text(app.i18n.text(error_key(*error)))
                    .size(12)
                    .style(theme::YELLOW.text()),
                theme::compact_button(app.i18n.text(Key::Retry)).on_press(Message::Files(
                    crate::app::files::Event::Retry(directory.to_path_buf())
                )),
            ]
            .spacing(6),
        );
    }
    body
}

fn file_tabs(app: &App) -> Element<'_, Message> {
    responsive(move |size| file_tabs_bar(app, size.width))
        .height(theme::FILE_TABS_HEIGHT)
        .into()
}

fn file_tabs_bar(app: &App, available_width: f32) -> Element<'_, Message> {
    const MARKDOWN_BUTTONS: [(MarkdownViewMode, Icon, Key, &str); 3] = [
        (
            MarkdownViewMode::Source,
            Icon::MarkdownSource,
            Key::MarkdownSource,
            "markdown-source",
        ),
        (
            MarkdownViewMode::Split,
            Icon::MarkdownSplit,
            Key::MarkdownSplit,
            "markdown-split",
        ),
        (
            MarkdownViewMode::Preview,
            Icon::Image,
            Key::MarkdownPreview,
            "markdown-preview",
        ),
    ];
    let tab = app.preview();
    let markdown_mode = app.markdown_view_mode();
    let ready =
        app.preview().error.is_none() && !app.preview().loading && app.preview().editor.is_none();
    let comparison = ready
        && app
            .preview()
            .document
            .as_ref()
            .is_some_and(|document| document.comparison.is_some());
    let folding = comparison
        && app
            .preview()
            .document
            .as_ref()
            .is_some_and(|document| document.can_fold());
    let controls = usize::from(comparison) + usize::from(folding);
    let markdown_width = if markdown_mode.is_some() {
        MARKDOWN_BUTTONS.len() as f32 * theme::FILE_TAB_TOOL_HEIGHT + theme::FILE_TAB_TOOL_GAP
    } else {
        0.
    };
    let tabs_width: f32 = app
        .tabs
        .files
        .iter()
        .filter(|preview| !preview.title.is_empty())
        .map(|preview| {
            tab_label::width(&tab_label::label(&preview.title, preview.label_revision()))
        })
        .sum();
    let remaining = (available_width
        - theme::FILE_TAB_TOOL_GAP
        - markdown_width
        - controls as f32 * (theme::FILE_TAB_TOOL_SIZE + theme::FILE_TAB_TOOL_GAP))
        .max(0.);
    let overflow = tabs_width > remaining + 0.5;

    let tabs = app
        .tabs
        .files
        .iter()
        .filter(|preview| !preview.title.is_empty())
        .map(|preview| {
            let active = preview.id == app.preview().id;
            let label = tab_label::label(&preview.title, preview.label_revision());
            let width = tab_label::width(&label);
            let mut description = if let Some(revision) = preview.label_revision() {
                format!("{}\n{}", preview.title, revision)
            } else {
                preview.title.clone()
            };
            if preview
                .document
                .as_ref()
                .is_some_and(|document| document.comparison_failed)
            {
                description.push_str(&format!("\n{}", app.i18n.text(Key::ComparisonFailed)));
            }
            if let Some(buffer) = &preview.editor {
                let status = buffer.error.or_else(|| {
                    if buffer.saving {
                        Some(Key::EditorSaving)
                    } else if buffer.document.dirty() {
                        Some(Key::EditorUnsaved)
                    } else {
                        None
                    }
                });
                if let Some(status) = status {
                    description.push_str(&format!("\n{}", app.i18n.text(status)));
                }
            }
            let select = tooltip::view(
                container(
                    row![
                        if preview.remote {
                            icon(Icon::Server, theme::MUTED)
                        } else {
                            file_icons::file(Path::new(&preview.title))
                        },
                        text(label)
                            .font(if preview.temporary {
                                iced::Font {
                                    style: iced::font::Style::Italic,
                                    ..theme::font()
                                }
                            } else {
                                theme::font()
                            })
                            .style(
                                preview
                                    .path
                                    .as_deref()
                                    .filter(|_| !preview.remote && preview.revision.is_none())
                                    .and_then(|path| app.changes.status.status(path, false))
                                    .or_else(|| preview
                                        .editor
                                        .as_ref()
                                        .filter(|buffer| buffer.document.dirty())
                                        .map(|_| crate::domain::changes::FileStatus::Modified))
                                    .map(theme::file_status)
                                    .unwrap_or(theme::TEXT)
                                    .text()
                            )
                            .size(theme::FILE_TAB_TEXT_SIZE)
                            .shaping(text::Shaping::Advanced)
                            .wrapping(text::Wrapping::None),
                    ]
                    .spacing(theme::FILE_TAB_LABEL_GAP)
                    .height(Length::Fill)
                    .align_y(Alignment::Center),
                )
                .padding([4., theme::FILE_TAB_PADDING])
                .width(Length::Fill)
                .height(theme::FILE_TABS_HEIGHT - 2.)
                .id(format!("file-tab-{}", preview.id)),
                text(description).size(12),
                tooltip::Position::Bottom,
            );
            let close = tooltip::view(
                container(
                    button(
                        if preview
                            .editor
                            .as_ref()
                            .is_some_and(|buffer| buffer.document.dirty())
                        {
                            text("•")
                                .size(18)
                                .style(
                                    if preview
                                        .editor
                                        .as_ref()
                                        .is_some_and(|buffer| buffer.error.is_some())
                                    {
                                        theme::RED.text()
                                    } else {
                                        theme::YELLOW.text()
                                    },
                                )
                                .into()
                        } else {
                            icon(Icon::Close, theme::FAINT)
                        },
                    )
                    .on_press(Message::CloseTab(preview.id))
                    .padding(4)
                    .width(theme::FILE_TAB_CLOSE_WIDTH)
                    .height(28)
                    .style(theme::action(false)),
                )
                .id(format!("close-file-tab-{}", preview.id)),
                text(app.i18n.text(Key::CloseTab)).size(12),
                tooltip::Position::Bottom,
            );
            mouse_area(
                button(column![
                    row![select, close, space().width(theme::FILE_TAB_END_GAP)]
                        .align_y(Alignment::Center),
                    container(space())
                        .height(2)
                        .width(Length::Fill)
                        .style(theme::tab_indicator(active)),
                ])
                .width(width)
                .padding(0)
                .on_press(Message::SelectTab(preview.id))
                .style(theme::file_tab(active)),
            )
            .on_middle_press(Message::CloseTab(preview.id))
            .into()
        });
    let mut bar = row![
        scrollable(Row::with_children(tabs))
            .id("file-tabs")
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::default().width(4).scroller_width(4),
            ))
            .width(Length::Fill)
            .height(theme::FILE_TABS_HEIGHT)
            .style(theme::scroll),
    ]
    .spacing(theme::FILE_TAB_TOOL_GAP)
    .align_y(Alignment::Center);
    if tab.local_svg() {
        let label = if tab.svg_source {
            Key::SvgShowImage
        } else {
            Key::SvgEditSource
        };
        let available =
            !tab.loading && tab.editor.as_ref().is_none_or(|buffer| !buffer.protected());
        bar = bar.push(
            theme::compact_button(app.i18n.text(label))
                .style(theme::action(false))
                .on_press_maybe(available.then_some(Message::ToggleSvgSource(tab.id))),
        );
    }
    if overflow {
        bar = bar.push(tooltip::view(
            context_menu::anchored(
                container(
                    button(container(icon(Icon::Down, theme::MUTED)).center(Length::Fill))
                        .width(theme::FILE_TAB_TOOL_SIZE)
                        .height(theme::FILE_TAB_TOOL_HEIGHT)
                        .padding([4, 8])
                        .style(theme::action(app.tabs.menu.is_some())),
                )
                .id("open-tabs"),
                Message::OpenTabMenu,
            ),
            text(app.i18n.text(Key::OpenTabs)).size(12),
            tooltip::Position::Bottom,
        ));
    }
    if comparison {
        let split = app.workspace.preferences.diff_view == DiffView::SideBySide;
        bar = bar.push(
            container(tooltip::view(
                button(
                    container(icon(
                        if split {
                            Icon::UnifiedDiff
                        } else {
                            Icon::SplitDiff
                        },
                        theme::MUTED,
                    ))
                    .center(Length::Fill),
                )
                .padding([4, 8])
                .width(theme::FILE_TAB_TOOL_SIZE)
                .height(theme::FILE_TAB_TOOL_HEIGHT)
                .style(theme::action(split))
                .on_press(Message::ToggleDiffView(app.preview().id)),
                text(app.i18n.text(if split {
                    Key::DiffUnified
                } else {
                    Key::DiffSideBySide
                }))
                .size(12),
                tooltip::Position::Bottom,
            ))
            .id("toggle-diff-view"),
        );
    }
    if folding {
        let collapsed = app.preview().folding.collapsed;
        bar = bar.push(
            container(tooltip::view(
                button(
                    container(icon(
                        if collapsed {
                            Icon::ExpandLines
                        } else {
                            Icon::CollapseLines
                        },
                        theme::MUTED,
                    ))
                    .center(Length::Fill),
                )
                .padding([4, 8])
                .width(theme::FILE_TAB_TOOL_SIZE)
                .height(theme::FILE_TAB_TOOL_HEIGHT)
                .style(theme::action(collapsed))
                .on_press(Message::ToggleUnchanged(app.preview().id)),
                text(app.i18n.text(if collapsed {
                    Key::ExpandUnchanged
                } else {
                    Key::CollapseUnchanged
                }))
                .size(12),
                tooltip::Position::Bottom,
            ))
            .id("fold-unchanged"),
        );
    }
    if let Some(selected) = markdown_mode {
        let buttons = MARKDOWN_BUTTONS.into_iter().map(|(mode, icon, label, id)| {
            let button = IconButton::Inline(theme::FILE_TAB_TOOL_HEIGHT).button(
                icon,
                Some(Message::SetMarkdownViewMode(tab.id, mode)),
                mode == selected,
            );
            tooltip::view(
                container(button).id(id),
                text(app.i18n.text(label)).size(12),
                tooltip::Position::Bottom,
            )
            .into()
        });
        bar = bar.push(Row::with_children(buttons));
    }
    container(bar)
        .id("file-tabs-header")
        .padding(iced::Padding {
            right: theme::FILE_TAB_TOOL_GAP,
            ..iced::Padding::ZERO
        })
        .height(theme::FILE_TABS_HEIGHT)
        .style(theme::sidebar)
        .into()
}

fn source_pane(app: &App) -> Element<'_, Message> {
    let content: Element<'_, Message> = if let Some(buffer) = app.editor_buffer() {
        crate::ui::editor::view(app, buffer)
    } else if app.preview().loading
        && (app.preview().document.is_none() || app.preview().awaiting_editor())
    {
        center_message(&app.i18n, Key::Loading)
    } else if let Some(error) = app.preview().error {
        center_message(&app.i18n, error_key(error))
    } else if let Some(image) = &app.preview().image {
        container(
            widget::image(image.clone())
                .content_fit(iced::ContentFit::ScaleDown)
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .padding(16)
        .center(Length::Fill)
        .into()
    } else if let Some(document) = &app.preview().document {
        if let Some(comparison) = document
            .comparison
            .as_ref()
            .filter(|_| app.workspace.preferences.diff_view == DiffView::SideBySide)
        {
            diff::view(app, document, comparison)
        } else {
            responsive(move |viewport| {
                let size = app.workspace.preferences.code_size as f32;
                let height = size * (f32::from(app.workspace.preferences.code_line_height) / 100.);
                let gutter = if document.comparison.is_some() {
                    unified::gutter_width(document, size, app.workspace.preferences.line_numbers)
                        + 8.
                } else if app.workspace.preferences.line_numbers {
                    91.
                } else {
                    35.
                };
                let rail = if document.comparison.is_some() {
                    theme::CODE_SCROLLBAR_WIDTH
                } else {
                    0.
                };
                let width = (document.columns as f32 * size * 0.65 + gutter + 12. + rail)
                    .max(viewport.width);
                let count = app.preview().folding.len(document);
                let visible =
                    code::visible_rows(app.preview().scroll_y, viewport.height, height, count);
                let (start, end) = (visible.start, visible.end);
                let lines = (start..end).map(|row| {
                    let index = match app.preview().folding.row(row) {
                        code::folding::Row::Line(index) => index,
                        code::folding::Row::Fold(range) => {
                            if document.comparison.is_some() {
                                return unified::fold(app, document, range, height);
                            }
                            let mut args = FluentArgs::new();
                            args.set("start", document.line_info[range.start].number);
                            args.set("end", document.line_info[range.end - 1].number);
                            return container(
                                button(
                                    row![
                                        space().width(48),
                                        file_icons::chevron(false),
                                        text(app.i18n.format(Key::UnchangedLines, &args))
                                            .size(12)
                                            .wrapping(text::Wrapping::None),
                                    ]
                                    .spacing(8)
                                    .height(height)
                                    .align_y(Alignment::Center),
                                )
                                .padding([0, 8])
                                .width(Length::Fill)
                                .height(height)
                                .style(theme::fold_row)
                                .on_press(
                                    Message::ExpandUnchanged(app.preview().id, range.clone()),
                                ),
                            )
                            .id(format!("unchanged-{}-{}", app.preview().id, range.start))
                            .width(Length::Fill)
                            .into();
                        }
                    };
                    if let Some(line) = document
                        .comparison
                        .as_ref()
                        .and_then(|comparison| comparison.unified_line(index))
                    {
                        return unified::line(app, document, line, index, height);
                    }
                    let highlighted = app.preview().selection.highlights_line(
                        document,
                        code::selection::Lane::Unified,
                        index,
                    );
                    let spans = &document.lines[index];
                    let code = code::selection::line(
                        app,
                        document,
                        index,
                        code::selection::Lane::Unified,
                        index,
                        spans
                            .iter()
                            .map(|(content, color)| {
                                widget::span(content.as_str()).color(color.resolve(&app.theme()))
                            })
                            .collect::<Vec<_>>(),
                        size,
                    );
                    let info = document.line_info[index];
                    let color = info
                        .change
                        .map(|change| theme::line_change(change).resolve(&app.theme()))
                        .unwrap_or(iced::Color::TRANSPARENT);
                    let mut line = Row::new()
                        .spacing(10)
                        .height(height)
                        .width(Length::Fill)
                        .push(container(space()).width(3).height(height).style(move |_| {
                            container::Style {
                                background: Some(color.into()),
                                ..Default::default()
                            }
                        }));
                    if app.workspace.preferences.line_numbers {
                        line = line.push(
                            text(info.number)
                                .size(size)
                                .line_height(
                                    f32::from(app.workspace.preferences.code_line_height) / 100.,
                                )
                                .font(theme::selected_code_font(
                                    app.workspace.preferences.code_font,
                                ))
                                .style(theme::FAINT.text())
                                .width(46)
                                .align_x(Alignment::End),
                        );
                    }
                    let sign = match info.change {
                        Some(code::LineChange::Added) => "+",
                        Some(code::LineChange::Modified) => "~",
                        Some(code::LineChange::Removed) => "−",
                        None => "",
                    };
                    code::frozen_gutter::view(
                        container(
                            line.push(
                                text(sign)
                                    .font(theme::selected_code_font(
                                        app.workspace.preferences.code_font,
                                    ))
                                    .size(size)
                                    .line_height(
                                        f32::from(app.workspace.preferences.code_line_height)
                                            / 100.,
                                    )
                                    .color(color)
                                    .width(12),
                            )
                            .push(code),
                        )
                        .width(Length::Fill)
                        .style(move |theme| {
                            theme::source_row(
                                theme,
                                theme::diff_row(info.change)(theme),
                                highlighted,
                            )
                        }),
                        if app.workspace.preferences.line_numbers {
                            theme::SOURCE_GUTTER_WIDTH
                        } else {
                            theme::SOURCE_MARKERS_WIDTH
                        },
                    )
                });
                let scroll = scrollable(
                    container(column![
                        space().height(start as f32 * height),
                        code::rows(
                            (
                                app.preview().id,
                                document.fingerprint,
                                app.preview().preparation,
                                app.preview().folding.revision,
                                (
                                    app.preview().selection.revision,
                                    app.file_search().map(|search| (
                                        &search.query,
                                        search.selected,
                                        search.results.hits.len()
                                    ))
                                ),
                                (
                                    app.workspace.preferences.code_size,
                                    app.workspace.preferences.code_line_height,
                                    app.workspace.preferences.code_font
                                ),
                                app.workspace.preferences.line_numbers,
                                app.workspace.preferences.appearance,
                                app.i18n.text(Key::Loading),
                            ),
                            start..end,
                            lines
                        ),
                        space().height((count - end) as f32 * height)
                    ])
                    .padding(iced::Padding {
                        top: 0.,
                        right: rail,
                        bottom: 12.,
                        left: 0.,
                    })
                    .width(width),
                )
                .direction(scrollable::Direction::Both {
                    vertical: if document.comparison.is_some() {
                        code::overview::scrollbar()
                    } else {
                        scrollable::Scrollbar::default()
                    },
                    horizontal: scrollable::Scrollbar::default(),
                })
                .id("source-code")
                .wheel_scroll_scale(theme::CODE_WHEEL_SCROLL_SCALE)
                .style(if document.comparison.is_some() {
                    theme::code_scroll
                } else {
                    theme::scroll
                })
                .height(Length::Fill)
                .on_scroll(move |viewport| {
                    let offset = viewport.absolute_offset();
                    Message::CodeScrolled {
                        id: app.preview().id,
                        x: offset.x,
                        y: offset.y,
                    }
                })
                .width(Length::Fill);
                code::overview::view(
                    scroll,
                    document,
                    &app.preview().folding,
                    None,
                    code::overview::Layout {
                        rows: count,
                        row_height: height,
                        viewport,
                        content_width: width,
                    },
                )
            })
            .into()
        }
    } else {
        let no_project = app.workspace.directory().is_none();
        let has_spaces = !app.workspace.spaces.is_empty();
        let mut content = column![
            text(app.i18n.text(if no_project {
                if has_spaces {
                    Key::ChooseProject
                } else {
                    Key::Greeting
                }
            } else {
                Key::ChooseFile
            }))
            .size(30)
            .font(theme::semibold())
            .align_x(Alignment::Center),
            text(app.i18n.text(if no_project {
                if has_spaces {
                    Key::ChooseProjectDescription
                } else {
                    Key::WelcomeDescription
                }
            } else {
                Key::ChooseFileDescription
            }))
            .size(16)
            .line_height(1.5)
            .style(theme::MUTED.text())
            .align_x(Alignment::Center),
        ]
        .spacing(16)
        .align_x(Alignment::Center)
        .max_width(520);
        let show_action = if no_project {
            !has_spaces
        } else {
            !app.file_panel_visible
                || app.file_panel != crate::domain::workspace::FilePanel::Project
        };
        if show_action {
            content = content.push(space().height(8)).push(
                container(
                    theme::text_button(app.i18n.text(if no_project {
                        Key::NewSpace
                    } else {
                        Key::ProjectFiles
                    }))
                    .on_press_maybe(if no_project {
                        (app.loaded && !app.picking)
                            .then_some(Message::Space(crate::app::spaces::Message::New))
                    } else {
                        Some(Message::SelectFilePanel(
                            crate::domain::workspace::FilePanel::Project,
                        ))
                    })
                    .style(theme::primary),
                )
                .id("preview-empty-action"),
            );
        }
        container(content).padding(40).center(Length::Fill).into()
    };
    let content = if let Some((profile, path)) = &app.preview().pending_comparison {
        let key = if let Some(error) = app.preview().error {
            error_key(error)
        } else if app.preview().comparison_received() {
            Key::Loading
        } else {
            app.modules
                .deployment
                .comparison_error(profile, path)
                .as_ref()
                .map(crate::modules::deployment::error_key)
                .unwrap_or(Key::DeploymentLoadingRemote)
        };
        let heading = |key| {
            container(text(app.i18n.text(key)).size(12).style(theme::MUTED.text()))
                .padding([6, 12])
                .width(Length::Fill)
        };
        if app.workspace.preferences.diff_view == DiffView::Unified {
            column![heading(key), divider(), content]
                .height(Length::Fill)
                .into()
        } else {
            row![
                column![
                    heading(Key::DeploymentRemote),
                    divider(),
                    container(text(app.i18n.text(key)).size(13)).center(Length::Fill)
                ]
                .width(Length::FillPortion(1))
                .height(Length::Fill),
                vertical_divider(),
                column![heading(Key::DeploymentLocal), divider(), content]
                    .width(Length::FillPortion(1))
                    .height(Length::Fill)
            ]
            .height(Length::Fill)
            .into()
        }
    } else {
        content
    };
    if app.preview().editor.is_none() && !app.preview().loading && app.preview().document.is_some()
    {
        let id = app.preview().id;
        context_menu::trigger(content, move |point| {
            crate::app::editor::menu::message(crate::app::editor::menu::Event::OpenViewer(
                id, point,
            ))
        })
    } else {
        content
    }
}

fn file_panes(app: &App) -> Element<'_, Message> {
    let Some(mode @ (MarkdownViewMode::Split | MarkdownViewMode::Preview)) =
        app.markdown_view_mode()
    else {
        return source_pane(app);
    };
    let preferences = &app.workspace.preferences;
    let formatted = crate::ui::markdown::view(
        &app.markdown.items,
        &app.theme(),
        theme::selected_code_font(preferences.code_font),
        preferences.code_size,
    )
    .map(|url| app.preview().markdown_link(url));
    if mode == MarkdownViewMode::Preview {
        return formatted;
    }
    row![
        container(source_pane(app))
            .width(Length::FillPortion(1))
            .clip(true),
        vertical_divider(),
        container(formatted)
            .width(Length::FillPortion(1))
            .clip(true),
    ]
    .height(Length::Fill)
    .into()
}

fn preview(app: &App) -> Element<'_, Message> {
    let has_tabs = app
        .tabs
        .files
        .iter()
        .any(|preview| !preview.title.is_empty());
    let content = super::text_search::file_panel(app, file_panes(app));
    let content = crate::modules::deployment::target_popup(
        &app.modules.deployment,
        &app.i18n,
        content,
        crate::app::modules::deployment_message,
    );
    if has_tabs {
        let progress = if app.preview().loading {
            match app.preview().preparation {
                crate::app::tabs::Preparation::Text => 15.,
                crate::app::tabs::Preparation::Syntax => 45.,
                crate::app::tabs::Preparation::Changes => 80.,
            }
        } else {
            0.
        };
        // Keep the scroll widget in the same tree position when preparation finishes.
        let content = widget::stack![
            container(content)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(theme::code_surface),
            widget::progress_bar(0.0..=100., progress)
                .girth(2)
                .style(theme::source_progress)
        ]
        .height(Length::Fill);
        column![file_tabs(app), divider(), file_part_controls(app), content]
            .height(Length::Fill)
            .into()
    } else {
        content
    }
}

fn file_part_controls(app: &App) -> Element<'_, Message> {
    let Some(page) = app
        .preview()
        .document
        .as_ref()
        .and_then(|document| document.page)
    else {
        return Column::new().into();
    };
    let id = app.preview().id;
    let ready = !app.preview().loading;
    use crate::app::tabs::PageDirection;
    let control = |key, direction, enabled| {
        theme::compact_button(app.i18n.text(key))
            .on_press_maybe((ready && enabled).then_some(Message::ReadFilePart(id, direction)))
            .style(theme::action(false))
    };
    container(
        column![
            text(app.i18n.text(Key::FilePartsNotice))
                .size(12)
                .style(theme::MUTED.text()),
            row![
                control(
                    Key::PreviousFilePart,
                    PageDirection::Previous,
                    page.cursor.offset > 0
                ),
                control(Key::NextFilePart, PageDirection::Next, page.next.is_some()),
                control(Key::Refresh, PageDirection::Refresh, true),
            ]
            .spacing(4),
        ]
        .spacing(4),
    )
    .padding([6, 12])
    .width(Length::Fill)
    .into()
}

fn vertical_toolbar<'a>(
    content: Element<'a, Message>,
    divider_id: &'static str,
) -> Element<'a, Message> {
    row![
        container(content)
            .width(theme::FILE_PANEL_TOOL_SIZE + 2. * theme::FILE_PANEL_TOOL_INSET - 1.)
            .height(Length::Fill)
            .padding(iced::Padding {
                right: theme::FILE_PANEL_TOOL_INSET - 1.,
                ..iced::Padding::new(theme::FILE_PANEL_TOOL_INSET)
            })
            .style(theme::sidebar),
        container(vertical_divider()).id(divider_id),
    ]
    .into()
}

pub(super) fn short_path(value: &str, limit: usize) -> String {
    let count = value.chars().count();
    if count <= limit {
        value.to_owned()
    } else {
        format!("…{}", value.chars().skip(count - limit).collect::<String>())
    }
}
