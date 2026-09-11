# Phase 1 Data Model: rustfmt Formatter

No stored data changes. No port version change (`FormatterPort` is implemented, not widened).

## New adapter shapes (in-memory)

```
RustfmtFormatterOptions
  root        string                 workspace root (rustup cwd; path jail)
  binary?     string                 explicit binary (test seam; skips resolution)
  resolver?   (root) => string|null  injectable resolution (test seam)
  timeoutMs?  number                 default 10_000

RustfmtFormatter implements FormatterPort
  format({path, content}) → {content, changed}
    - ext !== "rs"  ⇒ passthrough (no spawn)           [router normally prevents this]
    - binary null   ⇒ throw Error("rustfmt is not installed for this workspace's toolchain (rustup component add rustfmt)")
    - spawn <bin> --edition <e>, cwd = dirname(root/path), stdin = content
    - exit 0        ⇒ {content: stdout, changed: stdout !== content}
    - exit != 0     ⇒ throw Error("rustfmt exited <code>: <first stderr line>")
    - timeout       ⇒ kill, throw Error("rustfmt was killed by <signal> after <ms>ms")

editionFor(root, path, read) → "2015" | "2018" | "2021" | "2024"
  nearest Cargo.toml above the file (within root): [package] edition
  edition.workspace = true or absent ⇒ root Cargo.toml [workspace.package] edition
  else "2021"

routingFormatter({ rules: [{ extensions: ["rs"], formatter }], fallback }) → FormatterPort
  format(req) ⇒ the first rule whose extension matches, else fallback
```

## Changed (text only): the write engine's formatter note

```
before: "<path>: the formatter failed; applied the verified unformatted content"
after:  "<path>: the formatter failed (<message>); applied the verified unformatted content"
```
