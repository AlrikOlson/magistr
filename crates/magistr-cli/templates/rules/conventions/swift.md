# Swift Conventions

## Toolchain

- SwiftLint enforced. Configuration in `.swiftlint.yml`. Treat warnings as errors.
- Swift Package Manager for dependency management.
- Target the latest stable Swift version. Use modern concurrency (`async`/`await`).

## Error Handling

- Use `throws` functions with typed errors (Swift 6+) or `Error`-conforming enums.
- No force-try (`try!`) without a comment justifying why failure is impossible.
- Use `Result<Success, Failure>` for async callbacks that predate `async/await`.
- Use `guard` for early returns on precondition failures.
- Use `defer` for cleanup that must run regardless of exit path.

## Naming

- `camelCase` for functions, methods, properties, and variables.
- `PascalCase` for types, protocols, and enums.
- Protocols describe capability: `Codable`, `Equatable`, `Sendable`. Not `CodableProtocol`.
- Boolean properties: `is`, `has`, `can`, `should` prefix.
- Factory methods: `make` prefix (`makeViewController`, `makeRequest`).

## Patterns

- Protocol-oriented design. Prefer protocols with default extensions over class inheritance.
- Value types (`struct`, `enum`) by default. Use `class` only when reference semantics are needed.
- `enum` with associated values for state machines and variants.
- `Codable` for serialization. Custom `CodingKeys` when API names differ from Swift names.
- Use `@MainActor` for UI-bound code. `Sendable` for cross-actor data.
- Property wrappers for cross-cutting concerns (`@Published`, `@AppStorage`, custom wrappers).

## Testing

- XCTest with `test` prefix on methods. `XCTestCase` subclasses.
- Use `XCTAssertEqual`, `XCTAssertThrowsError`, `XCTAssertNil`.
- Async tests with `async throws` test methods.
- Use `expectation(description:)` and `wait(for:timeout:)` only for callback-based APIs.
- Protocol-based dependency injection for testability.

## Anti-Patterns

- No force unwrap (`!`) without a comment justifying why nil is impossible.
- No Massive View Controller. Extract logic into view models, coordinators, or services.
- No `class` when `struct` would work. Default to value types.
- No implicitly unwrapped optionals (`String!`) except for `@IBOutlet` (and avoid those too in SwiftUI).
- No stringly-typed APIs. Use enums, key paths, or typed identifiers.
- No `Any` or `AnyObject` when a protocol or generic would work.
- No synchronous blocking on the main thread. Use `async/await` or dispatch to background.

## Concurrency

- Use Swift concurrency (`async`/`await`, `Task`, actors) over GCD in new code.
- Mark shared mutable state with `actor` or `@MainActor`.
- Use `TaskGroup` for structured concurrent work.
- Check cancellation with `Task.checkCancellation()` in long-running operations.
