use super::*;
use tempfile::TempDir;

/// A Magistr source checkout: the workflow templates directory with one
/// synthetic template per workflow, the runtime, both host adapters and
/// the vendored routing skill.
fn source_checkout() -> TempDir {
    let tmp = TempDir::new().expect("tempdir");
    let workflows = tmp.path().join(WORKFLOW_TEMPLATES);
    std::fs::create_dir_all(workflows.join("hosts")).expect("mkdir");
    for (name, _) in WorkflowSkills::workflow_paths(Host::Claude) {
        std::fs::write(
            workflows.join(format!("{name}.md")),
            format!("# {name} via {{{{invoke}}}}{name}\n{{{{next_recovery_guidance}}}}\n"),
        )
        .expect("template");
    }
    std::fs::write(
        workflows.join("runtime.md"),
        "# runtime\n{{reasoning_guidance}}\n",
    )
    .expect("runtime");
    std::fs::write(workflows.join("hosts/claude.md"), "claude adapter\n").expect("adapter");
    std::fs::write(workflows.join("hosts/codex.md"), "codex adapter\n").expect("adapter");
    let routing = tmp.path().join(ROUTING_TEMPLATE);
    std::fs::create_dir_all(routing.parent().expect("parent")).expect("mkdir");
    std::fs::write(&routing, "# routing skill\n").expect("routing");
    tmp
}

#[test]
fn an_ordinary_project_renders_the_embedded_templates() {
    let tmp = TempDir::new().expect("tempdir");
    let canonical = CanonicalSource::capture(tmp.path());
    let path = ".claude/skills/next/SKILL.md";
    assert!(!canonical.is_canonical(path));
    assert_eq!(
        canonical.resolve(path, "embedded".to_owned()),
        Ok("embedded".to_owned())
    );
}

#[test]
fn a_source_checkout_renders_the_live_templates_and_follows_later_edits() {
    let tmp = source_checkout();
    let canonical = CanonicalSource::capture(tmp.path());
    for host in [Host::Claude, Host::Codex] {
        for (name, path) in WorkflowSkills::workflow_paths(host) {
            assert!(canonical.is_canonical(path), "{path}");
            let expected = WorkflowSkills::render_workflow(
                host,
                &format!("# {name} via {{{{invoke}}}}{name}\n{{{{next_recovery_guidance}}}}\n"),
            );
            assert!(expected.contains(&format!("via {}{name}", host.invocation())));
            assert!(!expected.contains("{{"), "{expected}");
            assert_eq!(canonical.resolve(path, "embedded".to_owned()), Ok(expected));
        }
        let runtime = canonical
            .resolve(WorkflowSkills::runtime_path(host), "embedded".to_owned())
            .expect("runtime");
        assert!(runtime.starts_with("# runtime\n"));
        assert!(runtime.contains(magistr_engine::prompt::PromptConfig::REASONING_GUIDANCE));
        assert!(
            runtime.ends_with(&format!("{} adapter\n", host.name())),
            "{runtime}"
        );
        let routing = canonical
            .resolve(WorkflowSkills::routing_path(host), "embedded".to_owned())
            .expect("routing");
        assert!(routing.starts_with("# routing skill\n<!-- maintained with"));
    }

    // A later edit is followed by the next capture, without a rebuild.
    std::fs::write(
        tmp.path().join(WORKFLOW_TEMPLATES).join("next.md"),
        "# next, second edition\n",
    )
    .expect("edit");
    let again = CanonicalSource::capture(tmp.path());
    assert_eq!(
        again.resolve(".claude/skills/next/SKILL.md", "embedded".to_owned()),
        Ok("# next, second edition\n".to_owned())
    );
}

#[test]
fn a_missing_template_in_a_source_checkout_refuses_its_paths_and_never_falls_back() {
    let tmp = source_checkout();
    let workflows = tmp.path().join(WORKFLOW_TEMPLATES);
    std::fs::remove_file(workflows.join("next.md")).expect("remove");
    std::fs::remove_file(workflows.join("hosts/claude.md")).expect("remove");
    let canonical = CanonicalSource::capture(tmp.path());

    for path in [
        ".claude/skills/next/SKILL.md",
        ".agents/skills/next/SKILL.md",
    ] {
        let refusal = canonical
            .resolve(path, "embedded".to_owned())
            .expect_err("no fallback to the embedded copy");
        assert!(
            matches!(&refusal, Refusal::CanonicalUnavailable { source, .. } if source.ends_with("workflows/next.md")),
            "{refusal}"
        );
    }
    let refusal = canonical
        .resolve(
            WorkflowSkills::runtime_path(Host::Claude),
            "embedded".to_owned(),
        )
        .expect_err("the Claude runtime needs its adapter");
    assert!(
        matches!(&refusal, Refusal::CanonicalUnavailable { source, .. } if source.ends_with("hosts/claude.md")),
        "{refusal}"
    );
    // Inputs that are present still render; the Codex runtime has its adapter.
    assert!(
        canonical
            .resolve(
                WorkflowSkills::runtime_path(Host::Codex),
                "embedded".to_owned()
            )
            .is_ok()
    );
    assert!(
        canonical
            .resolve(".claude/skills/validate/SKILL.md", "embedded".to_owned())
            .is_ok()
    );
}

#[test]
fn a_home_repository_skill_wins_and_an_unreadable_one_refuses() {
    let tmp = TempDir::new().expect("tempdir");
    let skill = tmp.path().join("skills/workbench-routing/SKILL.md");
    std::fs::create_dir_all(skill.parent().expect("parent")).expect("mkdir");
    std::fs::write(&skill, "# the canonical routing skill\n").expect("skill");
    let canonical = CanonicalSource::capture(tmp.path());
    for (target, _) in CANONICAL_SOURCES {
        assert_eq!(
            canonical.resolve(target, "embedded".to_owned()),
            Ok("# the canonical routing skill\n".to_owned())
        );
    }

    #[cfg(unix)]
    {
        std::fs::remove_file(&skill).expect("remove");
        std::os::unix::fs::symlink(tmp.path().join("elsewhere"), &skill).expect("symlink");
        let canonical = CanonicalSource::capture(tmp.path());
        let refusal = canonical
            .resolve(CANONICAL_SOURCES[0].0, "embedded".to_owned())
            .expect_err("a symlinked canonical input is refused");
        assert!(matches!(refusal, Refusal::CanonicalUnavailable { .. }));
    }
}
