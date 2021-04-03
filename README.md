# bb8-rusqlite

![crates.io version](https://img.shields.io/crates/v/bb8-rusqlite)
![MIT licence](https://img.shields.io/crates/l/bb8-rusqlite)
[![Documentation](https://img.shields.io/docsrs/bb8-rusqlite)](https://docs.rs/bb8-rusqlite)
[![Build state](https://img.shields.io/github/workflow/status/LawnGnome/bb8-rusqlite/Rust)](https://github.com/LawnGnome/bb8-rusqlite/actions/workflows/rust.yml)

This crate provides a connection manager that you can use with
[bb8](https://github.com/djc/bb8) to provide a pool of
[rusqlite](https://github.com/rusqlite/rusqlite) connections.

## Example

See [`examples/basic.rs`](examples/basic.rs) for a self-contained example, but
essentially, this is how it looks:

```rs
let manager = RusqliteConnectionManager::new("my-database.db");
let pool = bb8::Pool::builder().build(manager).await?;

// ...

let conn = pool.get().await?;
// conn is a rusqlite::Connection, so do whatever you'd normally do with it!
```

## Caveats

### No in-memory databases

`rusqlite` allows in-memory databases to be created with the
`Connection::open_in_memory` family of methods. In memory SQLite databases are
per-connection, which means that having a pool of these connections would result
in each connection having its own, completely separate database!

As this would be rather confusing, no wrappers are provided for those methods.
Additionally, you shouldn't use `rusqlite::OpenFlags::SQLITE_OPEN_MEMORY`,
unless you like very weird bugs in your code.

### `rusqlite::Connection` is still very synchronous

`bb8` uses tokio heavily under the hood, and so does this connection manager.
However, you'll ultimately end up handling `rusqlite::Connection` instances, and
these do not provide any sort of async API.

If you care about concurrency, you should take care to avoid starving the
runtime for long periods of time by marking tasks using `Connection`s as
blocking. You can do this either by moving the `Connection` onto another
blocking task, or by using `tokio::task::block_in_place()`. In practice, the
latter is probably what you'll want in most cases: moving `Connection` instances
is fraught, since they don't implement `Sync`.

### tokio's multi-threaded runtime is required

Due to the aforementioned `Connection` synchronicity, `bb8-rusqlite` _must_ be
used with a multi-threaded executor so `tokio::task::block_in_place()` is
available.

## Future possibilities

[`bb8-diesel`](https://github.com/overdrivenpotato/bb8-diesel) takes an
interesting alternative approach: by wrapping the synchronous Diesel API using
`block_in_place()`, it provides a safer, still synchronous API that makes it
less likely to starve the thread pool.

An interesting future piece of work here would be to do the same: wrap
`rusqlite` types in wrappers using `block_in_place()`, and then implement a
connection manager that returns those, instead of raw `rusqlite::Connection`
instances. This is harder because `Connection` is a concrete type and not a
trait, but with a _lot_ of boilerplate and some `Deref` implementations to fall
back on, it should be possible. Selfishly, I just don't need it right now!
