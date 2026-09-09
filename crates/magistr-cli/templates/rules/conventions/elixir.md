# Elixir Conventions

## Toolchain

- Use `mix format` — enforced, no style debates.
- `mix credo --strict` for static analysis. Treat warnings as errors.
- Dialyzer with `@spec` and `@type` annotations on public functions.
- Specify Elixir version in `mix.exs` and `.tool-versions`.

## Error Handling

- Use `{:ok, value}` / `{:error, reason}` tuples for expected failures.
- Use `with` for chaining happy-path operations with clear error fallthrough.
- Reserve `raise` / `rescue` for truly exceptional, unexpected errors.
- Name error atoms descriptively: `:not_found`, `:unauthorized`, `:invalid_input`.
- Bang functions (`save!`, `fetch!`) raise on error — use only when failure is a bug.

## Naming

- `snake_case` for functions, variables, and atoms.
- `PascalCase` for modules.
- Prefix private functions with context if the module is large.
- Use `?` suffix for functions returning booleans: `valid?/1`.
- Use `!` suffix for functions that raise on error.

## Patterns

- Pattern matching everywhere — function heads, case, with, receive.
- Pipeline operator `|>` for data transformations. Each step should be readable standalone.
- GenServer for stateful processes. Keep state minimal and well-typed.
- Supervision trees for fault tolerance. Let it crash and restart.
- Use structs with `@enforce_keys` for domain data. Not bare maps.
- Context modules (e.g., `Accounts`, `Billing`) to group related functions.

## Testing

- ExUnit with `describe` / `test` blocks.
- Doctests for simple pure functions — keeps docs and tests in sync.
- `setup` and `setup_all` callbacks for shared test state.
- Use `Mox` for mocking — define behaviours, mock at boundaries.
- Property-based testing with `StreamData` for invariant verification.
- Use `Ecto.Adapters.SQL.Sandbox` for concurrent database tests.

## Anti-Patterns

- No `try/rescue` for control flow. Use pattern matching and `{:ok, _}` / `{:error, _}`.
- No large GenServer state. If state grows complex, split into multiple processes.
- No long pipelines (6+ stages) without intermediate variables for readability.
- No hardcoded configuration. Use `Application.get_env/3` or runtime config.
- No `String.to_atom/1` with user input — atom table is not garbage collected.
- No business logic in LiveView/controller modules — delegate to context modules.

## OTP

- One supervision tree per application concern.
- Use `DynamicSupervisor` for processes created at runtime.
- Prefer `GenServer.call/3` for synchronous operations, `cast/2` only when fire-and-forget is acceptable.
- Set timeouts on `GenServer.call/3` — do not rely on the default 5-second timeout in production.
