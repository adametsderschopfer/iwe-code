//! Present compact server lists, aligned forms, and focused settings dialogs.

use super::{error_key, style, tree};
use crate::{
    domain::{
        deployment::{Authentication, Connections, Error, Profile, Protocol, safe_text},
        workspace::Space,
    },
    i18n::{I18n, Key},
    infrastructure::deployment::transport::Secret,
    modules::deployment::state::{Event, Field, NameAction, ProfileTab, State},
    ui::{
        icons::{Icon, icon},
        modal,
        presentation::{IconButton, divider, vertical_divider},
        service_controls::action,
        theme,
    },
};
use iced::{
    Alignment, Element, Length,
    widget::{Column, button, column, container, row, scrollable, space, text},
};
use std::path::Path;

const LABEL_WIDTH: f32 = 128.;
const LIST_WIDTH: f32 = 176.;

#[derive(Clone, PartialEq)]
struct Choice<T> {
    value: T,
    label: String,
}
impl<T> std::fmt::Display for Choice<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.label)
    }
}

fn select<'a, T: Clone + PartialEq + 'a>(
    options: Vec<(T, String)>,
    selected: Option<&T>,
    on_select: impl Fn(T) -> Event + 'a,
) -> Element<'a, Event> {
    let options: Vec<_> = options
        .into_iter()
        .map(|(value, label)| Choice { value, label })
        .collect();
    let selected = options
        .iter()
        .find(|option| Some(&option.value) == selected)
        .cloned();
    theme::Select::Standard
        .view(options, selected, move |choice| on_select(choice.value))
        .width(Length::Fill)
        .into()
}

fn test_controls<'a>(
    state: &'a State,
    i18n: &I18n,
    event: Event,
    connection: Option<&crate::domain::deployment::Connection>,
    profile: Option<&str>,
) -> Element<'a, Event> {
    let detecting_root =
        profile.is_some() && state.operation_label == Some(Key::DeploymentAutodetectRoot);
    let relevant = (detecting_root || state.operation_label == Some(Key::DeploymentTest))
        && state.test_profile.as_deref() == profile
        && state.test_connection.is_some()
        && state.test_connection.as_ref() == connection;
    let outcome = if !relevant {
        None
    } else if state.busy {
        Some((Key::DeploymentChecking, theme::MUTED))
    } else {
        state.test_result.as_ref().map(|result| match result {
            Ok(()) if detecting_root => (Key::DeploymentRootDetected, theme::GREEN),
            Ok(()) => (Key::DeploymentConnected, theme::GREEN),
            Err(error) => (error_key(error), theme::RED),
        })
    };
    let mut controls = column![].spacing(8).width(Length::Fill);
    if let Some((key, color)) = outcome {
        controls = controls.push(
            container(text(i18n.text(key)).size(13).style(color.text()))
                .id("deployment-test-status")
                .width(Length::Fill),
        );
    }
    if relevant
        && !state.busy
        && let Some(Error::HostUnknown(fingerprint)) = &state.error
    {
        controls = controls
            .push(text(fingerprint).size(12).style(theme::MUTED.text()))
            .push(
                row![
                    action(i18n, Key::DeploymentTrust, Event::TrustHost),
                    action(i18n, Key::DeploymentCancel, Event::Cancel),
                ]
                .spacing(8),
            );
    }
    let mut buttons = row![
        container(
            theme::text_button(i18n.text(Key::DeploymentTest))
                .on_press_maybe((!state.busy).then_some(event))
        )
        .id("deployment-test-button"),
    ]
    .spacing(8);
    if relevant && state.busy {
        buttons = buttons.push(action(i18n, Key::DeploymentCancel, Event::Cancel));
    }
    controls = controls.push(buttons);
    row![space().width(LABEL_WIDTH), controls]
        .spacing(12)
        .into()
}

fn protocols(i18n: &I18n) -> Vec<(Protocol, String)> {
    vec![
        (Protocol::Sftp, "SFTP".into()),
        (Protocol::Ftp, "FTP".into()),
        (Protocol::Ftps, "FTPS".into()),
        (
            Protocol::FtpsImplicit,
            i18n.text(Key::DeploymentFtpsImplicit),
        ),
    ]
}

fn form_row<'a>(
    i18n: &I18n,
    key: Key,
    control: impl Into<Element<'a, Event>>,
) -> Element<'a, Event> {
    row![
        text(i18n.text(key)).size(13).width(LABEL_WIDTH),
        control.into()
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn list_row<'a>(
    name: &'a str,
    project: Option<&'a str>,
    selected: bool,
    event: Option<Event>,
    default: bool,
) -> Element<'a, Event> {
    let mut labels = column![text(name).size(13).wrapping(text::Wrapping::None)]
        .spacing(3)
        .width(Length::Fill);
    if let Some(project) = project {
        labels = labels.push(
            text(project)
                .size(11)
                .style(theme::MUTED.text())
                .wrapping(text::Wrapping::None),
        );
    }
    let mut line = row![icon(Icon::Server, theme::MUTED), labels]
        .spacing(6)
        .align_y(Alignment::Center);
    if default {
        line = line.push(icon(Icon::Check, theme::BLUE));
    }
    button(line)
        .on_press_maybe(event)
        .padding([7, 8])
        .width(Length::Fill)
        .style(theme::tree_item(selected))
        .into()
}

fn editor<'a>(
    tools: Element<'a, Event>,
    list: Column<'a, Event>,
    detail: Element<'a, Event>,
) -> Element<'a, Event> {
    let list = container(column![
        container(tools).padding([4, 6]),
        divider(),
        scrollable(list.spacing(2).padding(6))
            .style(theme::scroll)
            .height(Length::Fill)
    ])
    .width(LIST_WIDTH)
    .height(Length::Fill)
    .style(theme::sidebar);
    row![list, vertical_divider(), crate::ui::focus::form(detail)]
        .height(Length::Fill)
        .width(Length::Fill)
        .into()
}

fn footer<'a>(i18n: &I18n, save: Event, dirty: bool, saving: bool) -> Element<'a, Event> {
    row![
        space().width(Length::Fill),
        theme::text_button(i18n.text(if saving {
            Key::DeploymentSaving
        } else {
            Key::DeploymentSave
        }))
        .style(theme::primary)
        .on_press_maybe((dirty && !saving).then_some(save))
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

pub(crate) fn connections_view<'a>(
    state: &'a State,
    settings: &'a Connections,
    i18n: &'a I18n,
) -> Element<'a, Event> {
    let enabled = !state.saving && !state.busy;
    let selected = state.connection.as_ref();
    let tools = row![
        IconButton::Tool.view(
            i18n,
            Icon::Plus,
            Key::DeploymentNewConnection,
            enabled.then_some(Event::BeginName(NameAction::NewConnection))
        ),
        IconButton::Tool.view(
            i18n,
            Icon::Minus,
            Key::DeploymentDelete,
            (enabled && selected.is_some()).then_some(Event::DeleteConnection)
        ),
        IconButton::Tool.view(
            i18n,
            Icon::Duplicate,
            Key::DeploymentDuplicate,
            (enabled && selected.is_some())
                .then_some(Event::BeginName(NameAction::DuplicateConnection))
        ),
        IconButton::Tool.view(
            i18n,
            Icon::Rename,
            Key::DeploymentRename,
            (enabled && selected.is_some())
                .then_some(Event::BeginName(NameAction::RenameConnection))
        ),
    ]
    .spacing(2)
    .into();
    let mut list = column![];
    for connection in &settings.connections {
        let draft = selected.filter(|c| c.id == connection.id);
        list = list.push(list_row(
            draft.map_or(&connection.name, |c| &c.name),
            None,
            draft.is_some(),
            enabled.then(|| Event::EditConnection(connection.id.clone())),
            false,
        ));
    }
    if let Some(draft) =
        selected.filter(|c| !settings.connections.iter().any(|saved| saved.id == c.id))
    {
        list = list.push(list_row(&draft.name, None, true, None, false));
    }
    let body = if selected.is_some() {
        if state.saving {
            text(i18n.text(Key::DeploymentSaving)).into()
        } else {
            connection_form(state, settings, i18n)
        }
    } else {
        column![
            text(i18n.text(Key::DeploymentChooseConnection))
                .size(14)
                .style(theme::MUTED.text()),
            action(
                i18n,
                Key::DeploymentNewConnection,
                Event::BeginName(NameAction::NewConnection)
            )
        ]
        .spacing(12)
        .into()
    };
    let detail = container(
        column![
            scrollable(body).style(theme::scroll).height(Length::Fill),
            container(
                row![
                    icon(Icon::Warning, theme::YELLOW),
                    text(i18n.text(Key::DeploymentConnectionsHelp)).size(13)
                ]
                .spacing(8)
                .align_y(Alignment::Center)
            )
            .width(Length::Fill)
            .padding([10, 12])
            .style(style::warning),
            footer(
                i18n,
                Event::SaveConnection,
                state.connection_dirty(settings),
                state.saving
            )
        ]
        .spacing(16),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
    editor(tools, list, detail)
}

pub(super) fn connection_form<'a>(
    state: &'a State,
    settings: &'a Connections,
    i18n: &'a I18n,
) -> Element<'a, Event> {
    let Some(connection) = &state.connection else {
        return column![].into();
    };
    let mut form = column![
        form_row(
            i18n,
            Key::DeploymentName,
            theme::text_field("", &connection.name).on_input(|v| Event::Input(Field::Name, v))
        ),
        form_row(
            i18n,
            Key::DeploymentProtocol,
            select(protocols(i18n), Some(&connection.protocol), Event::Protocol)
        ),
        form_row(
            i18n,
            Key::DeploymentHost,
            row![
                theme::text_field("", &connection.host).on_input(|v| Event::Input(Field::Host, v)),
                text(i18n.text(Key::DeploymentPort)).size(13),
                container(
                    theme::text_field("", &state.port).on_input(|v| Event::Input(Field::Port, v))
                )
                .width(70)
            ]
            .spacing(8)
            .align_y(Alignment::Center)
        ),
        form_row(
            i18n,
            Key::DeploymentUsername,
            theme::text_field("", &connection.username)
                .on_input(|v| Event::Input(Field::Username, v))
        ),
    ]
    .spacing(12);
    if connection.protocol == Protocol::Sftp {
        let mode = match connection.authentication {
            Authentication::Password => 0,
            Authentication::Key(_) => 1,
            Authentication::SshConfig => 2,
            Authentication::Agent => 3,
        };
        let mut modes = vec![
            (0, i18n.text(Key::DeploymentPassword)),
            (1, i18n.text(Key::DeploymentSshIdentity)),
            (2, i18n.text(Key::DeploymentOpenSshAgent)),
        ];
        if mode == 3 {
            modes.push((3, i18n.text(Key::DeploymentAgent)));
        }
        form = form.push(form_row(
            i18n,
            Key::DeploymentAuthentication,
            select(modes, Some(&mode), |mode| {
                Event::Authentication(match mode {
                    1 => Authentication::Key(String::new()),
                    2 => Authentication::SshConfig,
                    3 => Authentication::Agent,
                    _ => Authentication::Password,
                })
            }),
        ));
        if let Authentication::Key(id) = &connection.authentication {
            form = form.push(form_row(
                i18n,
                Key::DeploymentSshIdentity,
                row![
                    select(
                        settings
                            .keys
                            .iter()
                            .map(|k| (k.id.clone(), k.name.clone()))
                            .collect(),
                        Some(id),
                        |id| Event::Authentication(Authentication::Key(id))
                    ),
                    IconButton::Tool.view(
                        i18n,
                        Icon::Plus,
                        Key::DeploymentAddIdentity,
                        Some(Event::NewKey)
                    ),
                    IconButton::Tool.view(
                        i18n,
                        Icon::Settings,
                        Key::DeploymentKeys,
                        Some(Event::OpenKeys)
                    ),
                ]
                .spacing(6),
            ));
        }
    }
    if !matches!(
        connection.authentication,
        Authentication::Agent | Authentication::SshConfig
    ) {
        let label = if matches!(connection.authentication, Authentication::Key(_)) {
            Key::DeploymentPassphrase
        } else {
            Key::DeploymentPassword
        };
        form = form
            .push(form_row(
                i18n,
                label,
                theme::text_field("", &state.secret.0)
                    .secure(true)
                    .on_input(|v| Event::Password(Secret(zeroize::Zeroizing::new(v)))),
            ))
            .push(
                row![
                    space().width(LABEL_WIDTH),
                    text(i18n.text(Key::DeploymentSecretHelp))
                        .size(12)
                        .style(theme::MUTED.text())
                        .width(Length::Fill)
                ]
                .spacing(12),
            );
    }
    if connection.protocol == Protocol::Sftp {
        form = form.push(
            row![
                space().width(LABEL_WIDTH),
                theme::checkbox(connection.uses_ssh_config())
                    .label(i18n.text(Key::DeploymentUseSshConfig))
                    .text_size(13)
                    .on_toggle_maybe(
                        (connection.authentication != Authentication::SshConfig)
                            .then_some(Event::ParseSshConfig)
                    )
            ]
            .spacing(12),
        );
    }
    if state.operation_label.is_none()
        && let Some(error) = &state.error
    {
        form = form.push(text(i18n.text(error_key(error))).size(13));
    }
    form.push(test_controls(
        state,
        i18n,
        Event::Test,
        Some(connection),
        None,
    ))
    .into()
}

pub(crate) fn profiles_view<'a>(
    state: &'a State,
    settings: &'a Connections,
    spaces: &'a [Space],
    active_space: Option<usize>,
    i18n: &'a I18n,
) -> Element<'a, Event> {
    let selected = state.profile.as_ref();
    let project_name = |profile: &'a Profile, owner: Option<usize>| {
        owner
            .and_then(|index| spaces.get(index))
            .into_iter()
            .chain(spaces.iter())
            .flat_map(|space| &space.projects)
            .find(|project| project.directory == profile.project)
            .map_or_else(
                || profile.project.file_name().and_then(|name| name.to_str()),
                |project| Some(project.name.as_str()),
            )
    };
    let has_projects = spaces.iter().any(|space| !space.projects.is_empty());
    let owner = selected.and_then(|draft| {
        spaces.iter().position(|space| {
            space
                .deployments
                .iter()
                .any(|profile| profile.id == draft.id)
        })
    });
    let destination = state.profile_destination.or(owner).or(active_space);
    let saved = owner.map_or(&[][..], |index| spaces[index].deployments.as_slice());
    let tools = row![
        IconButton::Tool.view(
            i18n,
            Icon::Plus,
            Key::DeploymentNewProfile,
            has_projects.then_some(Event::BeginName(NameAction::NewProfile))
        ),
        IconButton::Tool.view(
            i18n,
            Icon::Minus,
            Key::DeploymentDelete,
            (selected.is_some() && !state.busy).then_some(Event::DeleteProfile)
        ),
        IconButton::Tool.view(
            i18n,
            Icon::Duplicate,
            Key::DeploymentDuplicate,
            selected.map(|_| Event::BeginName(NameAction::DuplicateProfile))
        ),
        IconButton::Tool.view(
            i18n,
            Icon::Rename,
            Key::DeploymentRename,
            selected.map(|_| Event::BeginName(NameAction::RenameProfile))
        ),
    ]
    .spacing(2)
    .into();
    let mut list = column![];
    for (index, space) in spaces.iter().enumerate() {
        for profile in &space.deployments {
            let draft = selected.filter(|draft| draft.id == profile.id);
            let shown = draft.unwrap_or(profile);
            list = list.push(list_row(
                &shown.name,
                project_name(
                    shown,
                    if draft.is_some() {
                        destination
                    } else {
                        Some(index)
                    },
                ),
                draft.is_some(),
                Some(Event::EditProfile(profile.id.clone())),
                shown.default,
            ));
        }
    }
    if let Some(draft) = selected.filter(|_| owner.is_none()) {
        list = list.push(list_row(
            &draft.name,
            project_name(draft, destination),
            true,
            None,
            draft.default,
        ));
    }
    let body = if let Some(profile) = selected {
        profile_form(state, settings, profile, spaces, destination, i18n)
    } else {
        column![
            text(i18n.text(if has_projects {
                Key::DeploymentChooseProfile
            } else {
                Key::DeploymentSelectProject
            }))
            .size(14)
            .style(theme::MUTED.text())
        ]
        .into()
    };
    let detail = container(
        column![
            text(i18n.text(Key::DeploymentProfilesHelp))
                .size(12)
                .width(Length::Fill)
                .style(theme::MUTED.text()),
            profile_tabs(state.profile_tab, i18n),
            scrollable(body).style(theme::scroll).height(Length::Fill),
            footer(i18n, Event::SaveProfile, state.profile_dirty(saved), false)
        ]
        .spacing(12),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .into();
    editor(tools, list, detail)
}

pub(super) fn profile_tabs<'a>(active: ProfileTab, i18n: &I18n) -> Element<'a, Event> {
    let mut tabs = row![];
    for (tab, key, name) in [
        (
            ProfileTab::Connection,
            Key::DeploymentConnection,
            "connection",
        ),
        (ProfileTab::Mappings, Key::DeploymentMappings, "mappings"),
        (
            ProfileTab::Exclusions,
            Key::DeploymentExclusions,
            "exclusions",
        ),
    ] {
        let selected = active == tab;
        let label = container(text(i18n.text(key)).size(13).wrapping(text::Wrapping::None))
            .id(format!("deployment-profile-tab-label-{name}"));
        let tab = button(
            column![
                container(label).padding([9, 12]),
                container(space())
                    .id(format!("deployment-profile-tab-indicator-{name}"))
                    .height(2)
                    .width(Length::Fill)
                    .style(move |theme: &iced::Theme| container::Style {
                        background: selected.then(|| theme::BLUE.resolve(theme).into()),
                        ..Default::default()
                    }),
            ]
            .width(Length::Shrink),
        )
        .padding(0)
        .on_press(Event::ProfileTab(tab))
        .style(theme::action(false));
        tabs = tabs.push(container(tab).id(format!("deployment-profile-tab-{name}")));
    }
    column![
        scrollable(tabs)
            .id("deployment-profile-tabs")
            .direction(scrollable::Direction::Horizontal(
                scrollable::Scrollbar::hidden(),
            ))
            .width(Length::Fill)
            .style(theme::scroll),
        divider(),
    ]
    .spacing(0)
    .width(Length::Fill)
    .into()
}

fn profile_form<'a>(
    state: &'a State,
    settings: &'a Connections,
    profile: &'a Profile,
    spaces: &[Space],
    destination: Option<usize>,
    i18n: &'a I18n,
) -> Element<'a, Event> {
    let mut form = column![].spacing(12);
    match state.profile_tab {
        ProfileTab::Connection => {
            let connection = settings
                .connections
                .iter()
                .find(|connection| connection.id == profile.connection);
            let mut root_controls = row![
                theme::text_field("", &profile.root)
                    .on_input(|value| Event::Input(Field::Root, value)),
                IconButton::Tool.view(
                    i18n,
                    Icon::Folder,
                    Key::DeploymentBrowse,
                    (!state.busy).then_some(Event::BrowseRoot)
                )
            ]
            .align_y(Alignment::Center)
            .spacing(6);
            if connection.is_some_and(|connection| connection.protocol == Protocol::Sftp) {
                root_controls = root_controls.push(
                    container(
                        theme::text_button(i18n.text(Key::DeploymentAutodetectRoot))
                            .on_press_maybe((!state.busy).then_some(Event::AutodetectRoot)),
                    )
                    .id("deployment-autodetect-root"),
                );
            }
            let projects: Vec<_> = spaces
                .iter()
                .enumerate()
                .flat_map(|(index, space)| {
                    space.projects.iter().map(move |project| {
                        (
                            (index, project.directory.clone()),
                            format!("{} / {}", space.name, project.name),
                        )
                    })
                })
                .collect();
            let selected = destination
                .map(|index| (index, profile.project.clone()))
                .filter(|key| projects.iter().any(|(candidate, _)| candidate == key))
                .or_else(|| {
                    projects
                        .iter()
                        .find(|((_, path), _)| path == &profile.project)
                        .map(|(key, _)| key.clone())
                });
            form = form
                .push(form_row(
                    i18n,
                    Key::DeploymentName,
                    theme::text_field("", &profile.name)
                        .on_input(|v| Event::Input(Field::ProfileName, v)),
                ))
                .push(form_row(
                    i18n,
                    Key::DeploymentProject,
                    select(projects, selected.as_ref(), |(index, path)| {
                        Event::ProfileProject(index, path)
                    }),
                ))
                .push(form_row(
                    i18n,
                    Key::DeploymentConnection,
                    row![
                        select(
                            settings
                                .connections
                                .iter()
                                .map(|c| (c.id.clone(), c.name.clone()))
                                .collect(),
                            Some(&profile.connection),
                            Event::ProfileConnection
                        ),
                        IconButton::Tool.view(
                            i18n,
                            Icon::Settings,
                            Key::DeploymentConnections,
                            Some(Event::ShowConnections)
                        )
                    ]
                    .spacing(6),
                ))
                .push(form_row(i18n, Key::DeploymentRoot, root_controls))
                .push(test_controls(
                    state,
                    i18n,
                    Event::TestProfile,
                    connection,
                    Some(&profile.id),
                ))
                .push(
                    theme::switch(profile.default)
                        .label(i18n.text(Key::DeploymentDefault))
                        .on_toggle(Event::DefaultProfile),
                );
        }
        ProfileTab::Mappings => {
            form = form
                .push(form_row(
                    i18n,
                    Key::DeploymentProject,
                    text(profile.project.to_string_lossy().into_owned()).size(12),
                ))
                .push(
                    text(i18n.text(Key::DeploymentMappingHelp))
                        .size(12)
                        .style(theme::MUTED.text()),
                );
            for (index, mapping) in profile.mappings.iter().enumerate() {
                form = form.push(
                    container(
                        column![
                            row![
                                text((index + 1).to_string())
                                    .size(12)
                                    .style(theme::MUTED.text()),
                                space().width(Length::Fill),
                                IconButton::Tool.view(
                                    i18n,
                                    Icon::Close,
                                    Key::DeploymentRemove,
                                    Some(Event::RemoveMapping(index))
                                )
                            ]
                            .align_y(Alignment::Center),
                            form_row(
                                i18n,
                                Key::DeploymentLocalFolder,
                                row![
                                    theme::text_field(
                                        "",
                                        &profile.project.join(&mapping.local).to_string_lossy()
                                    )
                                    .on_input(move |v| { Event::MappingLocal(index, v) }),
                                    IconButton::Tool.view(
                                        i18n,
                                        Icon::Folder,
                                        Key::DeploymentBrowse,
                                        Some(Event::PickLocalMapping(index))
                                    )
                                ]
                                .spacing(8)
                            ),
                            form_row(
                                i18n,
                                Key::DeploymentRemoteFolder,
                                row![
                                    theme::text_field("", &mapping.remote)
                                        .on_input(move |v| Event::MappingRemote(index, v)),
                                    IconButton::Tool.view(
                                        i18n,
                                        Icon::Folder,
                                        Key::DeploymentBrowse,
                                        (!state.busy).then_some(Event::BrowseMapping(index))
                                    )
                                ]
                                .spacing(8)
                            ),
                        ]
                        .spacing(8),
                    )
                    .padding(12)
                    .width(Length::Fill)
                    .style(theme::sidebar),
                );
            }
            form = form.push(action(i18n, Key::DeploymentAddMapping, Event::AddMapping));
        }
        ProfileTab::Exclusions => {
            form = form.push(
                text(i18n.text(Key::DeploymentExclusionsHelp))
                    .size(12)
                    .style(theme::MUTED.text()),
            );
            for (index, path) in profile.excluded.iter().enumerate() {
                form = form.push(
                    row![
                        theme::text_field("", &path.to_string_lossy())
                            .on_input(move |v| Event::Exclusion(index, v)),
                        IconButton::Tool.view(
                            i18n,
                            Icon::Close,
                            Key::DeploymentRemove,
                            Some(Event::RemoveExclusion(index))
                        )
                    ]
                    .spacing(8),
                );
            }
            form = form.push(action(
                i18n,
                Key::DeploymentAddExclusion,
                Event::AddExclusion,
            ));
        }
    }
    if let Some(error @ (Error::Configuration | Error::Mapping)) = &state.error {
        form = form.push(text(i18n.text(error_key(error))).size(13));
    }
    form.into()
}

pub(super) fn keys_view<'a>(
    state: &'a State,
    settings: &'a Connections,
    i18n: &'a I18n,
) -> Element<'a, Event> {
    let mut list = column![
        text(i18n.text(Key::DeploymentKeysHelp))
            .size(12)
            .style(theme::MUTED.text())
    ]
    .spacing(12);
    for key in &settings.keys {
        list = list.push(
            row![
                column![
                    text(&key.name).size(13),
                    text(key.path.to_string_lossy().into_owned())
                        .size(12)
                        .style(theme::MUTED.text())
                ]
                .width(Length::Fill),
                IconButton::Tool.view(
                    i18n,
                    Icon::Rename,
                    Key::DeploymentEdit,
                    Some(Event::EditKey(key.id.clone()))
                ),
                IconButton::Tool.view(
                    i18n,
                    Icon::Close,
                    Key::DeploymentDelete,
                    Some(Event::DeleteKey(key.id.clone()))
                )
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        );
    }
    if let Some(error @ Error::Configuration) = &state.error {
        list = list.push(text(i18n.text(error_key(error))).size(13));
    }
    list.push(action(i18n, Key::DeploymentAddIdentity, Event::NewKey))
        .push(
            modal::actions().push(
                theme::text_button(i18n.text(Key::DeploymentClose)).on_press(Event::CloseDialog),
            ),
        )
        .into()
}

pub(crate) fn settings_dialog<'a>(
    state: &'a State,
    settings: &'a Connections,
    i18n: &'a I18n,
) -> Option<Element<'a, Event>> {
    let (title, content): (Key, Element<'a, Event>) = if state.pending.is_some() {
        (
            Key::DeploymentUnsaved,
            column![
                text(i18n.text(Key::DeploymentDiscardHelp)).size(13),
                modal::actions()
                    .push(
                        theme::text_button(i18n.text(Key::DeploymentCancel))
                            .on_press(Event::CloseDialog)
                    )
                    .push(
                        theme::text_button(i18n.text(Key::DeploymentDiscard))
                            .style(theme::danger)
                            .on_press(Event::DiscardChanges)
                    )
            ]
            .spacing(16)
            .into(),
        )
    } else if state.browsing {
        let selected = state
            .selected
            .as_ref()
            .filter(|entry| entry.directory && !entry.symlink);
        let mut browser = column![
            container(scrollable(tree(state, state.browsing_root(), 0)).style(theme::scroll))
                .id("deployment-folder-tree")
                .height(320)
                .width(Length::Fill),
        ]
        .spacing(12);
        if state.busy {
            browser = browser.push(text(i18n.text(Key::Loading)).size(12));
        }
        if let Some(error) = &state.error {
            browser = browser.push(
                text(i18n.text(super::error_key(error)))
                    .size(12)
                    .style(theme::RED.text()),
            );
        }
        browser = browser.push(
            modal::actions()
                .push(
                    theme::text_button(i18n.text(Key::DeploymentCancel))
                        .on_press(Event::StopBrowsing),
                )
                .push(
                    theme::text_button(i18n.text(Key::DeploymentUseRoot))
                        .on_press(Event::UseRoot(state.browsing_root().to_owned())),
                )
                .push(
                    theme::text_button(i18n.text(Key::DeploymentUseFolder))
                        .style(theme::primary)
                        .on_press_maybe(selected.map(|entry| Event::UseRoot(entry.path.clone()))),
                ),
        );
        (Key::DeploymentBrowse, browser.into())
    } else if let Some(name_action) = state.naming {
        let mut content = column![form_row(
            i18n,
            Key::DeploymentName,
            theme::text_field("", &state.name_value)
                .on_input(|v| Event::Input(Field::DraftName, v))
        )]
        .spacing(12);
        if name_action == NameAction::NewConnection {
            content = content.push(form_row(
                i18n,
                Key::DeploymentProtocol,
                select(
                    protocols(i18n),
                    Some(&state.new_protocol),
                    Event::NewProtocol,
                ),
            ));
        }
        if let Some(error) = &state.error {
            content = content.push(
                text(i18n.text(super::error_key(error)))
                    .size(12)
                    .style(theme::RED.text()),
            );
        }
        content = content.push(
            modal::actions()
                .push(
                    theme::text_button(i18n.text(Key::DeploymentCancel))
                        .on_press(Event::CloseDialog),
                )
                .push(
                    theme::text_button(i18n.text(Key::DeploymentContinue))
                        .style(theme::primary)
                        .on_press_maybe(
                            (!state.name_value.trim().is_empty() && safe_text(&state.name_value))
                                .then_some(Event::ConfirmName),
                        ),
                ),
        );
        let title = match name_action {
            NameAction::NewConnection => Key::DeploymentNewConnection,
            NameAction::NewProfile => Key::DeploymentNewProfile,
            NameAction::DuplicateConnection | NameAction::DuplicateProfile => {
                Key::DeploymentDuplicate
            }
            _ => Key::DeploymentRename,
        };
        (title, content.into())
    } else if state.key_dialog {
        let mut content = column![
            text(i18n.text(Key::DeploymentKeysHelp))
                .size(12)
                .style(theme::MUTED.text()),
            form_row(
                i18n,
                Key::DeploymentName,
                theme::text_field("", &state.key_name)
                    .on_input(|v| Event::Input(Field::KeyName, v))
            ),
            form_row(
                i18n,
                Key::DeploymentKeyPath,
                row![
                    theme::text_field("", &state.key_path)
                        .on_input(|v| Event::Input(Field::KeyPath, v)),
                    IconButton::Tool.view(
                        i18n,
                        Icon::Folder,
                        Key::DeploymentChooseFile,
                        Some(Event::PickKey)
                    )
                ]
                .spacing(6)
            ),
        ]
        .spacing(12);
        if let Some(error) = &state.key_error {
            content = content.push(
                text(i18n.text(if *error == Error::Configuration {
                    Key::DeploymentKeyError
                } else {
                    error_key(error)
                }))
                .size(13),
            );
        }
        content = content.push(
            modal::actions()
                .push(
                    theme::text_button(i18n.text(Key::DeploymentCancel))
                        .on_press(Event::CloseDialog),
                )
                .push(
                    theme::text_button(i18n.text(Key::DeploymentSave))
                        .style(theme::primary)
                        .on_press(Event::AddKey),
                ),
        );
        (
            if state.key_editing.is_some() {
                Key::DeploymentSshIdentity
            } else {
                Key::DeploymentAddIdentity
            },
            content.into(),
        )
    } else if state.keys_open {
        (Key::DeploymentKeys, keys_view(state, settings, i18n))
    } else {
        return None;
    };
    Some(
        modal::content(i18n.text(title))
            .push(container(scrollable(content).style(theme::scroll)).max_height(480))
            .into(),
    )
}

pub(super) fn profile_selector<'a>(
    profiles: &'a [Profile],
    selected: Option<&Profile>,
    project: Option<&Path>,
) -> Element<'a, Event> {
    select(
        profiles
            .iter()
            .filter(|p| Some(p.project.as_path()) == project)
            .map(|p| (p.id.clone(), p.name.clone()))
            .collect(),
        selected.map(|p| &p.id),
        Event::SelectProfile,
    )
}
