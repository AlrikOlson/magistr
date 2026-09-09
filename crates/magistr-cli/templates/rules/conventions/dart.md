# Dart Conventions

## Toolchain

- Dart SDK or Flutter SDK for compilation and tooling.
- `dart analyze` with `--fatal-infos` in CI — treat all info-level diagnostics as errors.
- `dart format` enforced — no style debates. Line length default is 80.
- `dart pub get` for dependency resolution. Use `pubspec.lock` for reproducible builds.

## Error Handling

- Use `try/catch` for recoverable errors. Catch specific exception types, not bare `catch`.
- Avoid catching `Error` — errors indicate programmer mistakes, not runtime conditions.
- Use `Result`-like patterns with sealed classes for domain errors: `sealed class Result<T> { ... }`.
- Prefer returning error types over throwing exceptions for expected failure modes.
- Use `assert()` in development for invariant checking — assertions are removed in release builds.

## Naming

- `lowerCamelCase` for variables, functions, methods, and parameters.
- `UpperCamelCase` for classes, enums, typedefs, and type parameters.
- `lowercase_with_underscores` for file names, library names, and package names.
- `SCREAMING_CAPS` only for compile-time constants.
- Prefix private members with `_`. Prefix boolean getters/methods with `is`, `has`, `can`, `should`.

## Patterns

- Prefer `final` over `var` when the variable is not reassigned.
- Use `const` constructors for immutable value objects.
- Prefer named parameters over positional for functions with 3+ parameters.
- Use `extension` types and methods to add functionality without subclassing.
- Use `sealed` classes for exhaustive pattern matching (Dart 3+).
- Prefer `record` types for simple data grouping: `(String name, int age)`.
- Use `switch` expressions with pattern matching over if/else chains.

## Null Safety

- Avoid `!` (null assertion operator) — handle null explicitly with `??`, `?.`, or pattern matching.
- Use `late` only when initialization is guaranteed before access. Prefer nullable with null check.
- Annotate return types explicitly — do not rely on type inference for public APIs.

## Testing

- Test files in `test/` directory, named `{name}_test.dart`.
- Use `test()` and `group()` from `package:test`.
- Use `expect()` with matchers: `expect(result, equals(42))`.
- Use `setUp()` and `tearDown()` for test fixtures.
- Use `mockito` or `mocktail` for mocking dependencies.

## Anti-Patterns

- No `dynamic` unless interfacing with untyped APIs (JSON, platform channels).
- No `print()` in production code — use a logging package (`package:logging`).
- No mutable global state — use dependency injection or provider patterns.
- No `as` casts without a preceding `is` check — prefer pattern matching.
- No `Future.catchError` — use `try/catch` in `async` functions instead.
