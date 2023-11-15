# `ogrim`: macro for building XML from inline XML

[<img alt="CI status of main" src="https://img.shields.io/github/actions/workflow/status/elan-ev/ogrim/ci.yml?branch=main&label=CI&logo=github&logoColor=white&style=for-the-badge" height="23">](https://github.com/elan-ev/ogrim/actions/workflows/ci.yml)
[<img alt="Crates.io Version" src="https://img.shields.io/crates/v/ogrim?logo=rust&style=for-the-badge" height="23">](https://crates.io/crates/ogrim)
[<img alt="docs.rs" src="https://img.shields.io/crates/v/ogrim?color=blue&label=docs&style=for-the-badge" height="23">](https://docs.rs/ogrim)

XML builder macro letting you write XML inside Rust code (similar to `serde_json::json!`).
Features:

- Value interpolation (with escaping of course)
    - Interpolate lists or optional attributes with `<foo {..iter}>`
- Auto close tags for convenience (e.g. `<foo>"body"</>`)
- Minimal memory allocations (only the `String` being built allocates)
- Choice between minimized and pretty XML

```rust
use ogrim::xml;

let cat_name = "Tony";
let doc = xml!(
    <?xml version="1.0" ?>
    <zoo name="Lorem Ipsum" openingYear={2000 + 13}>
        <cat>{cat_name}</>
        <dog>"Barbara"</>
    </>
);

println!("{}", doc.as_str()); // Print XML
```

See [**the documentation**](https://docs.rs/ogrim) for more information and examples.


---

## License

Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this project by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
