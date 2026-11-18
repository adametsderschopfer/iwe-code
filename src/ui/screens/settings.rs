use super::workspace::divider;
use crate::{
    app::{App, Message, SettingsTab},
    domain::{shortcuts::Command, workspace::Appearance},
    i18n::{Key, Language},
    ui::theme,
};
use iced::{
    Alignment, Element, Length,
    widget::{button, column, container, pick_list, row, scrollable, slider, text, toggler},
};

#[derive(Clone, PartialEq, Eq)]
struct SelectOption<T> {
    value: T,
    label: String,
}

impl<T> std::fmt::Display for SelectOption<T> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.label)
    }
}

pub(crate) fn view(app: &App) -> Element<'_, Message> {
    view_for(app, None)
}

pub(crate) fn deployment_view(app: &App, connections: bool) -> Element<'_, Message> {
    view_for(app, Some(connections))
}

fn view_for(app: &App, deployment: Option<bool>) -> Element<'_, Message> {
    let selected = match deployment {
        Some(true) => SettingsTab::Connections,
        Some(false) => SettingsTab::Deployment,
        None => app.settings_tab,
    };
    let t = |key| app.i18n.text(key);
    let preferences = &app.workspace.preferences;
    let mut tabs = column![].spacing(4);
    let sections = [
        (SettingsTab::General, Key::General, "general"),
        (SettingsTab::Code, Key::Code, "code"),
        (SettingsTab::Terminal, Key::Terminal, "terminal"),
        (
            SettingsTab::LanguageServers,
            Key::LanguageServers,
            "languages",
        ),
        (SettingsTab::Modules, Key::Modules, "modules"),
        (
            SettingsTab::Connections,
            Key::DeploymentConnections,
            "connections",
        ),
        (
            SettingsTab::Deployment,
            Key::DeploymentProfiles,
            "deployment",
        ),
        (SettingsTab::Shortcuts, Key::KeyboardShortcuts, "shortcuts"),
        (SettingsTab::About, Key::About, "about"),
    ];
    for &(tab, key, id) in &sections {
        if matches!(tab, SettingsTab::Connections | SettingsTab::Deployment) {
            continue;
        }
        let group = match tab {
            SettingsTab::General => Some(Key::SettingsAppearanceGroup),
            SettingsTab::Code => Some(Key::SettingsEditingGroup),
            SettingsTab::Modules => Some(Key::SettingsToolsGroup),
            SettingsTab::Shortcuts => Some(Key::SettingsApplicationGroup),
            _ => None,
        };
        if let Some(group) = group {
            tabs = tabs.push(
                container(text(t(group)).size(11).style(theme::MUTED.text())).padding(
                    iced::Padding {
                        top: 14.,
                        right: 12.,
                        bottom: 4.,
                        left: 12.,
                    },
                ),
            );
        }
        tabs = tabs.push(
            container(
                button(text(t(key)).size(13))
                    .padding([8, 12])
                    .width(Length::Fill)
                    .on_press(Message::SelectSettingsTab(tab))
                    .style(theme::action(selected == tab)),
            )
            .id(format!("settings-tab-{id}")),
        );
    }
    let section: Element<'_, Message> = match selected {
        SettingsTab::General => {
            let languages = [
                (Language::English, Key::LanguageEnglish),
                (Language::Russian, Key::LanguageRussian),
                (Language::Spanish, Key::LanguageSpanish),
            ]
            .map(|(value, key)| SelectOption {
                value,
                label: t(key),
            });
            let selected = languages
                .iter()
                .find(|option| option.value == app.i18n.language())
                .cloned();
            column![
                group_heading(t(Key::SettingsAppearanceGroup)),
                setting(
                    t(Key::Language),
                    t(Key::LanguageDescription),
                    container(
                        pick_list(languages, selected, |option| Message::SetLanguage(
                            option.value
                        ))
                        .text_size(13)
                        .padding([8, 12])
                        .width(180)
                        .style(theme::select)
                        .menu_style(theme::select_menu)
                    )
                    .id("settings-language-select")
                    .into()
                ),
                setting(
                    t(Key::WindowZoom),
                    t(Key::WindowZoomDescription),
                    row![
                        container(
                            slider(
                                75..=200,
                                app.window_zoom_preview(),
                                Message::PreviewWindowZoom
                            )
                            .on_release(Message::ApplyWindowZoom)
                            .scroll_with_wheel(false)
                            .step(5_u16)
                            .width(180)
                            .style(theme::font_slider)
                        )
                        .id("settings-window-zoom"),
                        text(format!("{}%", app.window_zoom_preview()))
                            .size(13)
                            .width(48),
                    ]
                    .spacing(16)
                    .align_y(Alignment::Center)
                    .into()
                ),
                setting(
                    t(Key::Appearance),
                    t(Key::AppearanceDescription),
                    row([Appearance::Dark, Appearance::Light].map(|appearance| {
                        button(
                            text(t(match appearance {
                                Appearance::Dark => Key::Dark,
                                Appearance::Light => Key::Light,
                            }))
                            .size(13),
                        )
                        .padding([8, 14])
                        .on_press(Message::SetAppearance(appearance))
                        .style(theme::action(preferences.appearance == appearance))
                        .into()
                    }))
                    .spacing(6)
                    .into()
                ),
            ]
            .spacing(0)
            .into()
        }
        SettingsTab::Code => column![
            group_heading(t(Key::SettingsTypographyGroup)),
            typography(app, false),
            setting(
                t(Key::FontSize),
                t(Key::FontSizeDescription),
                row![
                    slider(10..=32, preferences.code_size, Message::SetCodeSize)
                        .width(180)
                        .style(theme::font_slider),
                    text(format!("{} px", preferences.code_size))
                        .size(13)
                        .width(48),
                ]
                .spacing(16)
                .align_y(Alignment::Center)
                .into()
            ),
            group_heading(t(Key::SettingsEditingGroup)),
            setting(
                t(Key::LineNumbers),
                t(Key::LineNumbersDescription),
                toggler(preferences.line_numbers)
                    .style(theme::toggle)
                    .on_toggle(Message::SetLineNumbers)
                    .into()
            ),
            setting(
                t(Key::RestoreProjectTabs),
                t(Key::RestoreProjectTabsDescription),
                toggler(preferences.restore_project_tabs)
                    .style(theme::toggle)
                    .on_toggle(Message::SetRestoreProjectTabs)
                    .into()
            ),
        ]
        .spacing(0)
        .into(),
        SettingsTab::Terminal => column![
            group_heading(t(Key::SettingsTypographyGroup)),
            typography(app, true),
            setting(
                t(Key::FontSize),
                t(Key::FontSizeDescription),
                row![
                    slider(10..=32, preferences.terminal_size, Message::SetTerminalSize)
                        .width(180)
                        .style(theme::font_slider),
                    text(format!("{} px", preferences.terminal_size))
                        .size(13)
                        .width(48),
                ]
                .spacing(16)
                .align_y(Alignment::Center)
                .into()
            ),
        ]
        .into(),
        SettingsTab::LanguageServers => language_servers(app),
        SettingsTab::Modules => iced::widget::column![
            crate::ui::modules::settings(app),
            crate::ui::modules::external_settings(app),
        ]
        .spacing(16)
        .into(),
        SettingsTab::Connections => crate::modules::deployment::connections_view(
            &app.modules.deployment,
            &app.workspace.deployment,
            &app.i18n,
        )
        .map(crate::app::modules::deployment_message),
        SettingsTab::Deployment => crate::modules::deployment::profiles_view(
            &app.modules.deployment,
            &app.workspace.deployment,
            &app.workspace.spaces,
            app.workspace.selected_space,
            &app.i18n,
        )
        .map(crate::app::modules::deployment_message),
        SettingsTab::Shortcuts => column![
            shortcut_group(
                t(Key::Code),
                "settings-shortcuts-code",
                vec![
                    setting(
                        t(Key::GoToLine),
                        t(Key::GoToLineDescription),
                        shortcut_control(app, Command::GoToLine),
                    ),
                    setting(
                        t(Key::GoToDefinition),
                        t(Key::GoToDefinitionDescription),
                        shortcut_control(app, Command::GoToDefinition),
                    ),
                    setting(
                        t(Key::FormatCode),
                        t(Key::FormatCodeDescription),
                        shortcut_control(app, Command::FormatCode),
                    ),
                    setting(
                        t(Key::DuplicateLine),
                        t(Key::DuplicateLineDescription),
                        shortcut_control(app, Command::DuplicateLine),
                    ),
                    setting(
                        t(Key::GoToFile),
                        t(Key::GoToFileDescription),
                        shortcut_control(app, Command::GoToFile),
                    ),
                    setting(
                        t(Key::FindFile),
                        t(Key::FindFile),
                        shortcut_control(app, Command::Search),
                    ),
                    setting(
                        t(Key::FindFiles),
                        t(Key::FindFiles),
                        shortcut_control(app, Command::FindInFiles),
                    ),
                    setting(
                        t(Key::EditorSave),
                        t(Key::EditorSaveDescription),
                        shortcut_control(app, Command::SaveFile),
                    ),
                    setting(
                        t(Key::CopyCodeSelection),
                        t(Key::CopyCodeSelectionDescription),
                        shortcut_control(app, Command::Copy),
                    ),
                    setting(
                        t(Key::SelectAllCode),
                        t(Key::SelectAllCodeDescription),
                        shortcut_control(app, Command::SelectAll),
                    ),
                ],
            ),
            shortcut_group(
                t(Key::ShortcutTabs),
                "settings-shortcuts-tabs",
                vec![
                    setting(
                        t(Key::CloseTab),
                        t(Key::CloseActiveTabDescription),
                        shortcut_control(app, Command::CloseTab),
                    ),
                    setting(
                        t(Key::ReopenClosedTab),
                        t(Key::ReopenClosedTabDescription),
                        shortcut_control(app, Command::ReopenTab),
                    ),
                ],
            ),
            shortcut_group(
                t(Key::Deployment),
                "settings-shortcuts-deployment",
                vec![
                    setting(
                        t(Key::DeploymentCompare),
                        String::new(),
                        shortcut_control(app, Command::DeploymentCompare)
                    ),
                    setting(
                        t(Key::DeploymentUpload),
                        String::new(),
                        shortcut_control(app, Command::DeploymentUpload)
                    ),
                ]
            ),
        ]
        .spacing(24)
        .into(),
        SettingsTab::About => column![
            setting(
                t(Key::AppTitle),
                t(Key::AboutDescription),
                text(env!("CARGO_PKG_VERSION"))
                    .size(13)
                    .style(theme::MUTED.text())
                    .into(),
            ),
            crate::ui::updates::settings(app)
        ]
        .spacing(24)
        .into(),
    };
    let heading = sections
        .iter()
        .find(|(tab, _, _)| *tab == selected)
        .map_or(Key::General, |(_, key, _)| *key);
    let navigation = container(
        scrollable(tabs.padding(12))
            .style(theme::scroll)
            .height(Length::Fill),
    )
    .id("settings-navigation")
    .width(theme::SETTINGS_SIDEBAR_WIDTH)
    .height(Length::Fill)
    .style(theme::sidebar);
    let body: Element<'_, Message> = if deployment.is_some() {
        container(section)
            .id("settings-content")
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    } else {
        scrollable(
            container(
                column![
                    section,
                    text(t(Key::SettingsSaved))
                        .size(12)
                        .style(theme::MUTED.text())
                ]
                .spacing(20)
                .width(Length::Fill)
                .max_width(theme::SETTINGS_CONTENT_WIDTH),
            )
            .padding(theme::SETTINGS_CONTENT_INSET)
            .width(Length::Fill),
        )
        .id("settings-content")
        .style(theme::scroll)
        .height(Length::Fill)
        .into()
    };
    let content = column![
        container(text(t(heading)).size(20).font(theme::semibold()))
            .id("settings-section-heading")
            .padding([16., theme::SETTINGS_CONTENT_INSET]),
        divider(),
        body,
    ]
    .height(Length::Fill)
    .width(Length::Fill);
    let (base, window) = match deployment {
        Some(true) => (content.into(), app.windows.connections),
        Some(false) => (content.into(), app.windows.deployment),
        None => (
            row![navigation, super::workspace::vertical_divider(), content]
                .height(Length::Fill)
                .width(Length::Fill)
                .into(),
            app.windows.settings,
        ),
    };
    let base = if deployment.is_some() || app.shortcuts.recording.is_none() {
        let dismiss = window.map_or(Message::Navigate(crate::app::Navigation::Projects), |id| {
            Message::WindowShortcut(id, Box::new(Message::DismissDialog))
        });
        crate::ui::context_menu::dismiss_on_escape(base, dismiss)
    } else {
        base
    };
    if deployment.is_some_and(|connections| app.modules.deployment.has_settings_dialog(connections))
        && let Some(dialog) = crate::modules::deployment::settings_dialog(
            &app.modules.deployment,
            &app.workspace.deployment,
            &app.i18n,
        )
    {
        super::modal::overlay(
            base,
            dialog.map(crate::app::modules::deployment_message),
            640.,
            crate::app::modules::deployment_message(crate::modules::deployment::Event::CloseDialog),
        )
    } else {
        base
    }
}

fn language_servers(app: &App) -> Element<'_, Message> {
    use crate::domain::language::Language as DocumentLanguage;
    let t = |key| app.i18n.text(key);
    let mut rows = column![
        text(t(Key::LanguageServersDescription))
            .size(13)
            .style(theme::MUTED.text()),
        divider(),
    ];
    let web = "https://github.com/hrsh7th/vscode-langservers-extracted";
    for (language, name, install) in [
        (
            DocumentLanguage::Python,
            "Python",
            "https://microsoft.github.io/pyright/#/installation",
        ),
        (DocumentLanguage::Go, "Go", "https://go.dev/gopls/"),
        (
            DocumentLanguage::TypeScript,
            "JavaScript / TypeScript",
            "https://github.com/typescript-language-server/typescript-language-server#installing",
        ),
        (
            DocumentLanguage::Php,
            "PHP",
            "https://intelephense.com/docs",
        ),
        (
            DocumentLanguage::Rust,
            "Rust",
            "https://rust-analyzer.github.io/book/installation.html",
        ),
        (
            DocumentLanguage::Markdown,
            "Markdown",
            "https://github.com/artempyanykh/marksman#how-to-install",
        ),
        (DocumentLanguage::Json, "JSON", web),
        (
            DocumentLanguage::Yaml,
            "YAML",
            "https://github.com/redhat-developer/yaml-language-server",
        ),
        (
            DocumentLanguage::Toml,
            "TOML",
            "https://github.com/tamasfe/taplo/releases",
        ),
        (
            DocumentLanguage::Shell,
            "Shell",
            "https://github.com/bash-lsp/bash-language-server#installation",
        ),
        (DocumentLanguage::Html, "HTML", web),
        (DocumentLanguage::Css, "CSS", web),
    ] {
        let Some((program, _)) = language.server_command() else {
            continue;
        };
        let id = language.id();
        let configured = app.workspace.preferences.language_servers.get(id);
        let status = match app.editor.server_checks.get(id) {
            Some(true) => Some(Key::LanguageServerFound),
            Some(false) => Some(Key::LanguageServerMissing),
            None => None,
        };
        let detail = configured.map_or(program, String::as_str);
        let action = |key, message, name| {
            container(
                button(text(t(key)).size(12))
                    .on_press(message)
                    .padding([6, 9])
                    .style(theme::action(false)),
            )
            .id(format!("language-server-{name}-{id}"))
        };
        let mut actions = row![
            action(
                Key::LanguageServerDetect,
                Message::DetectLanguageServer(language),
                "detect"
            ),
            action(
                Key::LanguageServerChoosePath,
                Message::ChooseLanguageServer(language),
                "path"
            ),
            action(
                Key::LanguageServerInstall,
                Message::OpenUrl(install.to_owned()),
                "install"
            ),
        ]
        .spacing(4)
        .align_y(Alignment::Center);
        if configured.is_some() {
            actions = actions.push(action(
                Key::LanguageServerAutomatic,
                Message::ResetLanguageServer(language),
                "automatic",
            ));
        }
        let mut description = column![
            text(name).size(13).font(theme::semibold()),
            container(
                text(detail)
                    .size(12)
                    .style(theme::MUTED.text())
                    .wrapping(text::Wrapping::None)
            )
            .width(Length::Fill)
            .clip(true),
        ]
        .spacing(3)
        .width(Length::Fill);
        if let Some(status) = status {
            description = description.push(text(t(status)).size(12).style(theme::MUTED.text()));
        }
        rows = rows
            .push(
                container(
                    row![description, actions]
                        .spacing(12)
                        .align_y(Alignment::Center),
                )
                .padding([10, 0])
                .width(Length::Fill),
            )
            .push(divider());
    }
    rows.into()
}

fn group_heading<'a>(title: String) -> Element<'a, Message> {
    container(text(title).size(12).style(theme::MUTED.text()))
        .padding(iced::Padding {
            top: 4.,
            right: 0.,
            bottom: 4.,
            left: 0.,
        })
        .into()
}

fn typography(app: &App, terminal: bool) -> Element<'_, Message> {
    use crate::domain::workspace::CodeFont;
    let prefs = &app.workspace.preferences;
    let (font, height) = if terminal {
        (prefs.terminal_font, prefs.terminal_line_height)
    } else {
        (prefs.code_font, prefs.code_line_height)
    };
    let choices = [
        (CodeFont::System, app.i18n.text(Key::SystemFont)),
        (CodeFont::JetBrainsMono, "JetBrains Mono".to_owned()),
        (CodeFont::FiraCode, "Fira Code".to_owned()),
    ]
    .map(|(value, label)| SelectOption { value, label });
    let selected = choices.iter().find(|option| option.value == font).cloned();
    column![
        setting(
            app.i18n.text(Key::FontFamily),
            app.i18n.text(Key::FontFamilyDescription),
            container(
                pick_list(choices, selected, move |option| Message::SetFont(
                    terminal,
                    option.value
                ))
                .text_size(13)
                .padding([8, 12])
                .width(180)
                .style(theme::select)
                .menu_style(theme::select_menu)
            )
            .id("settings-font-family-select")
            .into()
        ),
        setting(
            app.i18n.text(Key::LineSpacing),
            app.i18n.text(Key::LineSpacingDescription),
            row![
                slider(110..=220, height, move |height| Message::SetLineHeight(
                    terminal, height
                ))
                .step(5_u16)
                .width(180)
                .style(theme::font_slider),
                text(format!("{:.2}", f32::from(height) / 100.))
                    .size(13)
                    .width(48),
            ]
            .spacing(16)
            .align_y(Alignment::Center)
            .into()
        ),
    ]
    .into()
}

fn shortcut_group<'a>(
    title: String,
    id: &'static str,
    shortcuts: Vec<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(
        column![
            text(title).size(14).font(theme::semibold()),
            column(shortcuts).spacing(0),
        ]
        .spacing(8),
    )
    .id(id)
    .width(Length::Fill)
    .into()
}

fn shortcut_control(app: &App, command: Command) -> Element<'_, Message> {
    use crate::app::shortcuts::Event;
    let label = app.workspace.preferences.shortcuts.binding(command).label();
    let mut content = column![
        button(text(label).size(13))
            .padding([8, 14])
            .on_press(Message::Shortcut(Event::Record(command)))
            .style(theme::action(false)),
    ]
    .spacing(8);
    if app.workspace.preferences.shortcuts.conflict(command) {
        content = content.push(text(app.i18n.text(Key::ShortcutConflict)).size(13));
    }
    container(content)
        .id(format!("shortcut-{command:?}"))
        .into()
}

pub(crate) fn shortcut_recording_overlay<'a>(
    app: &'a App,
    base: Element<'a, Message>,
) -> Element<'a, Message> {
    if app.shortcuts.recording.is_none() {
        return base;
    }
    let cancel = Message::Shortcut(crate::app::shortcuts::Event::Cancel);
    let mut content = column![
        text(app.i18n.text(Key::ShortcutRecording))
            .size(20)
            .font(theme::semibold()),
        text(app.i18n.text(Key::ShortcutInstructions))
            .size(13)
            .style(theme::MUTED.text()),
    ]
    .spacing(14);
    if let Some(error) = app.shortcuts.error {
        content = content.push(text(app.i18n.text(error)).size(13).style(theme::RED.text()));
    }
    content = content.push(row![
        iced::widget::space().width(Length::Fill),
        container(
            button(text(app.i18n.text(Key::Cancel)).size(13))
                .on_press(cancel.clone())
                .padding([8, 14])
                .style(theme::action(false))
        )
        .id("shortcut-recording-cancel"),
    ]);
    super::modal::overlay(
        base,
        container(content).id("shortcut-recording-dialog").into(),
        440.,
        cancel,
    )
}

pub(crate) fn setting<'a>(
    title: String,
    description: String,
    control: Element<'a, Message>,
) -> Element<'a, Message> {
    let mut label = column![text(title).size(14)].spacing(4);
    if !description.is_empty() {
        label = label.push(
            text(description)
                .size(13)
                .line_height(1.3)
                .style(theme::MUTED.text()),
        );
    }
    column![
        row![
            label.width(Length::Fill),
            container(control)
                .width(theme::SETTINGS_CONTROL_WIDTH)
                .align_x(iced::alignment::Horizontal::Right),
        ]
        .spacing(24)
        .align_y(Alignment::Start)
        .padding([14, 0]),
        divider(),
    ]
    .width(Length::Fill)
    .into()
}
