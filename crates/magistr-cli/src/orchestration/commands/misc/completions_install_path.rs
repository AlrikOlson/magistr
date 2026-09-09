/// Determine the install path for shell completions.
///
/// Returns `None` if the path cannot be determined (e.g., HOME not set).
pub fn completions_install_path(shell: clap_complete::Shell) -> Option<std::path::PathBuf> {
    from_lookup(shell, |key| std::env::var(key).ok())
}

fn from_lookup(
    shell: clap_complete::Shell,
    get: impl Fn(&str) -> Option<String>,
) -> Option<std::path::PathBuf> {
    let home = get("HOME")?;
    let home = std::path::Path::new(&home);

    // Respect XDG_DATA_HOME for bash/zsh when appropriate
    let xdg_data = get("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| home.join(".local/share"));

    match shell {
        clap_complete::Shell::Bash => {
            // ~/.local/share/bash-completion/completions/magistr
            Some(xdg_data.join("bash-completion/completions/magistr"))
        }
        clap_complete::Shell::Zsh => {
            // ~/.local/share/zsh/site-functions/_jig
            Some(xdg_data.join("zsh/site-functions/_jig"))
        }
        clap_complete::Shell::Fish => {
            // ~/.config/fish/completions/magistr.fish
            let fish_dir = get("XDG_CONFIG_HOME")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| home.join(".config"));
            Some(fish_dir.join("fish/completions/magistr.fish"))
        }
        clap_complete::Shell::PowerShell => {
            // PowerShell completions need manual setup; no standard path
            None
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "completions_install_path.tests.rs"]
mod tests;
