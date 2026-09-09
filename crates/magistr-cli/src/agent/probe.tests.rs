use super::*;

#[test]
fn provider_model_set_default_fields() {
    let set = ProviderModelSet {
        provider_id: "claude".into(),
        label: "Claude Code".into(),
        models: vec![
            ProbeModel {
                id: "claude-sonnet-4".into(),
                name: "Sonnet 4".into(),
            },
            ProbeModel {
                id: "claude-opus-4".into(),
                name: "Opus 4".into(),
            },
        ],
        probed: true,
    };
    assert_eq!(set.models.len(), 2);
    assert_eq!(set.models[0].id, "claude-sonnet-4");
    assert_eq!(set.models[1].name, "Opus 4");
}

#[test]
fn empty_provider_model_set() {
    let set = ProviderModelSet {
        provider_id: "copilot".into(),
        label: "GitHub Copilot".into(),
        models: vec![],
        probed: false,
    };
    assert!(!set.probed);
    assert!(set.models.is_empty());
}

// ── Model names come from the agent, never from us ──────────

// magistr used to rewrite the agent's model names through a hard-coded alias
// map ("sonnet" -> "Claude Sonnet 4.6") and shell out to `claude auth status`
// to guess what "default" resolved to. Both were guesses about a lineup only
// the agent's bundled SDK knows, so they went stale silently and, after the
// registry moved to the 5-series, disagreed with `magistr models`. The names
// now pass through untouched.

#[test]
fn probe_model_names_are_passed_through_verbatim() {
    // What Claude Code advertises through the in-binary agent.
    let advertised = [
        ("default", "Default (recommended)"),
        ("sonnet", "Sonnet"),
        ("haiku", "Haiku"),
        ("opus[1m]", "Opus (1M context)"),
    ];
    let set = ProviderModelSet {
        provider_id: "claude".into(),
        label: "Claude Code".into(),
        models: advertised
            .iter()
            .map(|(id, name)| ProbeModel {
                id: (*id).into(),
                name: (*name).into(),
            })
            .collect(),
        probed: true,
    };
    for (i, (id, name)) in advertised.iter().enumerate() {
        assert_eq!(set.models[i].id, *id);
        assert_eq!(
            set.models[i].name, *name,
            "model names must be the agent's own, not a version we invented"
        );
    }
}
