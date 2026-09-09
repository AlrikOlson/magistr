# C# Conventions

## Toolchain

- Enable nullable reference types: `<Nullable>enable</Nullable>`.
- Use latest C# language version. Prefer `record` types, pattern matching, and file-scoped namespaces.
- `dotnet format` enforced. EditorConfig in repository root.
- Roslyn analyzers in CI. Treat warnings as errors: `<TreatWarningsAsErrors>true</TreatWarningsAsErrors>`.

## Error Handling

- Use exceptions for unexpected errors. Return `Result<T>` or nullable types for expected failures.
- No empty catch blocks. At minimum, log the exception.
- Use specific exception types: `ArgumentNullException`, `InvalidOperationException`, not generic `Exception`.
- `using` statements or `using` declarations for `IDisposable` resources.
- Use `ConfigureAwait(false)` in library code to avoid deadlocks.

## Naming

- `PascalCase` for classes, methods, properties, events, namespaces, and public fields.
- `camelCase` for local variables, parameters, and lambda parameters.
- `_camelCase` for private fields (with underscore prefix).
- `SCREAMING_SNAKE_CASE` is not idiomatic — use `PascalCase` even for constants.
- Prefix interfaces with `I`: `IRepository`, `ILogger`.
- Async methods end with `Async`: `GetUserAsync`, `SaveAsync`.

## Patterns

- `record` types for immutable value objects. `record struct` for small, stack-allocated values.
- Pattern matching with `switch` expressions and property/positional patterns.
- `async/await` throughout. No `.Result` or `.Wait()` on tasks — it causes deadlocks.
- LINQ for collection queries. Prefer method syntax for complex queries, query syntax for joins.
- Dependency injection via constructor injection. Register in `IServiceCollection`.
- `IDisposable` / `IAsyncDisposable` for resource cleanup. Implement dispose pattern correctly.

## Testing

- xUnit (preferred) or NUnit. One test class per production class.
- FluentAssertions for readable assertions: `result.Should().Be(expected)`.
- `[Theory]` with `[InlineData]` or `[MemberData]` for parameterized tests.
- Use `[Fact]` for single-case tests.
- NSubstitute or Moq for mocking interfaces.
- Use `ITestOutputHelper` for test logging.

## Anti-Patterns

- No `async void` except for event handlers. Always return `Task` or `Task<T>`.
- No `.Result` or `.Wait()` on tasks — use `await` instead.
- No God classes. Follow Single Responsibility Principle.
- No `dynamic` unless interoperating with COM or dynamic languages.
- No `object` parameters or return types when a generic or specific type works.
- No string concatenation in loops — use `StringBuilder` or string interpolation.
- No `null` returns for collections — return `Array.Empty<T>()` or `Enumerable.Empty<T>()`.

## Project Structure

- One project per concern: `*.Domain`, `*.Application`, `*.Infrastructure`, `*.Api`.
- Separate test projects: `*.Tests`, `*.IntegrationTests`.
- Use `Directory.Build.props` for shared MSBuild properties across projects.
