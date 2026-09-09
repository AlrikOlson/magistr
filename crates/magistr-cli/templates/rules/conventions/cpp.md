# C++ Conventions

## Toolchain

- Modern C++ (C++17 minimum, C++20 preferred). Specify standard in CMakeLists.txt or build config.
- `clang-format` enforced — configuration in `.clang-format`. No style debates.
- `clang-tidy` for static analysis. Treat warnings as errors in CI.
- Use CMake (3.20+) or your project's established build system.

## Error Handling

- Use exceptions for truly exceptional errors. Use return codes or `std::expected` (C++23) / `std::optional` for expected failures.
- RAII ensures cleanup — destructors handle resource release. No manual `free()` or `delete`.
- Use `std::error_code` for system-level errors when exceptions are disabled.
- Document exception guarantees: noexcept, basic, strong, or no guarantee.

## Naming

- Follow project convention consistently. Common styles:
  - `snake_case` for functions, variables, namespaces (STL style).
  - `PascalCase` for classes, structs, enums, and type aliases.
  - `SCREAMING_SNAKE_CASE` for macros and compile-time constants.
- `m_` or trailing `_` prefix for member variables (pick one, be consistent).
- Namespaces match directory structure.

## Patterns

- RAII for all resource management. `std::unique_ptr` for exclusive ownership, `std::shared_ptr` only when shared ownership is genuinely needed.
- Move semantics: implement move constructors/assignment for resource-owning types.
- `std::string_view` and `std::span` for non-owning references to contiguous data.
- `constexpr` for compile-time computation. `const` by default.
- Ranges (C++20) for algorithm composition.
- Templates for generic code, but prefer concepts (C++20) for constraints over SFINAE.

## Testing

- Google Test or Catch2 for unit tests.
- `TEST()` / `TEST_F()` (GTest) or `TEST_CASE` / `SECTION` (Catch2).
- Use matchers for readable assertions.
- CTest for test orchestration in CMake projects.
- Use temporary directories for filesystem tests — never write to fixed paths.

## Anti-Patterns

- No raw `new` / `delete`. Use smart pointers or RAII containers.
- No C-style casts: `(int)x`. Use `static_cast`, `dynamic_cast`, `reinterpret_cast` explicitly.
- No `using namespace std` in headers. Fine in `.cpp` files at narrow scope.
- No macros when `constexpr`, `inline`, or templates would work.
- No `void*` for generic programming — use templates or `std::variant`.
- No manual memory management when a container (`std::vector`, `std::array`) works.
- No `#pragma once` in public library headers if portability is a concern (use include guards).

## Build

- Compile with `-Wall -Wextra -Wpedantic` (GCC/Clang) or `/W4` (MSVC). Treat warnings as errors.
- Enable sanitizers (ASan, UBSan, TSan) in debug/CI builds.
- Use `ccache` or `sccache` for build acceleration.
