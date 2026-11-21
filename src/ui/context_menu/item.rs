//! Keep action labels aligned and distinguish destructive commands in context menus.

use super::{TEXT_SIZE, row};
use crate::{
    i18n::Key,
    ui::{
        ellipsis,
        icons::{Icon, sized_icon},
        theme,
    },
};
use iced::{
    Alignment, Element, Theme,
    widget::{Button, Row, button, space},
};

const ICON_SIZE: f32 = 16.;
const ICON_SPACING: f32 = 8.;
pub(crate) const ICON_COLUMN_WIDTH: f32 = ICON_SIZE + ICON_SPACING;

fn symbol(key: Key) -> Option<Icon> {
    use Key::*;
    Some(match key {
        FileTrash | DeleteSpace | GitDelete | GitBulkDelete | GitHistoryDeleteTag => Icon::Trash,
        RemoveProjectFromSpace | RemoveSelectedProjects => Icon::Minus,
        FileCopy | CopyPath | CopyPathMenu | CopyAbsolutePath | GitHistoryCopy | FileDuplicate
        | DuplicateLine => Icon::Duplicate,
        FileCut => Icon::Cut,
        FilePaste => Icon::Paste,
        FileNew | FileNewFile | AddProject => Icon::Plus,
        ProjectOpenFolder | FileNewFolder | FileReveal | DeploymentOpenLocal => Icon::Folder,
        FileRename | EditSpace | EditorRenameLocal | GitRename | GitHistoryMessage => Icon::Rename,
        FindFile | FindFiles | EditorUsages => Icon::Search,
        GoToDefinition | GitHistoryTip => Icon::Locate,
        PinProject | UnpinProject => Icon::Pin,
        SidebarMenuBack => Icon::Back,
        DeploymentSettings => Icon::Settings,
        DeploymentCompare | DeploymentCompareLocal => Icon::SplitDiff,
        DeploymentUpload | GitPush | GitBulkPush | GitHistoryPushTag => Icon::Upload,
        DeploymentDownload | GitBulkPull => Icon::Fetch,
        DeploymentOpenRemote | GitHistoryBrowser => Icon::External,
        GitNewBranch => Icon::Branch,
        GitUpdate => Icon::Refresh,
        GitHistoryTag => Icon::Tag,

        Terminal => Icon::Terminal,
        _ => return None,
    })
}

pub(crate) fn label<'a, M: 'a>(key: Key, value: String) -> Row<'a, M> {
    icon_label(symbol(key), value)
}

pub(crate) fn icon_label<'a, M: 'a>(symbol: Option<Icon>, value: String) -> Row<'a, M> {
    let icon: Element<'a, M> = match symbol {
        Some(icon) => sized_icon(icon, theme::MUTED, ICON_SIZE),
        None => space().width(ICON_SIZE).into(),
    };
    iced::widget::row![icon, ellipsis::text(value, TEXT_SIZE)]
        .spacing(ICON_SPACING)
        .align_y(Alignment::Center)
}

pub(crate) fn action_style(
    key: Key,
    selected: bool,
) -> impl Fn(&Theme, button::Status) -> button::Style {
    let destructive = matches!(
        key,
        Key::FileTrash
            | Key::DeleteSpace
            | Key::RemoveProjectFromSpace
            | Key::RemoveSelectedProjects
            | Key::GitDelete
            | Key::GitBulkDelete
            | Key::GitHistoryDeleteTag
            | Key::GitHistoryReset
    );
    move |theme, status| {
        let mut style = theme::action(selected)(theme, status);
        if destructive && status != button::Status::Disabled {
            style.text_color = theme::RED.resolve(theme);
        }
        style
    }
}

pub(crate) fn action<'a, M: Clone + 'a>(key: Key, value: String) -> Button<'a, M> {
    row(label(key, value)).style(action_style(key, false))
}
