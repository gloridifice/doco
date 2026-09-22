use super::plan::Plan;
use crate::{Project, markdown, templates};
use anyhow::{Context, Result, bail};

fn is_managed(text: &str) -> bool {
    markdown::body_lines(text).iter().any(|(_, line)| {
        regex::Regex::new(r"^<!-- doco:managed template=v[0-9]+ -->$")
            .unwrap()
            .is_match(line.trim())
    })
}

fn is_unmarked_template(old: &str, generated: &str) -> bool {
    let normalized = markdown::normalize(generated);
    let unmarked = normalized.replace(&format!("{}\n", templates::MARKER), "");
    markdown::normalize(old) == unmarked
}

fn parsed_skill_version(text: &str) -> Option<u64> {
    let candidates: Vec<_> = markdown::body_lines(text)
        .into_iter()
        .map(|(_, line)| line.trim())
        .filter(|line| line.starts_with("<!-- doco:skill"))
        .collect();
    let [candidate] = candidates.as_slice() else {
        return None;
    };
    let captures = regex::Regex::new(r"^<!-- doco:skill version=v(0|[1-9][0-9]*) -->$")
        .unwrap()
        .captures(candidate)?;
    captures.get(1)?.as_str().parse().ok()
}

fn bundled_skill() -> &'static str {
    templates::FILES
        .iter()
        .find_map(|(path, content)| (*path == "SKILL.md").then_some(content.as_str()))
        .expect("built-in SKILL.md exists")
}

fn bundled_version() -> Result<u64> {
    let version = parsed_skill_version(bundled_skill())
        .context("built-in SKILL.md has no unique valid skill version")?;
    if version == 0 {
        bail!("built-in SKILL.md skill version must be greater than v0");
    }
    Ok(version)
}

pub fn update(old: Option<&str>, generated: &str, refresh: bool) -> Result<String> {
    if let Some(text) = old.and_then(|s| s.strip_prefix('\u{feff}')) {
        return Ok(format!(
            "\u{feff}{}",
            update(Some(text), generated, refresh)?
        ));
    }
    let Some(old) = old else {
        return Ok(generated.to_string());
    };
    let normalized = markdown::normalize(generated);
    if markdown::normalize(old) == normalized {
        return Ok(old.to_string());
    }
    if is_unmarked_template(old, generated) {
        return Ok(markdown::styled(&normalized, old));
    }
    if !is_managed(old) {
        bail!("same-name file is not recognized as doco-managed; --refresh cannot overwrite it");
    }
    if !refresh {
        bail!(
            "managed file differs; --refresh explicitly overwrites this generated file, including manual edits\n{}",
            similar::TextDiff::from_lines(old, &markdown::styled(&normalized, old))
                .unified_diff()
                .context_radius(2)
        );
    }
    Ok(markdown::styled(&normalized, old))
}

pub fn install(plan: &mut Plan, project: &Project, directory: &str, refresh: bool) {
    let skill_relative = format!("{directory}/SKILL.md");
    let existing_skill = match plan.effective_text(project, &skill_relative) {
        Ok(text) => text,
        Err(error) => {
            plan.conflicts.push(format!("{skill_relative}: {error:#}"));
            return;
        }
    };
    if existing_skill.is_none() && project.path(directory).is_dir() {
        match std::fs::read_dir(project.path(directory)).map(|mut entries| entries.next().is_some())
        {
            Ok(true) => {
                plan.conflicts.push(format!(
                    "{directory}: nonempty same-name skill has no SKILL.md; manual merge required"
                ));
                return;
            }
            Err(e) => {
                plan.conflicts.push(format!("{directory}: {e}"));
                return;
            }
            _ => {}
        }
    }

    let current_version = match bundled_version() {
        Ok(version) => version,
        Err(error) => {
            plan.conflicts.push(format!("built-in skill: {error:#}"));
            return;
        }
    };
    let upgrade = match existing_skill.as_deref() {
        None => true,
        Some(old) => {
            if !is_managed(old) && !is_unmarked_template(old, bundled_skill()) {
                plan.conflicts.push(format!(
                    "{skill_relative}: same-name file is not recognized as doco-managed; --refresh cannot overwrite it"
                ));
                return;
            }
            current_version > parsed_skill_version(old).unwrap_or(0)
        }
    };
    let replace_managed = refresh || upgrade;

    let mut files = Vec::new();
    for (file, content) in templates::FILES
        .iter()
        .filter(|(file, _)| *file != "SKILL.md")
        .chain(
            templates::FILES
                .iter()
                .filter(|(file, _)| *file == "SKILL.md"),
        )
    {
        let relative = format!("{directory}/{file}");
        if let Some(op) = plan.prepare_file(project, &relative, |old| {
            update(old, content, replace_managed)
        }) {
            files.push(op);
        }
    }
    plan.file_group(
        project,
        &format!("skill bundle {directory}"),
        directory,
        files,
    );
}

#[cfg(test)]
mod tests {
    use super::{parsed_skill_version, update};
    use crate::templates::MARKER;

    #[test]
    fn generated_line_endings_do_not_create_false_conflicts() {
        let generated = format!("{MARKER}\r\n# Skill\r\n");
        let installed = format!("{MARKER}\n# Skill\n");
        assert_eq!(
            update(Some(&installed), &generated, false).unwrap(),
            installed
        );

        let unmarked = "# Skill\n";
        assert_eq!(
            update(Some(unmarked), &generated, false).unwrap(),
            installed
        );
    }

    #[test]
    fn parses_one_canonical_skill_version_and_defaults_other_forms_to_v0() {
        for (text, expected) in [
            ("<!-- doco:skill version=v0 -->", Some(0)),
            ("<!-- doco:skill version=v1 -->", Some(1)),
            ("<!-- doco:skill version=v10 -->", Some(10)),
            ("no version", None),
            ("<!-- doco:skill version=v01 -->", None),
            ("<!-- doco:skill version=1 -->", None),
            (
                "<!-- doco:skill version=v1 -->\n<!-- doco:skill version=v2 -->",
                None,
            ),
            (
                "```md\n<!-- doco:skill version=v9 -->\n```\n<!-- doco:skill version=v2 -->",
                Some(2),
            ),
            ("<!-- doco:skill version=v18446744073709551616 -->", None),
        ] {
            assert_eq!(parsed_skill_version(text), expected, "{text}");
        }
    }
}
