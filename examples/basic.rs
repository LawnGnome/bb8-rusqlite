use std::path::Path;

use bb8_rusqlite::RusqliteConnectionManager;
use rusqlite::named_params;
use tempfile::NamedTempFile;
use tokio::task;

async fn example(path: &Path) -> anyhow::Result<()> {
    let manager = RusqliteConnectionManager::new(path);
    let pool = bb8::Pool::builder().build(manager).await?;
    let conn = pool.get().await?;

    // rusqlite::Connection is synchronous, so good practice is to use
    // block_in_place() to ensure that we don't starve the tokio runtime of
    // available non-blocking threads to do work on. (Of course, in this trivial
    // example, there's no actual need for this.)
    let value = task::block_in_place(move || -> anyhow::Result<i32> {
        conn.execute("CREATE TABLE t (a INTEGER)", [])?;
        conn.execute(
            "INSERT INTO t (a) VALUES (:a)",
            named_params! {
                ":a": 42,
            },
        )?;

        Ok(conn.query_row("SELECT a FROM t", [], |row| row.get(0))?)
    })?;

    println!("we stored this value: {}", value);
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let temp = NamedTempFile::new()?;

    // Set up a runtime manually so we ensure all bb8 and rusqlite cleanup is
    // done before temp cleanup, otherwise we end up with a race condition
    // between the temporary file being removed and SQLite doing its final
    // write.
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async { example(temp.path()).await })?;
    drop(rt);

    Ok(())
}
