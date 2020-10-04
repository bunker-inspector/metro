## A library for parallel stream processing in Rust (in extremely early development. If you are here, this is not worth looking at)

[![CircleCI](https://circleci.com/gh/bunker-inspector/metro.svg?style=svg)](https://circleci.com/gh/bunker-inspector/metro)
]

Ex:
```rust
let v = vec![1, 2, 3, 4];

let out = Stream::from(v)
    .map(&|i| i + 1)
    .map(&|i| i.to_string())
    .collect();

// => ["2", "3", "4", "5"]
```
