;; Example lang-pack guest: classify-file answers a constant
;; {"class":"source"} for every payload. Deliberately input-independent — the
;; point of the example is the conformance shape (always terminate, always
;; return JSON, never trap), which the TCK's fuzz corpus verifies.
(component
  (core module $m
    (memory (export "mem") 1)
    (data (i32.const 64) "{\"class\":\"source\"}")
    (func (export "realloc") (param i32 i32 i32 i32) (result i32)
      (i32.const 1024))
    (func (export "classify") (param i32 i32) (result i32)
      (i32.store (i32.const 16) (i32.const 64))
      (i32.store (i32.const 20) (i32.const 18))
      (i32.const 16)))
  (core instance $i (instantiate $m))
  (func (export "invoke") (param "payload" string) (result string)
    (canon lift (core func $i "classify") (memory $i "mem") (realloc (func $i "realloc"))))
)
