//! Resolve semantic colors against the current appearance.

use super::Theme;
use crate::domain::theme::ColorRole;
use iced::{Color, widget::text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tone(pub(crate) ColorRole);

impl Tone {
    pub(crate) fn resolve(self, theme: &Theme) -> Color {
        theme.color(self.0)
    }

    pub(crate) fn text(self) -> impl Fn(&Theme) -> text::Style {
        move |theme| text::Style {
            color: Some(self.resolve(theme)),
        }
    }
}

pub(super) const BACKGROUND: Tone = Tone(ColorRole::Background);
pub(super) const CODE_BACKGROUND: Tone = Tone(ColorRole::CodeBackground);
pub(super) const SIDEBAR: Tone = Tone(ColorRole::Sidebar);
pub(super) const ELEVATED: Tone = Tone(ColorRole::Elevated);
pub(super) const HOVER: Tone = Tone(ColorRole::Hover);
pub(super) const SELECTED: Tone = Tone(ColorRole::Selected);
pub(super) const TREE_SELECTED: Tone = Tone(ColorRole::TreeSelected);
pub(super) const COMPLETION_SELECTED: Tone = Tone(ColorRole::CompletionSelected);
pub(crate) const CODE_SELECTION: Tone = Tone(ColorRole::CodeSelection);
pub(super) const DIALOG: Tone = Tone(ColorRole::Dialog);
pub(super) const FOLD: Tone = Tone(ColorRole::Fold);
pub(super) const INPUT_BORDER: Tone = Tone(ColorRole::InputBorder);
pub(super) const INPUT_SELECTION: Tone = Tone(ColorRole::InputSelection);
pub(super) const OUTLINE: Tone = Tone(ColorRole::Outline);
pub(super) const PRIMARY: Tone = Tone(ColorRole::Primary);
pub(super) const PRIMARY_HOVER: Tone = Tone(ColorRole::PrimaryHover);
pub(super) const DANGER_HOVER: Tone = Tone(ColorRole::DangerHover);
pub(super) const PRIMARY_DISABLED: Tone = Tone(ColorRole::PrimaryDisabled);
pub(super) const PRIMARY_TEXT: Tone = Tone(ColorRole::PrimaryText);
pub(super) const THUMB: Tone = Tone(ColorRole::Thumb);
pub(super) const THUMB_HOVER: Tone = Tone(ColorRole::ThumbHover);
pub(crate) const TEXT: Tone = Tone(ColorRole::Text);
pub(crate) const MARKDOWN_TEXT: Tone = Tone(ColorRole::MarkdownText);
pub(crate) const MARKDOWN_INLINE_CODE: Tone = Tone(ColorRole::MarkdownInlineCode);
pub(crate) const MUTED: Tone = Tone(ColorRole::Muted);
pub(crate) const FAINT: Tone = Tone(ColorRole::Faint);
pub(crate) const LINE: Tone = Tone(ColorRole::Line);
pub(crate) const GREEN: Tone = Tone(ColorRole::Green);
pub(crate) const UPDATE_BLUE: Tone = Tone(ColorRole::UpdateBlue);
pub(crate) const BLUE: Tone = Tone(ColorRole::Blue);
pub(crate) const RED: Tone = Tone(ColorRole::Red);
pub(crate) const YELLOW: Tone = Tone(ColorRole::Yellow);
pub(crate) const PURPLE: Tone = Tone(ColorRole::Purple);
pub(crate) const PINK: Tone = Tone(ColorRole::Pink);
pub(crate) const PIPELINE_GREEN: Tone = Tone(ColorRole::PipelineGreen);
pub(crate) const PIPELINE_RED: Tone = Tone(ColorRole::PipelineRed);
pub(crate) const PIPELINE_BLUE: Tone = Tone(ColorRole::PipelineBlue);
pub(crate) const PIPELINE_AMBER: Tone = Tone(ColorRole::PipelineAmber);
pub(crate) const PIPELINE_GRAY: Tone = Tone(ColorRole::PipelineGray);
pub(crate) const PIPELINE_GLYPH: Tone = Tone(ColorRole::PipelineGlyph);
pub(crate) const GIT_ADDED: Tone = Tone(ColorRole::GitAdded);
pub(crate) const DIFF_MODIFIED: Tone = Tone(ColorRole::DiffModified);
pub(crate) const GIT_REMOVED: Tone = Tone(ColorRole::GitRemoved);
pub(super) const UNIFIED_MODIFIED: Tone = Tone(ColorRole::UnifiedModified);
pub(super) const FILE_ADDED: Tone = Tone(ColorRole::FileAdded);
pub(super) const FILE_MODIFIED: Tone = Tone(ColorRole::FileModified);
pub(crate) const ICON_GRAY: Tone = Tone(ColorRole::IconGray);
pub(crate) const ICON_BLUE: Tone = Tone(ColorRole::IconBlue);
pub(crate) const ICON_YELLOW: Tone = Tone(ColorRole::IconYellow);
pub(crate) const ICON_CYAN: Tone = Tone(ColorRole::IconCyan);
pub(crate) const ICON_ORANGE: Tone = Tone(ColorRole::IconOrange);
pub(crate) const ICON_PINK: Tone = Tone(ColorRole::IconPink);
