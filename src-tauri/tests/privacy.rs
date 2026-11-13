use quill_chat::services::database::Database;
use rusqlite::{Connection, params};
use std::fs;
use tempfile::tempdir;

#[test]
fn upgrade_recovers_committed_wal_and_encrypts_future_frames() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("quill.sqlite3");
    let legacy = Connection::open(&path).unwrap();
    legacy
        .execute_batch(include_str!("../src/services/schema.sql"))
        .unwrap();
    let marker = "history-privacy-regression-plaintext-marker";
    legacy
        .execute("INSERT INTO chats(title) VALUES(?)", [marker])
        .unwrap();
    legacy
        .execute(
            "INSERT INTO turns(chat,prompt,reply,model,status) VALUES(1,?,?, 'model','complete')",
            [marker, marker],
        )
        .unwrap();
    legacy
        .execute(
            "INSERT INTO files(turn,name,mime,data) VALUES(1,'file.txt','text/plain',?)",
            [marker],
        )
        .unwrap();
    legacy
        .execute("INSERT INTO chat_goals VALUES(1,?)", [marker])
        .unwrap();
    legacy
        .execute(
            "INSERT INTO turn_usage(turn,chat,input_tokens,output_tokens) VALUES(1,1,5,7)",
            [],
        )
        .unwrap();
    legacy
        .execute_batch(
            "INSERT INTO chats(id,title) VALUES(51,'removed');DELETE FROM chats WHERE id=51;",
        )
        .unwrap();
    legacy.execute("PRAGMA user_version=17", []).unwrap();
    // Leave a committed WAL as after a sudden exit, using a copy of the live fixture.
    let upgraded = tempdir().unwrap();
    let target = upgraded.path().join("quill.sqlite3");
    fs::copy(&path, &target).unwrap();
    fs::copy(
        path.with_extension("sqlite3-wal"),
        target.with_extension("sqlite3-wal"),
    )
    .unwrap();
    fs::copy(
        path.with_extension("sqlite3-shm"),
        target.with_extension("sqlite3-shm"),
    )
    .unwrap();
    drop(legacy);
    let db = Database::with_key(target.clone(), &[7; 32]);
    db.initialize().unwrap();
    for ext in ["sqlite3-wal", "sqlite3-shm", "sqlite3-journal"] {
        assert!(
            !target.with_extension(ext).exists(),
            "old sidecar remains: {ext}"
        );
    }
    let c = db.connect().unwrap();
    for sql in [
        "SELECT title FROM chats",
        "SELECT reply FROM turns",
        "SELECT data FROM files",
        "SELECT goal FROM chat_goals",
    ] {
        assert_eq!(
            c.query_row(sql, [], |r| r.get::<_, String>(0)).unwrap(),
            marker
        );
    }
    assert_eq!(
        c.query_row("PRAGMA user_version", [], |r| r.get::<_, i32>(0))
            .unwrap(),
        17
    );
    assert_eq!(
        c.query_row("SELECT output_tokens FROM turn_usage", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        7
    );
    c.execute("INSERT INTO chats(title) VALUES(?)", params![marker])
        .unwrap();
    assert_eq!(c.last_insert_rowid(), 52);
    for file in [&target, &target.with_extension("sqlite3-wal")] {
        let bytes = fs::read(file).unwrap();
        assert!(!bytes.windows(marker.len()).any(|w| w == marker.as_bytes()));
        assert!(!bytes.starts_with(b"SQLite format 3"));
    }
    assert!(
        Connection::open(&target)
            .unwrap()
            .query_row("SELECT count(*) FROM chats", [], |r| r.get::<_, i64>(0))
            .is_err()
    );
    assert!(
        Database::with_key(target.clone(), &[8; 32])
            .initialize()
            .is_err()
    );
    drop(c);
    db.initialize().unwrap();
}

#[test]
fn corrupt_legacy_source_is_preserved() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("quill.sqlite3");
    let bad = b"SQLite format 3\0not a valid database";
    fs::write(&path, bad).unwrap();
    assert!(
        Database::with_key(path.clone(), &[7; 32])
            .initialize()
            .is_err()
    );
    assert_eq!(fs::read(path).unwrap(), bad);
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
}

#[test]
fn orphan_sidecars_are_preserved_and_block_creation() {
    for ext in ["sqlite3-wal", "sqlite3-shm", "sqlite3-journal"] {
        let dir = tempdir().unwrap();
        let path = dir.path().join("quill.sqlite3");
        let orphan = path.with_extension(ext);
        fs::write(&orphan, b"pending recovery").unwrap();
        assert!(
            Database::with_key(path.clone(), &[7; 32])
                .initialize()
                .is_err()
        );
        assert!(!path.exists());
        assert_eq!(fs::read(orphan).unwrap(), b"pending recovery");
    }
}
#[test]
fn persistent_legacy_journal_is_retired() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("quill.sqlite3");
    let c = Connection::open(&path).unwrap();
    c.execute_batch("PRAGMA journal_mode=PERSIST; CREATE TABLE sample(value); INSERT INTO sample VALUES(1); UPDATE sample SET value=2;").unwrap();
    drop(c);
    assert!(path.with_extension("sqlite3-journal").exists());
    let db = Database::with_key(path.clone(), &[7; 32]);
    db.initialize().unwrap();
    assert!(!path.with_extension("sqlite3-journal").exists());
    assert_eq!(
        db.connect()
            .unwrap()
            .query_row("SELECT value FROM sample", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn active_wal_reader_blocks_replacement_until_retry() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("quill.sqlite3");
    let writer = Connection::open(&path).unwrap();
    writer
        .execute_batch(
            "PRAGMA journal_mode=WAL; CREATE TABLE sample(value); INSERT INTO sample VALUES(1);",
        )
        .unwrap();
    let reader = Connection::open(&path).unwrap();
    reader
        .execute_batch("BEGIN; SELECT * FROM sample;")
        .unwrap();
    writer.execute("UPDATE sample SET value=2", []).unwrap();
    let db = Database::with_key(path.clone(), &[3; 32]);
    assert!(db.initialize().is_err());
    assert!(fs::read(&path).unwrap().starts_with(b"SQLite format 3"));
    assert!(path.with_extension("sqlite3-wal").exists());
    drop(reader);
    drop(writer);
    db.initialize().unwrap();
    assert_eq!(
        db.connect()
            .unwrap()
            .query_row("SELECT value FROM sample", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
}

#[test]
fn hot_rollback_journal_is_recovered_before_encryption() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("quill.sqlite3");
    let c = Connection::open(&path).unwrap();
    c.execute_batch(
        "PRAGMA journal_mode=DELETE; PRAGMA synchronous=FULL; PRAGMA cache_size=4;
        CREATE TABLE sample(id INTEGER PRIMARY KEY, value BLOB);
        WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<120)
        INSERT INTO sample SELECT i,zeroblob(4096) FROM n;
        BEGIN IMMEDIATE; UPDATE sample SET value=randomblob(4096);",
    )
    .unwrap();
    let recovered = tempdir().unwrap();
    let target = recovered.path().join("quill.sqlite3");
    fs::copy(&path, &target).unwrap();
    fs::copy(
        path.with_extension("sqlite3-journal"),
        target.with_extension("sqlite3-journal"),
    )
    .unwrap();
    drop(c);
    let db = Database::with_key(target.clone(), &[4; 32]);
    db.initialize().unwrap();
    assert_eq!(
        db.connect()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM sample WHERE value=zeroblob(4096)",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        120
    );
    assert!(!target.with_extension("sqlite3-journal").exists());
}
