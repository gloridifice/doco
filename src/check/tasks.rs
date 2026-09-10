use crate::markdown;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub struct Task {
    pub id: String,
    pub done: bool,
    pub dependencies: Vec<String>,
    pub acceptance: bool,
}
#[derive(Default)]
pub struct Tasks {
    pub items: Vec<Task>,
    pub errors: Vec<String>,
    pub verification: bool,
    pub blockers: Vec<String>,
}

fn field(line: &str) -> Option<(String, &str)> {
    let line = line.trim().trim_start_matches(['-', '*', '+']).trim();
    let (key, value) = line.split_once(':').or_else(|| line.split_once('：'))?;
    Some((key.trim().to_lowercase(), value.trim()))
}
pub fn is_none(value: &str) -> bool {
    matches!(
        value
            .trim()
            .trim_end_matches(['.', '。'])
            .to_lowercase()
            .as_str(),
        "none"
            | "no"
            | "无"
            | "无阻塞"
            | "无未决问题"
            | "not applicable"
            | "n/a"
            | "-"
            | "已解决"
            | "resolved"
    )
}
pub fn parse(text: &str) -> Tasks {
    let mut out = Tasks::default();
    let task_pattern =
        regex::Regex::new(r"^\s*[-*+] \[([ x])\] ([1-9][0-9]*\.[1-9][0-9]*)\s+(.+)$").unwrap();
    let possible = regex::Regex::new(r"^\s*[-*+]\s+\[[^\]]*\]").unwrap();
    let deps_split = regex::Regex::new(r"[,，、\s]+").unwrap();
    let id_pattern = regex::Regex::new(r"^[1-9][0-9]*\.[1-9][0-9]*$").unwrap();
    for (_, line) in markdown::content_lines(text) {
        if let Some(cap) = task_pattern.captures(line) {
            out.items.push(Task {
                id: cap[2].to_string(),
                done: &cap[1] == "x",
                dependencies: Vec::new(),
                acceptance: false,
            });
            continue;
        }
        if possible.is_match(line) {
            out.errors.push(format!(
                "invalid task (expected '- [ ] 1.1 action' or '- [x] 1.1 action'): {line}"
            ));
        }
        if let Some((key, value)) = field(line) {
            match key.as_str() {
                "dependencies" | "depends on" | "依赖" => {
                    if let Some(task) = out.items.last_mut() {
                        if !is_none(value) {
                            for id in deps_split.split(value).filter(|s| !s.is_empty()) {
                                if !id_pattern.is_match(id) {
                                    out.errors
                                        .push(format!("{}: invalid dependency {id}", task.id));
                                }
                                task.dependencies.push(id.to_string());
                            }
                        }
                    } else {
                        out.errors.push("dependency outside a task".into());
                    }
                }
                "acceptance" | "完成条件" => {
                    if let Some(task) = out.items.last_mut() {
                        task.acceptance |= !value.is_empty();
                    }
                }
                "verification" | "验证" | "验证记录" => out.verification |= !value.is_empty(),
                "blocked" | "blocker" | "阻塞" | "阻塞原因" if !is_none(value) => {
                    out.blockers.push(line.trim().to_string());
                }
                _ => {}
            }
        }
    }
    out.verification |= markdown::sections(text).iter().any(|s| {
        markdown::title_matches(&s.title, &["Verification", "验证", "验证记录", "执行证据"])
            && !s.content.is_empty()
    });
    if out.items.is_empty() {
        out.errors.push("no executable tasks found".into());
    }
    let mut ids = BTreeMap::new();
    for task in &out.items {
        if ids.insert(task.id.clone(), task).is_some() {
            out.errors.push(format!("duplicate task ID {}", task.id));
        }
        if !task.acceptance {
            out.errors
                .push(format!("{}: missing Acceptance/完成条件", task.id));
        }
    }
    for task in &out.items {
        for dep in &task.dependencies {
            match ids.get(dep) {
                None => out
                    .errors
                    .push(format!("{}: unknown dependency {dep}", task.id)),
                Some(required) if task.done && !required.done => out.errors.push(format!(
                    "{} is checked but dependency {dep} is unfinished",
                    task.id
                )),
                _ => {}
            }
        }
    }
    // Kahn's algorithm avoids recursion depth depending on user-supplied documents.
    let mut resolved = BTreeSet::new();
    loop {
        let before = resolved.len();
        for task in &out.items {
            if task.dependencies.iter().all(|d| resolved.contains(d)) {
                resolved.insert(task.id.clone());
            }
        }
        if before == resolved.len() {
            break;
        }
    }
    if ids.keys().any(|id| !resolved.contains(id))
        && out
            .items
            .iter()
            .all(|t| t.dependencies.iter().all(|d| ids.contains_key(d)))
    {
        out.errors.push("task dependency cycle".into());
    }
    out
}

pub fn blockers(text: &str) -> Vec<String> {
    markdown::content_lines(text)
        .into_iter()
        .filter_map(|(_, line)| {
            let (key, value) = field(line)?;
            ([
                "blocked",
                "blocker",
                "阻塞",
                "阻塞原因",
                "未决问题",
                "open questions",
            ]
            .contains(&key.as_str())
                && !is_none(value))
            .then(|| line.to_string())
        })
        .collect()
}
