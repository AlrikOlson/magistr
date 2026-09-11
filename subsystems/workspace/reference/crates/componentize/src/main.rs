//! componentize — encode a wit-bindgen core module as a component.
//!
//! Usage: `cargo run -p componentize --release -- <core.wasm> <out.wasm>`
//!
//! The input must be a `wasm32-wasip1` core module whose component-type
//! metadata was embedded by the `wit_bindgen::generate!` macro, with zero
//! WASI imports (the guest crates enforce that by construction — no IO, no
//! clocks, no HashMap). No adapter is linked: a module that smuggles a WASI
//! import in fails loudly here rather than shipping a component the host's
//! default-deny would refuse.

use anyhow::{Context, bail};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        bail!("usage: componentize <core.wasm> <out.wasm>");
    };
    let module = std::fs::read(&input).with_context(|| format!("reading {input}"))?;
    let component = wit_component::ComponentEncoder::default()
        .module(&module)
        .context("loading core module (is component-type metadata embedded?)")?
        .validate(true)
        .encode()
        .context("encoding component")?;
    std::fs::write(&output, &component).with_context(|| format!("writing {output}"))?;
    println!(
        "componentize: {input} ({} bytes) -> {output} ({} bytes)",
        module.len(),
        component.len()
    );
    Ok(())
}
