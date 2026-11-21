//! Map Git changes to current editor lines, including deletion anchors at EOF.

use similar::{DiffTag, TextDiff};
use std::{ops::Range, time::Duration};

#[derive(Debug, Clone)]
pub(crate) struct Hunk {
    pub rows: Range<usize>,
    pub old: Range<usize>,
    pub new: Range<usize>,
}

pub(crate) fn compare(before: &str, after: &str) -> Vec<Hunk> {
    if before == after {
        return Vec::new();
    }
    let diff = TextDiff::configure()
        .timeout(Duration::from_millis(250))
        .diff_lines(before, after);
    let last = after.bytes().filter(|byte| *byte == b'\n').count();
    diff.ops()
        .iter()
        .filter(|op| op.tag() != DiffTag::Equal)
        .map(|op| {
            let new = op.new_range();
            let start = new.start.min(last);
            Hunk {
                old: op.old_range(),
                rows: start..new.end.max(start + 1),
                new,
            }
        })
        .collect()
}

pub(crate) fn at(hunks: &[Hunk], row: usize) -> Option<usize> {
    let index = hunks.partition_point(|hunk| hunk.rows.end <= row);
    hunks.get(index)?.rows.contains(&row).then_some(index)
}

#[cfg(test)]
mod tests;
