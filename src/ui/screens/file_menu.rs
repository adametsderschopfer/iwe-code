//! Show local file commands and their confirmation dialogs.

use crate::{
    app::{
        App, Message,
        files::{
            Event, PathFormat,
            operations::{self, Command},
        },
    },
    i18n::Key,
    ui::{
        context_menu,
        icons::{Icon, icon},
        theme,
    },
};
use iced::{
    Alignment, Element, Length, Point,
    widget::{
        button, column, container, mouse_area, opaque, responsive, row, scrollable, space, stack,
        text, text_input,
    },
};

pub(super) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(menu) = &app.files.menu else {
        return base;
    };
    let relative_key = if app.path_repository_root(&menu.path).is_some() {
        Key::CopyRepositoryPath
    } else {
        Key::CopyProjectPath
    };
    let popup = responsive(move |available| {
        let width = 250_f32.min(available.width);
        let protected = app.file_source_protected(&menu.path);
        let is_root = app.workspace.directory() == Some(&menu.path);
        let deployment = !is_root && app.module_enabled(crate::modules::ModuleId::Deployment);
        let items = 4
            + if protected { 0 } else { 6 }
            + if deployment { 2 } else { 0 }
            + usize::from(!is_root);
        let height = (items as f32 * 27. + 12.).min(available.height);
        let action = |key, event| {
            context_menu::row(text(app.i18n.text(key)).size(context_menu::TEXT_SIZE))
                .on_press_maybe(event)
        };
        let command = |command| Some(operations::message(operations::Event::Command(command)));
        let arrow = |key, event| {
            mouse_area(
                button(
                    row![
                        text(app.i18n.text(key)).size(context_menu::TEXT_SIZE),
                        space().width(Length::Fill),
                        icon(Icon::Right, theme::MUTED),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .on_press(event)
                .padding(context_menu::ROW_PADDING)
                .width(Length::Fill)
                .style(theme::action(false)),
            )
        };
        let child_width = 300_f32.min(available.width);
        let origin = Point::new(
            menu.point.x.clamp(0., (available.width - width).max(0.)),
            menu.point.y.clamp(0., (available.height - height).max(0.)),
        );
        let main_path = mouse_area(
            button(
                row![
                    text(app.i18n.text(Key::CopyPathMenu)).size(context_menu::TEXT_SIZE),
                    space().width(Length::Fill),
                    icon(Icon::Right, theme::MUTED),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .on_press(Message::Files(Event::CopyPath(PathFormat::Absolute)))
            .padding(context_menu::ROW_PADDING)
            .width(Length::Fill)
            .style(theme::action(false)),
        )
        .on_enter(Message::Files(Event::CopyOptions));
        let deployment_action = |key, compare| {
            context_menu::row(text(app.i18n.text(key)).size(context_menu::TEXT_SIZE)).on_press(
                crate::app::modules::deployment_message(crate::modules::deployment::Event::Run(
                    if compare {
                        crate::modules::deployment::Operation::Compare(menu.path.clone())
                    } else {
                        crate::modules::deployment::Operation::Upload(menu.path.clone())
                    },
                )),
            )
        };
        let create = Message::Files(Event::CreateOptions);
        let mut menu_items = column![arrow(Key::FileNew, create.clone()).on_enter(create)];
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
        let max_height = (available.height - 16.).max(0.);
        let menu_content =
            container(scrollable(main).height(Length::Shrink)).max_height(max_height);
        let mut layers = vec![panel(menu_content.into(), origin, width)];
        if menu.copy_open || menu.create_open {
            let x = if origin.x + width + child_width <= available.width {
                origin.x + width
            } else {
                (origin.x - child_width).max(0.)
            };
            let offset = if menu.create_open {
                0.
            } else if protected {
                53.
            } else {
                211.
            };
            let position = Point::new(
                x,
                (origin.y + offset).min((available.height - 104.).max(0.)),
            );
            let option = |key, format| {
                button(
                    text(app.i18n.text(key))
                        .size(context_menu::TEXT_SIZE)
                        .wrapping(text::Wrapping::None),
                )
                .on_press(Message::Files(Event::CopyPath(format)))
                .padding(context_menu::ROW_PADDING)
                .width(Length::Fill)
                .style(theme::action(false))
            };
            let children = if menu.create_open {
                column![
                    action(Key::FileNewFile, command(Command::CreateFile)),
                    action(Key::FileNewFolder, command(Command::CreateFolder)),
                ]
            } else {
                column![
                    option(Key::CopyAbsolutePath, PathFormat::Absolute),
                    option(relative_key, PathFormat::Relative),
                ]
            }
            .spacing(4);
            layers.push(panel(children.into(), position, child_width));
        }
        iced::widget::Stack::with_children(layers)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    });
    stack![
        base,
        opaque(
            mouse_area(popup)
                .on_press(Message::Files(Event::DismissMenu))
                .on_right_press(Message::Files(Event::DismissMenu))
        )
    ]
    .into()
}

fn panel<'a>(content: Element<'a, Message>, point: Point, width: f32) -> Element<'a, Message> {
    container(context_menu::surface(content, width))
        .padding(iced::Padding {
            left: point.x,
            top: point.y,
            ..iced::Padding::ZERO
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

pub(super) fn dialog<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(draft) = &app.files.operations.draft else {
        return base;
    };
    let msg = operations::message;
    let busy = app.files.operations.busy || app.editor.pending.is_some();
    let mut content = column![text(app.i18n.text(draft.title)).size(21)].spacing(12);
    if let Some(parent) = &draft.parent {
        let root = app.workspace.directory().map(|root| root.as_path());
        let folder = root
            .and_then(|root| parent.strip_prefix(root).ok())
            .unwrap_or(parent);
        if !folder.as_os_str().is_empty() {
            content = content.push(text(folder.to_string_lossy()).size(13));
        }
        content = content.push(
            text_input(&app.i18n.text(Key::FileName), &draft.name)
                .id(operations::NAME_INPUT)
                .on_input_maybe(
                    (!busy).then_some(|name| operations::message(operations::Event::Name(name))),
                )
                .on_submit_maybe((!busy).then(|| msg(operations::Event::Confirm)))
                .padding(9),
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
        row![
            button(text(app.i18n.text(Key::Cancel)))
                .style(theme::secondary)
                .on_press_maybe((!busy).then(|| msg(operations::Event::Cancel))),
            button(text(app.i18n.text(draft.title)))
                .style(theme::primary)
                .on_press_maybe((!busy).then(|| msg(operations::Event::Confirm))),
        ]
        .spacing(8),
    );
    crate::ui::modal::overlay(base, content.into(), 480., Message::DismissDialog)
}
