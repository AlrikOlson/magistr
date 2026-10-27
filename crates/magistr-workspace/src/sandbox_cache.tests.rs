use super::*;

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
    let other = build_engine().expect("different engine");
    let module = cached_wasi_module(&other, guest).expect("engine-specific module");
    assert!(Engine::same(module.engine(), &other));
}
