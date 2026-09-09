use super::*;

#[tokio::test]
async fn direct_mcp_interview_calls_acknowledge_without_claiming_effects() {
    let tools = ToolRegistry::new().build_mcp_tools();
    for (name, input, expected) in [
        (
            "magistr_ask",
            json!({"question": "Which stack?", "category": "architecture"}),
            "does not display UI",
        ),
        (
            "magistr_recommend",
            json!({}),
            "does not apply configuration changes",
        ),
    ] {
        let tool = tools.iter().find(|tool| tool.name() == name).unwrap();
        let result = tool.invoke(input).await.unwrap();
        let output = serde_json::to_string(&result).unwrap();
        assert!(output.contains(expected), "{output}");
        assert!(!output.contains("received and applied"), "{output}");
        assert!(!output.contains("Question displayed"), "{output}");
    }
}

#[test]
fn registry_extracts_recommend() {
    let reg = ToolRegistry::new();
    let input = json!({
        "mcp_servers": [{
            "key": "context7",
            "name": "Context7",
            "description": "Up-to-date docs"
        }],
        "conventions": ["Use Result, not unwrap"],
        "suggested_description": "A Rust CLI tool"
    });

    let output = reg.try_extract("magistr_recommend", &input);
    assert!(output.is_some());
    match output.unwrap() {
        ToolOutput::Recommend(recs) => {
            assert_eq!(recs.mcp_servers.len(), 1);
            assert_eq!(recs.mcp_servers[0].key, "context7");
            assert_eq!(recs.conventions.len(), 1);
            assert_eq!(
                recs.suggested_description.as_deref(),
                Some("A Rust CLI tool")
            );
        }
        other => panic!("Expected Recommend, got {other:?}"),
    }
}

#[test]
fn registry_extracts_with_mcp_prefix() {
    let reg = ToolRegistry::new();
    let input = json!({"conventions": ["test"]});
    let output = reg.try_extract("mcp__jig__magistr_recommend", &input);
    assert!(output.is_some());
    assert!(matches!(output.unwrap(), ToolOutput::Recommend(_)));
}

#[test]
fn registry_extracts_with_dash_prefix() {
    let reg = ToolRegistry::new();
    let input = json!({"conventions": ["test"]});
    // ACP reports MCP tools as "server-tool_name"
    let output = reg.try_extract("magistr-magistr_recommend", &input);
    assert!(output.is_some(), "should match magistr-magistr_recommend");
    assert!(matches!(output.unwrap(), ToolOutput::Recommend(_)));
}

#[test]
fn registry_returns_none_for_unknown() {
    let reg = ToolRegistry::new();
    let output = reg.try_extract("web_search", &json!({}));
    assert!(output.is_none());
}

#[test]
fn registry_returns_invalid_on_bad_input() {
    let reg = ToolRegistry::new();
    // mcp_servers should be an array, not a string
    let input = json!({"mcp_servers": "not an array"});
    let output = reg.try_extract("magistr_recommend", &input);
    assert!(output.is_some());
    assert!(matches!(output.unwrap(), ToolOutput::Invalid { .. }));
}

#[test]
fn is_magistr_tool_matches_all_forms() {
    let reg = ToolRegistry::new();
    assert!(reg.is_magistr_tool("magistr_recommend"));
    assert!(reg.is_magistr_tool("mcp__jig__magistr_recommend"));
    assert!(reg.is_magistr_tool("magistr-magistr_recommend")); // ACP dash format
    // Any server prefix still resolves to the bare tool name
    assert!(reg.is_magistr_tool("mcp__other__magistr_recommend"));
    assert!(!reg.is_magistr_tool("web_search"));
    assert!(!reg.is_magistr_tool("mcp__jig__web_search"));
}

#[test]
fn normalize_strips_mcp_prefix() {
    assert_eq!(
        normalize_tool_name("mcp__jig__magistr_recommend"),
        "magistr_recommend"
    );
    assert_eq!(
        normalize_tool_name("magistr-magistr_recommend"),
        "magistr_recommend"
    );
    assert_eq!(
        normalize_tool_name("magistr_recommend"),
        "magistr_recommend"
    );
    assert_eq!(normalize_tool_name("web_search"), "web_search");
}

#[test]
fn tool_names_returns_all() {
    let reg = ToolRegistry::new();
    let names = reg.tool_names();
    assert!(names.contains(&"magistr_recommend"));
    assert!(names.contains(&"magistr_ask"));
}

#[test]
fn build_mcp_tools_creates_valid_tools() {
    let reg = ToolRegistry::new();
    let tools = reg.build_mcp_tools();
    assert_eq!(tools.len(), 2);
    let names: Vec<_> = tools.iter().map(|t| t.name()).collect();
    assert!(names.contains(&"magistr_recommend"));
    assert!(names.contains(&"magistr_ask"));
}

#[test]
fn recommend_handles_empty_input() {
    let reg = ToolRegistry::new();
    let input = json!({});
    let output = reg.try_extract("magistr_recommend", &input).unwrap();
    // All fields have #[serde(default)], so empty object is valid
    match output {
        ToolOutput::Recommend(recs) => {
            assert!(recs.mcp_servers.is_empty());
            assert!(recs.conventions.is_empty());
            assert!(recs.suggested_description.is_none());
        }
        other => panic!("Expected Recommend, got {other:?}"),
    }
}

// ═══════════════════════════════════════════════════════════════════
//  E2E: Realistic ACP tool call extraction
// ═══════════════════════════════════════════════════════════════════

#[test]
fn e2e_acp_dash_format_full_recommendations() {
    // Simulates the exact tool name and payload the ACP backend sends
    // when the AI calls magistr_recommend via the magistr MCP server.
    let reg = ToolRegistry::new();
    let input = json!({
        "mcp_servers": [
            {
                "key": "context7",
                "name": "Context7",
                "description": "Up-to-date library docs",
                "command": "npx",
                "args": ["-y", "@upstash/context7-mcp@latest"],
                "category": "docs"
            },
            {
                "key": "playwright",
                "name": "Playwright MCP",
                "description": "Browser automation and testing",
                "command": "npx",
                "args": ["-y", "@anthropic/playwright-mcp@latest"],
                "category": "testing"
            }
        ],
        "agents": [
            {"key": "cursor", "reason": "Best for Rust with inline completions"},
            {"key": "copilot", "reason": "Strong GitHub integration"}
        ],
        "conventions": [
            "Use ? operator for error propagation, never unwrap() in library code",
            "Prefer impl Trait over dyn Trait for return types",
            "Write doc comments on all public items"
        ],
        "tips": [
            "Enable clippy in CI with -D warnings",
            "Use cargo-watch for fast iteration cycles"
        ],
        "suggested_description": "A Rust CLI tool for scaffolding AI dev environments"
    });

    // The exact tool name ACP reports
    let output = reg.try_extract("magistr-magistr_recommend", &input);
    assert!(
        output.is_some(),
        "magistr-magistr_recommend must be recognized"
    );

    match output.unwrap() {
        ToolOutput::Recommend(recs) => {
            assert_eq!(recs.mcp_servers.len(), 2);
            assert_eq!(recs.mcp_servers[0].key, "context7");
            assert_eq!(recs.mcp_servers[1].key, "playwright");
            assert_eq!(recs.mcp_servers[0].command, "npx");
            assert_eq!(recs.agents.len(), 2);
            assert_eq!(recs.agents[0].key, "cursor");
            assert_eq!(recs.conventions.len(), 3);
            assert_eq!(recs.tips.len(), 2);
            assert_eq!(
                recs.suggested_description.as_deref(),
                Some("A Rust CLI tool for scaffolding AI dev environments")
            );
        }
        other => panic!("Expected Recommend, got {other:?}"),
    }
}

#[test]
fn e2e_all_name_formats_extract_same_data() {
    // All three formats should extract identical recommendations
    let reg = ToolRegistry::new();
    let input = json!({
        "conventions": ["Use snake_case"],
        "tips": ["Run cargo fmt"],
        "suggested_description": "Test project"
    });

    let names = [
        "magistr_recommend",           // bare
        "mcp__jig__magistr_recommend", // Claude Code format
        "magistr-magistr_recommend",   // ACP dash format
    ];

    for name in names {
        let output = reg.try_extract(name, &input);
        assert!(output.is_some(), "{name} should be recognized");
        match output.unwrap() {
            ToolOutput::Recommend(recs) => {
                assert_eq!(
                    recs.conventions,
                    vec!["Use snake_case"],
                    "conventions mismatch for {name}"
                );
                assert_eq!(recs.tips, vec!["Run cargo fmt"], "tips mismatch for {name}");
                assert_eq!(
                    recs.suggested_description.as_deref(),
                    Some("Test project"),
                    "description mismatch for {name}"
                );
            }
            other => panic!("Expected Recommend for {name}, got {other:?}"),
        }
    }
}

#[test]
fn e2e_non_jig_mcp_tools_not_extracted() {
    // Other MCP tools (magistr-scope, etc.) should not match
    let reg = ToolRegistry::new();
    let input = json!({"query": "test"});

    for name in [
        "mcp__magistr-scope__scope_search",
        "mcp__magistr-scope__scope_read",
        "mcp__magistr-scope__scope_modules",
        "mcp__magistr-scope__scope_grep",
        "web_search",
    ] {
        assert!(
            reg.try_extract(name, &input).is_none(),
            "{name} should NOT be extracted as a magistr tool"
        );
    }
}

#[test]
fn e2e_dash_format_partial_recommendations() {
    // AI might provide only some fields — all should be optional
    let reg = ToolRegistry::new();

    // Only conventions
    let output = reg.try_extract(
        "magistr-magistr_recommend",
        &json!({"conventions": ["test"]}),
    );
    match output.unwrap() {
        ToolOutput::Recommend(recs) => {
            assert_eq!(recs.conventions.len(), 1);
            assert!(recs.mcp_servers.is_empty());
            assert!(recs.agents.is_empty());
            assert!(recs.tips.is_empty());
            assert!(recs.suggested_description.is_none());
        }
        other => panic!("Expected Recommend, got {other:?}"),
    }

    // Only agents
    let output = reg.try_extract(
        "magistr-magistr_recommend",
        &json!({
            "agents": [{"key": "cursor", "reason": "good"}]
        }),
    );
    match output.unwrap() {
        ToolOutput::Recommend(recs) => {
            assert_eq!(recs.agents.len(), 1);
            assert!(recs.conventions.is_empty());
        }
        other => panic!("Expected Recommend, got {other:?}"),
    }
}

#[test]
fn e2e_dash_format_invalid_input_returns_invalid() {
    let reg = ToolRegistry::new();
    // mcp_servers expects array of objects, not a string
    let input = json!({"mcp_servers": "bad"});
    let output = reg.try_extract("magistr-magistr_recommend", &input);
    assert!(output.is_some());
    assert!(
        matches!(output.unwrap(), ToolOutput::Invalid { .. }),
        "bad input through dash format should return Invalid"
    );
}

#[test]
fn normalize_handles_edge_cases() {
    // magistr-scope tools (should strip to tool name via MCP format)
    assert_eq!(
        normalize_tool_name("mcp__magistr-scope__scope_search"),
        "scope_search"
    );
    assert_eq!(
        normalize_tool_name("mcp__magistr-scope__scope_read"),
        "scope_read"
    );
    // Bare names pass through
    assert_eq!(
        normalize_tool_name("magistr_recommend"),
        "magistr_recommend"
    );
    // Double-underscore format
    assert_eq!(normalize_tool_name("mcp__x__tool"), "tool");
    // Single component (no separator)
    assert_eq!(normalize_tool_name("simple"), "simple");
}

// ═══════════════════════════════════════════════════════════════════
//  magistr_ask tool
// ═══════════════════════════════════════════════════════════════════

#[test]
fn registry_extracts_ask_bare_name() {
    let reg = ToolRegistry::new();
    let input = json!({
        "question": "What kind of project is this?",
        "choices": ["CLI", "Web App", "API", "Library"]
    });
    let output = reg.try_extract("magistr_ask", &input);
    assert!(output.is_some());
    match output.unwrap() {
        ToolOutput::Ask(q) => {
            assert_eq!(q.question, "What kind of project is this?");
            assert_eq!(q.choices.len(), 4);
            assert_eq!(q.choices[0].label(), "CLI");
            assert_eq!(q.choices[1].label(), "Web App");
            assert_eq!(q.choices[2].label(), "API");
            assert_eq!(q.choices[3].label(), "Library");
            assert!(q.allow_freeform); // default true
        }
        other => panic!("Expected Ask, got {other:?}"),
    }
}

#[test]
fn registry_extracts_ask_dash_format() {
    let reg = ToolRegistry::new();
    let input = json!({"question": "Any preferences?"});
    let output = reg.try_extract("magistr-magistr_ask", &input);
    assert!(output.is_some(), "magistr-magistr_ask should be recognized");
    assert!(matches!(output.unwrap(), ToolOutput::Ask(_)));
}

#[test]
fn registry_extracts_ask_mcp_format() {
    let reg = ToolRegistry::new();
    let input = json!({"question": "Test?"});
    let output = reg.try_extract("mcp__jig__magistr_ask", &input);
    assert!(output.is_some());
    assert!(matches!(output.unwrap(), ToolOutput::Ask(_)));
}

#[test]
fn ask_without_choices() {
    let reg = ToolRegistry::new();
    let input = json!({"question": "What does this project do?"});
    match reg.try_extract("magistr_ask", &input).unwrap() {
        ToolOutput::Ask(q) => {
            assert_eq!(q.question, "What does this project do?");
            assert!(q.choices.is_empty());
            assert!(q.allow_freeform);
        }
        other => panic!("Expected Ask, got {other:?}"),
    }
}

#[test]
fn ask_with_freeform_disabled() {
    let reg = ToolRegistry::new();
    let input = json!({
        "question": "Pick one:",
        "choices": ["A", "B"],
        "allow_freeform": false
    });
    match reg.try_extract("magistr_ask", &input).unwrap() {
        ToolOutput::Ask(q) => {
            assert!(!q.allow_freeform);
            assert_eq!(q.choices.len(), 2);
        }
        other => panic!("Expected Ask, got {other:?}"),
    }
}

#[test]
fn ask_missing_question_returns_invalid() {
    let reg = ToolRegistry::new();
    let input = json!({"choices": ["A", "B"]});
    let output = reg.try_extract("magistr_ask", &input);
    assert!(output.is_some());
    assert!(
        matches!(output.unwrap(), ToolOutput::Invalid { .. }),
        "missing required 'question' field should be Invalid"
    );
}

#[test]
fn is_magistr_tool_recognizes_ask() {
    let reg = ToolRegistry::new();
    assert!(reg.is_magistr_tool("magistr_ask"));
    assert!(reg.is_magistr_tool("magistr-magistr_ask"));
    assert!(reg.is_magistr_tool("mcp__jig__magistr_ask"));
}

// ═══════════════════════════════════════════════════════════════════
//  E2E: ask + recommend interaction
// ═══════════════════════════════════════════════════════════════════

#[test]
fn e2e_ask_then_recommend_both_extracted() {
    let reg = ToolRegistry::new();

    // AI asks a question first
    let ask_output = reg.try_extract(
        "magistr-magistr_ask",
        &json!({
            "question": "Do you prefer tabs or spaces?",
            "choices": ["Tabs", "Spaces"]
        }),
    );
    assert!(matches!(ask_output.unwrap(), ToolOutput::Ask(_)));

    // Then AI submits recommendations
    let rec_output = reg.try_extract(
        "magistr-magistr_recommend",
        &json!({
            "conventions": ["Use spaces"],
            "suggested_description": "A project"
        }),
    );
    assert!(matches!(rec_output.unwrap(), ToolOutput::Recommend(_)));
}
