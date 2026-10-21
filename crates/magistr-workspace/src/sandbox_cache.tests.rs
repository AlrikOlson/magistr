use super::*;

#[test]
fn compiled_code_survives_engine_restart_and_guest_changes_invalidate_it() {
    let dir = tempfile::tempdir().expect("cache directory");
    let mut config = wasmtime::CacheConfig::new();
    config.with_directory(dir.path());
    let cache = wasmtime::Cache::new(config).expect("cache");
    let first = build_engine(Some(cache.clone())).expect("first engine");
    let guest = b"(module (func (export \"run\") (result i32) i32.const 42))";
    Module::new(&first, guest).expect("cold compile");
    assert_eq!(cache.cache_misses(), 1);
    drop(first);

    let second = build_engine(Some(cache.clone())).expect("restarted engine");
    let module = Module::new(&second, guest).expect("cached compile");
    assert_eq!(cache.cache_hits(), 1);
    let mut store = Store::new(&second, ());
    store.set_fuel(100).expect("fuel");
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).expect("instance");
    assert_eq!(
        instance
            .get_typed_func::<(), i32>(&mut store, "run")
            .expect("export")
            .call(&mut store, ())
            .expect("call"),
        42
    );
    Module::new(
        &second,
        b"(module (func (export \"run\") (result i32) i32.const 43))",
    )
    .expect("changed guest");
    assert_eq!(cache.cache_misses(), 2);
}

#[test]
fn concurrent_requests_share_compilation_but_not_execution_state() {
    let engine = engine().expect("engine");
    let guest = b"(module (global $n (mut i32) (i32.const 0))
        (func (export \"run\") (result i32)
            global.get $n i32.const 1 i32.add global.set $n global.get $n))";
    let modules: Vec<_> = std::thread::scope(|scope| {
        (0..8)
            .map(|_| scope.spawn(|| cached_wasi_module(engine, guest).expect("compile")))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|thread| thread.join().expect("join"))
            .collect()
    });
    for module in &modules {
        assert!(Module::same(&modules[0], module));
        let mut store = Store::new(engine, ());
        store.set_fuel(100).expect("fuel");
        let instance = wasmtime::Instance::new(&mut store, module, &[]).expect("instance");
        assert_eq!(
            instance
                .get_typed_func::<(), i32>(&mut store, "run")
                .expect("export")
                .call(&mut store, ())
                .expect("call"),
            1
        );
    }
    let other = build_engine(None).expect("different engine");
    let module = cached_wasi_module(&other, guest).expect("engine-specific module");
    assert!(Engine::same(module.engine(), &other));
}
