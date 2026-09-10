//! Small mechanical Markdown helpers; not a semantic document validator.
use anyhow::{Result, bail};
use pulldown_cmark::{Event, Parser, Tag};
use std::{
    ops::Range,
    path::{Component, Path, PathBuf},
};

/// Byte offsets preserve untouched text, BOMs and line endings during insertion.
pub fn body_lines(text: &str) -> Vec<(Range<usize>, &str)> {
    let code: Vec<_> = Parser::new(text)
        .into_offset_iter()
        .filter_map(|(e, r)| matches!(e, Event::Start(Tag::CodeBlock(_))).then_some(r))
        .collect();
    let mut offset = 0;
    text.split_inclusive('\n')
        .filter_map(|line| {
            let range = offset..offset + line.len();
            offset += line.len();
            if code
                .iter()
                .any(|r| r.start < range.end && r.end > range.start)
            {
                return None;
            }
            Some((
                range,
                line.trim_end_matches(['\r', '\n'])
                    .trim_start_matches('\u{feff}'),
            ))
        })
        .collect()
}

/// Visible non-code lines for task/section fields. Root navigation markers use
/// body_lines instead because their standalone HTML comments are meaningful.
pub fn content_lines(text: &str) -> Vec<(Range<usize>, &str)> {
    let mut in_comment = false;
    let delimiters = regex::Regex::new(r"<!--|-->").unwrap();
    body_lines(text)
        .into_iter()
        .filter(|(_, line)| {
            let hidden = in_comment || line.contains("<!--");
            for token in delimiters.find_iter(line) {
                in_comment = token.as_str() == "<!--";
            }
            !hidden
        })
        .collect()
}

pub fn safe_append(text: &str) -> Result<()> {
    let without_bom = text.trim_start_matches('\u{feff}');
    if without_bom.lines().next() == Some("---")
        && !without_bom
            .lines()
            .skip(1)
            .any(|l| matches!(l.trim(), "---" | "..."))
    {
        bail!("unclosed front matter or ambiguous leading separator; manual merge required");
    }
    // CommonMark accepts EOF-closed fences; appending there would silently enter the example.
    let mut fence: Option<(char, usize)> = None;
    for line in text.lines() {
        let line = line.trim_start_matches('\u{feff}');
        let trimmed = line.trim_start();
        if line.len() - trimmed.len() > 3 {
            continue;
        }
        let Some(first) = trimmed.chars().next() else {
            continue;
        };
        if first != '`' && first != '~' {
            continue;
        }
        let count = trimmed.chars().take_while(|c| *c == first).count();
        if count < 3 {
            continue;
        }
        match fence {
            None => fence = Some((first, count)),
            Some((kind, size))
                if kind == first && count >= size && trimmed[count..].trim().is_empty() =>
            {
                fence = None
            }
            _ => {}
        }
    }
    if fence.is_some() {
        bail!("unclosed Markdown fence; cannot safely insert navigation");
    }
    let body = body_lines(text)
        .into_iter()
        .map(|(_, s)| s)
        .collect::<Vec<_>>()
        .join("\n");
    let mut comment = false;
    for token in regex::Regex::new(r"<!--|-->").unwrap().find_iter(&body) {
        match token.as_str() {
            "<!--" if !comment => comment = true,
            "-->" if comment => comment = false,
            _ => bail!("ambiguous HTML comment structure"),
        }
    }
    if comment {
        bail!("unclosed HTML comment");
    }
    // Raw HTML can absorb subsequent Markdown. Deliberately reject uncertain containers.
    let lower = body.to_ascii_lowercase();
    for tag in [
        "script", "style", "pre", "textarea", "details", "div", "table",
    ] {
        let open = regex::Regex::new(&format!(r"<{tag}(?:\s|>)"))
            .unwrap()
            .find_iter(&lower)
            .count();
        let close = lower.matches(&format!("</{tag}>")).count();
        if open != close {
            bail!("unbalanced HTML <{tag}>; manual merge required");
        }
    }
    Ok(())
}

pub fn newline(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}
pub fn normalize(text: &str) -> String {
    text.trim_start_matches('\u{feff}').replace("\r\n", "\n")
}
pub fn styled(text: &str, original: &str) -> String {
    text.replace('\n', newline(original))
}

#[derive(Debug)]
pub struct Section {
    pub title: String,
    pub content: String,
    pub body: Range<usize>,
}
pub fn sections(text: &str) -> Vec<Section> {
    let headers: Vec<_> = content_lines(text)
        .into_iter()
        .filter_map(|(r, line)| line.strip_prefix("## ").map(|t| (r, t.trim().to_string())))
        .collect();
    headers
        .iter()
        .enumerate()
        .map(|(i, (r, title))| {
            let end = headers
                .get(i + 1)
                .map_or(text.len(), |(next, _)| next.start);
            Section {
                title: title.clone(),
                content: text[r.end..end].trim().to_string(),
                body: r.end..end,
            }
        })
        .collect()
}
pub fn title_matches(title: &str, names: &[&str]) -> bool {
    let title = title
        .trim_start_matches(|c: char| c.is_ascii_digit() || ".、) ".contains(c))
        .to_lowercase();
    names.iter().any(|n| title == n.to_lowercase())
}
pub fn result_section(text: &str) -> Option<Section> {
    sections(text)
        .into_iter()
        .find(|s| title_matches(&s.title, &["结果", "Result", "Results", "Outcome"]))
}

pub fn links(text: &str) -> Vec<String> {
    let attributes =
        regex::Regex::new(r#"(?i)\b(?:href|src)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#).unwrap();
    let mut links = Vec::new();
    for event in Parser::new(text) {
        match event {
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => {
                links.push(dest_url.to_string())
            }
            Event::Code(code) if code.contains('/') || code.ends_with(".md") => {
                links.push(code.to_string())
            }
            Event::Html(html) | Event::InlineHtml(html)
                if !html.trim_start().starts_with("<!--") =>
            {
                for cap in attributes.captures_iter(&html) {
                    if let Some(target) = cap.get(1).or_else(|| cap.get(2)).or_else(|| cap.get(3)) {
                        links.push(target.as_str().to_string());
                    }
                }
            }
            _ => {}
        }
    }
    links
}

/// Compare reference paths using platform path semantics (managed names are ASCII).
pub fn within(path: &Path, parent: &Path) -> bool {
    #[cfg(windows)]
    {
        let normalize = |p: &Path| {
            let value = p.to_string_lossy().replace('\\', "/").to_lowercase();
            if let Some(unc) = value.strip_prefix("//?/unc/") {
                format!("//{unc}")
            } else {
                value.strip_prefix("//?/").unwrap_or(&value).to_string()
            }
        };
        let path = normalize(path);
        let parent = normalize(parent);
        path == parent || path.starts_with(&format!("{}/", parent.trim_end_matches('/')))
    }
    #[cfg(not(windows))]
    {
        path.starts_with(parent)
    }
}

/// Resolve only local paths. URL decoding makes `%77ork/` no escape from archive checks.
pub fn local_link(base: &Path, target: &str) -> Option<PathBuf> {
    let target = target.split(['#', '?']).next()?.trim();
    if target.is_empty()
        || target.contains("://")
        || target.starts_with("mailto:")
        || target.starts_with("doco:")
    {
        return None;
    }
    let mut bytes = Vec::new();
    let raw = target.as_bytes();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] == b'%' && i + 2 < raw.len() {
            if let Ok(value) = u8::from_str_radix(std::str::from_utf8(&raw[i + 1..i + 3]).ok()?, 16)
            {
                bytes.push(value);
                i += 3;
                continue;
            }
        }
        bytes.push(raw[i]);
        i += 1;
    }
    let decoded = String::from_utf8(bytes).ok()?.replace('\\', "/");
    let mut path = PathBuf::new();
    for part in base.join(decoded).components() {
        match part {
            Component::ParentDir => {
                path.pop();
            }
            Component::CurDir => {}
            _ => path.push(part),
        }
    }
    Some(path)
}
