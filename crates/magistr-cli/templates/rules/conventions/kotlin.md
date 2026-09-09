# Kotlin Conventions

## Toolchain

- Kotlin official style guide enforced via `ktlint` or `detekt`.
- Target latest stable Kotlin version. Use Gradle with Kotlin DSL (`build.gradle.kts`).
- Enable explicit API mode for library modules: `-Xexplicit-api=strict`.

## Error Handling

- Use `Result<T>` or sealed class hierarchies for expected failures.
- Throw exceptions only for truly exceptional, unexpected errors.
- No empty catch blocks. At minimum, log the exception.
- Use `runCatching {}` for wrapping throwable code into `Result`.
- Use `require()` and `check()` for preconditions (they throw `IllegalArgumentException` / `IllegalStateException`).

## Naming

- `camelCase` for functions, properties, and local variables.
- `PascalCase` for classes, interfaces, objects, enums, and type aliases.
- `SCREAMING_SNAKE_CASE` for compile-time constants (`const val`).
- Boolean properties: `is`, `has`, `can` prefix.
- Extension functions named as if they were members of the receiver type.

## Patterns

- `data class` for value types. No Java-style builders when a data class with default params works.
- `sealed class` / `sealed interface` for restricted type hierarchies with exhaustive `when`.
- Extension functions for adding behavior without inheritance.
- Scope functions: `let` for null checks, `apply` for configuration, `run` for transformations, `also` for side effects.
- Coroutines with structured concurrency for async code. No raw threads.
- Use `object` declarations for singletons. No hand-rolled singleton patterns.

## Testing

- JUnit 5 or Kotest for test framework. Pick one per project.
- MockK for mocking — Kotlin-native, supports coroutines.
- Use `@Nested` (JUnit 5) or nested `describe`/`context` (Kotest) for test grouping.
- `@TempDir` for filesystem tests.
- Use `runTest` from `kotlinx-coroutines-test` for testing suspend functions.

## Anti-Patterns

- No `!!` (non-null assertion) without a comment explaining why null is impossible here.
- No Java-style builders when `data class` with named/default parameters works.
- No `var` when `val` is sufficient. Default to immutable.
- No platform types leaking from Java interop — add explicit nullability annotations.
- No `GlobalScope.launch` — use structured concurrency with proper scope.
- No `companion object` as a dumping ground — keep it focused on factory methods and constants.

## Coroutines

- Use `CoroutineScope` from the lifecycle owner (Activity, ViewModel, Application).
- Use `withContext(Dispatchers.IO)` for blocking I/O, not `Dispatchers.Default`.
- Handle cancellation: use `ensureActive()` or check `isActive` in long-running loops.
- Use `Flow` for reactive streams. Prefer `StateFlow` / `SharedFlow` over LiveData in new code.
