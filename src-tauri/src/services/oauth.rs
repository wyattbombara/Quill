use crate::error::{Result, invalid};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header, jwk::JwkSet};
use reqwest::{Client, Url};
use ring::{
    digest,
    rand::{SecureRandom, SystemRandom},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    time::timeout,
};

pub const ISSUER: &str = "https://auth.openai.com";
pub const RESOURCE: &str = "https://api.openai.com/v1";
pub const TOKEN: &str = "https://auth.openai.com/api/accounts/oauth/token";
pub const DYNAMIC: &str = "dynamic_agent_client";
const SCOPES: &str =
    "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct";

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0; N];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| invalid("Secure random generator unavailable."))?;
    Ok(bytes)
}
pub fn client() -> Result<Client> {
    Ok(Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(25))
        .build()?)
}

pub struct Attempt {
    pub state: String,
    pub nonce: String,
    pub verifier: String,
    pub redirect: String,
}
impl Attempt {
    pub fn new(port: u16) -> Result<Self> {
        Ok(Self {
            state: URL_SAFE_NO_PAD.encode(random::<32>()?),
            nonce: URL_SAFE_NO_PAD.encode(random::<32>()?),
            verifier: URL_SAFE_NO_PAD.encode(random::<32>()?),
            redirect: format!("http://127.0.0.1:{port}/auth/callback"),
        })
    }
    pub fn authorize(
        &self,
        host: &str,
        client_id: Option<&str>,
        hint: Option<&str>,
        reconsent: bool,
    ) -> Result<Url> {
        let mut url = Url::parse("https://auth.openai.com/api/accounts/authorize")
            .map_err(|_| invalid("Invalid authorization URL."))?;
        let challenge =
            URL_SAFE_NO_PAD.encode(digest::digest(&digest::SHA256, self.verifier.as_bytes()));
        let mut q = url.query_pairs_mut();
        q.extend_pairs([
            ("client_id", client_id.unwrap_or(DYNAMIC)),
            ("ext_agent_host_id", host),
            ("response_type", "code"),
            ("redirect_uri", &self.redirect),
            ("resource", RESOURCE),
            ("scope", SCOPES),
            ("nonce", &self.nonce),
            ("state", &self.state),
            ("code_challenge_method", "S256"),
            ("code_challenge", &challenge),
        ]);
        if client_id.is_none() {
            q.append_pair("agent_name_hint", "Quill");
        }
        if let Some(hint) = hint {
            q.append_pair("id_token_hint", hint);
        }
        if reconsent {
            q.append_pair("prompt", "consent");
        }
        drop(q);
        Ok(url)
    }
    pub fn callback(
        &self,
        target: &str,
        client_id: Option<&str>,
    ) -> Option<Result<(String, String)>> {
        if target.split('?').next() != Some("/auth/callback") {
            return None;
        }
        let url = Url::parse(&format!("http://127.0.0.1{target}")).ok()?;
        let mut params = HashMap::new();
        for (k, v) in url.query_pairs() {
            if params.insert(k, v).is_some() {
                return None;
            }
        }
        if params.get("state")?.as_ref() != self.state {
            return None;
        }
        if params.contains_key("error") {
            return Some(Err(invalid("ChatGPT sign-in was declined or cancelled.")));
        }
        let issued = params.get("client_id").map(|v| v.as_ref()).or(client_id);
        Some(match (params.get("code"), issued) {
            (Some(code), Some(id))
                if !code.is_empty()
                    && !id.is_empty()
                    && id != DYNAMIC
                    && client_id.is_none_or(|expected| expected == id) =>
            {
                Ok((code.to_string(), id.to_owned()))
            }
            _ => Err(invalid(
                "Incomplete or mismatched ChatGPT registration. Start sign-in again.",
            )),
        })
    }
    pub async fn listen(
        &self,
        listener: &TcpListener,
        id: Option<&str>,
    ) -> Result<(String, String)> {
        loop {
            let (mut socket, _) = listener
                .accept()
                .await
                .map_err(|_| invalid("Sign-in callback unavailable."))?;
            let mut line = String::new();
            let read = timeout(
                Duration::from_secs(2),
                BufReader::new((&mut socket).take(16384)).read_line(&mut line),
            )
            .await;
            let mut parts = line.split_ascii_whitespace();
            let result =
                if matches!(read, Ok(Ok(_))) && line.ends_with('\n') && parts.next() == Some("GET")
                {
                    parts.next().and_then(|target| self.callback(target, id))
                } else {
                    None
                };
            let (code, message) = if result.is_some() {
                (
                    "200 OK",
                    "Return to Quill to finish sign-in. You can close this tab.",
                )
            } else {
                ("400 Bad Request", "Invalid sign-in callback.")
            };
            let response = format!(
                "HTTP/1.1 {code}\r\nContent-Type: text/plain; charset=utf-8\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{message}",
                message.len()
            );
            let _ = timeout(
                Duration::from_secs(1),
                socket.write_all(response.as_bytes()),
            )
            .await;
            if let Some(result) = result {
                return result;
            }
        }
    }
}

#[derive(Deserialize)]
pub struct Metadata {
    pub issuer: String,
    pub jwks_uri: String,
    pub revocation_endpoint: String,
}
pub async fn metadata(http: &Client) -> Result<Metadata> {
    let m: Metadata = http
        .get(format!("{ISSUER}/.well-known/openid-configuration"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    if m.issuer != ISSUER
        || [&m.jwks_uri, &m.revocation_endpoint]
            .iter()
            .any(|s| !s.starts_with("https://"))
    {
        return Err(invalid("Invalid OpenAI sign-in configuration."));
    }
    Ok(m)
}
#[derive(Default, Deserialize)]
pub struct PlanClaims {
    pub chatgpt_plan_type: Option<String>,
}
#[derive(Deserialize)]
pub struct Identity {
    #[serde(default, rename = "https://api.openai.com/auth")]
    pub plan: PlanClaims,
    pub sub: String,
    pub email: Option<String>,
    pub nonce: Option<String>,
    pub azp: Option<String>,
    pub aud: serde_json::Value,
}
pub fn verify(jwt: &str, keys: &JwkSet, client_id: &str, nonce: Option<&str>) -> Result<Identity> {
    let reject = || invalid("ChatGPT identity verification failed. Sign in again.");
    let header = decode_header(jwt).map_err(|_| reject())?;
    let jwk = keys
        .find(&header.kid.ok_or_else(reject)?)
        .ok_or_else(reject)?;
    let key = DecodingKey::from_jwk(jwk).map_err(|_| reject())?;
    let mut rules = Validation::new(Algorithm::RS256);
    rules.set_issuer(&[ISSUER]);
    rules.set_audience(&[client_id]);
    rules.set_required_spec_claims(&["iss", "sub", "aud", "exp"]);
    rules.validate_nbf = true;
    rules.leeway = 5;
    let identity = decode::<Identity>(jwt, &key, &rules)
        .map_err(|_| reject())?
        .claims;
    if identity.sub.is_empty()
        || nonce.is_some_and(|n| identity.nonce.as_deref() != Some(n))
        || identity.azp.as_deref().is_some_and(|a| a != client_id)
        || (identity.aud.as_array().is_some_and(|a| a.len() > 1) && identity.azp.is_none())
    {
        return Err(reject());
    }
    Ok(identity)
}
#[derive(Serialize, Deserialize)]
pub struct Session {
    pub access: String,
    pub refresh: Option<String>,
    pub id_token: String,
    pub expires: u64,
    pub scope: String,
}
impl Session {
    pub fn permitted(&self) -> bool {
        ["resource.invoke", "chatgpt.tokens.use.direct"]
            .iter()
            .all(|required| self.scope.split_whitespace().any(|s| s == *required))
    }
}
#[derive(Deserialize)]
pub struct Tokens {
    access_token: Option<String>,
    refresh_token: Option<String>,
    pub id_token: Option<String>,
    expires_in: Option<u64>,
    scope: Option<String>,
    token_type: Option<String>,
}
impl Tokens {
    pub fn into_session(self, previous: Option<Session>) -> Result<Session> {
        let mut session = previous.unwrap_or(Session {
            access: String::new(),
            refresh: None,
            id_token: String::new(),
            expires: 0,
            scope: String::new(),
        });
        if let Some(id) = self.id_token {
            session.id_token = id
        }
        if let Some(scope) = self.scope {
            session.scope = scope
        }
        session.access = self.access_token.unwrap_or_default();
        if self.refresh_token.is_some() {
            session.refresh = self.refresh_token;
        }
        if session.permitted()
            && (session.access.is_empty()
                || !self
                    .token_type
                    .as_deref()
                    .is_some_and(|t| t.eq_ignore_ascii_case("bearer"))
                || self.expires_in.unwrap_or(0) == 0)
        {
            return Err(invalid(
                "OpenAI returned an incomplete access grant. Sign in again.",
            ));
        }
        session.expires = now().saturating_add(self.expires_in.unwrap_or(0));
        Ok(session)
    }
}
pub async fn exchange(http: &Client, fields: &[(&str, &str)]) -> Result<Tokens> {
    let r = http.post(TOKEN).form(fields).send().await?;
    if !r.status().is_success() {
        let status = r.status();
        let body: serde_json::Value = r.json().await.unwrap_or_default();
        return Err(invalid(if body["error"] == "invalid_grant" {
            "ChatGPT authorization expired or was revoked. Sign in again.".into()
        } else {
            format!("OpenAI token exchange failed ({status}). Try signing in again.")
        }));
    }
    Ok(r.json().await?)
}
pub async fn identity(
    http: &Client,
    id_token: &str,
    id: &str,
    nonce: Option<&str>,
) -> Result<Identity> {
    let m = metadata(http).await?;
    let keys = http
        .get(m.jwks_uri)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    verify(id_token, &keys, id, nonce)
}
#[derive(Clone, Serialize, Deserialize)]
pub struct PlanModel {
    pub slug: String,
    pub display_name: String,
    visibility: String,
}
pub async fn models(http: &Client, token: &str) -> Result<Vec<PlanModel>> {
    #[derive(Deserialize)]
    struct Catalog {
        models: Vec<PlanModel>,
    }
    let response = http
        .get(format!("{RESOURCE}/models"))
        .bearer_auth(token)
        .send()
        .await?
        .error_for_status()?;
    let mut entries = response.json::<Catalog>().await?.models;
    entries.retain(|m| m.visibility == "list" && !m.slug.is_empty());
    Ok(entries)
}
