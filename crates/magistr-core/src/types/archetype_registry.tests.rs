use super::*;

#[test]
fn all_supported_languages_have_entries() {
    for lang in SUPPORTED_LANGUAGES {
        let entry = lookup(lang, None);
        assert!(
            entry.is_some(),
            "missing registry entry for language: {lang}"
        );
        let entry = entry.unwrap();
        assert_eq!(entry.language, *lang);
        assert!(!entry.quality_gate.check.is_empty());
        assert!(!entry.quality_gate.test.is_empty());
        assert!(!entry.quality_gate.lint.is_empty());
        assert!(!entry.format.command.is_empty());
        assert!(!entry.source_patterns.is_empty());
    }
}

#[test]
fn all_supported_frameworks_have_entries() {
    for (lang, fw) in SUPPORTED_FRAMEWORKS {
        let entry = lookup(lang, Some(fw));
        assert!(entry.is_some(), "missing registry entry for {lang}/{fw}");
        let entry = entry.unwrap();
        assert!(
            entry.framework.is_some(),
            "framework entry for {lang}/{fw} should set framework"
        );
    }
}

#[test]
fn framework_fallback_to_language() {
    // An unknown framework for a known language should fall back.
    let entry = lookup("rust", Some("unknown-framework"));
    assert!(entry.is_some());
    assert!(entry.unwrap().framework.is_none());
}

#[test]
fn unknown_language_returns_none() {
    assert!(lookup("brainfuck", None).is_none());
}

#[test]
fn default_archetype_builds_correctly() {
    let arch = default_archetype("api", "rust", None, "services/api", ProjectRole::Backend);
    assert!(arch.is_some());
    let arch = arch.unwrap();
    assert_eq!(arch.id, "api");
    assert_eq!(arch.language, "rust");
    assert_eq!(arch.root.to_str().unwrap(), "services/api");
    assert_eq!(arch.role, ProjectRole::Backend);
    assert!(arch.quality_gate.check.contains("cargo"));
}

#[test]
fn nextjs_entry_has_correct_patterns() {
    let entry = lookup("typescript", Some("nextjs")).unwrap();
    assert_eq!(entry.framework, Some("nextjs"));
    assert!(entry.source_patterns.iter().any(|p| p.contains("tsx")));
}

#[test]
fn django_entry_has_manage_py() {
    let entry = lookup("python", Some("django")).unwrap();
    assert!(entry.quality_gate.check.contains("manage.py"));
}

#[test]
fn rails_entry_has_rspec() {
    let entry = lookup("ruby", Some("rails")).unwrap();
    assert!(entry.quality_gate.test.contains("rspec"));
}

#[test]
fn spring_entry_has_mvnw() {
    let entry = lookup("java", Some("spring")).unwrap();
    assert!(entry.quality_gate.check.contains("mvnw"));
}

#[test]
fn aspnet_entry_has_dotnet() {
    let entry = lookup("csharp", Some("aspnet")).unwrap();
    assert!(entry.quality_gate.check.contains("dotnet"));
}

#[test]
fn laravel_entry_has_artisan() {
    let entry = lookup("php", Some("laravel")).unwrap();
    assert!(entry.quality_gate.test.contains("artisan"));
}

#[test]
fn phoenix_entry_has_mix() {
    let entry = lookup("elixir", Some("phoenix")).unwrap();
    assert!(entry.quality_gate.check.contains("mix"));
}
