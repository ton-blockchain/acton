<img width="150px" src="docs/img/logo.png">

# Acton

Blazingly fast shit.

## Building

```
cargo build --bin acton
```

In release mode:

```
cargo build --bin acton --profile release
```

## Run

```
target/debug/acton test foo_test.tolk
# or target/release/acton test foo_test.tolk
```

## Testing

### Annotations

#### skip — Skip the test

```
@custom("skip")
get fun test_something() {}
```

#### fail_with — Requires termination with the given exit code

```
@custom("fail_with", 10)
get fun test_something() {}
```

> Note: number literals only for now!
