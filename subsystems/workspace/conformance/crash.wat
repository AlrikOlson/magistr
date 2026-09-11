
    (component
      (core module $m
        (func (export "crash") unreachable))
      (core instance $i (instantiate $m))
      (func (export "crash") (canon lift (core func $i "crash")))
    )
    