# Python Conventions

## Toolchain

- Type hints on all function signatures. `mypy --strict` or `pyright` for type checking.
- `ruff` for linting and formatting (or `black` + `flake8` if already configured).
- Minimum Python version declared in `pyproject.toml`.

## Error Handling

- No bare `except:` or `except Exception:` without re-raising or logging.
- Use specific exception types. Create custom exceptions for domain errors.
- Use context managers (`with` statements) for resource management.
- Wrap external errors with context: `raise AppError("context") from original`.

## Naming

- `snake_case` for functions, methods, variables, and modules.
- `PascalCase` for classes and type aliases.
- `SCREAMING_SNAKE_CASE` for module-level constants.
- Prefix private attributes with `_`. No double underscore unless name-mangling is needed.

## Patterns

- `dataclasses` or Pydantic `BaseModel` for structured data. No raw dicts for known shapes.
- Type aliases for complex types: `UserId = NewType("UserId", int)`.
- Generators and iterators for lazy evaluation over large sequences.
- Context managers for setup/teardown patterns.
- Use `pathlib.Path` over `os.path` for file operations.
- Use f-strings for formatting. No `%` or `.format()` for new code.

## Typing

- Use `from __future__ import annotations` for PEP 604 union syntax (`X | Y`) in older Python.
- Prefer `collections.abc` types for parameters: `Sequence`, `Mapping`, `Iterable` over `list`, `dict`.
- Use `TypeVar` for generic functions. Prefer `bound=` over `constraints=` when possible.
- Use `Protocol` for structural subtyping (duck typing with type safety).
- Use `TypedDict` for dictionary shapes at API boundaries. Prefer `total=False` and mark required keys.
- Use `Literal["a", "b"]` for fixed string/int sets instead of plain `str`.
- Use `@overload` to type functions with multiple calling conventions.

## Packaging

- Use `pyproject.toml` as the single source of truth for project metadata (`[project]` table).
- Prefer `uv` or `hatch` for project management. Fall back to `poetry` if already configured.
- Declare Python version requirement: `requires-python = ">=3.11"`.
- Use `src/` layout: source in `src/package_name/`, tests in `tests/`. Avoids import shadowing.
- Entry points go in `[project.scripts]`: `my-cli = "my_package.cli:main"`.
- Build with `build` backend. Prefer `hatchling` or `setuptools` with `pyproject.toml`.

## Virtual Environments

- Always use a virtual environment. Never `pip install` into system Python.
- Prefer `uv venv` (fast) or `python -m venv .venv`. Store in `.venv/` (gitignored).
- Pin all dependencies: `uv lock` or `pip freeze > requirements.lock`. Separate dev deps.
- Use `uv pip compile` or `pip-tools` for reproducible dependency resolution.
- Document Python version and venv setup in `README.md` or `Makefile`.

## Async

- Use `asyncio` for I/O-bound concurrency. Use `async def` / `await` consistently.
- Prefer `asyncio.TaskGroup` (Python 3.11+) over `asyncio.gather` for structured concurrency.
- Use `asyncio.Queue` for producer/consumer patterns. Avoid shared mutable state.
- Never call blocking I/O from async code — use `asyncio.to_thread()` for sync wrappers.
- Use `httpx` (async) over `requests` (sync) in async codebases.

## Testing

- `pytest` with `test_` prefix on functions and files.
- Fixtures for shared setup. `conftest.py` for cross-module fixtures.
- `@pytest.mark.parametrize` for testing multiple inputs against the same logic.
- Use `tmp_path` fixture for filesystem tests — never write to fixed paths.
- Test error cases with `pytest.raises(SpecificError)`.
- Use `pytest-asyncio` with `@pytest.mark.asyncio` for async test functions.
- Use `coverage` with `pytest-cov`. Target ≥80% line coverage on new code.

## Anti-Patterns

- No mutable default arguments: `def f(items=[])` is a bug. Use `None` and initialize inside.
- No `from module import *` — explicit imports only.
- No `type: ignore` without a specific error code and comment.
- No global mutable state. Pass dependencies explicitly.
- No print-based debugging in committed code. Use `logging` module.
- No nested functions more than two levels deep — extract to module-level.

## Dependencies

- Pin versions in `requirements.txt` or use `poetry.lock` / `uv.lock`.
- Use virtual environments. Never install into system Python.
- Prefer stdlib solutions when reasonable (`json`, `pathlib`, `dataclasses`, `enum`).
- Separate production and development dependencies. Use `[project.optional-dependencies]` for extras.
