//! Kit-owned abuse fixtures: the enforcing sandbox must kill both, typed. A
//! sandbox that lets either survive is not the sandbox the report claims.

/// An invoke export that never returns — the fuel meter is the only thing
/// that ends it. Canonical-ABI `(string) -> string`.
pub(super) const SPIN_WAT: &str = r#"
(component
  (core module $m
    (memory (export "mem") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (i32.const 1024))
    (func (export "spin") (param i32 i32) (result i32)
      (loop $l (br $l))
      (i32.const 16)))
  (core instance $i (instantiate $m))
  (func (export "invoke") (param "payload" string) (result string)
    (canon lift (core func $i "spin") (memory (core memory $i "mem")) (realloc (core func $i "realloc"))))
)
"#;

/// Grows memory until the resource limiter kills it (trap_on_grow_failure
/// makes the breach deterministic and typed).
pub(super) const GROW_WAT: &str = r#"
(component
  (core module $m
    (memory (export "mem") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (i32.const 1024))
    (func (export "grow") (param i32 i32) (result i32)
      (loop $l
        (drop (memory.grow (i32.const 1)))
        (br $l))
      (i32.const 16)))
  (core instance $i (instantiate $m))
  (func (export "invoke") (param "payload" string) (result string)
    (canon lift (core func $i "grow") (memory (core memory $i "mem")) (realloc (core func $i "realloc"))))
)
"#;
