
    (module
      (import "wasi_snapshot_preview1" "fd_prestat_get"
        (func $prestat (param i32 i32) (result i32)))
      (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
      (memory (export "memory") 1)
      (func (export "_start")
        (call $exit (call $prestat (i32.const 3) (i32.const 0)))
        unreachable)
    )
    