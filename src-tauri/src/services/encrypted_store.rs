use crate::error::{Result, invalid};
use rusqlite::{Connection, OpenFlags, params};
use std::{
    fmt::Write as _,
    fs::{File, OpenOptions},
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct Database {
    path: PathBuf,
    key: Arc<Zeroizing<String>>,
}

const SQLITE_HEADER: &[u8] = b"SQLite format 3\0";

fn plaintext(path: &Path) -> Result<bool> {
    let mut file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e.into()),
    };
    let mut header = [0; 16];
    match file.read_exact(&mut header) {
        Ok(()) => Ok(header == SQLITE_HEADER),
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e.into()),
    }
}

// Never unlink an unexamined WAL or hot rollback journal. SQLite owns recovery.
fn ensure_no_sidecars(path: &Path) -> Result<()> {
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut sidecar = path.as_os_str().to_os_string();
        sidecar.push(suffix);
        if Path::new(&sidecar).try_exists()? {
            return Err(invalid(format!(
                "The SQLite {suffix} sidecar is still present. Close other database users and retry. The original files have been kept."
            )));
        }
    }
    Ok(())
}

impl Database {
    pub fn from_keychain(path: PathBuf) -> Result<Self> {
        let entry = keyring::Entry::new("studio.quill.desktopchat", "history-key-v1")?;
        let bytes = match entry.get_secret() {
            Ok(bytes) => Zeroizing::new(bytes),
            Err(keyring::Error::NoEntry) => {
                if path.try_exists()? && path.metadata()?.len() > 0 && !plaintext(&path)? {
                    return Err(invalid(
                        "The key for this encrypted history is missing from your OS credential store. Restore the original key to reopen it.",
                    ));
                }
                let secret = Zeroizing::new(super::oauth::random::<32>()?);
                entry.set_secret(secret.as_slice())?;
                let stored = Zeroizing::new(entry.get_secret()?);
                if stored.as_slice() != secret.as_slice() {
                    return Err(invalid(
                        "The history key could not be verified in the OS credential store.",
                    ));
                }
                stored
            }
            Err(e) => return Err(e.into()),
        };
        let key = bytes
            .as_slice()
            .try_into()
            .map_err(|_| invalid("Invalid history key in the OS credential store."))?;
        Ok(Self::with_key(path, key))
    }

    pub fn with_key(path: PathBuf, bytes: &[u8; 32]) -> Self {
        let mut raw = Zeroizing::new(String::with_capacity(67));
        raw.push_str("x'");
        for byte in bytes {
            write!(&mut *raw, "{byte:02x}").expect("write to String");
        }
        raw.push('\'');
        Self {
            path,
            key: Arc::new(raw),
        }
    }

    fn open_keyed(&self, path: &Path, flags: OpenFlags) -> Result<Connection> {
        let db = Connection::open_with_flags(path, flags)?;
        // SQLite ignores unknown PRAGMAs. Refuse a build without the SQLCipher codec.
        let _: String = db
            .query_row("PRAGMA cipher_version", [], |r| r.get(0))
            .map_err(|_| invalid("Database encryption support is unavailable."))?;
        db.pragma_update(None, "key", self.key.as_str())
            .map_err(|_| invalid("Could not apply the history encryption key."))?;
        db.query_row("SELECT count(*) FROM sqlite_schema",[],|r|r.get::<_,i64>(0))
            .map_err(|_|invalid("Could not unlock the chat database. Check the OS credential store and the database file."))?;
        db.busy_timeout(Duration::from_secs(5))?;
        db.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA secure_delete=ON; PRAGMA temp_store=MEMORY;",
        )?;
        Ok(db)
    }

    pub fn connect(&self) -> Result<Connection> {
        self.open_keyed(&self.path, OpenFlags::SQLITE_OPEN_READ_WRITE)
    }

    pub fn initialize(&self) -> Result<crate::model::Preferences> {
        if !self.path.try_exists()? || self.path.metadata()?.len() == 0 {
            ensure_no_sidecars(&self.path)?;
        }
        if plaintext(&self.path)? {
            self.migrate()?;
        }
        let db = self.open_keyed(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )?;
        db.execute_batch(include_str!("schema.sql"))?;
        super::database::read_preferences(&db)
    }

    fn migrate(&self) -> Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| invalid("Missing database directory."))?;
        // Keeping the temporary database beside the original allows an atomic replacement.
        let staged = tempfile::Builder::new()
            .prefix(".quill-encrypt-")
            .tempfile_in(parent)?
            .into_temp_path();
        let mut source =
            Connection::open_with_flags(&self.path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        source.busy_timeout(Duration::from_secs(5))?;
        source.execute_batch("PRAGMA temp_store=MEMORY;")?;
        // Access the schema first so SQLite recovers any hot rollback journal.
        source.query_row("SELECT count(*) FROM sqlite_schema", [], |r| {
            r.get::<_, i64>(0)
        })?;
        let journal: String = source.query_row("PRAGMA journal_mode", [], |r| r.get(0))?;
        if journal.eq_ignore_ascii_case("wal") {
            let busy: i64 =
                source.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| r.get(0))?;
            if busy != 0 {
                return Err(invalid(
                    "A database reader is still using the old WAL. Close it and retry.",
                ));
            }
        }
        // DELETE mode retires old sidecars; an exclusive lock keeps the export stable.

        let mode: String = source.query_row("PRAGMA journal_mode=DELETE", [], |r| r.get(0))?;
        if mode != "delete" {
            return Err(invalid(
                "Close other Quill windows before upgrading history.",
            ));
        }
        source.execute_batch("PRAGMA locking_mode=EXCLUSIVE;")?;
        source.execute(
            "ATTACH DATABASE ? AS encrypted KEY ?",
            params![
                staged
                    .to_str()
                    .ok_or_else(|| invalid("Invalid database path."))?,
                self.key.as_str()
            ],
        )?;
        let tx = source.transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)?;
        tx.query_row("SELECT sqlcipher_export('encrypted')", [], |_| Ok(()))?;
        // Preserve high-water marks even when the highest-numbered chats have been deleted.
        let sequences: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM main.sqlite_schema WHERE name='sqlite_sequence')",
            [],
            |r| r.get(0),
        )?;
        if sequences {
            tx.execute_batch("DELETE FROM encrypted.sqlite_sequence; INSERT INTO encrypted.sqlite_sequence(name,seq) SELECT name,seq FROM main.sqlite_sequence;")?;
        }
        let version: i64 = tx.query_row("PRAGMA main.user_version", [], |r| r.get(0))?;
        // A source write in DELETE mode retires an old persistent rollback journal.
        tx.pragma_update(Some("main"), "user_version", version)?;
        tx.pragma_update(Some("encrypted"), "user_version", version)?;
        tx.commit()?;
        source.execute_batch("DETACH DATABASE encrypted")?;
        let check = self.open_keyed(&staged, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let result: String = check.query_row("PRAGMA integrity_check", [], |r| r.get(0))?;
        if result != "ok" {
            return Err(invalid(
                "The encrypted copy failed validation. Your existing database was kept.",
            ));
        }
        drop(check);
        source.close().map_err(|(_, e)| e)?;
        ensure_no_sidecars(&self.path)?;
        ensure_no_sidecars(&staged)?;
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(&staged)?
            .sync_all()?;
        staged.persist(&self.path).map_err(|e| e.error)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    }

    pub async fn run<T, F>(&self, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
    {
        let db = self.clone();
        tokio::task::spawn_blocking(move || work(&mut db.connect()?)).await?
    }
}
