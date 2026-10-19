//! Curated MCP server registry for stack-aware recommendations.
//!
//! Provides a categorized catalog of well-known MCP servers with metadata
//! for filtering by language, framework, and use-case. Used by the guided
//! init interview to suggest relevant servers.

use std::fmt;

// ── Built-in servers ──────────────────────────────────────────────────

/// MCP servers that magistr always registers — not user-selectable, always present.
pub(crate) const BUILTIN_MCP_SERVERS: &[&str] = &["magistr"];

// ── Types ──────────────────────────────────────────────────────────────

/// How to verify that a server's prerequisites are available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallCheck {
    /// Always available via npx (auto-installed on demand).
    Npx,
    /// Requires a binary on PATH (e.g. `docker`).
    Binary(&'static str),
    /// Npx-based but requires an environment variable to be set.
    NpxWithEnv(&'static str),
    /// Requires both a binary on PATH and an environment variable.
    BinaryWithEnv(&'static str, &'static str),
}

/// Result of checking whether a server's prerequisites are met.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InstallStatus {
    /// All prerequisites are available.
    Available,
    /// A required binary was not found on PATH.
    MissingBinary(&'static str),
    /// A required environment variable is not set.
    MissingEnvVar(&'static str),
}

impl InstallStatus {
    /// Whether the server is ready to use.
    pub fn is_available(&self) -> bool {
        matches!(self, Self::Available)
    }
}

impl InstallCheck {
    /// Check whether the prerequisites for this server are met, over
    /// injected probes: `binary_exists` answers for a name on PATH,
    /// `env_exists` for a variable the caller captured. Nothing here
    /// reads the process environment.
    pub(crate) fn check_with(
        self,
        binary_exists: impl Fn(&str) -> bool,
        env_exists: impl Fn(&str) -> bool,
    ) -> InstallStatus {
        match self {
            Self::Npx => InstallStatus::Available,
            Self::Binary(bin) => {
                if binary_exists(bin) {
                    InstallStatus::Available
                } else {
                    InstallStatus::MissingBinary(bin)
                }
            }
            Self::NpxWithEnv(var) => {
                if env_exists(var) {
                    InstallStatus::Available
                } else {
                    InstallStatus::MissingEnvVar(var)
                }
            }
            Self::BinaryWithEnv(bin, var) => {
                if !binary_exists(bin) {
                    InstallStatus::MissingBinary(bin)
                } else if !env_exists(var) {
                    InstallStatus::MissingEnvVar(var)
                } else {
                    InstallStatus::Available
                }
            }
        }
    }
}

/// Check if a binary exists on PATH using a simple path search.
fn which_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(name);
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

/// A curated MCP server entry with rich metadata.
#[derive(Debug, Clone)]
pub(crate) struct McpServerEntry {
    /// Server key (used in `.mcp.json` mcpServers object).
    pub key: &'static str,
    /// Human-readable display name.
    pub name: &'static str,
    /// Human-readable one-line description.
    pub description: &'static str,
    /// Executable command (e.g. `npx`).
    pub command: &'static str,
    /// Command arguments (e.g. `["-y", "@anthropic/mcp-server-memory"]`).
    pub args: &'static [&'static str],
    /// How to install (shown as hint to user).
    pub install_hint: &'static str,
    /// URL for more information.
    pub url: &'static str,
    /// Use-case categories.
    pub categories: &'static [Category],
    /// Languages this is especially useful for (empty = all).
    pub languages: &'static [&'static str],
    /// Frameworks this is especially useful for (empty = all).
    pub frameworks: &'static [&'static str],
    /// Whether this server is enabled by default in the wizard.
    pub default_enabled: bool,
    /// How to verify this server's prerequisites are available.
    pub install_check: InstallCheck,
    /// Pinned package version (e.g. `"0.6.2"`, or `"latest"`).
    pub version: &'static str,
    /// Minimum Node.js version required, if applicable.
    pub min_node_version: Option<&'static str>,
}

impl McpServerEntry {
    /// Build the JSON value for this server's `.mcp.json` entry.
    pub fn to_mcp_value(&self) -> serde_json::Value {
        serde_json::json!({
            "command": self.command,
            "args": self.args,
            "type": "stdio"
        })
    }

    /// Whether this server is relevant for a given language + framework.
    pub fn is_relevant(&self, language: &str, framework: &str) -> bool {
        let lang_match = self.languages.is_empty()
            || self
                .languages
                .iter()
                .any(|l| l.eq_ignore_ascii_case(language));
        let fw_match = self.frameworks.is_empty()
            || self
                .frameworks
                .iter()
                .any(|f| f.eq_ignore_ascii_case(framework));
        lang_match && fw_match
    }

    /// Check whether this server's prerequisites are available: its binary
    /// on PATH and its variable in the captured environment `env_exists`
    /// answers for.
    pub fn check_available(&self, env_exists: impl Fn(&str) -> bool) -> InstallStatus {
        self.install_check.check_with(which_exists, env_exists)
    }
}

/// Use-case category for an MCP server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Category {
    /// Documentation lookup / knowledge retrieval.
    Docs,
    /// Browser testing and automation.
    Testing,
    /// Database access.
    Database,
    /// DevOps, CI/CD, deployment.
    DevOps,
    /// Web search / information retrieval.
    Search,
    /// Browser automation (non-testing).
    Browser,
    /// File system operations.
    FileSystem,
    /// Code intelligence / analysis.
    CodeIntel,
    /// Memory / knowledge persistence.
    Memory,
    /// Reasoning and planning tools.
    Reasoning,
    /// Cloud infrastructure and services.
    Cloud,
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Docs => write!(f, "docs"),
            Self::Testing => write!(f, "testing"),
            Self::Database => write!(f, "database"),
            Self::DevOps => write!(f, "devops"),
            Self::Search => write!(f, "search"),
            Self::Browser => write!(f, "browser"),
            Self::FileSystem => write!(f, "filesystem"),
            Self::CodeIntel => write!(f, "code-intel"),
            Self::Memory => write!(f, "memory"),
            Self::Reasoning => write!(f, "reasoning"),
            Self::Cloud => write!(f, "cloud"),
        }
    }
}

// ── Registry ───────────────────────────────────────────────────────────

/// Full catalog of curated MCP servers.
pub(crate) static CATALOG: &[McpServerEntry] = &[
    // ── Documentation ──────────────────────────────────────────────
    McpServerEntry {
        key: "context7",
        name: "Context7",
        description: "Up-to-date library docs — prevents hallucination",
        command: "npx",
        args: &["-y", "@upstash/context7-mcp@0.0.19"],
        install_hint: "npx (auto-installed)",
        url: "https://github.com/upstash/context7",
        categories: &[Category::Docs],
        languages: &[],
        frameworks: &[],
        default_enabled: true,
        install_check: InstallCheck::Npx,
        version: "0.0.19",
        min_node_version: Some("18"),
    },
    // ── Testing ────────────────────────────────────────────────────
    McpServerEntry {
        key: "playwright",
        name: "Playwright",
        description: "Browser test automation — E2E testing",
        command: "npx",
        args: &["-y", "@playwright/mcp@0.0.28"],
        install_hint: "npx (auto-installed)",
        url: "https://github.com/microsoft/playwright-mcp",
        categories: &[Category::Testing, Category::Browser],
        languages: &["typescript", "javascript"],
        frameworks: &["next", "react", "vue", "svelte", "angular"],
        default_enabled: false,
        install_check: InstallCheck::Npx,
        version: "0.0.28",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "cypress",
        name: "Cypress",
        description: "Component and E2E testing for web apps",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-cypress@1.0.0"],
        install_hint: "npx (auto-installed)",
        url: "https://github.com/anthropics/mcp-servers",
        categories: &[Category::Testing, Category::Browser],
        languages: &["typescript", "javascript"],
        frameworks: &["next", "react", "vue", "svelte", "angular"],
        default_enabled: false,
        install_check: InstallCheck::Npx,
        version: "1.0.0",
        min_node_version: Some("18"),
    },
    // ── Database ───────────────────────────────────────────────────
    McpServerEntry {
        key: "postgres",
        name: "PostgreSQL",
        description: "Direct SQL queries and schema inspection",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-postgres@0.6.2"],
        install_hint: "npx (auto-installed), requires POSTGRES_URL env var",
        url: "https://github.com/anthropics/mcp-servers/tree/main/src/postgres",
        categories: &[Category::Database],
        languages: &[],
        frameworks: &["next", "django", "rails", "spring", "express", "fastapi"],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("POSTGRES_URL"),
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "sqlite",
        name: "SQLite",
        description: "Lightweight SQL queries for local databases",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-sqlite@0.6.2"],
        install_hint: "npx (auto-installed)",
        url: "https://github.com/anthropics/mcp-servers/tree/main/src/sqlite",
        categories: &[Category::Database],
        languages: &[],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::Npx,
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "redis",
        name: "Redis",
        description: "Key-value store queries and cache inspection",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-redis@0.6.2"],
        install_hint: "npx (auto-installed), requires REDIS_URL env var",
        url: "https://github.com/anthropics/mcp-servers",
        categories: &[Category::Database],
        languages: &[],
        frameworks: &["next", "express", "django", "rails", "fastapi"],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("REDIS_URL"),
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "mongodb",
        name: "MongoDB",
        description: "Document database queries and collection inspection",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-mongodb@0.6.2"],
        install_hint: "npx (auto-installed), requires MONGODB_URI env var",
        url: "https://github.com/anthropics/mcp-servers",
        categories: &[Category::Database],
        languages: &[],
        frameworks: &["next", "express", "django", "fastapi"],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("MONGODB_URI"),
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "mysql",
        name: "MySQL",
        description: "SQL queries and schema inspection for MySQL/MariaDB",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-mysql@0.6.2"],
        install_hint: "npx (auto-installed), requires MYSQL_URL env var",
        url: "https://github.com/anthropics/mcp-servers",
        categories: &[Category::Database],
        languages: &[],
        frameworks: &["django", "rails", "spring", "express", "laravel"],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("MYSQL_URL"),
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    // ── DevOps / Integration ───────────────────────────────────────
    McpServerEntry {
        key: "github-mcp",
        name: "GitHub",
        description: "Issues, PRs, repos, actions, code search",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-github@0.6.2"],
        install_hint: "npx (auto-installed), requires GITHUB_TOKEN env var",
        url: "https://github.com/anthropics/mcp-servers/tree/main/src/github",
        categories: &[Category::DevOps],
        languages: &[],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("GITHUB_TOKEN"),
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "sentry",
        name: "Sentry",
        description: "Search and analyze error tracking issues",
        command: "npx",
        args: &["-y", "@sentry/mcp-server-sentry@0.7.0"],
        install_hint: "npx (auto-installed), requires SENTRY_AUTH_TOKEN env var",
        url: "https://github.com/getsentry/sentry-mcp",
        categories: &[Category::DevOps],
        languages: &[],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("SENTRY_AUTH_TOKEN"),
        version: "0.7.0",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "linear",
        name: "Linear",
        description: "Issue tracking — create, search, update tickets",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-linear@0.6.2"],
        install_hint: "npx (auto-installed), requires LINEAR_API_KEY env var",
        url: "https://github.com/anthropics/mcp-servers",
        categories: &[Category::DevOps],
        languages: &[],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("LINEAR_API_KEY"),
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "docker",
        name: "Docker",
        description: "Container management — build, run, inspect images",
        command: "docker",
        args: &["mcp", "server"],
        install_hint: "Install Docker Desktop or Docker Engine",
        url: "https://docs.docker.com/",
        categories: &[Category::DevOps],
        languages: &[],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::Binary("docker"),
        version: "latest",
        min_node_version: None,
    },
    McpServerEntry {
        key: "vercel",
        name: "Vercel",
        description: "Deploy and manage Vercel projects and deployments",
        command: "npx",
        args: &["-y", "@vercel/mcp@1.0.0"],
        install_hint: "npx (auto-installed), requires VERCEL_TOKEN env var",
        url: "https://vercel.com/docs",
        categories: &[Category::DevOps, Category::Cloud],
        languages: &["typescript", "javascript"],
        frameworks: &["next", "react", "vue", "svelte"],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("VERCEL_TOKEN"),
        version: "1.0.0",
        min_node_version: Some("18"),
    },
    // ── Cloud ──────────────────────────────────────────────────────
    McpServerEntry {
        key: "aws",
        name: "AWS",
        description: "AWS service management — S3, Lambda, DynamoDB, etc.",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-aws@0.6.2"],
        install_hint: "npx (auto-installed), requires AWS credentials configured",
        url: "https://github.com/anthropics/mcp-servers",
        categories: &[Category::Cloud],
        languages: &[],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("AWS_ACCESS_KEY_ID"),
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "cloudflare",
        name: "Cloudflare",
        description: "Workers, KV, R2, and DNS management",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-cloudflare@0.6.2"],
        install_hint: "npx (auto-installed), requires CLOUDFLARE_API_TOKEN env var",
        url: "https://github.com/anthropics/mcp-servers",
        categories: &[Category::Cloud],
        languages: &["typescript", "javascript"],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("CLOUDFLARE_API_TOKEN"),
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    // ── Browser / Search ───────────────────────────────────────────
    McpServerEntry {
        key: "puppeteer",
        name: "Puppeteer",
        description: "Headless Chrome for scraping and testing",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-puppeteer@0.6.2"],
        install_hint: "npx (auto-installed)",
        url: "https://github.com/anthropics/mcp-servers/tree/main/src/puppeteer",
        categories: &[Category::Browser],
        languages: &[],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::Npx,
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "brave-search",
        name: "Brave Search",
        description: "Web search via Brave Search API",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-brave-search@0.6.2"],
        install_hint: "npx (auto-installed), requires BRAVE_API_KEY env var",
        url: "https://github.com/anthropics/mcp-servers/tree/main/src/brave-search",
        categories: &[Category::Search],
        languages: &[],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("BRAVE_API_KEY"),
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    // ── Memory / Reasoning ─────────────────────────────────────────
    McpServerEntry {
        key: "memory",
        name: "Memory",
        description: "Knowledge graph that persists across sessions",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-memory@0.6.2"],
        install_hint: "npx (auto-installed)",
        url: "https://github.com/anthropics/mcp-servers/tree/main/src/memory",
        categories: &[Category::Memory],
        languages: &[],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::Npx,
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "sequential-thinking",
        name: "Sequential Thinking",
        description: "Structured step-by-step problem decomposition",
        command: "npx",
        args: &[
            "-y",
            "@modelcontextprotocol/server-sequential-thinking@1.2.0",
        ],
        install_hint: "npx (auto-installed)",
        url: "https://github.com/modelcontextprotocol/servers/tree/main/src/sequentialthinking",
        categories: &[Category::Reasoning],
        languages: &[],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::Npx,
        version: "1.2.0",
        min_node_version: Some("18"),
    },
    // ── File System ────────────────────────────────────────────────
    McpServerEntry {
        key: "filesystem",
        name: "Filesystem",
        description: "Read, write, search, and manage project files",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-filesystem@0.6.2"],
        install_hint: "npx (auto-installed)",
        url: "https://github.com/anthropics/mcp-servers/tree/main/src/filesystem",
        categories: &[Category::FileSystem],
        languages: &[],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::Npx,
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    // ── Code Intelligence ──────────────────────────────────────────
    McpServerEntry {
        key: "eslint",
        name: "ESLint",
        description: "Lint and auto-fix JavaScript/TypeScript code",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-eslint@0.6.2"],
        install_hint: "npx (auto-installed)",
        url: "https://github.com/anthropics/mcp-servers",
        categories: &[Category::CodeIntel],
        languages: &["typescript", "javascript"],
        frameworks: &[],
        default_enabled: false,
        install_check: InstallCheck::Npx,
        version: "0.6.2",
        min_node_version: Some("18"),
    },
    McpServerEntry {
        key: "graphql",
        name: "GraphQL",
        description: "Schema inspection and query execution",
        command: "npx",
        args: &["-y", "@anthropic/mcp-server-graphql@0.6.2"],
        install_hint: "npx (auto-installed), requires GRAPHQL_ENDPOINT env var",
        url: "https://github.com/anthropics/mcp-servers",
        categories: &[Category::CodeIntel, Category::Database],
        languages: &["typescript", "javascript"],
        frameworks: &["next", "react", "vue", "angular"],
        default_enabled: false,
        install_check: InstallCheck::NpxWithEnv("GRAPHQL_ENDPOINT"),
        version: "0.6.2",
        min_node_version: Some("18"),
    },
];

// ── Query helpers ──────────────────────────────────────────────────────

/// Return all catalog entries.
pub(crate) fn all() -> &'static [McpServerEntry] {
    CATALOG
}

/// Return all servers relevant for the given language + framework.
///
/// Results are sorted: language-specific matches first, then universal servers.
pub(crate) fn recommend(language: &str, framework: &str) -> Vec<&'static McpServerEntry> {
    let mut specific: Vec<&McpServerEntry> = Vec::new();
    let mut universal: Vec<&McpServerEntry> = Vec::new();

    for entry in CATALOG {
        if !entry.is_relevant(language, framework) {
            continue;
        }
        if entry.languages.is_empty() && entry.frameworks.is_empty() {
            universal.push(entry);
        } else {
            specific.push(entry);
        }
    }

    specific.extend(universal);
    specific
}

/// Look up a server by key.
pub(crate) fn find(key: &str) -> Option<&'static McpServerEntry> {
    CATALOG.iter().find(|e| e.key == key)
}

#[cfg(test)]
#[path = "registry.tests.rs"]
mod tests;
