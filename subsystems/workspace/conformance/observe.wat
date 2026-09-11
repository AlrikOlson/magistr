
    (component
      (import "observe" (func $obs (param "n" u32) (result u32)))
      (core func $obs_core (canon lower (func $obs)))
      (core module $m
        (import "cap" "observe" (func $obs (param i32) (result i32)))
        (func (export "run") (result i32)
          (drop (call $obs (i32.const 7)))
          (drop (call $obs (i32.const 11)))
          (call $obs (i32.const 13))))
      (core instance $i (instantiate $m (with "cap" (instance (export "observe" (func $obs_core))))))
      (func (export "run") (result u32) (canon lift (core func $i "run")))
    )
    