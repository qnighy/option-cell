## OptionCell: OnceCell but derivable from Option

This library provides an equivalent of [OnceCell](https://doc.rust-lang.org/stable/std/cell/struct.OnceCell.html), but it guarantees layout compatibility with `Option<T>`, providing additional transmute helpers.

## Known use-cases

- Implementing the [unification algorithm](https://en.wikipedia.org/wiki/Unification_(computer_science)) without exposing the interior mutability to the user or unnecessarily cloning the value.

## Usage

```txt
cargo add option-cell
```

```rust
use option_cell::OptionCell;

let mut options = vec![None, None];
let cells = OptionCell::from_mut_slice(&mut options);
cells[0].set(1).unwrap();
```

## Development

Check with MIRI:

```
cargo +nightly miri test
```
