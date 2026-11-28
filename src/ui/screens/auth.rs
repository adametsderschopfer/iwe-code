use crate::ui::modal;
use crate::ui::theme::iced;
use crate::{
    app::{
        App, Message,
        auth::{Message as AuthMessage, Stage},
        error_key,
    },
    domain::auth::Secret,
    i18n::Key,
    ui::theme,
};
use iced::{
    Alignment, Element, Font, Length,
    widget::{row, space, text},
};

pub(super) fn overlay<'a>(app: &'a App, base: Element<'a, Message>) -> Element<'a, Message> {
    let Some(dialog) = &app.auth_dialog else {
        return base;
    };
    let mut content = modal::content(app.i18n.text(Key::SignIn)).push(
        text(format!(
            "{} · {}",
            dialog.account.provider.name(),
            dialog.account.host
        ))
        .size(14)
        .style(theme::MUTED.text()),
    );
    match dialog.stage {
        Stage::Starting | Stage::Connecting => {
            content = content.push(text(app.i18n.text(Key::Connecting)).size(14));
        }
        Stage::Waiting => {
            content = content.push(text(app.i18n.text(Key::BrowserWaiting)).size(14));
            if let Some(prompt) = &dialog.prompt {
                if let Some(code) = &prompt.code {
                    content = content
                        .push(
                            text(app.i18n.text(Key::DeviceCodeDescription))
                                .size(13)
                                .style(theme::MUTED.text()),
                        )
                        .push(
                            row![
                                text(code).font(Font::MONOSPACE).size(28),
                                space().width(Length::Fill),
                                theme::compact_button(app.i18n.text(Key::CopyAuthCode))
                                    .style(theme::action(false))
                                    .on_press(Message::Auth(AuthMessage::CopyCode)),
                            ]
                            .align_y(Alignment::Center),
                        );
                }
                content = content.push(
                    theme::text_button(app.i18n.text(Key::OpenBrowser))
                        .style(theme::primary)
                        .on_press(Message::OpenUrl(prompt.url.clone())),
                );
            }
        }
        Stage::Token => {
            content = content
                .push(
                    text(app.i18n.text(
                        if dialog.account.provider == crate::domain::repository::Provider::GitLab {
                            Key::TokenDescription
                        } else {
                            Key::GitHubTokenDescription
                        },
                    ))
                    .size(14)
                    .line_height(1.5)
                    .style(theme::MUTED.text()),
                )
                .push(text(app.i18n.text(Key::TokenLabel)).size(14))
                .push(
                    theme::text_field(&app.i18n.text(Key::TokenPlaceholder), dialog.token.expose())
                        .secure(true)
                        .style(theme::validated_input(
                            dialog.error == Some(crate::domain::Failure::InvalidToken),
                        ))
                        .on_input(|value| {
                            Message::Auth(AuthMessage::TokenChanged(Secret::new(value)))
                        })
                        .on_submit(Message::Auth(AuthMessage::SubmitToken)),
                );
            let token_path =
                if dialog.account.provider == crate::domain::repository::Provider::GitLab {
                    "/-/user_settings/personal_access_tokens"
                } else {
                    "/settings/tokens"
                };
            content = content.push(
                theme::compact_button(app.i18n.text(Key::CreateToken))
                    .style(theme::action(false))
                    .on_press(Message::OpenUrl(format!(
                        "{}{token_path}",
                        dialog.account.base_url()
                    ))),
            );
        }
        Stage::Failed => {}
    }
    if let Some(error) = dialog.error {
        content = content.push(
            text(app.i18n.text(error_key(error)))
                .size(13)
                .style(theme::RED.text()),
        );
    }
    let mut actions = modal::actions().push(
        theme::text_button(app.i18n.text(Key::CancelSignIn))
            .on_press(Message::Auth(AuthMessage::Cancel)),
    );
    if dialog.stage == Stage::Token {
        actions = actions.push(
            theme::text_button(app.i18n.text(Key::SignIn))
                .style(theme::primary)
                .on_press_maybe(
                    (!dialog.token.is_empty()).then_some(Message::Auth(AuthMessage::SubmitToken)),
                ),
        );
    } else if dialog.stage == Stage::Failed {
        actions = actions.push(
            theme::text_button(app.i18n.text(Key::Retry))
                .style(theme::primary)
                .on_press(Message::SignIn),
        );
    }
    content = content.push(space().height(4)).push(actions);
    super::modal::overlay(
        base,
        content.into(),
        520.,
        Message::Auth(AuthMessage::Cancel),
    )
}
