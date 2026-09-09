# PHP Conventions

## Toolchain

- PHP 8.1+ with strict types enabled: `declare(strict_types=1);` in every file.
- Composer for dependency management. Commit `composer.lock` for reproducible builds.
- PHPStan (level 8+) or Psalm for static analysis. Treat all issues as errors in CI.
- PHP-CS-Fixer or PHP_CodeSniffer for formatting — enforce PSR-12 coding standard.

## Error Handling

- Use typed exceptions: extend `\RuntimeException` or `\LogicException` for domain errors.
- Always catch specific exception types, not bare `\Exception` or `\Throwable`.
- Use `try/catch` for recoverable errors. Let uncaught exceptions propagate to the error handler.
- Return `null` or a result object for expected "not found" cases — do not throw for flow control.
- Use `set_error_handler()` to convert legacy warnings/notices into exceptions.

## Naming

- `PascalCase` for classes, interfaces, traits, and enums.
- `camelCase` for methods, functions, and variables.
- `SCREAMING_SNAKE_CASE` for constants.
- `snake_case` for file names only in framework-specific contexts (e.g., Laravel migrations).
- Prefix interfaces with the behavior they define: `Renderable`, `Cacheable`, not `IRenderable`.
- Prefix boolean methods with `is`, `has`, `can`, `should`.

## Patterns

- Use constructor property promotion: `public function __construct(private readonly string $name)`.
- Use `readonly` properties and classes for immutable data (PHP 8.2+).
- Use enums (PHP 8.1+) over class constants for type-safe value sets.
- Prefer composition over inheritance. Use traits sparingly — they break LSP.
- Use named arguments for functions with optional parameters: `fn(timeout: 30)`.
- Use `match` expressions over `switch` — exhaustive, no fall-through.
- Use typed properties and return types everywhere. Avoid `mixed` except at serialization boundaries.

## Dependency Injection

- Constructor injection for required dependencies. No service locator pattern.
- Use interfaces for dependencies that may have multiple implementations.
- Avoid static methods for anything that needs testing or swapping.
- Use DI container bindings, not `new` in business logic.

## Testing

- PHPUnit for unit and integration tests.
- Test files named `{Name}Test.php` in `tests/` directory.
- Use `setUp()` and `tearDown()` for fixtures.
- Use data providers for parameterized tests: `#[DataProvider('provider')]`.
- Mock external dependencies with PHPUnit mocks or Mockery.
- Use `assertSame()` over `assertEquals()` for strict comparison.

## Anti-Patterns

- No `@` error suppression operator — fix the root cause.
- No `eval()` or `extract()` — security risks.
- No `global` keyword — pass dependencies explicitly.
- No `var_dump()` or `dd()` in committed code — use structured logging.
- No raw SQL without parameterized queries — always use prepared statements.
- No `array` for structured data — use typed DTOs or value objects.
