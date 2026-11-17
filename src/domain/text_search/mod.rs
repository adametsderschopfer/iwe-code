//! Match source text without depending on widgets or storage.
use regex::{Regex, RegexBuilder};
use std::ops::Range;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Options {
    pub case: bool,
    pub word: bool,
    pub regex: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Match {
    pub range: Range<usize>,
    pub line: usize,
    pub column: usize,
    pub byte_column: usize,
    pub end_byte_column: usize,
    pub end_column: usize,
    pub text: String,
}

pub(crate) fn compile(query: &str, options: &Options) -> Result<Regex, regex::Error> {
    let pattern = if options.regex {
        query.to_owned()
    } else {
        regex::escape(query)
    };
    let pattern = if options.word {
        format!(r"\b(?:{pattern})\b")
    } else {
        pattern
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(!options.case)
        .multi_line(true)
        .size_limit(2 * 1024 * 1024)
        .build()
}

pub(crate) fn matches(text: &str, pattern: &Regex, limit: usize) -> Vec<Match> {
    let mut line = 1;
    let mut start = 0;
    pattern
        .find_iter(text)
        .take(limit)
        .map(|found| {
            line += text.as_bytes()[start..found.start()]
                .iter()
                .filter(|byte| **byte == b'\n')
                .count();
            start = found.start();
            let line_start = text[..found.start()].rfind('\n').map_or(0, |at| at + 1);
            let line_end = text[found.start()..]
                .find('\n')
                .map_or(text.len(), |at| at + found.start());
            Match {
                range: found.range(),
                byte_column: found.start() - line_start,
                end_byte_column: found.end().min(line_end) - line_start,
                line,
                column: text[line_start..found.start()].chars().count(),
                end_column: text[line_start..found.end().min(line_end)].chars().count(),
                text: text[line_start..line_end].chars().take(1000).collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
