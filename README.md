# crdts_derive

[![crates.io](https://img.shields.io/crates/v/crdts_derive.svg)](https://crates.io/crates/crdts_derive)

## TODO

- [x] CmRDT
- [ ] CvRDT

## Usage

### Add the dependency

Add the `crdts` and `crdts_derive` dependency to `Cargo.toml`:

```toml
[dependencies]
crdts = "7.3"
crdts_derive = "7.3"
```

### Custom CRDT struct

```rust
use crdts::{CmRDT, GCounter, Map, Orswot};
use crdts_derive::crdt;

#[crdt(u64)]
#[derive(Default, Debug)]
pub struct Data {
    a: Orswot<String, String>,
    b: Map<u64, Orswot<Vec<u8>, u64>, u64>,
    c: Orswot<Vec<u8>, u64>,
    d: GCounter<u64>,
}

#[test]
fn test() {
    let mut controller = Data::default();
    let actor = 1;
    let counter = 1;
    let dot = crdts::Dot::new(actor, counter);
    let op1 = controller.a.add(
        format!("{actor}-{counter}"),
        controller.a.read().derive_add_ctx(actor.to_string()),
    );

    let add_ctx = controller.b.read_ctx().derive_add_ctx(actor);
    let op2 = controller
        .b
        .update(actor, add_ctx, |v, a| v.add(vec![actor as u8; 20], a));

    let op3 = controller.c.add(
        vec![actor as u8; 20],
        controller.c.read().derive_add_ctx(actor),
    );

    let op4 = controller.d.inc(actor);
    controller.apply(DataCrdtOp {
        dot,
        a_op: Some(op1),
        b_op: Some(op2),
        c_op: Some(op3),
        d_op: Some(op4),
    });
    println!("{:#?}", controller);
}

```

## Compatible crdts versions

Compatibility of `crdts_derive` versions:

| `crdts_derive` | `crdts` |
| :--           | :--    |
| `7.3`         | `7.3`  |