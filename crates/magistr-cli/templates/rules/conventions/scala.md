# Scala Conventions

## Toolchain

- Scala 3 preferred for new projects. Scala 2.13 for legacy compatibility.
- sbt for build management. Use `build.sbt` with explicit dependency versions.
- Scalafmt for formatting — enforce via `sbt scalafmtCheckAll` in CI.
- Scalafix for linting and automated refactoring rules.

## Error Handling

- Use `Either[Error, T]` or `Try[T]` for fallible operations — avoid throwing exceptions.
- Reserve exceptions for truly exceptional conditions (I/O failures, system errors).
- Use `Option[T]` instead of null — Scala has no excuse for `NullPointerException`.
- Pattern match on error types: `case Left(NotFound(id)) => ...`.
- Use `cats.effect.IO` or `ZIO` for effectful error handling in functional codebases.

## Naming

- `camelCase` for methods, values, and variables.
- `PascalCase` for classes, traits, objects, and type aliases.
- `SCREAMING_SNAKE_CASE` for constants in companion objects.
- Package names are `lowercase.dotted`: `com.example.myapp.service`.
- Type parameters: single uppercase letters (`A`, `B`, `F`) or descriptive (`Key`, `Value`).
- Boolean methods: prefix with `is`, `has`, `can`, `should`.

## Patterns

- Prefer `val` over `var` — immutability by default.
- Use `case class` for data types — gives you `equals`, `hashCode`, `copy`, and pattern matching.
- Use `sealed trait` (Scala 2) or `enum` (Scala 3) for algebraic data types.
- Prefer pattern matching over `isInstanceOf`/`asInstanceOf`.
- Use for-comprehensions for chaining monadic operations (`Option`, `Either`, `Future`).
- Use extension methods (Scala 3) or implicit classes (Scala 2) to extend types without inheritance.
- Use `given`/`using` (Scala 3) or `implicit` for dependency injection and type class instances.

## Collections

- Use immutable collections by default (`List`, `Vector`, `Map`, `Set`).
- Prefer `Vector` over `List` for random access. Use `List` for prepend-heavy workloads.
- Chain transformations: `.map`, `.filter`, `.flatMap`, `.collect`.
- Use `.view` for lazy evaluation on large collections to avoid intermediate allocations.
- Avoid `mutable.Buffer` in public APIs — convert to immutable at boundaries.

## Testing

- ScalaTest or MUnit for test framework.
- Test files named `{Name}Spec.scala` in `src/test/scala/`.
- Use `describe`/`it` (FunSpec) or `test` (FunSuite) style — pick one and be consistent.
- Use property-based testing with ScalaCheck for invariant verification.
- Use `beforeAll`/`afterAll` for shared fixtures. Prefer `beforeEach` for test isolation.

## Anti-Patterns

- No `null` — use `Option[T]`. Configure Scalafix to flag null usage.
- No `Any` or `AnyRef` in type signatures — use proper types or generics.
- No `var` in case classes or public APIs.
- No blocking inside `Future` — use `blocking { }` wrapper or a separate execution context.
- No wildcard imports (`import foo._`) except for DSLs and test utilities.
- No implicit conversions (Scala 2) — use extension methods instead.
