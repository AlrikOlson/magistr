# Project Conventions

## Clean Code

- Single responsibility: one function does one thing.
- Functions stay short (under 30 lines). If longer, extract.
- Guard clauses over nested conditionals. Return early.
- Names describe intent: `getUserById` not `getData`, `isValid` not `check`.
- No magic numbers or strings. Use named constants.
- Delete dead code. Do not comment it out.

## Naming Consistency

- Pick one naming convention per category and apply it uniformly:
  - Functions and variables: `camelCase` or `snake_case` (match language idiom).
  - Types and classes: `PascalCase`.
  - Constants: `SCREAMING_SNAKE_CASE`.
- File names match the primary export or class they contain.
- Test files are co-located with source or in a parallel test directory.

## Error Handling

- Handle errors explicitly. Never silently swallow exceptions or error codes.
- Use the language's idiomatic error mechanism (exceptions, Result types, error codes).
- Add context when propagating errors — what operation failed and why.
- Validate inputs at system boundaries. Do not trust external data.

## Testing

- Write tests alongside implementation. At minimum: happy path, error path, edge cases.
- One assertion concept per test. Clear test names describe the scenario.
- Use the project's test framework. Follow its conventions for file naming and structure.
- Test error paths, not just success paths.
- Use temporary directories for filesystem tests. Never write to fixed paths.

## Dependencies

- Minimize dependency count. Prefer standard library solutions when reasonable.
- Pin dependency versions. Update deliberately, not automatically.
- Audit new dependencies for maintenance status and security.

## Security

- No secrets in code. Use environment variables or secret managers.
- Validate and sanitize all external input.
- Use parameterized queries for database access. No string concatenation.
- Follow the principle of least privilege for file access and API permissions.

## Code Organization

- Separate concerns: data access, business logic, and presentation in distinct layers.
- Keep modules focused. If a file exceeds 300 lines, consider splitting.
- Group related functionality together. Alphabetical ordering within groups.
- Public API surface should be minimal. Hide implementation details.
