
    (component
      (core module $m
        (memory 1)
        (func (export "hog") (result i32)
          (loop $l
            (drop (memory.grow (i32.const 1)))
            (br $l))
          (i32.const 0))
      )
      (core instance $i (instantiate $m))
      (func (export "hog") (result u32) (canon lift (core func $i "hog")))
    )
    