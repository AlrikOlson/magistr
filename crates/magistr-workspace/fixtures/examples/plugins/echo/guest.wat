;; The smallest conforming tool guest: invoke(payload) returns the payload
;; verbatim. The payload is the host's canonical {v, tool, args} JSON, so the
;; result is always valid JSON — which is all the tool contract asks.
(component
  (core module $m
    (memory (export "mem") 1)
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (i32.const 1024))
    (func (export "echo") (param i32 i32) (result i32)
      (i32.store (i32.const 16) (local.get 0))
      (i32.store (i32.const 20) (local.get 1))
      (i32.const 16)))
  (core instance $i (instantiate $m))
  (func (export "invoke") (param "payload" string) (result string)
    (canon lift (core func $i "echo") (memory (core memory $i "mem")) (realloc (core func $i "realloc"))))
)
