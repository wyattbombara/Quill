use quill_chat::services::oauth::{Attempt, DYNAMIC, Session, Tokens, verify};
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[test]
fn reconnect_and_explicit_billing_survive_restart() {
    use quill_chat::services::{
        accounts,
        database::{Database, begin_with_model},
    };
    let directory = tempfile::tempdir().unwrap();
    let db = Database::with_key(directory.path().join("fixture.sqlite"), &[42;32]);
    db.initialize().unwrap();
    let mut connection = db.connect().unwrap();
    let host = accounts::host(&connection).unwrap();
    assert!(host.starts_with("urn:uuid:"));
    assert!(accounts::status(&connection).unwrap().active.is_none());
    connection.execute("INSERT INTO oauth_accounts(id,subject,label,permitted) VALUES('example','user','Sample',0)",[]).unwrap();
    connection
        .execute("UPDATE oauth_host SET active='example'", [])
        .unwrap();
    drop(connection);
    db.initialize().unwrap();
    connection = db.connect().unwrap();
    assert_eq!(accounts::host(&connection).unwrap(), host);
    let status = accounts::status(&connection).unwrap();
    assert_eq!(status.active.as_deref(), Some("example"));
    assert!(!status.accounts[0].connected && !status.accounts[0].permitted);
    connection
        .execute("INSERT INTO chats(title) VALUES('Sample')", [])
        .unwrap();
    let turn = begin_with_model(
        &mut connection,
        1,
        "Test".into(),
        vec![],
        Some("plan-only".into()),
    )
    .unwrap();
    assert_eq!(turn.model, "plan-only");
    let saved: String = connection
        .query_row("SELECT model FROM turns WHERE id=?", [turn.id], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(saved, turn.model);
}

#[test]
fn new_and_returning_authorizations_are_bound_to_the_host_and_attempt() {
    let a = Attempt::new(4321).unwrap();
    let b = Attempt::new(4321).unwrap();
    assert_ne!(a.verifier, b.verifier);
    assert_ne!(a.state, b.state);
    assert_ne!(a.nonce, b.nonce);
    let url = a.authorize("urn:uuid:fixture", None, None, false).unwrap();
    let q: std::collections::HashMap<_, _> = url.query_pairs().collect();
    assert_eq!(q["client_id"], DYNAMIC);
    assert_eq!(q["redirect_uri"], "http://127.0.0.1:4321/auth/callback");
    assert_eq!(q["agent_name_hint"], "Quill");
    assert_eq!(q["code_challenge_method"], "S256");
    assert!(!url.as_str().contains(&a.verifier));
    assert_eq!(q["code_challenge"].len(), 43);
    let url = a
        .authorize(
            "urn:uuid:fixture",
            Some("issued"),
            Some("fixture-hint"),
            true,
        )
        .unwrap();
    let q: std::collections::HashMap<_, _> = url.query_pairs().collect();
    assert!(!q.contains_key("agent_name_hint"));
    assert_eq!(q["prompt"], "consent");
    assert_eq!(q["id_token_hint"], "fixture-hint");
    let callback = format!("/auth/callback?state={}&code=sample", a.state);
    assert!(a.callback(&callback, None).unwrap().is_err());
    assert_eq!(
        a.callback(&callback, Some("issued")).unwrap().unwrap().1,
        "issued"
    );
    assert!(
        a.callback(&(callback.clone() + "&client_id=wrong"), Some("issued"))
            .unwrap()
            .is_err()
    );
    assert!(a.callback(&(callback + "&state=duplicate"), None).is_none());
    assert!(
        a.callback("/auth/callback?state=wrong&error=access_denied", None)
            .is_none()
    );
    assert!(
        a.callback(
            &format!("/auth/callback?state={}&error=access_denied", a.state),
            None
        )
        .unwrap()
        .is_err()
    );
}
#[tokio::test]
async fn loopback_ignores_unrelated_traffic_and_completes_once() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
    let addr = listener.local_addr().unwrap();
    let a = Attempt::new(addr.port()).unwrap();
    let state = a.state.clone();
    let handler = tokio::spawn(async move { a.listen(&listener, None).await });
    for (path, status) in [
        ("/favicon.ico".to_string(), "400"),
        (
            format!("/auth/callback?code=mock&client_id=issued&state={state}"),
            "200",
        ),
    ] {
        let mut socket = TcpStream::connect(addr).await.unwrap();
        socket
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: {addr}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        let mut response = String::new();
        socket.read_to_string(&mut response).await.unwrap();
        assert!(response.starts_with(&format!("HTTP/1.1 {status}")));
        assert!(!response.contains(&state));
    }
    assert_eq!(
        handler.await.unwrap().unwrap(),
        ("mock".into(), "issued".into())
    );
}
#[test]
fn verifies_signature_audience_and_nonce() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/identity.json")).unwrap();
    let keys = serde_json::from_value(fixture["keys"].clone()).unwrap();
    let token = fixture["token"].as_str().unwrap();
    assert_eq!(
        verify(token, &keys, "quill-test", Some("once"))
            .unwrap()
            .sub,
        "fixture"
    );
    assert!(verify(token, &keys, "wrong", Some("once")).is_err());
    assert!(verify(token, &keys, "quill-test", Some("wrong")).is_err());
    assert!(verify(&format!("{token}AAAA"), &keys, "quill-test", Some("once")).is_err());
    assert!(
        verify(
            fixture["expired"].as_str().unwrap(),
            &keys,
            "quill-test",
            Some("once")
        )
        .is_err()
    );
    assert!(
        verify(
            fixture["wrong_issuer"].as_str().unwrap(),
            &keys,
            "quill-test",
            Some("once")
        )
        .is_err()
    );
}
#[test]
fn grant_and_refresh_require_plan_permission() {
    let tokens: Tokens =
        serde_json::from_value(json!({"id_token":"identity","scope":"openid email"})).unwrap();
    assert!(!tokens.into_session(None).unwrap().permitted());
    let previous = Session {
        access: "expired".into(),
        refresh: Some("old".into()),
        id_token: "identity".into(),
        scope: "chatgpt.tokens.use.direct resource.invoke".into(),
        expires: 0,
    };
    let tokens:Tokens=serde_json::from_value(json!({"access_token":"new","token_type":"Bearer","refresh_token":"replacement","expires_in":3600})).unwrap();
    let session = tokens.into_session(Some(previous)).unwrap();
    assert!(session.permitted());
    assert_eq!(session.id_token, "identity");
    assert_eq!(session.refresh.as_deref(), Some("replacement"));
    let revoked: Tokens = serde_json::from_value(json!({"scope":"openid"})).unwrap();
    assert!(!revoked.into_session(Some(session)).unwrap().permitted());
}
