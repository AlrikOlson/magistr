//! Launch an interactive native fixture window.
//!
//! With no argument, show the busy fixture. `magistr-gui2 05` opens reference view 05
//! at its logical capture size. `magistr-gui2 sink` opens the component catalogue.

fn main() -> Result<(), magistr_gui2::Gui2Error> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();
    let view = std::env::args().nth(1);
    magistr_gui2::open("magistr", view.as_deref())
}
