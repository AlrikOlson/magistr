//! Run the cockpit on a real terminal.
//!
//! ```sh
//! cargo run -p magistr-cli --example cockpit
//! ```

use magistr::application::workflow_client::EngineClient;
use magistr_cockpit::cockpit::{Model, Summary};
use magistr_cockpit::theme::Theme;
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::term::{RealTerminal, Terminal as _};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::current_dir()?;
    // A viewer: observe the store without taking the host lease.
    let engines = match EngineClient::observe(&root) {
        Ok(engines) => engines,
        Err(err) => {
            println!(
                "magistr: cannot open the think-and-ship store for {}: {err}",
                root.display()
            );
            return Err(err.into());
        }
    };
    println!(
        "project {} · feed {}",
        engines.reader().project_id(),
        engines.socket().display()
    );

    let theme = Theme::from_os();
    let terminal = RealTerminal::stdio()?;
    let size = terminal.size()?;
    let runtime = Runtime::new(terminal, Options::from_os());
    let inbox = runtime.inbox();
    let tokio_runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let model = tokio_runtime.block_on(async move {
        let feed = engines.subscribe(inbox);
        let model = runtime.run(Model::new(size).with_theme(theme)).await;
        feed.abort();
        model
    })?;
    println!("{}", Summary::of(&model));
    Ok(())
}
