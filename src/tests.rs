use futures::future::join_all;
use tempfile::tempdir;

use super::*;

#[derive(Debug)]
struct TempDir {
    dir: tempfile::TempDir,
}

impl TempDir {
    fn new() -> anyhow::Result<Self> {
        Ok(Self { dir: tempdir()? })
    }

    fn file(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn connect_error() -> Result<(), anyhow::Error> {
    let temp = TempDir::new()?;

    // Set up a connection manager with a read only flag for a non-existent
    // database, which should result in a connection error.
    let manager = RusqliteConnectionManager::new_with_flags(
        &temp.file("connect_error.db"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    );
    let pool = bb8::Pool::builder().build(manager).await?;

    // Ensure that the connection error here propagates up from connect().
    pool.dedicated_connection().await.expect_err("this connection must fail, since the database doesn't exist and the read only flag was provided");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn flags() -> Result<(), anyhow::Error> {
    let temp = TempDir::new()?;

    // First, we'll create a database without any flags so that we have
    // something to connect to.
    let path = temp.file("flags.db");
    let conn = Connection::open(&path)?;
    conn.execute("CREATE TABLE t (a INTEGER)", [])?;
    drop(conn);

    // Now we can set up a connection manager with the read only flag set that
    // shouldn't result in connection errors.
    let manager =
        RusqliteConnectionManager::new_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY);
    let pool = bb8::Pool::builder().build(manager).await?;

    // Grab a connection, and then do something to generate an error, which will
    // prove that the flags were passed down correctly.
    let conn = pool.get().await?;
    conn.execute("INSERT INTO t (a) VALUES (?)", [42])
        .expect_err("writing to a read-only database must fail");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn plain() -> Result<(), anyhow::Error> {
    let temp = TempDir::new()?;
    let manager = RusqliteConnectionManager::new(&temp.file("plain.db"));
    let pool = bb8::Pool::builder().build(manager).await?;

    // Ensure we get a valid connection when we ask for one.
    let first = pool.get().await?;
    first.execute("CREATE TABLE t (a INTEGER)", [])?;

    // Now let's ensure concurrent access is sensible by inserting on another
    // connection.
    let second = pool.get().await?;
    second.execute("INSERT INTO t (a) VALUES (?)", [42])?;

    // Now we'll spawn a bunch of tasks to query, all of which should get the
    // right value.
    join_all((0..8).map(|_| {
        let local_pool = pool.clone();
        tokio::spawn(async move {
            let conn = local_pool.get().await.unwrap();
            let v: i32 = conn
                .query_row("SELECT a FROM t", [], |row| row.get(0))
                .unwrap();
            assert_eq!(v, 42);
        })
    }))
    .await;

    Ok(())
}
