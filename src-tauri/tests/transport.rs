use quill_chat::{
    error::Error,
    model::StreamEvent,
    services::{
        api::stream,
        database::{Database, Request},
    },
};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::TcpListener,
    thread,
    time::Duration,
};
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

fn fixture(frames: &'static str) -> (reqwest::RequestBuilder, thread::JoinHandle<()>) {
    let server = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = server.local_addr().unwrap();
    let worker = thread::spawn(move || {
        let (mut socket, _) = server.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut reader = BufReader::new(&socket);
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            assert!(reader.read_line(&mut line).unwrap() > 0);
            if line == "\r\n" {
                break;
            }
            if let Some(n) = line.to_lowercase().strip_prefix("content-length:") {
                length = n.trim().parse().unwrap();
            }
        }
        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["stream"], true);
        assert_eq!(json["store"], false);
        socket
            .write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
        for bytes in frames.as_bytes().chunks(3) {
            socket.write_all(bytes).unwrap();
        }
    });
    let post = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(4))
        .build()
        .unwrap()
        .post(format!("http://{address}/v1/responses"));
    (post, worker)
}
fn request() -> Request {
    Request {
        id: 1,
        model: "fixture".into(),
        input: vec![],
    }
}
#[tokio::test]
async fn response_model_is_saved_on_the_originating_turn() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database(temp.path().join("model.sqlite3"));
    db.initialize().unwrap();
    db.run(|c| {
        c.execute("INSERT INTO chats(id,title) VALUES(1,'Chat')", [])?;
        c.execute(
            "INSERT INTO turns(chat,prompt,model,status) VALUES(1,'First','alias','streaming')",
            [],
        )?;
        Ok(())
    })
    .await
    .unwrap();
    let (post, server) = fixture(
        "data: {\"type\":\"response.completed\",\"response\":{\"model\":\"gpt-4.1-mini-2025-04-14\"}}\n\n",
    );
    stream(
        post,
        &db,
        request(),
        &CancellationToken::new(),
        &Channel::new(|_| Ok(())),
        &mut String::new(),
    )
    .await
    .unwrap();
    server.join().unwrap();
    let turns = db
        .run(|c| quill_chat::services::database::turns(c, 1))
        .await
        .unwrap();
    assert_eq!(turns[0].model, "gpt-4.1-mini-2025-04-14");
}
#[tokio::test]
async fn reads_fragmented_unicode_and_requires_completion() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database(temp.path().join("test.sqlite3"));
    db.initialize().unwrap();
    let (post, server) = fixture(
        ": heartbeat\r\n\r\ndata: {\"type\":\"response.output_text.delta\",\"delta\":\"🐦\"}\r\n\r\ndata: {\"type\":\"response.completed\"}\r\n\r\n",
    );
    let channel = Channel::<StreamEvent>::new(|_| Ok(()));
    let mut reply = String::new();
    stream(
        post,
        &db,
        request(),
        &CancellationToken::new(),
        &channel,
        &mut reply,
    )
    .await
    .unwrap();
    server.join().unwrap();
    assert_eq!(reply, "🐦");
    let (post, server) =
        fixture("data: {\"type\":\"response.output_text.delta\",\"delta\":\"unfinished\"}\n\n");
    reply.clear();
    assert!(
        stream(
            post,
            &db,
            request(),
            &CancellationToken::new(),
            &channel,
            &mut reply
        )
        .await
        .is_err()
    );
    server.join().unwrap();
    assert_eq!(reply, "unfinished");
}
#[tokio::test]
async fn stop_drops_a_request_waiting_for_headers() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database(temp.path().join("test.sqlite3"));
    db.initialize().unwrap();
    let server = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let address = server.local_addr().unwrap();
    let token = CancellationToken::new();
    let signal = token.clone();
    let observer = thread::spawn(move || {
        let (mut socket, _) = server.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut header = [0u8; 256];
        assert!(socket.read(&mut header).unwrap() > 0);
        signal.cancel();
        // The request is in flight. Its connection should close without a server response.
        let mut rest = Vec::new();
        if let Err(e) = socket.read_to_end(&mut rest) {
            assert_eq!(e.kind(), std::io::ErrorKind::ConnectionReset);
        }
    });
    let post = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(4))
        .build()
        .unwrap()
        .post(format!("http://{address}"));
    let result = stream(
        post,
        &db,
        request(),
        &token,
        &Channel::new(|_| Ok(())),
        &mut String::new(),
    )
    .await;
    assert!(matches!(result, Err(Error::Cancelled)));
    tokio::task::spawn_blocking(move || observer.join().unwrap())
        .await
        .unwrap();
}

#[tokio::test]
async fn preserves_refusals_and_reports_incomplete_reason() {
    let temp = tempfile::tempdir().unwrap();
    let db = Database(temp.path().join("test.sqlite3"));
    db.initialize().unwrap();
    db.run(|db| {
        db.execute("INSERT INTO turn_usage(turn,chat) VALUES(1,1)", [])?;
        Ok(())
    })
    .await
    .unwrap();
    let (post, server) = fixture(concat!(
        "data: {\"type\":\"response.refusal.delta\",\"delta\":\"Partial\"}\n\n",
        "data: {\"type\":\"response.incomplete\",\"response\":{\"incomplete_details\":{\"reason\":\"max_output_tokens\"},\"usage\":{\"input_tokens\":19,\"output_tokens\":3}}}\n\n"
    ));
    let mut reply = String::new();
    let result = stream(
        post,
        &db,
        request(),
        &CancellationToken::new(),
        &Channel::new(|_| Ok(())),
        &mut reply,
    )
    .await;
    assert_eq!(result.unwrap_err().to_string(), "max_output_tokens");
    assert_eq!(reply, "Partial");
    let report = db
        .run(|db| quill_chat::services::database::usage_report(db, Some(1)))
        .await
        .unwrap();
    assert_eq!(
        (
            report.chat.reported,
            report.chat.input_tokens,
            report.chat.output_tokens
        ),
        (1, 19, 3)
    );
    server.join().unwrap();
}
