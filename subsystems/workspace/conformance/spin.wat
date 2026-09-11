
    (component
      (core module $m
        (func (export "spin") (loop $l br $l)))
      (core instance $i (instantiate $m))
      (func (export "spin") (canon lift (core func $i "spin")))
    )
    