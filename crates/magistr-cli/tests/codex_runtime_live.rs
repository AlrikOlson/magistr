//! Opt-in acceptance against installed tools, confined to a disposable project.
#![cfg(unix)]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

struct Rpc {
    child: Child,
    input: Option<ChildStdin>,
    messages: mpsc::Receiver<Value>,
    errors: Arc<Mutex<Vec<String>>>,
    next_id: u64,
}

impl Rpc {
    fn start(mut command: Command) -> Self {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start disposable protocol server");
        let input = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let stderr = child.stderr.take().expect("stderr");
        let (send, messages) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Ok(message) = serde_json::from_str(&line)
                    && send.send(message).is_err()
                {
                    break;
                }
            }
        });
        let errors = Arc::new(Mutex::new(Vec::new()));
        let captured = Arc::clone(&errors);
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let mut errors = captured.lock().expect("stderr lock");
                if errors.len() < 30 {
                    errors.push(line);
                }
            }
        });
        Self {
            child,
            input: Some(input),
            messages,
            errors,
            next_id: 1,
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        writeln!(
            self.input.as_mut().expect("open stdin"),
            "{}",
            json!({"jsonrpc":"2.0", "method":method, "params":params})
        )
        .expect("send protocol notification");
        self.input
            .as_mut()
            .expect("open stdin")
            .flush()
            .expect("flush protocol notification");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        writeln!(
            self.input.as_mut().expect("open stdin"),
            "{}",
            json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params})
        )
        .expect("send protocol request");
        self.input
            .as_mut()
            .expect("open stdin")
            .flush()
            .expect("flush protocol request");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let message = self
                .messages
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|error| {
                    panic!(
                        "{method}: {error}; stderr: {:?}",
                        self.errors.lock().expect("stderr")
                    )
                });
            if message.get("id") == Some(&json!(id)) {
                assert!(message.get("error").is_none(), "{method}: {message}");
                return message["result"].clone();
            }
        }
    }

    fn app_initialize(&mut self) {
        self.request(
            "initialize",
            json!({"clientInfo":{"name":"magistr_setup_acceptance","version":"0.1.0"}}),
        );
        self.notify("initialized", json!({}));
    }

    fn mcp_initialize(&mut self) {
        self.request("initialize", json!({"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"magistr_setup_acceptance","version":"0.1.0"}}));
        self.notify("notifications/initialized", json!({}));
    }
}

impl Drop for Rpc {
    fn drop(&mut self) {
        // Only this test's own disposable child is stopped. Its workspace is
        // unique, so no existing Workspace connection or lock is disturbed.
        drop(self.input.take());
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn write(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().expect("parent")).expect("fixture directories");
    std::fs::write(path, content).expect("fixture file");
}

fn skill(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name).join("SKILL.md");
    write(
        &path,
        &format!(
            "---\nname: {name}\ndescription: Disposable acceptance skill.\n---\nProbe only.\n"
        ),
    );
    path
}

fn skills(server: &mut Rpc, cwd: &Path) -> BTreeMap<PathBuf, bool> {
    let result = server.request("skills/list", json!({"cwds":[cwd],"forceReload":true}));
    let rows = result["data"][0]["skills"].as_array().expect("skill list");
    rows.iter()
        .map(|skill| {
            (
                PathBuf::from(skill["path"].as_str().expect("skill path")),
                skill["enabled"].as_bool().expect("skill enabled"),
            )
        })
        .collect()
}

#[test]
#[ignore = "requires installed Codex; no model calls; disposable project only"]
fn installed_codex_discovers_relocated_project_and_generated_mcp_tools() {
    let codex = which::which("codex").expect("install Codex before the live acceptance test");
    let temp = tempfile::tempdir().expect("disposable fixture");
    let base = temp.path().canonicalize().expect("canonical fixture");
    let original = base.join("original project");
    let project = base.join("relocated project - café 🚀");
    let config_home = base.join("isolated codex home");
    let user_home = base.join("isolated user home");
    let bin = base.join("bin");
    std::fs::create_dir_all(&original).expect("original directory");
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .arg(&original)
            .status()
            .expect("initialize fixture Git repository")
            .success()
    );
    std::fs::create_dir_all(&bin).expect("bin");
    std::fs::create_dir_all(&user_home).expect("home");
    symlink(
        assert_cmd::cargo::cargo_bin!("magistr"),
        bin.join("magistr"),
    )
    .expect("portable magistr PATH");
    let paths = std::iter::once(bin.clone())
        .chain(std::env::split_paths(
            &std::env::var_os("PATH").expect("PATH"),
        ))
        .collect::<Vec<_>>();
    let path_env = std::env::join_paths(paths).expect("PATH list");
    let environment = |command: &mut Command| {
        command
            .env("CODEX_HOME", &config_home)
            .env("HOME", &user_home)
            .env("PATH", &path_env);
    };
    let mut setup = Command::new(assert_cmd::cargo::cargo_bin!("magistr"));
    setup
        .args(["init", "--yes", "--agent", "codex"])
        .arg(&original);
    environment(&mut setup);
    let output = setup.output().expect("generate project");
    assert!(
        output.status.success(),
        "setup failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::rename(&original, &project)
        .expect("relocate generated project without reinitializing");
    let nested = project.join("packages/app");
    let project_probe = skill(&project.join(".agents/skills"), "project-probe");
    let nested_probe = skill(&nested.join(".agents/skills"), "nested-probe");
    let duplicate = config_home.join("skills/next/SKILL.md");
    let personal = skill(&config_home.join("skills"), "personal-probe");
    write(
        &config_home.join("config.toml"),
        &format!(
            "[projects.{}]\ntrust_level = 'trusted'\n[[skills.config]]\npath = {}\nenabled = false\n[mcp_servers.magistr/traces]\ncommand = 'standalone-must-not-launch'\nargs = ['serve']\nenabled = true\n",
            toml::Value::String(project.to_str().expect("UTF8").to_owned()),
            toml::Value::String(personal.to_str().expect("UTF8").to_owned())
        ),
    );
    let config_path = project.join(".codex/config.toml");
    let mut project_config = std::fs::read_to_string(&config_path).expect("generated config");
    project_config.push_str(
        "\n[[skills.config]]\npath = '../.agents/skills/project-probe/SKILL.md'\nenabled = false\n",
    );
    write(&config_path, &project_config);
    write(
        &nested.join(".codex/config.toml"),
        "[[skills.config]]\npath = '../.agents/skills/nested-probe/SKILL.md'\nenabled = false\n",
    );
    let personal_before =
        std::fs::read(config_home.join("config.toml")).expect("personal config before");

    // Normal installed Codex discovers generated project configuration and
    // skills from root and nested cwd before introducing a personal collision.
    for cwd in [&project, &nested] {
        let mut command = Command::new(&codex);
        command.current_dir(cwd).arg("app-server");
        environment(&mut command);
        let mut server = Rpc::start(command);
        server.app_initialize();
        let config = server.request("config/read", json!({"cwd":cwd,"includeLayers":true}));
        assert_eq!(
            config["config"]["mcp_servers"]["magistr"]["command"],
            "magistr",
            "normal discovery at {}",
            cwd.display()
        );
        let discovered = skills(&mut server, cwd);
        for name in [
            "craft",
            "init",
            "next",
            "next-loop",
            "roadmap",
            "roadmap-refresh",
            "signals",
            "validate",
            "workspace-routing",
        ] {
            assert_eq!(
                discovered.get(&project.join(".agents/skills").join(name).join("SKILL.md")),
                Some(&true),
                "normal skill discovery: {name}"
            );
        }
        assert!(!discovered.contains_key(&duplicate));
    }
    skill(&config_home.join("skills"), "next");
    write(
        &project.join("acceptance.txt"),
        "portable_codex_acceptance\n",
    );

    for cwd in [&project, &nested] {
        let mut command = Command::new(project.join(".codex/run"));
        command
            .current_dir(&project)
            .arg("-C")
            .arg(cwd)
            .arg("app-server");
        environment(&mut command);
        let mut server = Rpc::start(command);
        server.app_initialize();
        let config = server.request("config/read", json!({"cwd":cwd,"includeLayers":true}));
        assert_eq!(
            config["config"]["mcp_servers"]["magistr"]["enabled"], false,
            "effective fixture config: {config}"
        );
        {
            let name = "magistr";
            let endpoint = &config["config"]["mcp_servers"][name];
            assert_eq!(endpoint["command"], "magistr");
            assert_eq!(endpoint["args"], json!(["mcp", "--dir", "."]));
        }
        let discovered = skills(&mut server, cwd);
        for name in [
            "craft",
            "init",
            "next",
            "next-loop",
            "roadmap",
            "roadmap-refresh",
            "signals",
            "validate",
            "workspace-routing",
        ] {
            assert_eq!(
                discovered.get(&project.join(".agents/skills").join(name).join("SKILL.md")),
                Some(&true),
                "project skill {name} must be enabled at {}",
                cwd.display()
            );
        }
        assert_eq!(discovered.get(&duplicate), Some(&false));
        assert_eq!(discovered.get(&personal), Some(&false));
        assert_eq!(discovered.get(&project_probe), Some(&false));
        if cwd == &nested {
            assert_eq!(discovered.get(&nested_probe), Some(&false));
        }
    }
    assert_eq!(
        std::fs::read(config_home.join("config.toml")).expect("personal config after"),
        personal_before
    );
    // Outside the project, direct Codex keeps the personal next skill enabled.
    let mut outside = Command::new(&codex);
    outside.current_dir(&user_home).arg("app-server");
    environment(&mut outside);
    let mut outside = Rpc::start(outside);
    outside.app_initialize();
    let outside_config = outside.request(
        "config/read",
        json!({"cwd":user_home,"includeLayers":false}),
    );
    assert_eq!(
        outside_config["config"]["mcp_servers"]["magistr"]["enabled"],
        true
    );
    assert_eq!(
        outside_config["config"]["mcp_servers"]["magistr"]["command"],
        "standalone-must-not-launch"
    );
    assert_eq!(
        skills(&mut outside, &user_home).get(&duplicate),
        Some(&true)
    );
    drop(outside);

    let configuration: toml::Table = project_config.parse().expect("generated TOML");
    for (name, required_tools) in [(
        "magistr",
        &["context", "search", "read", "edit", "discover", "execute"][..],
    )] {
        let endpoint = &configuration["mcp_servers"][name];
        let mut command = Command::new(endpoint["command"].as_str().expect("command"));
        command.args(
            endpoint["args"]
                .as_array()
                .expect("args")
                .iter()
                .map(|value| OsString::from(value.as_str().expect("argument"))),
        );
        command.current_dir(&nested);
        environment(&mut command);
        let mut server = Rpc::start(command);
        server.mcp_initialize();
        let list = server.request("tools/list", json!({}));
        let tool_names = list["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .map(|tool| tool["name"].as_str().expect("name"))
            .collect::<Vec<_>>();
        for required in required_tools {
            assert!(tool_names.contains(required), "{name} lacks {required}");
        }
        {
            let status = server.request(
                "tools/call",
                json!({"name":"execute","arguments":{"source":"return magistr.call(\"workspace_status\", {});"}}),
            );
            assert_ne!(
                status.get("isError"),
                Some(&Value::Bool(true)),
                "Workspace status failed: {status}"
            );
            assert!(
                status.to_string().contains(project.to_str().expect("UTF8")),
                "Workspace root must be relocated project, not nested or old path: {status}"
            );
            let read = server.request(
                "tools/call",
                json!({"name":"read","arguments":{"file":"acceptance.txt","budget":1000}}),
            );
            assert_workspace(&read, "portable_codex_acceptance");
            let search = server.request("tools/call", json!({"name":"search","arguments":{"query":"portable_codex_acceptance","mode":"lexical","path":"acceptance.txt","budget":1000}}));
            assert_workspace(&search, "acceptance.txt");

            // A second connection attaches to the same native owner.
            let mut second = Command::new(endpoint["command"].as_str().expect("command"));
            second
                .args(
                    endpoint["args"]
                        .as_array()
                        .expect("args")
                        .iter()
                        .map(|value| value.as_str().expect("argument")),
                )
                .current_dir(&nested);
            environment(&mut second);
            let mut second = Rpc::start(second);
            second.mcp_initialize();
            let read = second.request(
                "tools/call",
                json!({"name":"read","arguments":{"file":"acceptance.txt","budget":1000}}),
            );
            assert_workspace(&read, "portable_codex_acceptance");
            drop(second);
            let read = server.request(
                "tools/call",
                json!({"name":"read","arguments":{"file":"acceptance.txt","budget":1000}}),
            );
            assert_workspace(&read, "portable_codex_acceptance");
        }
    }
}

fn assert_workspace(result: &Value, expected: &str) {
    assert_ne!(
        result.get("isError"),
        Some(&Value::Bool(true)),
        "Workspace MCP error: {result}"
    );
    let content = result["content"].as_array().expect("Workspace content");
    let envelope: Value = content
        .iter()
        .filter_map(|part| part["text"].as_str())
        .find_map(|text| serde_json::from_str(text).ok())
        .expect("Workspace JSON envelope");
    assert_eq!(
        envelope["ok"], true,
        "Workspace operation failed: {envelope}"
    );
    assert!(
        envelope.to_string().contains(expected),
        "Workspace result lacks {expected}: {envelope}"
    );
}
