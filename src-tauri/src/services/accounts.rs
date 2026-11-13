use crate::{
    error::{Result, invalid},
    services::{
        database::Database,
        oauth::{self, PlanModel, Session},
    },
};
use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

#[derive(Serialize)]
pub struct Profile {
    pub id: String,
    pub label: String,
    pub connected: bool,
    pub permitted: bool,
}
#[derive(Serialize)]
pub struct Capacity {
    pub seats: usize,
    pub tier: String,
    pub recent: usize,
}
pub fn parallel_capacity(c: &Connection) -> Result<Capacity> {
    let (active,tier): (Option<String>,Option<String>) = c.query_row(
        "SELECT h.active,p.tier FROM oauth_host h LEFT JOIN account_plans p ON h.active=p.account
         LEFT JOIN oauth_accounts a ON a.id=h.active WHERE h.singleton=1 AND (h.active IS NULL OR a.permitted=1 AND a.session IS NOT NULL)",
        [], |r| Ok((r.get(0)?, r.get(1)?))).optional()?.unwrap_or_default();
    let recent = c.query_row(
        "SELECT count(*) FROM request_times WHERE account IS ? AND started>unixepoch()-3600",
        [&active],
        |r| r.get::<_, usize>(0),
    )?;
    let tier = tier.unwrap_or_else(|| "unknown".into()).to_lowercase();
    let seats = match tier.as_str() {
        "pro" => {
            if recent < 20 {
                5
            } else {
                4
            }
        }
        "plus" | "team" | "business" | "enterprise" | "edu" | "go" => 4,
        _ => 3,
    };
    Ok(Capacity {
        seats,
        tier,
        recent,
    })
}
#[derive(Serialize)]
pub struct Status {
    pub parallel: Capacity,
    pub active: Option<String>,
    pub accounts: Vec<Profile>,
    pub models: Vec<PlanModel>,
    pub model: Option<String>,
    pub welcome: bool,
}
pub fn status(c: &Connection) -> Result<Status> {
    let active: Option<String> = c
        .query_row("SELECT active FROM oauth_host WHERE singleton=1", [], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    let mut q = c.prepare(
        "SELECT id,label,session IS NOT NULL,permitted FROM oauth_accounts ORDER BY rowid",
    )?;
    let accounts = q
        .query_map([], |r| {
            Ok(Profile {
                id: r.get(0)?,
                label: r.get(1)?,
                connected: r.get(2)?,
                permitted: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let details: Option<(String, Option<String>, bool)> = c
        .query_row(
            "SELECT models,model,permitted AND NOT welcomed FROM oauth_accounts WHERE id=?",
            [&active],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (catalog, model, welcome) = details.unwrap_or_else(|| ("[]".into(), None, false));
    Ok(Status {
        parallel: parallel_capacity(c)?,
        active,
        accounts,
        models: serde_json::from_str(&catalog)?,
        model,
        welcome,
    })
}
pub fn host(c: &Connection) -> Result<String> {
    c.execute(
        "INSERT OR IGNORE INTO oauth_host(singleton,host) VALUES(1,?)",
        [format!("urn:uuid:{}", uuid::Uuid::new_v4())],
    )?;
    Ok(
        c.query_row("SELECT host FROM oauth_host WHERE singleton=1", [], |r| {
            r.get(0)
        })?,
    )
}
// Credential Manager has a per-entry size limit. Wrap arbitrarily long token
// sets in SQLite, with just the wrapping key in the native credential store.
fn vault(create: bool) -> Result<LessSafeKey> {
    let entry = keyring::Entry::new("studio.quill.desktopchat", "oauth-wrapping-key-v1")?;
    let key = match entry.get_secret() {
        Ok(key) => key,
        Err(keyring::Error::NoEntry) if create => {
            let bytes = oauth::random::<32>()?;
            entry.set_secret(&bytes)?;
            bytes.to_vec()
        }
        Err(e) => return Err(e.into()),
    };
    Ok(LessSafeKey::new(
        UnboundKey::new(&AES_256_GCM, &key)
            .map_err(|_| invalid("Invalid key in the system keychain."))?,
    ))
}
fn seal(id: &str, session: &Session) -> Result<Vec<u8>> {
    let nonce = oauth::random::<12>()?;
    let mut bytes = serde_json::to_vec(session)?;
    vault(true)?
        .seal_in_place_append_tag(
            Nonce::assume_unique_for_key(nonce),
            Aad::from(id),
            &mut bytes,
        )
        .map_err(|_| invalid("Could not protect ChatGPT credentials."))?;
    let mut blob = nonce.to_vec();
    blob.extend(bytes);
    Ok(blob)
}
fn unseal(id: &str, blob: &mut [u8]) -> Result<Session> {
    let (nonce, cipher) = blob
        .split_at_mut_checked(12)
        .ok_or_else(|| invalid("Damaged ChatGPT session."))?;
    let plain = vault(false)?
        .open_in_place(
            Nonce::try_assume_unique_for_key(nonce)
                .map_err(|_| invalid("Invalid session nonce."))?,
            Aad::from(id),
            cipher,
        )
        .map_err(|_| invalid("Could not unlock this ChatGPT session. Sign in again."))?;
    Ok(serde_json::from_slice(plain)?)
}
pub struct Saved {
    pub sub: String,
    pub session: Option<Session>,
}
pub async fn saved(db: &Database, id: &str) -> Result<Saved> {
    let id = id.to_owned();
    db.run(move |c| {
        let (sub, blob): (String, Option<Vec<u8>>) = c.query_row(
            "SELECT subject,session FROM oauth_accounts WHERE id=?",
            [&id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(Saved {
            sub,
            session: blob.map(|mut b| unseal(&id, &mut b)).transpose()?,
        })
    })
    .await
}
pub async fn save_login(
    db: &Database,
    id: String,
    sub: String,
    label: String,
    tier: Option<String>,
    session: Session,
) -> Result<()> {
    db.run(move|c|{
        let blob=seal(&id,&session)?;
        let tx=c.transaction()?;
        let changed=tx.execute("INSERT INTO oauth_accounts(id,subject,label,session,permitted) VALUES(?,?,?,?,?)
            ON CONFLICT(id) DO UPDATE SET label=excluded.label,session=excluded.session,permitted=excluded.permitted,models='[]'
            WHERE subject=excluded.subject",params![id,sub,label,blob,session.permitted()])?;
        if changed!=1{return Err(invalid("This registration belongs to a different account."))}
        tx.execute("INSERT OR REPLACE INTO account_plans(account,tier) VALUES(?,?)",params![id,tier])?;
        tx.execute("UPDATE oauth_host SET active=? WHERE singleton=1",[id])?;
        tx.commit()?;Ok(())
    }).await
}
pub async fn access(db: &Database, http: &reqwest::Client, id: &str) -> Result<String> {
    let saved = saved(db, id).await?;
    let old = saved
        .session
        .ok_or_else(|| invalid("Sign in to ChatGPT in Settings."))?;
    if !old.permitted() {
        return Err(invalid(
            "ChatGPT plan use is disabled. Authorize it in Settings.",
        ));
    }
    if old.expires > oauth::now() + 60 {
        return Ok(old.access);
    }
    let refresh = old
        .refresh
        .as_deref()
        .ok_or_else(|| invalid("Your ChatGPT session expired. Sign in again."))?;
    let tokens = oauth::exchange(
        http,
        &[
            ("grant_type", "refresh_token"),
            ("client_id", id),
            ("refresh_token", refresh),
            ("resource", oauth::RESOURCE),
        ],
    )
    .await?;
    let mut renewed_tier = None;
    if let Some(jwt) = &tokens.id_token {
        let identity = oauth::identity(http, jwt, id, None).await?;
        renewed_tier = Some(identity.plan.chatgpt_plan_type);
        if identity.sub != saved.sub {
            return Err(invalid(
                "The renewed ChatGPT identity changed. Sign in again.",
            ));
        }
    }
    let fresh = tokens.into_session(Some(old))?;
    let id = id.to_owned();
    db.run(move |c| {
        let blob = seal(&id, &fresh)?;
        let tx = c.transaction()?;
        if let Some(tier) = renewed_tier {
            tx.execute(
                "INSERT OR REPLACE INTO account_plans(account,tier) VALUES(?,?)",
                params![id, tier],
            )?;
        }
        tx.execute(
            "UPDATE oauth_accounts SET session=?,permitted=? WHERE id=?",
            params![blob, fresh.permitted(), id],
        )?;
        tx.commit()?;
        if !fresh.permitted() {
            return Err(invalid(
                "ChatGPT plan permission was removed. Sign in again.",
            ));
        }
        Ok(fresh.access)
    })
    .await
}
pub async fn catalog(db: &Database, http: &reqwest::Client, id: &str) -> Result<()> {
    let token = access(db, http, id).await?;
    let models = oauth::models(http, &token).await?;
    let id = id.to_owned();
    db.run(move |c| {
        let prev: Option<String> =
            c.query_row("SELECT model FROM oauth_accounts WHERE id=?", [&id], |r| {
                r.get(0)
            })?;
        let model = prev
            .filter(|m| models.iter().any(|v| v.slug == *m))
            .or_else(|| models.first().map(|m| m.slug.clone()));
        c.execute(
            "UPDATE oauth_accounts SET models=?,model=? WHERE id=?",
            params![serde_json::to_string(&models)?, model, id],
        )?;
        Ok(())
    })
    .await
}
