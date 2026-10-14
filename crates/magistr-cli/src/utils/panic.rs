//! Panic diagnostics close registered child process groups without writing protocol bytes to stdout.
use color_eyre::eyre;

pub fn install_panic_hook() -> eyre::Result<()> {
    color_eyre::install()?;
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        super::child_guard::kill_all_children();
        default_hook(info);
    }));
    Ok(())
}
