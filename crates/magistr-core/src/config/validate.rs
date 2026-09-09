use crate::types::{Formatter, Linter, PackageManager, Stack, TestFramework};

pub use super::validation_warning::ValidationWarning;

/// Validate that tools referenced by the detected stack are available on PATH.
///
/// Returns a list of warnings for tools that couldn't be found.
/// This is advisory only — scaffolding proceeds regardless.
pub fn validate_stack(stack: &Stack) -> Vec<ValidationWarning> {
    let mut warnings = Vec::new();

    let checks: [Option<(&str, &str)>; 4] = [
        package_manager_check(stack.package_manager),
        test_framework_check(stack.test_framework),
        linter_check(stack.linter),
        formatter_check(stack.formatter),
    ];

    for check in checks.into_iter().flatten() {
        let (cmd, hint) = check;
        if !command_exists(cmd) {
            warnings.push(ValidationWarning {
                tool: cmd.to_string(),
                install_hint: hint.to_string(),
            });
        }
    }

    warnings
}

fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn package_manager_check(pm: PackageManager) -> Option<(&'static str, &'static str)> {
    match pm {
        PackageManager::Cargo => Some(("cargo", "https://rustup.rs")),
        PackageManager::Npm => Some(("npm", "https://nodejs.org")),
        PackageManager::Pnpm => Some(("pnpm", "npm install -g pnpm")),
        PackageManager::Yarn => Some(("yarn", "npm install -g yarn")),
        PackageManager::Bun => Some(("bun", "https://bun.sh")),
        PackageManager::Pip => Some(("pip", "python -m ensurepip")),
        PackageManager::Uv => Some(("uv", "pip install uv")),
        PackageManager::Poetry => Some(("poetry", "pip install poetry")),
        PackageManager::Go => Some(("go", "https://go.dev/dl")),
        PackageManager::Maven => Some(("mvn", "https://maven.apache.org/install.html")),
        PackageManager::Gradle => Some(("gradle", "https://gradle.org/install")),
        PackageManager::Bundler => Some(("bundle", "gem install bundler")),
        PackageManager::Mix => Some(("mix", "https://elixir-lang.org/install.html")),
        PackageManager::Dotnet => Some(("dotnet", "https://dotnet.microsoft.com/download")),
        _ => None,
    }
}

fn test_framework_check(tf: TestFramework) -> Option<(&'static str, &'static str)> {
    match tf {
        // Built-in test frameworks don't need separate binary checks
        TestFramework::CargoTest | TestFramework::GoTest | TestFramework::Exunit => None,
        TestFramework::Pytest => Some(("pytest", "pip install pytest")),
        TestFramework::Rspec => Some(("rspec", "gem install rspec")),
        _ => None, // Jest/Vitest/Mocha are npx'd, not global binaries
    }
}

fn linter_check(l: Linter) -> Option<(&'static str, &'static str)> {
    match l {
        Linter::Clippy => None, // Part of rustup
        Linter::Eslint => None, // npx'd
        Linter::Biome => None,  // npx'd
        Linter::Ruff => Some(("ruff", "pip install ruff")),
        Linter::Flake8 => Some(("flake8", "pip install flake8")),
        Linter::GolangciLint => Some((
            "golangci-lint",
            "go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest",
        )),
        Linter::Rubocop => Some(("rubocop", "gem install rubocop")),
        Linter::Credo => None, // Mix dep
        Linter::Swiftlint => Some(("swiftlint", "brew install swiftlint")),
        Linter::Ktlint => Some(("ktlint", "brew install ktlint")),
        Linter::None => None,
    }
}

fn formatter_check(f: Formatter) -> Option<(&'static str, &'static str)> {
    match f {
        Formatter::Rustfmt => None,  // Part of rustup
        Formatter::Prettier => None, // npx'd
        Formatter::Biome => None,    // npx'd
        Formatter::Ruff => Some(("ruff", "pip install ruff")),
        Formatter::Black => Some(("black", "pip install black")),
        Formatter::Gofmt => None, // Part of Go
        Formatter::Rubocop => Some(("rubocop", "gem install rubocop")),
        Formatter::MixFormat => None, // Part of Mix
        Formatter::Swiftformat => Some(("swiftformat", "brew install swiftformat")),
        Formatter::Ktfmt => Some(("ktfmt", "brew install ktfmt")),
        Formatter::DotnetFormat => None, // Part of dotnet
        Formatter::None => None,
    }
}

#[cfg(test)]
#[path = "validate.tests.rs"]
mod tests;
