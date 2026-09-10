use crate::{markdown, templates};
use anyhow::{Result, bail};
use std::ops::Range;

const START: &str = "<!-- DOCO:START -->";
const END: &str = "<!-- DOCO:END -->";

pub fn block_range(text: &str) -> Result<Option<Range<usize>>> {
    let lines = markdown::body_lines(text);
    let markers: Vec<_> = lines
        .iter()
        .filter(|(_, l)| matches!(l.trim(), START | END))
        .collect();
    if markers.is_empty() {
        return Ok(None);
    }
    if markers.len() != 2 || markers[0].1.trim() != START || markers[1].1.trim() != END {
        bail!(
            "missing, nested or duplicate DOCO markers; manual repair required even with --refresh"
        );
    }
    Ok(Some(markers[0].0.start..markers[1].0.end))
}
fn suspect(text: &str) -> bool {
    markdown::body_lines(text).iter().any(|(_, line)| {
        let lower = line.to_lowercase();
        let trimmed = lower.trim();
        (trimmed.starts_with('#') && (trimmed.contains("doco") || trimmed.contains("多科")))
            || (trimmed.contains("skills/doco") || trimmed.contains("doco skill"))
            || (trimmed.starts_with("- ") && trimmed.contains("doco "))
    })
}
fn generated_block(skill: &str) -> String {
    format!("{START}\n{}{END}\n", templates::navigation(skill))
}

pub fn update(existing: Option<&str>, title: &str, skill: &str, refresh: bool) -> Result<String> {
    if let Some(text) = existing.and_then(|s| s.strip_prefix('\u{feff}')) {
        return Ok(format!(
            "\u{feff}{}",
            update(Some(text), title, skill, refresh)?
        ));
    }
    let block = generated_block(skill);
    let Some(text) = existing else {
        return Ok(format!("# {title}\n\n{block}"));
    };
    markdown::safe_append(text)?;
    if let Some(range) = block_range(text)? {
        let mut outside = text.to_string();
        outside.replace_range(range.clone(), "");
        if suspect(&outside) {
            bail!(
                "suspected additional custom doco workflow outside managed block; manual merge required"
            );
        }
        if markdown::normalize(&text[range.clone()]).trim_end() == block.trim_end() {
            return Ok(text.to_string());
        }
        if !refresh {
            bail!(
                "managed navigation differs; review diff and use --refresh to replace this block only\n{}",
                similar::TextDiff::from_lines(&text[range], &markdown::styled(&block, text))
                    .unified_diff()
            );
        }
        let mut output = text.to_string();
        output.replace_range(range, &markdown::styled(&block, text));
        return Ok(output);
    }
    // Adopt a complete known unmarked body, but never a fenced example.
    let plain = markdown::styled(&templates::navigation(skill), text);
    let body = markdown::body_lines(text);
    let matches: Vec<_> = text
        .match_indices(plain.trim_end_matches(['\r', '\n']))
        .filter(|(start, matched)| {
            body.iter().any(|(r, _)| r.start == *start)
                && text
                    .as_bytes()
                    .get(start + matched.len())
                    .is_none_or(|b| matches!(b, b'\r' | b'\n'))
        })
        .collect();
    if matches.len() > 1 {
        bail!("multiple unmarked doco navigation bodies; manual merge required");
    }
    if let Some((start, matched)) = matches.first() {
        let end = start + matched.len();
        let mut outside = text.to_string();
        outside.replace_range(*start..end, "");
        if suspect(&outside) {
            bail!("additional custom doco workflow; manual merge required");
        }
        let nl = markdown::newline(text);
        let mut output = text.to_string();
        output.replace_range(*start..end, &format!("{START}{nl}{matched}{nl}{END}"));
        return Ok(output);
    }
    if suspect(text) {
        bail!("suspected custom doco workflow; preserve original and merge manually");
    }
    let nl = markdown::newline(text);
    let separator = if text.is_empty() || text.ends_with(&format!("{nl}{nl}")) {
        "".to_string()
    } else if text.ends_with('\n') {
        nl.to_string()
    } else {
        format!("{nl}{nl}")
    };
    Ok(format!(
        "{text}{separator}{}",
        markdown::styled(&block, text)
    ))
}

/// Deliberately supports only an unambiguous standalone project-root import.
pub fn imports_agents(text: &str) -> Result<bool> {
    let mut found = false;
    let import = regex::Regex::new(r"(?:^|\s)@[^\s`]+\.md").unwrap();
    for (_, line) in markdown::body_lines(text) {
        let trimmed = line.trim();
        if trimmed == "@AGENTS.md" || trimmed == "@./AGENTS.md" {
            if found {
                bail!("duplicate @AGENTS.md imports; manual merge required");
            }
            found = true;
        } else if import.is_match(line) {
            bail!("complex or unsupported Markdown import: {line}; manual review required");
        }
    }
    Ok(found)
}

pub fn visible_skill(text: &str) -> Result<Option<String>> {
    let text = text.trim_start_matches('\u{feff}');
    markdown::safe_append(text)?;
    let Some(range) = block_range(text)? else {
        if suspect(text) {
            bail!(
                "imported AGENTS.md has custom/unmarked doco instructions; initialize or merge it explicitly first"
            );
        }
        return Ok(None);
    };
    for path in [
        ".agents/skills/doco",
        ".pi/skills/doco",
        ".claude/skills/doco",
    ] {
        if markdown::normalize(&text[range.clone()]).trim_end() == generated_block(path).trim_end()
        {
            return Ok(Some(path.to_string()));
        }
    }
    bail!("imported doco navigation is not a recognized valid template; manual review required")
}
