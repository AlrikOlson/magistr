# TypeScript Conventions

## Toolchain

- Strict mode enabled (`"strict": true` in tsconfig). No exceptions.
- `npx tsc --noEmit` for type checking. Treat type errors as build failures.
- ESLint or Biome for linting — warnings are errors in CI.
- Prettier or Biome for formatting — no style debates.

## Error Handling

- No untyped `catch (e)`. Always narrow: `catch (e) { if (e instanceof SpecificError) ... }`.
- Use discriminated unions or a `Result<T, E>` pattern for expected failures.
- Throw only for truly exceptional cases. Return error types for expected ones.
- Wrap external errors with context before re-throwing.

## Naming

- `camelCase` for functions, variables, parameters, and methods.
- `PascalCase` for types, interfaces, classes, enums, and React components.
- `SCREAMING_SNAKE_CASE` for constants and environment variable keys.
- Prefix interfaces with descriptive nouns, not `I` — `UserService` not `IUserService`.

## Patterns

- Discriminated unions for state machines and variant types. Exhaustive `switch` with `never` default.
- Zod or Valibot for runtime validation at system boundaries. Infer types from schemas.
- Prefer `const` over `let`. Never use `var`.
- Prefer `readonly` for properties that should not be reassigned.
- Use `satisfies` operator to validate types without widening.
- Prefer `Map`/`Set` over plain objects when keys are dynamic.

## ESM & Module System

- Use ESM (`import`/`export`) exclusively. No CommonJS (`require`/`module.exports`) in new code.
- Set `"type": "module"` in `package.json` and `"module": "ESNext"` in tsconfig.
- Use `.js` extensions in import specifiers when targeting Node.js ESM (TypeScript resolves `.ts` → `.js`).
- Avoid dynamic `import()` unless code-splitting is intentional (lazy routes, heavy deps).
- Use `exports` field in `package.json` for dual CJS/ESM packages. Map subpath exports explicitly.
- Prefer `node:` protocol for Node.js built-ins: `import { readFile } from "node:fs/promises"`.

## Monorepo

- Use workspace protocol (`workspace:*`) for internal package references.
- Prefer `turborepo` or `nx` for task orchestration. Define `dependsOn` for correct build order.
- Shared types go in a dedicated `@scope/types` package. Never duplicate type definitions.
- Each package has its own `tsconfig.json` extending a shared base config at the workspace root.
- Use project references (`"references"` in tsconfig) for incremental builds across packages.

## Bundling

- Tree-shake: mark packages as `"sideEffects": false` when safe. Avoid top-level side effects.
- Use `tsup`, `esbuild`, or `vite` for library bundling. Emit both ESM and CJS when needed.
- Configure path aliases in both `tsconfig.json` and the bundler to avoid divergence.
- Set `target` appropriately: `ES2022` for Node.js 18+, `ES2020` for broad browser support.
- Use source maps in development, omit or use `hidden` source maps in production.

## Testing

- `.test.ts` or `.spec.ts` companion files alongside source.
- `describe` / `it` blocks with clear names: `it("returns 404 when user not found")`.
- Mock sparingly — prefer real implementations and integration tests over mocks.
- One assertion concept per test. Multiple `expect()` calls are fine if they test one behavior.
- Use `vitest` for unit tests (ESM-native, fast). Configure with `vitest.config.ts`.
- Use `msw` (Mock Service Worker) for API mocking in tests and development.

## Anti-Patterns

- No `any`. Use `unknown` and narrow, or define a proper type.
- No `@ts-ignore` or `@ts-expect-error` without a comment explaining the workaround.
- No `!` non-null assertion without a comment justifying why null is impossible.
- No `as` type assertions when a type guard or discriminated union would work.
- No `enum` for simple string unions — use `type Status = "active" | "inactive"` instead.
- No barrel files (`index.ts` re-exports) in large projects — they break tree-shaking.
- No default exports — use named exports for refactoring safety.

## Dependencies

- Audit bundle size impact before adding a dependency. Use `bundlephobia` or `pkg-size`.
- Prefer native APIs (`fetch`, `URL`, `structuredClone`) over libraries when available.
- Keep `devDependencies` separate from `dependencies`.
- Lock files (`package-lock.json`, `pnpm-lock.yaml`) are committed and reviewed.
