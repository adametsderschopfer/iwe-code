use super::{Event, Kind, Operation, State, Tab};
use crate::{
    domain::images::ImageRef,
    i18n::{I18n, Key},
    ui::icons::{Icon, sized_icon},
    ui::{
        context_menu::separator,
        service_controls::{action, choice, divider, entry, field, heading, menu, select},
        theme,
    },
};
use iced::{
    Element, Length,
    widget::{button, column, container, pick_list, row, scrollable, space, text},
};
impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
impl Operation {
    fn label(self) -> Key {
        match self {
            Self::Scale => Key::KubeScale,
            Self::Restart => Key::KubeRestart,
            Self::Image => Key::KubeChangeImage,
            Self::Forward => Key::KubeForward,
        }
    }
}
pub(crate) fn panel<'a>(
    s: &'a State,
    i: &'a I18n,
    docker: bool,
    terminal: bool,
) -> Element<'a, Event> {
    let contexts = s
        .contexts
        .iter()
        .map(|(n, _)| n.clone())
        .collect::<Vec<_>>();
    let mut header = row![
        sized_icon(Icon::Kubernetes, theme::BLUE, 20.),
        heading(i.text(Key::Kubernetes)),
        pick_list(
            contexts,
            (!s.target.context.is_empty()).then(|| s.target.context.clone()),
            Event::Context
        )
        .placeholder(i.text(Key::KubeContext))
        .text_size(12)
        .style(select)
        .menu_style(menu),
        action(i, Key::KubeConfig, Some(Event::Config)),
        action(i, Key::Refresh, Some(Event::Refresh))
    ]
    .spacing(5);
    if s.target.file.is_some() {
        header = header.push(action(i, Key::KubeStandard, Some(Event::Standard)))
    }
    if !s.target.context.is_empty() {
        header = header.push(action(
            i,
            Key::KubeManifest,
            (!s.busy).then_some(Event::Manifest),
        ))
    }
    let close = crate::ui::tooltip::view(
        button(sized_icon(Icon::Close, theme::MUTED, 16.))
            .padding(4)
            .on_press(Event::Hide)
            .style(theme::icon_action(false)),
        text(i.text(Key::HidePanel)).size(12),
        crate::ui::tooltip::Position::Bottom,
    );
    let mut body = column![
        container(
            row![header.wrap(), space().width(Length::Fill), close]
                .align_y(iced::Alignment::Center)
        )
        .padding(5),
        separator()
    ]
    .height(Length::Fill);
    if let Some(k) = s.notice {
        body = body.push(text(i.text(k)).size(13))
    }
    if s.busy {
        body = body.push(
            text(i.text(Key::Loading))
                .size(12)
                .style(theme::MUTED.text()),
        )
    }
    if !s.status.is_empty() {
        body = body.push(text(&s.status).size(12))
    }
    if let Some(p) = &s.plan {
        let mut preview = column![
            heading(i.text(Key::KubePreview)),
            choice(i, Key::KubeContext, &s.target.context)
        ]
        .padding(8)
        .spacing(6);
        for c in &p.0 {
            preview = preview.push(heading(c.label.clone()));
            if c.secret_changed {
                preview = preview.push(text(i.text(Key::KubeSecretChange)).size(12));
            }
            preview = preview.push(
                text(if c.diff.is_empty() {
                    i.text(Key::KubeNoDiff)
                } else {
                    c.diff.clone()
                })
                .font(iced::Font::MONOSPACE)
                .size(12),
            )
        }
        return body
            .push(scrollable(preview).height(Length::Fill))
            .push(
                row![
                    action(i, Key::Cancel, Some(Event::Cancel)),
                    action(i, Key::KubeApply, Some(Event::Confirm))
                ]
                .padding(5)
                .spacing(6),
            )
            .into();
    }
    if let (Some(d), Some(r)) = (&s.draft, &s.selected) {
        let mut form = column![
            heading(i.text(d.op.label())),
            choice(i, Key::KubeContext, &s.target.context),
            choice(i, Key::KubeNamespace, &r.ns),
            heading(format!("{}/{}", s.kind, r.name))
        ]
        .spacing(6)
        .padding(8);
        if d.op == Operation::Image
            && let Some(c) = s.container()
        {
            form = form
                .push(choice(i, Key::KubeContainer, &c.name))
                .push(choice(i, Key::KubeOldImage, &c.image))
        }
        if d.op != Operation::Restart {
            let k = match d.op {
                Operation::Image => Key::KubeNewImage,
                Operation::Scale => Key::KubeReplicas,
                _ => Key::KubeLocalPort,
            };
            if d.review {
                form = form.push(choice(i, k, &d.input))
            } else {
                form = form
                    .push(text(i.text(k)).size(12))
                    .push(field(i, k, &d.input, Event::Input))
            }
            if d.op == Operation::Image && docker && !d.review {
                form = form.push(action(
                    i,
                    Key::KubePick,
                    Some(Event::PickRegistry(ImageRef::parse(&d.input))),
                ))
            }
            if d.op == Operation::Forward {
                form = if d.review {
                    form.push(choice(i, Key::KubeRemotePort, &d.remote))
                } else {
                    form.push(text(i.text(Key::KubeRemotePort)).size(12))
                        .push(field(i, Key::KubeRemotePort, &d.remote, Event::Remote))
                }
            }
        }
        return body
            .push(scrollable(form))
            .push(
                row![
                    action(i, Key::Cancel, Some(Event::Cancel)),
                    action(
                        i,
                        if d.review {
                            Key::KubeConfirm
                        } else {
                            Key::RegistryReview
                        },
                        s.valid(d).then_some(if d.review {
                            Event::Confirm
                        } else {
                            Event::Review
                        })
                    )
                ]
                .spacing(6)
                .padding(5),
            )
            .into();
    }
    if s.target.context.is_empty() {
        return body.into();
    }
    let mut filters = row![
        pick_list(&Kind::ALL[..], Some(s.kind), Event::Kind)
            .text_size(12)
            .style(select)
            .menu_style(menu)
    ]
    .spacing(5);
    if s.kind != Kind::Node {
        let all = i.text(Key::KubeAll);
        if s.namespaces.is_empty() {
            filters = filters
                .push(
                    container(field(
                        i,
                        Key::KubeNamespace,
                        &s.ns_input,
                        Event::NamespaceInput,
                    ))
                    .width(150),
                )
                .push(action(
                    i,
                    Key::RegistryOpen,
                    super::valid_namespace(&s.ns_input)
                        .then(|| Event::Namespace(s.ns_input.clone())),
                ))
                .push(action(
                    i,
                    Key::KubeAll,
                    Some(Event::Namespace(String::new())),
                ));
        } else {
            let mut options = vec![all.clone()];
            options.extend(s.namespaces.iter().cloned());
            let current = if s.target.namespace.is_empty() {
                all.clone()
            } else {
                s.target.namespace.clone()
            };
            filters = filters.push(
                pick_list(options, Some(current), move |choice| {
                    Event::Namespace(if choice == all { String::new() } else { choice })
                })
                .text_size(12)
                .style(select)
                .menu_style(menu),
            );
        }
    }
    body = body
        .push(container(filters.wrap()).padding(5))
        .push(separator());
    let mut list = column![].width(Length::FillPortion(2));
    if let Some(p) = &s.parent {
        list = list.push(entry(
            format!("Deployment/{}  ×", p.name),
            false,
            Event::Kind(Kind::Pod),
        ))
    }
    let cells = |name: String, ns: String, status: String| {
        let cell = |v, portion| {
            container(crate::ui::ellipsis::text(v, 12.)).width(Length::FillPortion(portion))
        };
        let mut columns = row![cell(name, 4)].spacing(6);
        if s.kind != Kind::Node {
            columns = columns.push(cell(ns, 3))
        }
        columns.push(cell(status, 2))
    };
    list = list
        .push(
            container(cells(
                i.text(Key::DockerName),
                i.text(Key::KubeNamespace),
                i.text(Key::DockerStatus),
            ))
            .padding([4, 8]),
        )
        .push(separator());
    if s.rows.is_empty() && !s.busy && s.notice.is_none() {
        list = list.push(text(i.text(Key::KubeEmpty)).size(12))
    }
    list = list.push(
        scrollable(column(s.rows.iter().skip(s.page * 50).take(50).map(|r| {
            let selected = s.selected.as_ref().is_some_and(|v| v.uid == r.uid);
            button(cells(r.name.clone(), r.ns.clone(), r.status.clone()))
                .width(Length::Fill)
                .padding([5, 8])
                .on_press(Event::Select(r.clone()))
                .style(move |t, state| {
                    let mut style = theme::tree_item(selected)(t, state);
                    style.border.radius = Default::default();
                    style
                })
                .into()
        })))
        .height(Length::Fill),
    );
    if s.rows.len() > 50 {
        list = list.push(
            row![
                action(
                    i,
                    Key::PreviousFilePart,
                    (s.page > 0).then(|| Event::Page(s.page.saturating_sub(1)))
                ),
                text(format!("{}/{}", s.page + 1, s.rows.len().div_ceil(50))).size(12),
                action(
                    i,
                    Key::NextFilePart,
                    ((s.page + 1) * 50 < s.rows.len()).then_some(Event::Page(s.page + 1))
                )
            ]
            .spacing(5),
        )
    }
    let mut panes = row![list].height(Length::Fill);
    if let (Some(r), Some(d)) = (&s.selected, &s.details) {
        let mut props = column![
            heading(format!("{}/{}", s.kind, r.name)),
            text(format!("{}   {}", r.ns, r.status)).size(12),
            row![
                action(i, Key::DockerDetails, Some(Event::Tab(Tab::Properties))),
                action(i, Key::KubeEvents, Some(Event::Tab(Tab::Events))),
                action(i, Key::KubeYaml, Some(Event::Tab(Tab::Yaml)))
            ]
            .spacing(4)
            .wrap()
        ]
        .spacing(6)
        .padding(8);
        match s.tab {
            Tab::Yaml => props = props.push(text(&d.yaml).size(12).font(iced::Font::MONOSPACE)),
            Tab::Events => match &d.events {
                Ok(v) if v.is_empty() => props = props.push(text(i.text(Key::KubeEmpty)).size(12)),
                Ok(v) => {
                    for e in v {
                        props = props.push(text(e).size(12))
                    }
                }
                Err(k) => props = props.push(text(i.text(*k)).size(12)),
            },
            Tab::Properties => {
                if !d.containers.is_empty() {
                    props = props.push(text(i.text(Key::KubeContainer)).size(12)).push(
                        pick_list(
                            d.containers
                                .iter()
                                .map(|c| c.name.clone())
                                .collect::<Vec<_>>(),
                            Some(s.container.clone()),
                            Event::Container,
                        )
                        .text_size(12)
                        .style(select)
                        .menu_style(menu),
                    )
                }
                if let Some(c) = s.container() {
                    props = props.push(choice(i, Key::DockerImage, &c.image));
                    if s.kind == Kind::Pod && docker {
                        props = props.push(action(
                            i,
                            Key::KubeOpenImage,
                            c.running
                                .clone()
                                .or_else(|| ImageRef::parse(&c.image))
                                .map(Event::OpenRegistry),
                        ))
                    }
                }
                let mut actions = row![].spacing(4);
                if s.kind == Kind::Deployment {
                    for op in [Operation::Image, Operation::Scale, Operation::Restart] {
                        actions = actions.push(action(i, op.label(), Some(Event::Begin(op))))
                    }
                    actions = actions
                        .push(action(i, Key::KubeRollout, Some(Event::Rollout)))
                        .push(action(i, Key::KubePods, Some(Event::Related)));
                }
                if s.kind == Kind::Pod {
                    actions = actions.push(action(
                        i,
                        Key::DockerLogs,
                        s.container().map(|_| Event::Logs),
                    ));
                    if terminal {
                        actions =
                            actions.push(action(i, Key::Terminal, s.shell().map(|_| Event::Shell)))
                    }
                }
                if matches!(s.kind, Kind::Pod | Kind::Service) {
                    actions = actions.push(action(
                        i,
                        Key::KubeForward,
                        Some(Event::Begin(Operation::Forward)),
                    ))
                }
                props = props.push(actions.wrap());
            }
        }
        panes = panes
            .push(divider())
            .push(container(scrollable(props)).width(Length::FillPortion(3)));
    }
    body = body.push(panes);
    if !s.sessions.is_empty() {
        let mut sessions = column![].spacing(3).padding(5);
        for (&id, session) in &s.sessions {
            let k = if session.forward {
                Key::KubeForward
            } else {
                Key::DockerLogs
            };
            let status = session.error.unwrap_or(if session.ended {
                Key::KubeEnded
            } else if session.forward && !session.ready {
                Key::Loading
            } else {
                Key::KubeLive
            });
            sessions = sessions.push(
                row![
                    heading(i.text(k)),
                    text(&session.label).size(12),
                    text(i.text(status)).size(12),
                    action(
                        i,
                        if session.ended {
                            Key::Dismiss
                        } else {
                            Key::DockerStop
                        },
                        Some(Event::CloseSession(id))
                    )
                ]
                .spacing(5)
                .wrap(),
            );
            if !session.forward {
                sessions = sessions.push(text(&session.text).size(12).font(iced::Font::MONOSPACE))
            }
        }
        body = body
            .push(separator())
            .push(container(scrollable(sessions)).max_height(150));
    }
    body.into()
}
