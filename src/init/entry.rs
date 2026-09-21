use crate::{markdown, templates};
use anyhow::{Result, bail};
use std::ops::Range;

const START: &str = "<!-- DOCO:START -->";
const END: &str = "<!-- DOCO:END -->";
const ENTRY_VERSION: u64 = 1;

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
            "invalid DOCO marker closure (missing, nested, duplicated, or out of order); delete the entire DOCO block and retry"
        );
    }
    Ok(Some(markers[0].0.start..markers[1].0.end))
}
fn generated_block(skill: &str) -> String {
    format!(
        "{START}\n<!-- doco:entry template=v{ENTRY_VERSION} -->\n{}{END}\n",
        templates::navigation(skill)
    )
}

fn parsed_version(block: &str) -> Option<u64> {
    let candidates: Vec<_> = markdown::body_lines(block)
        .into_iter()
        .map(|(_, line)| line.trim())
        .filter(|line| line.starts_with("<!-- doco:entry"))
        .collect();
    let [candidate] = candidates.as_slice() else {
        return None;
    };
    let captures = regex::Regex::new(r"^<!-- doco:entry template=v(0|[1-9][0-9]*) -->$")
        .unwrap()
        .captures(candidate)?;
    captures.get(1)?.as_str().parse().ok()
}

pub(crate) fn installed(text: &str, skill: &str) -> Result<bool> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    markdown::safe_append(text)?;
    if let Some(range) = block_range(text)? {
        let known = [
            ".agents/skills/doco",
            ".claude/skills/doco",
            ".pi/skills/doco",
            ".codex/skills/doco",
        ];
        let referenced: Vec<_> = known
            .into_iter()
            .filter(|path| text[range.clone()].contains(path))
            .collect();
        if referenced.len() > 1 {
            bail!("managed doco entry references multiple skill directories");
        }
        if let Some(referenced) = referenced.first()
            && *referenced != skill
        {
            bail!("managed doco entry references {referenced}, expected {skill}");
        }
        return Ok(true);
    }
    Ok(false)
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
        if markdown::normalize(&text[range.clone()]).trim_end() == block.trim_end() {
            return Ok(text.to_string());
        }
        let upgrade = ENTRY_VERSION > parsed_version(&text[range.clone()]).unwrap_or(0);
        if !refresh && !upgrade {
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
        return Ok(None);
    };
    for path in [".agents/skills/doco", ".claude/skills/doco"] {
        if markdown::normalize(&text[range.clone()]).trim_end() == generated_block(path).trim_end()
        {
            return Ok(Some(path.to_string()));
        }
    }
    bail!("imported doco navigation is not a recognized valid template; manual review required")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_invalid_and_duplicate_entry_versions_upgrade_from_v0() {
        let skill = ".agents/skills/doco";
        let generated = generated_block(skill);
        for old in [
            generated.replace("<!-- doco:entry template=v1 -->\n", ""),
            generated.replace("template=v1", "template=invalid"),
            generated.replace(
                "<!-- doco:entry template=v1 -->",
                "<!-- doco:entry template=v1 -->\n<!-- doco:entry template=v1 -->",
            ),
        ] {
            assert_eq!(
                update(Some(&old), "Project instructions", skill, false).unwrap(),
                generated
            );
        }
    }

    #[test]
    fn installed_requires_a_managed_navigation() {
        let skill = ".agents/skills/doco";
        assert!(installed(&generated_block(skill), skill).unwrap());
        assert!(!installed(&templates::navigation(skill), skill).unwrap());
        assert!(!installed("# Custom instructions\n", skill).unwrap());
    }
}
