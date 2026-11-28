use super::PipelineState;
use crate::ui::presentation::{IconButton, center_message, divider, error_key, icon_button, short};
use crate::ui::theme::iced;
use crate::ui::tooltip;
use crate::{
    domain::{
        Failure,
        jobs::Status,
        repository::{Pipeline, Provider},
    },
    i18n::Key,
    ui::{
        ellipsis,
        icons::{Icon, icon},
        theme,
    },
};
use Event as Message;
use iced::{
    Alignment, Element, Length,
    widget::{Column, button, column, container, row, scrollable, space, text},
};

pub(crate) fn view<'a>(app: View<'a>) -> Element<'a, Message> {
    let Some(remote) = app.remote else {
        return space().into();
    };
    let provider = remote.provider.map(Provider::name).unwrap_or(&remote.host);
    let account: Element<'_, Message> = if remote.provider.is_none() {
        space().into()
    } else {
        row![
            text(short(
                app.account_username.as_deref().unwrap_or_default(),
                16
            ))
            .size(11)
            .style(theme::MUTED.text()),
            theme::compact_button(app.i18n.text(if app.account_username.is_some() {
                Key::Disconnect
            } else {
                Key::SignIn
            }))
            .style(theme::action(false))
            .on_press_maybe(
                (!app.disconnecting).then_some(if app.account_username.is_some() {
                    Message::Disconnect
                } else {
                    Message::SignIn
                })
            ),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    };
    let header = row![
        icon(Icon::Play, theme::MUTED),
        text(app.i18n.text(Key::Pipelines))
            .size(14)
            .font(theme::semibold()),
        text(provider).size(12).style(theme::FAINT.text()),
        container(
            theme::compact_field(
                &app.i18n.text(if app.state.jobs.selected.is_some() {
                    Key::Jobs
                } else {
                    Key::SearchPipelines
                }),
                if app.state.jobs.selected.is_some() {
                    &app.state.job_query
                } else {
                    &app.state.query
                }
            )
            .id("pipeline-search")
            .on_input(Message::Search)
        )
        .width(Length::Fill)
        .max_width(320.),
        space().width(Length::Fill),
        text(short(&remote.project, 24))
            .size(12)
            .style(theme::MUTED.text()),
        account,
        IconButton::Compact.view(
            app.i18n,
            Icon::Refresh,
            Key::Refresh,
            Some(Message::Jobs(
                crate::modules::pipelines::jobs::Event::Overview(
                    crate::modules::pipelines::jobs::overview::Event::RefreshVisible
                )
            ))
        ),
        container(IconButton::Compact.view(
            app.i18n,
            Icon::Minus,
            Key::HidePanel,
            Some(Message::Hide)
        ))
        .id("pipelines-hide")
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let content: Element<'_, Message> = if app.state.jobs.selected.is_some() {
        jobs::view(app)
    } else if remote.provider.is_none() {
        container(
            column![
                text(app.i18n.text(Key::ChooseProvider)).size(14),
                row![
                    theme::text_button("GitHub")
                        .on_press(Message::ChooseProvider(Provider::GitHub)),
                    theme::text_button("GitLab")
                        .on_press(Message::ChooseProvider(Provider::GitLab))
                ]
                .spacing(8)
            ]
            .spacing(12)
            .align_x(Alignment::Center),
        )
        .center(Length::Fill)
        .into()
    } else {
        match &app.state.pipelines {
            PipelineState::Loading => center_message(app.i18n, Key::Loading),
            PipelineState::Idle => center_message(app.i18n, Key::Loading),
            PipelineState::Failed(error) => {
                let access = matches!(
                    error,
                    Failure::Access
                        | Failure::AuthExpired
                        | Failure::InvalidToken
                        | Failure::PermissionDenied
                );
                let action = if access {
                    Message::SignIn
                } else {
                    Message::RefreshPipelines
                };
                container(
                    row![
                        column![
                            text(app.i18n.text(error_key(*error))).size(14),
                            text(app.i18n.text(if access {
                                Key::SignInDescription
                            } else {
                                Key::RetryDescription
                            }))
                            .size(12)
                            .style(theme::MUTED.text())
                        ]
                        .spacing(6)
                        .width(Length::Fill),
                        theme::text_button(app.i18n.text(if access {
                            Key::SignIn
                        } else {
                            Key::Retry
                        }))
                        .on_press(action)
                        .style(theme::primary)
                    ]
                    .spacing(20)
                    .align_y(Alignment::Center),
                )
                .padding(24)
                .center_y(Length::Fill)
                .into()
            }
            PipelineState::Ready(runs) if runs.is_empty() => {
                center_message(app.i18n, Key::NoPipelines)
            }
            PipelineState::Ready(runs) => {
                let mut rows = Column::new();
                let mut matches = super::filtered_runs(runs, &app.state.query).peekable();
                if matches.peek().is_none() {
                    rows = rows.push(center_message(app.i18n, Key::FindNoResults));
                }
                for run in matches {
                    rows = rows.push(pipeline_row(app, run));
                }
                scrollable(rows)
                    .id("pipelines-list")
                    .width(Length::Fill)
                    .style(theme::scroll)
                    .height(Length::Fill)
                    .on_scroll(|viewport| {
                        let start = (viewport.absolute_offset().y / theme::PIPELINE_ROW_HEIGHT)
                            .floor() as usize;
                        let end = ((viewport.absolute_offset().y + viewport.bounds().height)
                            / theme::PIPELINE_ROW_HEIGHT)
                            .ceil() as usize
                            + 1;
                        Message::Jobs(crate::modules::pipelines::jobs::Event::Overview(
                            crate::modules::pipelines::jobs::overview::Event::Viewport(start..end),
                        ))
                    })
                    .into()
            }
        }
    };
    container(column![
        container(header)
            .height(theme::PANEL_HEADER_HEIGHT)
            .align_y(Alignment::Center)
            .padding([0, 8]),
        divider(),
        content
    ])
    .height(Length::Fill)
    .style(theme::panel)
    .into()
}

fn status(value: &str) -> (Key, theme::Tone) {
    status::label(Status::from_provider(value))
}

fn pipeline_row<'a>(app: View<'a>, run: &'a Pipeline) -> Element<'a, Message> {
    let title = if run.title.is_empty() {
        format!("#{}", run.id)
    } else {
        run.title.clone()
    };
    let mut metadata = row![
        icon(Icon::Branch, theme::MUTED),
        container(ellipsis::text(&run.branch, 11.)).width(Length::Fill),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    if let Some(commit) = &run.commit {
        metadata = metadata.push(tooltip::view(
            container(
                text(commit.chars().take(8).collect::<String>())
                    .size(11)
                    .font(iced::Font::MONOSPACE)
                    .style(theme::MUTED.text()),
            )
            .id(format!("pipeline-commit-{}", run.id)),
            text(commit).size(12),
            tooltip::Position::Top,
        ));
    }
    metadata = metadata.push(
        text(short(&run.date.replace('T', " "), 16))
            .size(11)
            .style(theme::FAINT.text()),
    );
    let info = column![ellipsis::text(title, 13.), metadata]
        .spacing(4)
        .width(Length::Fill);
    let summary = row![
        tooltip::view(
            status::badge(Status::from_provider(&run.status), false),
            text(app.i18n.text(status(&run.status).0)).size(12),
            tooltip::Position::Top
        ),
        info,
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    column![
        row![
            crate::ui::context_menu::trigger(
                container(
                    button(summary)
                        .width(Length::Fill)
                        .padding([8, 12])
                        .style(theme::action(false))
                        .on_press(Message::OpenUrl(run.url.clone()))
                )
                .id(format!("pipeline-{}", run.id))
                .width(Length::FillPortion(5)),
                move |point| {
                    Message::ExplainMenu(
                        crate::domain::explain::Target::Pipeline(run.clone()),
                        point,
                    )
                }
            ),
            container(overview::chain(app, run)).width(Length::FillPortion(4)),
            icon_button(
                app.i18n,
                Icon::External,
                Key::OpenBrowser,
                Message::OpenUrl(run.url.clone()),
                false
            ),
        ]
        .height(theme::PIPELINE_ROW_HEIGHT - 1.)
        .spacing(10)
        .align_y(Alignment::Center)
        .padding([0, 12]),
        divider(),
    ]
    .height(theme::PIPELINE_ROW_HEIGHT)
    .into()
}

mod jobs;
mod overview;
mod status;
pub(crate) use overview::overlay;

use crate::{domain::repository::Remote, i18n::I18n};
#[derive(Clone, Copy)]
pub(crate) struct View<'a> {
    pub state: &'a super::State,
    pub i18n: &'a I18n,
    pub remote: Option<&'a Remote>,
    pub account_username: &'a Option<String>,
    pub disconnecting: bool,
    pub visible: bool,
}
impl View<'_> {
    fn job_controls_blocked(self, id: u64) -> bool {
        let jobs = &self.state.jobs;
        self.disconnecting
            || jobs.pending.is_some()
            || if jobs.selected.as_ref().is_some_and(|run| run.id == id) {
                jobs.error.is_some() || jobs.action_error.is_some()
            } else {
                jobs.overview
                    .entries
                    .get(&id)
                    .is_none_or(|entry| entry.error.is_some())
            }
    }
}
#[derive(Debug, Clone)]
pub(crate) enum Event {
    Search(String),
    RefreshPipelines,
    Jobs(super::jobs::Event),
    SignIn,
    Disconnect,
    Hide,
    ChooseProvider(Provider),
    OpenUrl(String),
    ExplainMenu(crate::domain::explain::Target, iced::Point),
}
