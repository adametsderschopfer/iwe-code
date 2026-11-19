//! Match source text without depending on widgets or storage.
use regex::{Regex, RegexBuilder};
use std::{ops::Range, sync::Arc};

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
    pub text: Arc<str>,
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
    let mut line_start = 0;
    let mut line_end = text.find('\n').unwrap_or(text.len());
    let mut snippet: Option<Arc<str>> = None;
    let mut cursor = 0;
    let mut column = 0;
    pattern
        .find_iter(text)
        .take(limit)
        .map(|found| {
            while found.start() > line_end {
                line += 1;
                line_start = line_end + 1;
                line_end = text[line_start..]
                    .find('\n')
                    .map_or(text.len(), |offset| line_start + offset);
                cursor = line_start;
                column = 0;
                snippet = None;
            }
            let start_column = column + text[cursor..found.start()].chars().count();
            cursor = found.end().min(line_end);
            column = start_column + text[found.start()..cursor].chars().count();
            let snippet = snippet.get_or_insert_with(|| {
                text[line_start..line_end]
                    .chars()
                    .take(1000)
                    .collect::<String>()
                    .into()
            });
            Match {
                range: found.range(),
                byte_column: found.start() - line_start,
                end_byte_column: cursor - line_start,
                line,
                column: start_column,
                end_column: column,
                text: Arc::clone(snippet),
            }
        })
        .collect()
}

/// Build an edit within the editor budget, including capture expansion.
pub(crate) fn replace(
    text: &str,
    pattern: &Regex,
    replacement: &str,
    expand: bool,
    selected: Option<usize>,
    limit: usize,
) -> Result<Option<String>, super::Failure> {
    enum Part<'a> {
        Literal(std::borrow::Cow<'a, str>),
        Capture(usize),
    }
    let mut parts = Vec::new();
    if expand && replacement.contains('$') {
        let mut literal = String::new();
        regex_automata::util::interpolate::string(
            replacement,
            |index, literal| {
                if !literal.is_empty() {
                    parts.push(Part::Literal(std::mem::take(literal).into()));
                }
                parts.push(Part::Capture(index));
            },
            |name| {
                pattern
                    .capture_names()
                    .position(|candidate| candidate == Some(name))
            },
            &mut literal,
        );
        if !literal.is_empty() {
            parts.push(Part::Literal(literal.into()));
        }
    } else {
        parts.push(Part::Literal(replacement.into()));
    }
    let mut result = String::new();
    let mut append = |part: &str| {
        if part.len() > limit - result.len() {
            return Err(super::Failure::TooLarge);
        }
        result.push_str(part);
        Ok(())
    };
    let mut end = None;
    for captures in pattern.captures_iter(text).skip(selected.unwrap_or(0)) {
        let found = captures
            .get(0)
            .expect("Regex captures include the full match");
        append(&text[end.unwrap_or(0)..found.start()])?;
        for part in &parts {
            match part {
                Part::Literal(value) => append(value)?,
                Part::Capture(index) => {
                    if let Some(value) = captures.get(*index) {
                        append(value.as_str())?;
                    }
                }
            }
        }
        end = Some(found.end());
        if selected.is_some() {
            break;
        }
    }
    let Some(end) = end else { return Ok(None) };
    append(&text[end..])?;
    Ok(Some(result))
}

#[cfg(test)]
mod tests;
