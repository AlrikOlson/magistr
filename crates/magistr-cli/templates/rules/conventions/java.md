# Java Conventions

## Toolchain

- Target the latest LTS release (21+). Use `record` types, sealed interfaces, pattern matching.
- Build with Maven or Gradle. CI runs `mvn verify` or `gradle check`.
- Checkstyle or SpotBugs in CI. Treat warnings as errors.

## Error Handling

- Use checked exceptions only for recoverable conditions the caller must handle.
- Use unchecked exceptions (extending `RuntimeException`) for programming errors.
- Never catch `Exception` or `Throwable` broadly — catch specific types.
- No empty catch blocks. At minimum, log the exception.
- Use try-with-resources for all `AutoCloseable` resources.

## Naming

- `camelCase` for methods, variables, and parameters.
- `PascalCase` for classes, interfaces, enums, and records.
- `SCREAMING_SNAKE_CASE` for static final constants.
- Package names are all lowercase: `com.example.userservice`.
- Boolean methods: `is`, `has`, `can`, `should` prefix.

## Patterns

- `record` types for immutable data carriers. No getters/setters boilerplate.
- Sealed interfaces for algebraic data types with pattern matching.
- `Optional<T>` for nullable returns. Never pass `Optional` as a parameter.
- Streams for collection transformations. Prefer method references where readable.
- Dependency injection (constructor injection preferred). No field injection.
- Builder pattern for objects with many optional parameters.

## Testing

- JUnit 5 with `@Nested` classes for test grouping.
- AssertJ for fluent assertions: `assertThat(result).isEqualTo(expected)`.
- Mockito for test doubles — mock interfaces, not concrete classes.
- `@ParameterizedTest` with `@MethodSource` for table-driven tests.
- Integration tests in separate source set or tagged with `@Tag("integration")`.
- Use `@TempDir` for filesystem tests.

## Anti-Patterns

- No raw types: `List<String>` not `List`.
- No checked exceptions for control flow. Exceptions are exceptional.
- No null returns for collections — return empty collections instead.
- No God classes with 500+ lines. Split by responsibility.
- No `instanceof` chains — use sealed interfaces and pattern matching.
- No `static` utility classes when an instance with dependency injection is cleaner.
- No `System.out.println` — use SLF4J or java.util.logging.

## Dependencies

- Declare dependency versions in BOM or version catalog, not scattered in modules.
- Minimize transitive dependencies. Use `mvn dependency:tree` to audit.
- Prefer stdlib solutions: `java.time`, `java.net.http.HttpClient`, `java.util.stream`.
