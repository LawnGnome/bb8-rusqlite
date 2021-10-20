//! rusqlite support for the `bb8` connection pool. Note that in-memory
//! databases aren't supported, since they are always per-connection, and
//! therefore don't make sense in a pool environment.
#![deny(missing_docs, missing_debug_implementations)]

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use bb8::ManageConnection;
use rusqlite::{Connection, OpenFlags};

#[cfg(test)]
mod tests;

/// A `bb8::ManageConnection` implementation for `rusqlite::Connection`
/// instances.
#[derive(Clone, Debug)]
pub struct RusqliteConnectionManager(Arc<ConnectionOptions>);

#[derive(Debug)]
struct ConnectionOptions {
    mode: OpenMode,
    path: PathBuf,
}

#[derive(Debug)]
enum OpenMode {
    Plain,
    WithFlags {
        flags: rusqlite::OpenFlags,
    },
    WithFlagsAndVFS {
        flags: rusqlite::OpenFlags,
        vfs: String,
    },
}

/// Error wraps errors from both rusqlite and tokio.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// A rusqlite error.
    #[error("rusqlite error")]
    Rusqlite(#[from] rusqlite::Error),

    /// A tokio join handle error.
    #[error("tokio join error")]
    TokioJoin(#[from] tokio::task::JoinError),
}

impl RusqliteConnectionManager {
    /// Analogous to `rusqlite::Connection::open()`.
    pub fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        Self(Arc::new(ConnectionOptions {
            mode: OpenMode::Plain,
            path: path.as_ref().into(),
        }))
    }

    /// Analogous to `rusqlite::Connection::open_with_flags()`.
    pub fn new_with_flags<P>(path: P, flags: OpenFlags) -> Self
    where
        P: AsRef<Path>,
    {
        Self(Arc::new(ConnectionOptions {
            mode: OpenMode::WithFlags { flags },
            path: path.as_ref().into(),
        }))
    }

    /// Analogous to `rusqlite::Connection::open_with_flags_and_vfs()`.
    pub fn new_with_flags_and_vfs<P>(path: P, flags: OpenFlags, vfs: &str) -> Self
    where
        P: AsRef<Path>,
    {
        Self(Arc::new(ConnectionOptions {
            mode: OpenMode::WithFlagsAndVFS {
                flags,
                vfs: vfs.into(),
            },
            path: path.as_ref().into(),
        }))
    }
}

#[async_trait]
impl ManageConnection for RusqliteConnectionManager {
    type Connection = Connection;
    type Error = Error;

    async fn connect(&self) -> Result<Self::Connection, Self::Error> {
        let options = self.0.clone();

        // Technically, we don't need to use spawn_blocking() here, but doing so
        // means we won't inadvertently block this task for any length of time,
        // since rusqlite is inherently synchronous.
        Ok(tokio::task::spawn_blocking(move || match &options.mode {
            OpenMode::Plain => rusqlite::Connection::open(&options.path),
            OpenMode::WithFlags { flags } => {
                rusqlite::Connection::open_with_flags(&options.path, *flags)
            }
            OpenMode::WithFlagsAndVFS { flags, vfs } => {
                rusqlite::Connection::open_with_flags_and_vfs(&options.path, *flags, &vfs)
            }
        })
        .await??)
    }

    async fn is_valid(
        &self,
        conn: &mut bb8::PooledConnection<'_, Self>,
    ) -> Result<(), Self::Error> {
        // Matching bb8-postgres, we'll try to run a trivial query here. Using
        // block_in_place() gives better behavior if the SQLite call blocks for
        // some reason, but means that we depend on the tokio multi-threaded
        // runtime being active. (We can't use spawn_blocking() here because
        // Connection isn't Sync.)
        tokio::task::block_in_place(|| conn.execute("SELECT 1", []))?;
        Ok(())
    }

    fn has_broken(&self, _conn: &mut Self::Connection) -> bool {
        // There's no real concept of a "broken" connection in SQLite: if the
        // handle is still open, then we're good. (And we know the handle is
        // still open, because Connection::close() consumes the Connection, in
        // which case we're definitely not here.)
        false
    }
}
