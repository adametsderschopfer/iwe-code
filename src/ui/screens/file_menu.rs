//! Show local file commands and their confirmation dialogs.

use crate::ui::modal;
use crate::ui::theme::iced;
use crate::{
    app::{
        App, Message,
        files::{
            Event, PathFormat, Submenu,
            operations::{self, Command},
        },
    },
    i18n::Key,
    ui::{context_menu, ellipsis, file_icons, theme},
};
use iced::{
    Alignment, Element, Length,
    widget::{column, row, scrollable, text},
};
use std::path::Path;

pub(super) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(menu) = &app.files.menu else {
        return base;
    };
    let relative_key = if app.path_repository_root(&menu.path).is_some() {
        Key::CopyRepositoryPath
    } else {
        Key::CopyProjectPath
    };
    let protected = app.file_source_protected(&menu.path);
    let is_root = app.workspace.directory() == Some(&menu.path);
    let terminal = app.module_enabled(crate::modules::ModuleId::Terminal);
    let deployment = !is_root && app.module_enabled(crate::modules::ModuleId::Deployment);
    let parent = context_menu::overlay_with(
        base,
        menu.point,
        move |available| {
            let width = (250. + context_menu::ICON_COLUMN_WIDTH).min(available);
            let action =
                |key, event| context_menu::action(key, app.i18n.text(key)).on_press_maybe(event);
            let command = |command| Some(operations::message(operations::Event::Command(command)));
            let arrow = |key, kind: Submenu, press| {
                context_menu::branch(
                    kind.id(),
                    key,
                    app.i18n.text(key),
                    menu.submenu.is_some_and(|(active, _)| active == kind),
                    Some(Message::Files(Event::Options(kind))),
                    Some(press),
                )
            };
            let main_path = arrow(
                Key::CopyPathMenu,
                Submenu::CopyPath,
                Message::Files(Event::CopyPath(PathFormat::Absolute)),
            );
            let deployment_action = |key, compare| {
                context_menu::action(key, app.i18n.text(key)).on_press(
                    crate::app::modules::deployment_message(
                        crate::modules::deployment::Event::Run(if compare {
                            crate::modules::deployment::Operation::Compare(menu.path.clone())
                        } else {
                            crate::modules::deployment::Operation::Upload(menu.path.clone())
                        }),
                    ),
                )
            };
            let create = Message::Files(Event::Options(Submenu::New));
            let mut menu_items = column![arrow(Key::FileNew, Submenu::New, create)];
            if !protected {
                menu_items = menu_items
                    .push(action(Key::FileDuplicate, command(Command::Duplicate)))
                    .push(context_menu::separator())
                    .push(action(Key::FileCopy, command(Command::Copy)))
                    .push(action(Key::FileCut, command(Command::Cut)));
            }
            let mut main = menu_items.push(action(
                Key::FilePaste,
                command(Command::Paste).filter(|_| app.files.operations.can_paste()),
            ));
            if !protected {
                main = main
                    .push(context_menu::separator())
                    .push(action(Key::FileRename, command(Command::Rename)))
                    .push(action(Key::FileMove, command(Command::Move)))
                    .push(action(Key::FileTrash, command(Command::Trash)));
            }
            main = main
                .push(context_menu::separator())
                .push(main_path)
                .push(action(Key::FileReveal, command(Command::Reveal)));
            if terminal {
                main = main.push(action(
                    Key::FileOpenTerminal,
                    command(Command::OpenTerminal),
                ));
            }
            if !is_root {
                main = main.push(crate::ui::explain::submenu(
                    &app.i18n,
                    crate::domain::explain::Target::File(menu.path.clone()),
                    crate::ui::explain::choose,
                ));
            }
            if deployment {
                main = main
                    .push(context_menu::separator())
                    .push(deployment_action(Key::DeploymentCompare, true))
                    .push(deployment_action(Key::DeploymentUpload, false));
            }
            context_menu::surface(
                scrollable(main)
                    .style(theme::scroll)
                    .height(Length::Shrink)
                    .into(),
                width,
            )
        },
        Message::Files(Event::DismissMenu),
    );
    let active = menu
        .submenu
        .and_then(|(kind, anchor)| Some((anchor?, Message::Files(Event::CloseOptions(kind)))));
    context_menu::beside(
        parent,
        active,
        300.,
        move || {
            let action = |key, cmd| {
                context_menu::action(key, app.i18n.text(key))
                    .on_press(operations::message(operations::Event::Command(cmd)))
            };
            let option = |key, format| {
                context_menu::text_row(app.i18n.text(key))
                    .on_press(Message::Files(Event::CopyPath(format)))
            };
            let children = if matches!(menu.submenu, Some((Submenu::New, _))) {
                let mut create = column![
                    action(Key::FileNewFile, Command::CreateFile),
                    action(Key::FileNewFolder, Command::CreateFolder),
                    context_menu::separator(),
                ];
                for kind in app.new_file_kinds(&menu.path) {
                    let (key, name) = kind.preset();
                    let label = row![
                        file_icons::file(Path::new(name)),
                        ellipsis::text(app.i18n.text(key), context_menu::TEXT_SIZE),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center);
                    create = create.push(context_menu::row(label).on_press(operations::message(
                        operations::Event::Command(Command::CreateTypedFile(kind)),
                    )));
                }
                create
            } else {
                column![
                    option(Key::CopyAbsolutePath, PathFormat::Absolute),
                    option(relative_key, PathFormat::Relative),
                ]
            }
            .spacing(4);
            scrollable(children)
                .style(theme::scroll)
                .height(Length::Shrink)
                .into()
        },
        true,
    )
}

pub(super) fn dialog<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(draft) = &app.files.operations.draft else {
        return base;
    };
    let msg = operations::message;
    let busy = app.files.operations.busy || app.editor.pending.is_some();
    let mut content = modal::content(app.i18n.text(draft.title));
    if let Some(parent) = &draft.parent {
        let root = app.workspace.directory().map(|root| root.as_path());
        let folder = root
            .and_then(|root| parent.strip_prefix(root).ok())
            .unwrap_or(parent);
        if !folder.as_os_str().is_empty() {
            content = content.push(text(folder.to_string_lossy()).size(13));
        }
        content = content.push(
            theme::text_field(&app.i18n.text(Key::FileName), &draft.name)
                .id(operations::NAME_INPUT)
                .style(theme::validated_input(matches!(
                    draft.error,
                    Some(Key::FileInvalidName | Key::FileNameConflict)
                )))
                .on_input_maybe(
                    (!busy).then_some(|name| operations::message(operations::Event::Name(name))),
                )
                .on_submit_maybe((!busy).then(|| msg(operations::Event::Confirm))),
        );
    } else {
        let mut args = fluent_bundle::FluentArgs::new();
        args.set("name", draft.name.as_str());
        content = content.push(text(app.i18n.format(Key::FileTrashPrompt, &args)));
    }
    if let Some(error) = draft.error {
        content = content.push(text(app.i18n.text(error)).size(13).style(theme::RED.text()));
    }
    if busy {
        content = content.push(text(app.i18n.text(Key::FileBusy)).size(13));
    }
    content = content.push(
        modal::actions()
            .push(
                theme::text_button(app.i18n.text(Key::Cancel))
                    .on_press_maybe((!busy).then(|| msg(operations::Event::Cancel))),
            )
            .push(
                theme::text_button(app.i18n.text(draft.title))
                    .style(if draft.title == Key::FileTrash {
                        theme::danger
                    } else {
                        theme::primary
                    })
                    .on_press_maybe((!busy).then(|| msg(operations::Event::Confirm))),
            ),
    );
    crate::ui::modal::overlay(base, content.into(), 480., Message::DismissDialog)
}
