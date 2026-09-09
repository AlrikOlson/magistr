# Ruby Conventions

## Toolchain

- Rubocop enforced. Configuration in `.rubocop.yml`. Treat offenses as errors in CI.
- Use Bundler for dependency management. `bundle exec` for all commands.
- Specify Ruby version in `.ruby-version` or `Gemfile`.

## Error Handling

- Never `rescue Exception` — it catches `SignalException` and `SystemExit`. Use `rescue StandardError`.
- Create custom error classes inheriting from `StandardError` for domain errors.
- Use `begin/rescue/ensure` with specific exception types.
- Wrap external errors with context before re-raising.

## Naming

- `snake_case` for methods, variables, files, and directories.
- `PascalCase` for classes and modules.
- `SCREAMING_SNAKE_CASE` for constants.
- Predicate methods end with `?`: `valid?`, `empty?`, `admin?`.
- Dangerous methods end with `!`: `save!`, `update!` (mutating or raising variants).

## Patterns

- Duck typing — depend on behavior, not class. Use `respond_to?` if needed.
- Blocks and procs for callbacks and lazy evaluation.
- `Enumerable` methods (`map`, `select`, `reject`, `reduce`) over explicit loops.
- Keyword arguments for methods with more than two parameters.
- Use `Struct` or `Data` (Ruby 3.2+) for simple value objects.
- Freeze string literals in performance-sensitive code.

## Testing

- RSpec with `describe` / `context` / `it` blocks.
- `let` for lazy setup, `subject` for the thing under test.
- FactoryBot for test data — factories over fixtures.
- `shared_examples` for testing common behavior across classes.
- Use `WebMock` or `VCR` for HTTP stubbing. No real network calls in unit tests.
- `DatabaseCleaner` or transactional tests for database isolation.

## Anti-Patterns

- No monkey-patching stdlib classes in application code. Use refinements if truly necessary.
- No `rescue Exception`. Always rescue specific error types.
- No methods longer than 15 lines. Extract private methods or service objects.
- No `eval` or `send` with user input — security risk.
- No `puts` or `p` in committed code — use a logger.
- No deeply nested conditionals. Use guard clauses and early returns.
- No Rails callbacks for complex business logic — use service objects.

## Dependencies

- Pin gem versions in `Gemfile`. Use `Gemfile.lock` in applications (not in gems).
- Audit gems for maintenance status and known vulnerabilities.
- Prefer stdlib: `Net::HTTP`, `JSON`, `FileUtils`, `Pathname`.
