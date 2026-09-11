# Workbench example plugins

These are the SDK's own example plugins (SDD §8.5). Each directory is one
installable plugin: a `plugin.toml` capability manifest plus its component.
The components are WebAssembly component-model guests kept as WAT text — the
sandbox host accepts WAT directly, which keeps the examples reviewable.

They exist to be held to the same bar as everything else: the plugin TCK
(`runPluginTck`, `src/tools/tck.ts`) runs them **unmodified** in
`test/plugin-tck.test.ts`, and the M4/M5 gates require that to stay true.

| Plugin | What it shows |
|---|---|
| `echo/` | The smallest conforming tool plugin: one tool, no imports, echoes the canonical `{v, tool, args}` payload back as its JSON result. |
| `lang-pack-example/` | The lang-pack conformance convention: a `classify-file` tool (which routes the TCK's fuzz-corpus chapter through the live invoke lane) answering a constant `{"class":"source"}`. The real `workbench:lang-pack` world lowering arrives with M5. |
