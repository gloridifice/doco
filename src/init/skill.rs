use super::plan::Plan;
use crate::{Project, markdown, safety, templates};
use anyhow::{Result, bail};

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
    let unmarked = normalized.replace(&format!("{}\n", templates::MARKER), "");
    if markdown::normalize(old) == unmarked {
        return Ok(markdown::styled(&normalized, old));
    }
    let managed = markdown::body_lines(old).iter().any(|(_, line)| {
        regex::Regex::new(r"^<!-- doco:managed template=v[0-9]+ -->$")
            .unwrap()
            .is_match(line.trim())
    });
    if !managed {
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
    plan.directory(project, directory);
    let skill = project.path(format!("{directory}/SKILL.md"));
    if !skill.exists() && project.path(directory).is_dir() {
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
    for (file, content) in templates::FILES {
        plan.file(project, &format!("{directory}/{file}"), |old| {
            update(old, content, refresh)
        });
    }
}

pub fn compatible(project: &Project, directory: &str) -> Result<bool> {
    safety::inspect(project.root(), &project.path(directory))?;
    if !project.path(directory).exists() {
        return Ok(false);
    }
    for (file, generated) in templates::FILES {
        let path = project.path(format!("{directory}/{file}"));
        let Some(snapshot) = safety::snapshot(project.root(), &path)? else {
            return Ok(false);
        };
        let old = safety::decode(&snapshot.bytes)?;
        if markdown::normalize(&old) != markdown::normalize(generated) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::update;
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
}
