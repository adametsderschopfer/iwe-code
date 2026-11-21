//! Render keyboard-accessible editor actions and the local rename dialog.

use crate::{
    app::{
        App, Message,
        editor::menu::{Action, Event, SCROLL_ID, message},
    },
    i18n::Key,
    ui::{context_menu, modal, theme},
};
use iced::{
    Element, Length,
    keyboard::{self, key::Named},
    widget::{column, container, scrollable, text},
};

fn section(action: Action) -> Option<Key> {
    match action {
        Action::Line | Action::Definition | Action::Usages => Some(Key::EditorNavigate),
        Action::Rename | Action::Format => Some(Key::EditorRefactor),
        _ => None,
    }
}

pub(super) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let tab = app.preview().id;
    if let Some(form) = app.editor.rename.as_ref().filter(|form| form.tab == tab) {
        let valid = !form.value.trim().is_empty() && form.value != form.symbol;
        let content = modal::content(app.i18n.text(Key::EditorRenameLocal))
            .push(text(&form.symbol).size(13).style(theme::MUTED.text()))
            .push(
                theme::text_field(&app.i18n.text(Key::EditorNewName), &form.value)
                    .id("editor-rename")
                    .on_input(|v| message(Event::RenameInput(v)))
                    .on_submit_maybe(valid.then(|| message(Event::RenameSubmit))),
            )
            .push(
                modal::actions()
                    .push(
                        theme::text_button(app.i18n.text(Key::Cancel))
                            .on_press(message(Event::RenameCancel)),
                    )
                    .push(
                        theme::text_button(app.i18n.text(Key::FileRename))
                            .on_press_maybe(valid.then(|| message(Event::RenameSubmit)))
                            .style(theme::primary),
                    ),
            );
        return modal::overlay(base, content.into(), 420., message(Event::RenameCancel));
    }
    let Some(menu) = app.editor.menu.as_ref().filter(|menu| menu.tab == tab) else {
        return base;
    };
    let items = app.editor_menu_items(menu);
    let selected = items.get(menu.selected).copied();
    const ROW_HEIGHT: f32 = 28.;
    const HEADING_HEIGHT: f32 = 24.;
    let headings = items
        .chunk_by(|a, b| section(*a) == section(*b))
        .filter(|group| section(group[0]).is_some())
        .count();
    let height = items.len() as f32 * ROW_HEIGHT + headings as f32 * HEADING_HEIGHT;
    let overlay = context_menu::overlay(
        base,
        menu.point,
        iced::Size::new(
            300. + context_menu::ICON_COLUMN_WIDTH,
            height + 2. * f32::from(context_menu::SURFACE_PADDING),
        ),
        move || {
            let mut rows = column![];
            let mut previous_section = None;
            for (index, action) in items.iter().copied().enumerate() {
                let current = section(action);
                if current != previous_section
                    && let Some(key) = current
                {
                    rows = rows.push(column![
                        context_menu::separator(),
                        container(
                            text(app.i18n.text(key))
                                .size(12)
                                .font(theme::semibold())
                                .style(theme::MUTED.text())
                        )
                        .padding([0, context_menu::ROW_PADDING[1]])
                        .center_y(HEADING_HEIGHT - 1.)
                    ]);
                }
                previous_section = current;
                let key = action.key();
                let mut label = context_menu::label(key, app.i18n.text(key));
                let shortcut = action.shortcut(app);
                if !shortcut.is_empty() {
                    label = label.push(text(shortcut).size(12).style(theme::MUTED.text()));
                }
                rows = rows.push(
                    context_menu::row(label)
                        .height(ROW_HEIGHT)
                        .on_press(message(Event::Choose(action)))
                        .style(context_menu::action_style(key, index == menu.selected)),
                );
            }
            container(
                scrollable(rows)
                    .style(theme::scroll)
                    .id(SCROLL_ID)
                    .height(Length::Shrink),
            )
            .max_height(height)
            .into()
        },
        message(Event::Close),
    );
    context_menu::on_key(overlay, move |event| {
        let keyboard::Event::KeyPressed { key, .. } = event else {
            return None;
        };
        let choice = match key {
            keyboard::Key::Named(Named::Escape) => Event::Close,
            keyboard::Key::Named(Named::ArrowDown) => Event::Step(true),
            keyboard::Key::Named(Named::ArrowUp) => Event::Step(false),
            keyboard::Key::Named(Named::Enter | Named::Space) => Event::Choose(selected?),
            _ => return None,
        };
        Some(message(choice))
    })
}
