use crate::{
    error::{Result, invalid},
    services::{
        accounts::{self, Status},
        credentials,
        database::Database,
        oauth,
    },
};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tokio::{
    net::TcpListener,
    sync::Mutex,
    time::{Duration, timeout},
};
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct Auth {
    operations: Mutex<()>,
    pending: Mutex<Option<CancellationToken>>,
}
#[tauri::command]
pub async fn auth_status(db: State<'_, Database>) -> Result<Status> {
    db.run(|c| accounts::status(c)).await
}

#[tauri::command]
pub async fn cancel_login(auth: State<'_, Auth>) -> Result<()> {
    if let Some(cancel) = auth.pending.lock().await.as_ref() {
        cancel.cancel()
    }
    Ok(())
}

#[tauri::command]
pub async fn chatgpt_login(
    app: AppHandle,
    db: State<'_, Database>,
    auth: State<'_, Auth>,
    id: Option<String>,
) -> Result<()> {
    let _guard = auth
        .operations
        .try_lock()
        .map_err(|_| invalid("Another account operation is in progress."))?;
    let token = CancellationToken::new();
    *auth.pending.lock().await = Some(token.clone());
    let result = login(&app, &db, id, token).await;
    *auth.pending.lock().await = None;
    result
}
async fn login(
    app: &AppHandle,
    db: &Database,
    id: Option<String>,
    token: CancellationToken,
) -> Result<()> {
    let host = db.run(|c| accounts::host(c)).await?;
    let prev = match &id {
        Some(id) => Some(accounts::saved(db, id).await?),
        None => None,
    };
    let session = prev.as_ref().and_then(|a| a.session.as_ref());
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .map_err(|_| invalid("Could not open the local sign-in callback."))?;
    let attempt = oauth::Attempt::new(
        listener
            .local_addr()
            .map_err(|_| invalid("Missing callback address."))?
            .port(),
    )?;
    let url = attempt.authorize(
        &host,
        id.as_deref(),
        session.map(|s| s.id_token.as_str()),
        session.is_some_and(|s| !s.permitted()),
    )?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|_| invalid("Could not open your default browser."))?;
    let http = oauth::client()?;
    let (issued, identity, session) = timeout(
        Duration::from_secs(300),
        token.run_until_cancelled(async {
            let (code, issued) = attempt.listen(&listener, id.as_deref()).await?;
            let fields = [
                ("grant_type", "authorization_code"),
                ("code", &code),
                ("client_id", &issued),
                ("code_verifier", &attempt.verifier),
                ("redirect_uri", &attempt.redirect),
                ("resource", oauth::RESOURCE),
            ];
            let tokens = oauth::exchange(&http, &fields).await?;
            let jwt = tokens
                .id_token
                .as_deref()
                .ok_or_else(|| invalid("OpenAI did not return an identity token."))?;
            let identity = oauth::identity(&http, jwt, &issued, Some(&attempt.nonce)).await?;
            if prev.as_ref().is_some_and(|s| s.sub != identity.sub) {
                return Err(invalid(
                    "The signed-in account differs from the selected registration.",
                ));
            }
            Ok((issued, identity, tokens.into_session(None)?))
        }),
    )
    .await
    .map_err(|_| invalid("ChatGPT sign-in timed out."))?
    .ok_or_else(|| invalid("ChatGPT sign-in cancelled."))??;
    let permitted = session.permitted();
    let label = identity.email.unwrap_or_else(|| "ChatGPT account".into());
    accounts::save_login(
        db,
        issued.clone(),
        identity.sub,
        label,
        identity.plan.chatgpt_plan_type,
        session,
    )
    .await?;
    if permitted {
        accounts::catalog(db, &http, &issued).await?
    }
    Ok(())
}
#[tauri::command]
pub async fn use_api_key(db: State<'_, Database>, auth: State<'_, Auth>) -> Result<()> {
    let _guard = auth.operations.lock().await;
    db.run(|c| {
        c.execute("UPDATE oauth_host SET active=NULL", [])?;
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn plan_models(db: State<'_, Database>, auth: State<'_, Auth>) -> Result<()> {
    let _guard = auth.operations.lock().await;
    let status = db.run(|c| accounts::status(c)).await?;
    let id = status
        .active
        .ok_or_else(|| invalid("Select a ChatGPT account first."))?;
    accounts::catalog(&db, &oauth::client()?, &id).await
}
#[tauri::command]
pub async fn select_plan_model(
    db: State<'_, Database>,
    auth: State<'_, Auth>,
    id: String,
    model: String,
) -> Result<()> {
    let _guard = auth.operations.lock().await;
    db.run(move |c| {
        let s = accounts::status(c)?;
        if s.active.as_deref() != Some(&id) || !s.models.iter().any(|m| m.slug == model) {
            return Err(invalid(
                "Choose a model listed for the active ChatGPT account.",
            ));
        }
        c.execute(
            "UPDATE oauth_accounts SET model=? WHERE id=?",
            rusqlite::params![model, id],
        )?;
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn welcome_seen(db: State<'_, Database>, id: String) -> Result<()> {
    db.run(move |c| {
        c.execute("UPDATE oauth_accounts SET welcomed=1 WHERE id=?", [id])?;
        Ok(())
    })
    .await
}
#[tauri::command]
pub async fn chatgpt_logout(
    db: State<'_, Database>,
    auth: State<'_, Auth>,
    id: String,
) -> Result<bool> {
    let _guard = auth.operations.lock().await;
    let revoke = async {
        let http = oauth::client()?;
        let saved = accounts::saved(&db, &id).await?;
        let Some(refresh) = saved.session.and_then(|s| s.refresh) else {
            return Ok(true);
        };
        let endpoint = oauth::metadata(&http).await?.revocation_endpoint;
        for retry in 0..2 {
            let response = http
                .post(&endpoint)
                .form(&[
                    ("client_id", &*id),
                    ("token", &refresh),
                    ("token_type_hint", "refresh_token"),
                ])
                .send()
                .await;
            if let Ok(response) = response {
                if response.status().is_success() {
                    return Ok(true);
                }
                if !response.status().is_server_error() {
                    break;
                }
            }
            if retry == 0 {
                tokio::time::sleep(Duration::from_millis(400)).await
            }
        }
        Ok::<_, crate::error::Error>(false)
    }
    .await
    .unwrap_or(false);
    // Keep this profile selected and disconnected until the user chooses another billing method.
    db.run(move |c| {
        c.execute(
            "UPDATE oauth_accounts SET session=NULL,permitted=0,models='[]',model=NULL WHERE id=?",
            [id],
        )?;
        Ok(())
    })
    .await?;
    Ok(revoke)
}
pub async fn authorization(
    db: &Database,
    auth: &Auth,
    debate: Option<&crate::model::DebateOptions>,
) -> Result<(String, Option<String>, Option<String>)> {
    let _guard = auth.operations.lock().await;
    let status = db.run(|c| accounts::status(c)).await?;
    if let Some(options) = debate {
        super::super::services::debate::validate_agents(&options.run.agents)?;
        if options.account != status.active {
            return Err(invalid("The billing account changed. Reopen Roundtable."));
        }
        if status.active.is_some()
            && options
                .run
                .agents
                .iter()
                .any(|agent| !status.models.iter().any(|m| m.slug == agent.model))
        {
            return Err(invalid("Choose models available to this ChatGPT account."));
        }
    }
    if let Some(id) = status.active {
        let access = accounts::access(db, &oauth::client()?, &id).await?;
        let model = status
            .model
            .filter(|v| status.models.iter().any(|m| m.slug == *v))
            .ok_or_else(|| invalid("Refresh the ChatGPT models in Settings."))?;
        return Ok((access, Some(model), Some(id)));
    }
    let key = tokio::task::spawn_blocking(credentials::read)
        .await??
        .ok_or_else(|| invalid("Continue with ChatGPT or add an OpenAI API key in Settings."))?;
    Ok((key, None, None))
}

#[tauri::command]
pub async fn debate_models(
    db: State<'_, Database>,
    auth: State<'_, Auth>,
    client: State<'_, reqwest::Client>,
    account: Option<String>,
) -> Result<Vec<String>> {
    let _guard = auth.operations.lock().await;
    let status = db.run(|c| accounts::status(c)).await?;
    if status.active != account {
        return Err(invalid("The billing account changed. Reopen Roundtable."));
    }
    if account.is_some() {
        return Ok(status.models.into_iter().map(|m| m.slug).collect());
    }
    let key = tokio::task::spawn_blocking(credentials::read)
        .await??
        .ok_or_else(|| invalid("Connect a ChatGPT plan or save an API key in Settings."))?;
    #[derive(serde::Deserialize)]
    struct Model {
        id: String,
    }
    #[derive(serde::Deserialize)]
    struct Catalog {
        data: Vec<Model>,
    }
    let response = client
        .get("https://api.openai.com/v1/models")
        .bearer_auth(key)
        .send()
        .await?
        .error_for_status()?;
    let mut ids: Vec<_> = response
        .json::<Catalog>()
        .await?
        .data
        .into_iter()
        .map(|m| m.id)
        .filter(|id| {
            let name = id.strip_prefix("ft:").unwrap_or(id);
            (name.starts_with("gpt-") || ["o1", "o3", "o4"].iter().any(|p| name.starts_with(p)))
                && ![
                    "audio",
                    "image",
                    "realtime",
                    "transcribe",
                    "tts",
                    "search",
                    "instruct",
                ]
                .iter()
                .any(|p| name.contains(p))
        })
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}
