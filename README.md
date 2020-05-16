# parking

[![Build](https://github.com/stjepang/parking/workflows/Build%20and%20test/badge.svg)](
https://github.com/stjepang/parking/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](
https://github.com/stjepang/parking)
[![Cargo](https://img.shields.io/crates/v/parking.svg)](
https://crates.io/crates/parking)
[![Documentation](https://docs.rs/parking/badge.svg)](
https://docs.rs/parking)
[![Chat](https://img.shields.io/discord/701824908866617385.svg?logo=discord)](
https://discord.gg/x6m5Vvt)

Thread parking and unparking.

This is a copy of the
[`park()`]()https://doc.rust-lang.org/std/thread/fn.park.html/[`unpark()`](https://doc.rust-lang.org/std/thread/struct.Thread.html#method.unpark) mechanism from
the standard library.

## What is parking

Conceptually, each `Parker` has a token which is initially not present:

* The `Parker::park()` method blocks the current thread unless or until the token is
  available, at which point it automatically consumes the token. It may also return
  *spuriously*, without consuming the token.

* The `Parker::park_timeout()` method works the same as `Parker::park()`, but blocks until
  a timeout is reached.

* The `Parker::park_deadline()` method works the same as `Parker::park()`, but blocks until
  a deadline is reached.

* The `Parker::unpark()` and `Unparker::unpark()` methods atomically make the token
  available if it wasn't already. Because the token is initially absent, `Unparker::unpark()`
  followed by `Parker::park()` will result in the second call returning immediately.

## Analogy with channels

Another way of thinking about `Parker` is as a bounded
[channel](https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html) with capacity of 1.

Then, `Parker::park()` is equivalent to blocking on a
[receive](https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html) operation, and `Unparker::unpark()` is
equivalent to a non-blocking [send](https://doc.rust-lang.org/std/sync/mpsc/struct.SyncSender.html#method.try_send) operation.

## Examples

```rust
use std::thread;
use std::time::Duration;
use parking::Parker;

let p = Parker::new();
let u = p.unparker();

// Make the token available.
u.unpark();
// Wakes up immediately and consumes the token.
p.park();

thread::spawn(move || {
    thread::sleep(Duration::from_millis(500));
    u.unpark();
});

// Wakes up when `u.unpark()` provides the token, but may also wake up
// spuriously before that without consuming the token.
p.park();
```

## License

Licensed under either of

 * Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

#### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
