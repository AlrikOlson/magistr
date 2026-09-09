# Go Conventions

## Toolchain

- `gofmt` enforced — no style debates. Use `goimports` for import ordering.
- `go vet` and `golangci-lint` in CI. Treat warnings as errors.
- Go modules (`go.mod`) for dependency management. Run `go mod tidy` before commit.

## Error Handling

- Always check errors. Never discard with `_` unless explicitly justified with a comment.
- Wrap errors with context: `fmt.Errorf("fetching user %d: %w", id, err)`.
- Use sentinel errors (`var ErrNotFound = errors.New(...)`) for expected conditions.
- Use `errors.Is()` and `errors.As()` for error inspection — not type assertions.
- Return errors, do not panic. `panic` is only for unrecoverable programmer errors.

## Naming

- Short variable names in narrow scope: `i`, `r`, `ctx`. Descriptive names in wider scope.
- Exported identifiers: `PascalCase`. Unexported: `camelCase`.
- Acronyms are all-caps: `HTTPClient`, `userID`, not `HttpClient`, `userId`.
- Interface names describe behavior: `Reader`, `Stringer`, `Handler`. Not `IReader`.
- Package names are lowercase, single-word, no underscores: `http`, `user`, `auth`.

## Patterns

- Define interfaces at the consumer, not the producer. Keep interfaces small (1-3 methods).
- Table-driven tests for testing multiple cases against the same logic.
- Functional options pattern for configurable constructors: `func WithTimeout(d time.Duration) Option`.
- Use `context.Context` as the first parameter for functions that do I/O or may be cancelled.
- Embed structs for composition. Avoid deep inheritance-like hierarchies.
- Use channels for communication, mutexes for state. Do not mix.

## Modules

- One `go.mod` per deployable unit. Use `go mod tidy` before every commit.
- Use semantic versioning. Modules at `v2+` must use `/v2` path suffix.
- Vendor dependencies with `go mod vendor` for reproducible builds in CI.
- Replace directives (`replace` in `go.mod`) are for local development only — never commit them to shared branches.
- Use `go work` (workspaces) for multi-module monorepos during local development.
- Pin tool dependencies with `//go:generate go run tool@version` or a `tools.go` file with blank imports.

## Error Handling (Advanced)

- Use `fmt.Errorf("operation %s: %w", name, err)` to build an error chain. Always wrap with `%w`.
- Define error types with `type MyError struct { ... }` when callers need to inspect error fields.
- Group related sentinel errors as package-level vars: `var (ErrNotFound = errors.New(...))`.
- Use `errors.Join` (Go 1.20+) to combine multiple errors from concurrent operations.
- Never log and return the same error — do one or the other. Logging duplicates obscure root causes.
- Use custom error types implementing `Unwrap() []error` for multi-error unwrapping.

## Concurrency

- Do not start goroutines without a way to stop them (context cancellation or done channel).
- Use `sync.WaitGroup` or `errgroup.Group` to wait for goroutines.
- Prefer `sync.Map` only for specific patterns — a regular map with mutex is usually clearer.
- Use `errgroup.Group` with `SetLimit()` for bounded concurrency (worker pool pattern).
- Prefer `context.WithCancelCause` (Go 1.20+) over `context.WithCancel` for debuggable cancellations.
- Use `sync.Once` for lazy initialization. Use `sync.OnceValue` (Go 1.21+) for lazy singletons.
- Always acquire locks in the same order across goroutines to prevent deadlocks.
- Use `go vet -copylocks` to detect accidentally copied mutexes.

## Testing

- `_test.go` companion files in the same package.
- Subtests with `t.Run("case name", func(t *testing.T) {...})`.
- `testify/assert` or stdlib `testing` — pick one per project and be consistent.
- Use `t.Helper()` in test helper functions for accurate line reporting.
- Use `t.TempDir()` for filesystem tests — automatically cleaned up.
- Use `httptest.NewServer` for HTTP integration tests.
- Use build tags (`//go:build integration`) to separate slow integration tests from unit tests.
- Run with `-race` flag in CI: `go test -race ./...` to detect data races.
- Use `t.Parallel()` for tests that can run concurrently. Default to parallel when safe.

## Anti-Patterns

- No `init()` functions unless absolutely necessary (global registries only).
- No `panic()` in library code. Return errors.
- No interface pollution — do not define interfaces with 10+ methods.
- No package-level mutable state. Pass dependencies via struct fields or function parameters.
- No `reflect` unless building serialization or generic tooling.
- No `go:generate` without documenting what it generates and how to re-run.
- No `interface{}` — use `any` (Go 1.18+). Use generics over `any` when the type set is known.
