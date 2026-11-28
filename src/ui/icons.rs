//! Draw small, consistent outline icons without a platform icon font.

use crate::ui::theme::iced;
pub(super) mod cache;

use super::theme::Tone;

use iced::{
    Element, Point, Rectangle, Renderer, Theme, mouse,
    widget::canvas::{self, Frame, Geometry, Path, Stroke},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Icon {
    Docker,
    Kubernetes,
    Cube,
    Network,
    Relationships,
    Volume,
    Locate,
    More,
    Check,
    Warning,
    CollapseLines,
    ExpandLines,
    MarkdownSource,
    MarkdownSplit,
    Image,
    SplitDiff,
    Sidebar,
    UnifiedDiff,
    Back,
    Branch,
    Tag,
    Play,
    Settings,
    Terminal,
    Server,
    Plus,
    Minus,
    Duplicate,
    Cut,
    Paste,
    Trash,
    Pause,
    Stop,
    Rename,
    Right,
    Down,
    Up,
    Drag,
    Changes,
    Fetch,
    UpdateBranch,
    Upload,
    Update,
    Pin,
    Refresh,
    Close,
    Search,
    #[cfg(target_os = "windows")]
    Minimize,
    #[cfg(target_os = "windows")]
    Maximize,
    #[cfg(target_os = "windows")]
    Restore,
    Folder,
    External,
}

impl Icon {
    fn theme_key(self) -> &'static str {
        match self {
            Self::Docker => "docker",
            Self::Kubernetes => "kubernetes",
            Self::Cube => "cube",
            Self::Network => "network",
            Self::Relationships => "relationships",
            Self::Volume => "volume",
            Self::Locate => "locate",
            Self::More => "more",
            Self::Check => "check",
            Self::Warning => "warning",
            Self::CollapseLines => "collapse-lines",
            Self::ExpandLines => "expand-lines",
            Self::MarkdownSource => "markdown-source",
            Self::MarkdownSplit => "markdown-split",
            Self::Image => "image",
            Self::SplitDiff => "split-diff",
            Self::Sidebar => "sidebar",
            Self::UnifiedDiff => "unified-diff",
            Self::Back => "back",
            Self::Branch => "branch",
            Self::Tag => "tag",
            Self::Play => "play",
            Self::Settings => "settings",
            Self::Terminal => "terminal",
            Self::Server => "server",
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::Duplicate => "duplicate",
            Self::Cut => "cut",
            Self::Paste => "paste",
            Self::Trash => "trash",
            Self::Pause => "pause",
            Self::Stop => "stop",
            Self::Rename => "rename",
            Self::Right => "right",
            Self::Down => "down",
            Self::Up => "up",
            Self::Drag => "drag",
            Self::Changes => "changes",
            Self::Fetch => "fetch",
            Self::UpdateBranch => "update-branch",
            Self::Upload => "upload",
            Self::Update => "update",
            Self::Pin => "pin",
            Self::Refresh => "refresh",
            Self::Close => "close",
            Self::Search => "search",
            #[cfg(target_os = "windows")]
            Self::Minimize => "minimize",
            #[cfg(target_os = "windows")]
            Self::Maximize => "maximize",
            #[cfg(target_os = "windows")]
            Self::Restore => "restore",
            Self::Folder => "folder",
            Self::External => "external",
        }
    }
}

pub(super) fn paint_theme_icon(
    frame: &mut Frame,
    theme: &Theme,
    key: &str,
    color: iced::Color,
) -> bool {
    let Some(icon) = theme.icon(key) else {
        return false;
    };
    let bounds = frame.size();
    let scale = bounds.width.min(bounds.height) / 20.;
    frame.translate(iced::Vector::new(
        (bounds.width - 20. * scale) / 2.,
        (bounds.height - 20. * scale) / 2.,
    ));
    frame.scale(scale);
    if icon.fill {
        frame.fill(&icon.path, color);
    } else {
        frame.stroke(
            &icon.path,
            Stroke::default()
                .with_width(theme.icon_stroke())
                .with_color(color)
                .with_line_cap(canvas::LineCap::Round)
                .with_line_join(canvas::LineJoin::Round),
        );
    }
    true
}

pub(crate) fn icon<'a, Message: 'a>(icon: Icon, color: Tone) -> Element<'a, Message> {
    sized_icon(icon, color, 20.)
}

pub(crate) fn sized_icon<'a, Message: 'a>(
    icon: Icon,
    color: Tone,
    size: f32,
) -> Element<'a, Message> {
    canvas::Canvas::new(Drawing(icon, color))
        .width(size)
        .height(size)
        .into()
}

struct Drawing(Icon, Tone);

impl<Message> canvas::Program<Message, Theme> for Drawing {
    type State = cache::Cache<(Icon, Tone, u64)>;
    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        state.draw(
            (self.0, self.1, theme.revision()),
            renderer,
            bounds.size(),
            |frame| self.paint(frame, theme),
        )
    }
}

impl Drawing {
    fn paint(&self, frame: &mut Frame, theme: &Theme) {
        let color = self.1.resolve(theme);
        if paint_theme_icon(frame, theme, self.0.theme_key(), color) {
            return;
        }
        let bounds = frame.size();
        frame.scale(bounds.width.min(bounds.height) / 20.);
        let updating = matches!(self.0, Icon::Update);
        if updating {
            frame.fill(
                &Path::circle(Point::new(10., 10.), 10.),
                super::theme::UPDATE_BLUE.resolve(theme),
            );
        }
        let stroke = Stroke::default()
            .with_width(theme.icon_stroke())
            .with_color(if updating {
                super::theme::PIPELINE_GLYPH.resolve(theme)
            } else {
                self.1.resolve(theme)
            });
        let mut line = |points: &[(f32, f32)]| {
            let path = Path::new(|builder| {
                if let Some(&(x, y)) = points.first() {
                    builder.move_to(Point::new(x, y));
                }
                for &(x, y) in points.iter().skip(1) {
                    builder.line_to(Point::new(x, y));
                }
            });
            frame.stroke(&path, stroke);
        };
        match self.0 {
            Icon::Kubernetes => {
                for i in 0..7 {
                    let angle = std::f32::consts::TAU * i as f32 / 7.;
                    line(&[
                        (10. + 2. * angle.sin(), 10. - 2. * angle.cos()),
                        (10. + 8. * angle.sin(), 10. - 8. * angle.cos()),
                    ]);
                }
                frame.stroke(&Path::circle(Point::new(10., 10.), 5.8), stroke);
                frame.stroke(&Path::circle(Point::new(10., 10.), 2.), stroke);
            }
            Icon::Minus => line(&[(3., 10.), (17., 10.)]),
            Icon::Duplicate => {
                line(&[(7., 7.), (17., 7.), (17., 18.), (7., 18.), (7., 7.)]);
                line(&[(13., 4.), (13., 2.), (3., 2.), (3., 13.), (4., 13.)]);
            }
            Icon::Cut => {
                line(&[(6., 12.), (16., 2.)]);
                line(&[(6., 8.), (16., 18.)]);
                for y in [5.5, 14.5] {
                    frame.stroke(&Path::circle(Point::new(4.5, y), 2.5), stroke);
                }
            }
            Icon::Paste => {
                line(&[
                    (7., 4.),
                    (4., 4.),
                    (4., 18.),
                    (16., 18.),
                    (16., 4.),
                    (13., 4.),
                ]);
                frame.stroke(
                    &Path::rectangle(Point::new(7., 2.), iced::Size::new(6., 4.)),
                    stroke,
                );
            }
            Icon::Trash => {
                line(&[(3., 5.), (17., 5.)]);
                line(&[(7., 5.), (7., 2.), (13., 2.), (13., 5.)]);
                line(&[(5., 6.), (6., 18.), (14., 18.), (15., 6.)]);
                line(&[(8., 8.), (8.5, 15.)]);
                line(&[(12., 8.), (11.5, 15.)]);
            }
            Icon::Pause => {
                for x in [6., 12.] {
                    frame.fill(
                        &Path::rectangle(Point::new(x, 3.), iced::Size::new(2., 14.)),
                        self.1.resolve(theme),
                    );
                }
            }
            Icon::Stop => {
                frame.stroke(
                    &Path::rectangle(Point::new(4., 4.), iced::Size::new(12., 12.)),
                    stroke,
                );
            }
            Icon::Rename => {
                line(&[
                    (3., 13.),
                    (13., 3.),
                    (17., 7.),
                    (7., 17.),
                    (2., 18.),
                    (3., 13.),
                ]);
                line(&[(11., 5.), (15., 9.)]);
            }
            Icon::Server => {
                line(&[(3., 3.), (17., 3.), (17., 17.), (3., 17.), (3., 3.)]);
                line(&[(3., 10.), (17., 10.)]);
                line(&[(6., 6.), (8., 6.)]);
                line(&[(6., 13.), (8., 13.)]);
            }
            Icon::Cube => {
                line(&[
                    (10., 2.),
                    (17., 6.),
                    (17., 14.),
                    (10., 18.),
                    (3., 14.),
                    (3., 6.),
                    (10., 2.),
                ]);
                line(&[(3., 6.), (10., 10.), (17., 6.)]);
                line(&[(10., 10.), (10., 18.)]);
            }
            Icon::Relationships => {
                line(&[(5.5, 9.), (12.5, 4.5)]);
                line(&[(5.5, 11.), (13.5, 15.)]);
                for (x, y) in [(3.5, 10.), (15., 3.5), (16., 16.)] {
                    frame.stroke(&Path::circle(Point::new(x, y), 2.5), stroke);
                }
            }
            Icon::Network => {
                line(&[(10., 7.), (10., 10.)]);
                line(&[(5., 13.), (5., 10.), (15., 10.), (15., 13.)]);
                for (x, y) in [(7., 2.), (2., 13.), (12., 13.)] {
                    let device = Path::rectangle(Point::new(x, y), iced::Size::new(6., 5.));
                    frame.stroke(&device, stroke);
                }
            }
            Icon::Volume => {
                let path = Path::new(|pen| {
                    pen.move_to(Point::new(3., 5.));
                    pen.bezier_curve_to(
                        Point::new(3., 1.),
                        Point::new(17., 1.),
                        Point::new(17., 5.),
                    );
                    pen.bezier_curve_to(
                        Point::new(17., 9.),
                        Point::new(3., 9.),
                        Point::new(3., 5.),
                    );
                    pen.line_to(Point::new(3., 15.));
                    pen.bezier_curve_to(
                        Point::new(3., 19.),
                        Point::new(17., 19.),
                        Point::new(17., 15.),
                    );
                    pen.line_to(Point::new(17., 5.));
                });
                frame.stroke(&path, stroke);
            }
            Icon::Docker => {
                for (x, y) in [
                    (3., 9.),
                    (7., 9.),
                    (11., 9.),
                    (7., 5.),
                    (11., 5.),
                    (11., 1.),
                ] {
                    line(&[(x, y), (x + 3., y), (x + 3., y + 3.), (x, y + 3.), (x, y)]);
                }
                line(&[
                    (1., 13.),
                    (14., 13.),
                    (16., 11.),
                    (19., 11.),
                    (16., 15.),
                    (12., 18.),
                    (5., 18.),
                    (2., 16.),
                    (1., 13.),
                ]);
            }
            Icon::Locate => {
                line(&[(10., 2.), (10., 6.)]);
                line(&[(10., 14.), (10., 18.)]);
                line(&[(2., 10.), (6., 10.)]);
                line(&[(14., 10.), (18., 10.)]);
                frame.stroke(&Path::circle(Point::new(10., 10.), 5.), stroke);
            }
            Icon::More => {
                for x in [4., 10., 16.] {
                    frame.fill(
                        &Path::circle(Point::new(x, 10.), 1.5),
                        self.1.resolve(theme),
                    );
                }
            }
            Icon::Check => {
                line(&[(3., 10.), (8., 15.), (17., 5.)]);
            }
            Icon::Warning => {
                line(&[(10., 2.), (19., 18.), (1., 18.), (10., 2.)]);
                line(&[(10., 7.), (10., 11.)]);
                line(&[(10., 14.), (10., 15.)]);
            }
            Icon::MarkdownSource => {
                for (y, end) in [(4., 17.), (8., 14.), (12., 17.), (16., 11.)] {
                    line(&[(3., y), (end, y)]);
                }
            }
            Icon::MarkdownSplit => {
                for (y, end) in [(4., 9.), (8., 7.), (12., 9.), (16., 6.)] {
                    line(&[(2., y), (end, y)]);
                }
                frame.stroke(
                    &Path::rectangle(Point::new(11., 4.), iced::Size::new(7., 12.)),
                    stroke,
                );
            }
            Icon::Image => {
                line(&[(2., 15.), (7., 10.), (10., 13.), (13., 9.), (18., 14.)]);
                frame.stroke(
                    &Path::rectangle(Point::new(2., 3.), iced::Size::new(16., 14.)),
                    stroke,
                );
                frame.stroke(&Path::circle(Point::new(7., 7.), 1.), stroke);
            }
            Icon::SplitDiff => {
                line(&[(2., 3.), (18., 3.), (18., 17.), (2., 17.), (2., 3.)]);
                line(&[(10., 3.), (10., 17.)]);
                line(&[(4., 7.), (7., 7.)]);
                line(&[(13., 11.), (16., 11.)]);
            }
            Icon::Sidebar => {
                line(&[(2., 4.), (18., 4.), (18., 16.), (2., 16.), (2., 4.)]);
                line(&[(7., 4.), (7., 16.)]);
            }
            Icon::UnifiedDiff => {
                line(&[(2., 3.), (18., 3.), (18., 17.), (2., 17.), (2., 3.)]);
                line(&[(5., 7.), (15., 7.)]);
                line(&[(5., 10.), (15., 10.)]);
                line(&[(5., 13.), (15., 13.)]);
            }
            Icon::Changes => {
                line(&[(1., 10.), (7., 10.)]);
                line(&[(13., 10.), (19., 10.)]);
                frame.stroke(&Path::circle(Point::new(10., 10.), 3.), stroke);
            }
            Icon::Update => {
                line(&[(10., 5.), (10., 11.5)]);
                line(&[(7., 8.5), (10., 11.5), (13., 8.5)]);
                line(&[(5., 12.5), (5., 15.), (15., 15.), (15., 12.5)]);
            }
            Icon::Fetch => {
                line(&[(10., 2.), (10., 13.)]);
                line(&[(6., 9.), (10., 13.), (14., 9.)]);
                line(&[(3., 14.), (3., 18.), (17., 18.), (17., 14.)]);
            }
            Icon::UpdateBranch => {
                let path = Path::new(|builder| {
                    builder.move_to(Point::new(5., 5.));
                    builder.line_to(Point::new(5., 15.));
                    builder.move_to(Point::new(15., 3.));
                    builder.line_to(Point::new(15., 7.));
                    builder.bezier_curve_to(
                        Point::new(15., 10.),
                        Point::new(12., 12.),
                        Point::new(8., 12.),
                    );
                    builder.move_to(Point::new(11., 9.));
                    builder.line_to(Point::new(8., 12.));
                    builder.line_to(Point::new(11., 15.));
                });
                let rounded = stroke
                    .with_line_cap(canvas::LineCap::Round)
                    .with_line_join(canvas::LineJoin::Round);
                frame.stroke(&path, rounded);
                for y in [3., 17.] {
                    frame.stroke(&Path::circle(Point::new(5., y), 2.), rounded);
                }
            }
            Icon::Upload => {
                line(&[(10., 14.), (10., 3.)]);
                line(&[(6., 7.), (10., 3.), (14., 7.)]);
                line(&[(3., 14.), (3., 18.), (17., 18.), (17., 14.)]);
            }
            Icon::Pin => {
                line(&[
                    (7., 2.),
                    (14., 2.),
                    (13., 8.),
                    (16., 12.),
                    (4., 12.),
                    (7., 8.),
                    (7., 2.),
                ]);
                line(&[(10., 12.), (10., 19.)]);
            }
            Icon::CollapseLines => {
                line(&[(3., 10.), (17., 10.)]);
                line(&[(10., 1.), (10., 6.)]);
                line(&[(7., 3.), (10., 6.), (13., 3.)]);
                line(&[(10., 19.), (10., 14.)]);
                line(&[(7., 17.), (10., 14.), (13., 17.)]);
            }
            Icon::ExpandLines => {
                line(&[(3., 10.), (17., 10.)]);
                line(&[(10., 6.), (10., 1.)]);
                line(&[(7., 4.), (10., 1.), (13., 4.)]);
                line(&[(10., 14.), (10., 19.)]);
                line(&[(7., 16.), (10., 19.), (13., 16.)]);
            }
            Icon::Back => {
                line(&[(9., 4.), (3., 10.), (9., 16.)]);
                line(&[(3., 10.), (17., 10.)]);
            }
            Icon::Folder => line(&[
                (2., 5.),
                (8., 5.),
                (10., 7.),
                (18., 7.),
                (18., 17.),
                (2., 17.),
                (2., 5.),
            ]),
            Icon::Plus => {
                line(&[(10., 3.), (10., 17.)]);
                line(&[(3., 10.), (17., 10.)]);
            }
            Icon::Right => line(&[(7., 5.), (12., 10.), (7., 15.)]),
            Icon::Up => line(&[(5., 12.), (10., 7.), (15., 12.)]),
            Icon::Down => line(&[(5., 7.), (10., 12.), (15., 7.)]),
            Icon::Drag => {
                for y in [5., 10., 15.] {
                    line(&[(6., y), (7., y)]);
                    line(&[(13., y), (14., y)]);
                }
            }
            Icon::Terminal => {
                line(&[(3., 5.), (8., 10.), (3., 15.)]);
                line(&[(10., 15.), (17., 15.)]);
            }
            Icon::Close => {
                line(&[(5., 5.), (15., 15.)]);
                line(&[(5., 15.), (15., 5.)]);
            }
            Icon::Search => {
                line(&[(12.5, 12.5), (17., 17.)]);
                frame.stroke(&Path::circle(Point::new(8.5, 8.5), 5.), stroke);
            }
            #[cfg(target_os = "windows")]
            Icon::Minimize => line(&[(4., 10.), (16., 10.)]),
            #[cfg(target_os = "windows")]
            Icon::Maximize => {
                line(&[(4., 4.), (16., 4.), (16., 16.), (4., 16.), (4., 4.)]);
            }
            #[cfg(target_os = "windows")]
            Icon::Restore => {
                line(&[(7., 5.), (7., 3.), (17., 3.), (17., 13.), (15., 13.)]);
                line(&[(3., 7.), (13., 7.), (13., 17.), (3., 17.), (3., 7.)]);
            }
            Icon::External => {
                line(&[(11., 3.), (17., 3.), (17., 9.)]);
                line(&[(17., 3.), (9., 11.)]);
                line(&[(8., 3.), (3., 3.), (3., 17.), (17., 17.), (17., 12.)]);
            }
            Icon::Play => {
                line(&[(7., 5.), (15., 10.), (7., 15.), (7., 5.)]);
                frame.stroke(&Path::circle(Point::new(10., 10.), 9.), stroke);
            }
            Icon::Branch => {
                line(&[(5., 5.), (5., 15.)]);
                line(&[(15., 5.), (15., 9.), (5., 13.)]);
                for (x, y) in [(5., 3.), (5., 17.), (15., 3.)] {
                    frame.stroke(&Path::circle(Point::new(x, y), 2.), stroke);
                }
            }
            Icon::Tag => {
                line(&[
                    (2., 3.),
                    (10., 3.),
                    (18., 11.),
                    (11., 18.),
                    (2., 9.),
                    (2., 3.),
                ]);
                frame.stroke(&Path::circle(Point::new(6., 7.), 1.5), stroke);
            }
            Icon::Refresh => {
                let arc = Path::new(|builder| {
                    builder.move_to(Point::new(16., 7.));
                    builder.bezier_curve_to(
                        Point::new(14.9, 4.7),
                        Point::new(12.6, 3.2),
                        Point::new(10., 3.2),
                    );
                    builder.bezier_curve_to(
                        Point::new(6.2, 3.2),
                        Point::new(3.2, 6.2),
                        Point::new(3.2, 10.),
                    );
                    builder.bezier_curve_to(
                        Point::new(3.2, 13.8),
                        Point::new(6.2, 16.8),
                        Point::new(10., 16.8),
                    );
                    builder.bezier_curve_to(
                        Point::new(12.6, 16.8),
                        Point::new(14.8, 15.4),
                        Point::new(16., 13.2),
                    );
                });
                let rounded = stroke
                    .with_line_cap(canvas::LineCap::Round)
                    .with_line_join(canvas::LineJoin::Round);
                frame.stroke(&arc, rounded);
                let arrow = Path::new(|builder| {
                    builder.move_to(Point::new(16., 3.));
                    builder.line_to(Point::new(16., 7.));
                    builder.line_to(Point::new(12., 7.));
                });
                frame.stroke(&arrow, rounded);
            }
            Icon::Settings => {
                let cog = Path::new(|builder| {
                    for index in 0..32 {
                        let angle = index as f32 * std::f32::consts::TAU / 32.;
                        let radius = if index % 4 == 0 || index % 4 == 3 {
                            8.8
                        } else {
                            6.8
                        };
                        let point =
                            Point::new(10. + radius * angle.cos(), 10. + radius * angle.sin());
                        if index == 0 {
                            builder.move_to(point);
                        } else {
                            builder.line_to(point);
                        }
                    }
                    builder.close();
                });
                frame.stroke(&cog, stroke);
                frame.stroke(&Path::circle(Point::new(10., 10.), 3.), stroke);
            }
        }
    }
}
