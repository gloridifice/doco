use super::plan::Plan;
use crate::{Project, ui::PlainReporter};

#[test]
fn read_dependencies_are_rechecked_before_installation() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("AGENTS.md"), "Imported instructions\n").unwrap();
    let project = Project::open(dir.path()).unwrap();
    let mut plan = Plan::default();
    assert!(
        plan.effective_text(&project, "AGENTS.md")
            .unwrap()
            .is_some()
    );
    plan.file(&project, "CLAUDE.md", |_| Ok("@AGENTS.md\n".to_string()));
    std::fs::write(project.path("AGENTS.md"), "Concurrent incompatible edit\n").unwrap();
    let error = plan
        .apply(&project, &mut PlainReporter::new(Vec::new(), Vec::new()))
        .unwrap_err();
    assert!(error.to_string().contains("concurrent modification"));
    assert!(!project.path("CLAUDE.md").exists());
    assert_eq!(
        std::fs::read_to_string(project.path("AGENTS.md")).unwrap(),
        "Concurrent incompatible edit\n"
    );
}
