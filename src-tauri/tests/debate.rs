use quill_chat::{
    model::{AgentMode, AgentRun, DebateAgent, StreamEvent},
    services::{
        database::{self, Database},
        debate,
    },
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

fn read_request(socket: &TcpStream) -> Value {
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut reader = BufReader::new(socket);
    let mut length = 0;
    loop {
        let mut line = String::new();
        assert!(reader.read_line(&mut line).unwrap() > 0);
        if line == "\r\n" {
            break;
        }
        if let Some(s) = line.to_lowercase().strip_prefix("content-length:") {
            length = s.trim().parse().unwrap();
        }
    }
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes).unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn handle(mut socket: TcpStream, agree: bool) {
    let body = read_request(&socket);
    let direction = body["input"][0]["content"].as_str().unwrap();
    let profile_msg = &body["input"][1];
    assert_eq!(profile_msg["role"], "user");
    let profile: Value = serde_json::from_str(
        profile_msg["content"]
            .as_str()
            .unwrap()
            .strip_prefix("Your Roundtable seat profile: ")
            .unwrap(),
    )
    .unwrap();
    let name = profile["name"].as_str().unwrap();
    assert_eq!(profile["personality"], format!("Check evidence as {name}."));
    assert_eq!(body["model"], "shared-model");
    let answer = if direction.contains("Independently") {
        assert_eq!(body["input"].as_array().unwrap().len(), 3);
        format!("Opening from {}", body["model"])
    } else {
        let evidence = body["input"].as_array().unwrap().last().unwrap()["content"]
            .as_str()
            .unwrap();
        assert!(evidence.contains("Opening from"));
        assert!(evidence.contains("Seat 0"));
        if direction.contains("Compare these") {
            if agree {
                "CONSENSUS\nUse a media folder.".into()
            } else {
                "DISAGREEMENT\nHow will backups work?".into()
            }
        } else if direction.contains("Reply once") {
            assert!(evidence.contains("How will backups work?"));
            "Export the folder with the database.".into()
        } else {
            if evidence.contains("DISAGREEMENT") && evidence.contains("rebuttal") {
                assert!(evidence.contains("Export the folder"));
            }
            "Bundle the folder in the backup.".into()
        }
    };
    answer_stream(&mut socket, &answer);
}

fn answer_stream(socket: &mut TcpStream, answer: &str) {
    let delta = json!({"type":"response.output_text.delta","delta":answer});
    let end = json!({"type":"response.completed","response":{"model":"served-model","usage":{"input_tokens":7,"output_tokens":3}}});
    write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {delta}\n\ndata: {end}\n\n").unwrap();
}

fn agents(n: usize) -> Vec<DebateAgent> {
    (0..n)
        .map(|i| DebateAgent {
            name: format!("Seat {i}"),
            model: "shared-model".into(),
            focus: String::new(),
            personality: format!("Check evidence as Seat {i}."),
        })
        .collect()
}
async fn exercise(agree: bool, seats: usize) {
    let tmp = tempfile::tempdir().unwrap();
    let db = Database::with_key(tmp.path().join("roundtable.db"), &[42; 32]);
    db.initialize().unwrap();
    let models = agents(seats);
    let run = AgentRun {
        mode: AgentMode::Roundtable,
        agents: models.clone(),
    };
    let roster = run.clone();
    let req = db
        .run(move |c| {
            c.execute("INSERT INTO chats(title) VALUES('New chat')", [])?;
            database::begin_turn(c, 1, "Store my files?".into(), vec![], None, Some(roster))
        })
        .await
        .unwrap();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let endpoint = format!("http://{}/v1/responses", listener.local_addr().unwrap());
    let count = if agree {
        (seats + 1).min(8)
    } else {
        (2 * seats + 2).min(8)
    };
    let server = std::thread::spawn(move || {
        for _ in 0..count {
            let (socket, _) = listener.accept().unwrap();
            handle(socket, agree);
        }
    });
    let http = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let answer = debate::run(
        &db,
        &http,
        &endpoint,
        "test",
        1,
        req,
        &run,
        &CancellationToken::new(),
        &Channel::<StreamEvent>::new(|_| Ok(())),
    )
    .await
    .unwrap();
    assert_eq!(
        answer,
        if agree {
            "Use a media folder."
        } else {
            "Bundle the folder in the backup."
        }
    );
    server.join().unwrap();
    // Reopen the file to exercise schema backfill and history recovery.
    db.initialize().unwrap();
    db.run(move |c| {
        let turn = database::turns(c, 1)?.pop().unwrap();
        let saved = turn.debate.unwrap();
        assert_eq!(saved.run.agents, models);
        assert_eq!(saved.entries.len(), count);
        assert!(
            saved
                .entries
                .iter()
                .all(|e| e.model.as_deref() == Some("served-model"))
        );
        assert!(
            saved
                .entries
                .iter()
                .all(|e| !e.text.is_empty() && e.status == "complete")
        );
        assert_eq!(
            database::usage_report(c, Some(1))?.chat.requests,
            count as i64
        );
        c.execute(
            "UPDATE debates SET models=? WHERE turn=?",
            rusqlite::params![r#"["legacy-a","legacy-b"]"#, turn.id],
        )?;
        let legacy = debate::load(c, turn.id)?.unwrap();
        assert_eq!(legacy.run.agents[0].name, "Finch");
        assert_eq!(legacy.run.agents[1].model, "legacy-b");
        assert!(legacy.run.agents[0].personality.is_empty());
        c.execute("DELETE FROM chats", [])?;
        assert_eq!(
            c.query_row("SELECT COUNT(*) FROM debate_entries", [], |r| r
                .get::<_, i64>(0))?,
            0
        );
        assert_eq!(
            database::usage_report(c, None)?.device.output_tokens,
            count as i64 * 3
        );
        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn agreement_ends_after_review() {
    exercise(true, 2).await;
}
#[tokio::test]
async fn disagreement_collects_one_rebuttal_per_model() {
    exercise(false, 3).await;
}
#[test]
fn validates_names_and_limits() {
    let mut duplicate = agents(2);
    duplicate[1].name = " seat 0 ".into();
    assert!(debate::validate_agents(&duplicate).is_err());
    assert!(debate::validate_agents(&agents(1)).is_err());
    assert!(debate::validate_agents(&agents(9)).is_err());
    let mut long = agents(2);
    long[0].personality = "a".repeat(401);
    assert!(debate::validate_agents(&long).is_err());
}
#[tokio::test]
async fn eight_custom_agents_share_a_model() {
    exercise(false, 8).await;
}

#[tokio::test]
async fn stop_cancels_parallel_openings_before_review() {
    cancel_run(AgentMode::Roundtable).await;
}

#[tokio::test]
async fn stop_cancels_independent_replies() {
    cancel_run(AgentMode::Parallel).await;
}

async fn cancel_run(mode: AgentMode) {
    let temp = tempfile::tempdir().unwrap();
    let db = Database::with_key(temp.path().join("cancel.db"), &[42; 32]);
    db.initialize().unwrap();
    let models = agents(2);
    let run = AgentRun {
        mode,
        agents: models.clone(),
    };
    let roster = run.clone();
    let request = db
        .run(move |c| {
            c.execute("INSERT INTO chats(title) VALUES('New chat')", [])?;
            database::begin_turn(c, 1, "Question".into(), vec![], None, Some(roster))
        })
        .await
        .unwrap();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .unwrap();
    let endpoint = format!("http://{}/responses", listener.local_addr().unwrap());
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let server = tokio::spawn(async move {
        // Both models must connect before either gets an answer.
        let connections = [
            listener.accept().await.unwrap(),
            listener.accept().await.unwrap(),
        ];
        signal.cancel();
        tokio::time::sleep(Duration::from_millis(30)).await;
        drop(connections);
    });
    let http = reqwest::Client::builder().no_proxy().build().unwrap();
    let channel = Channel::<StreamEvent>::new(|_| Ok(()));
    let result = tokio::time::timeout(
        Duration::from_secs(5),
        debate::run(
            &db, &http, &endpoint, "test", 1, request, &run, &cancel, &channel,
        ),
    )
    .await
    .unwrap();
    assert!(matches!(result, Err(quill_chat::error::Error::Cancelled)));
    server.await.unwrap();
    db.run(move |c| {
        let entries = database::turns(c, 1)?
            .pop()
            .unwrap()
            .debate
            .unwrap()
            .entries;
        assert_eq!(entries.len(), 2);
        assert!(entries.iter().all(|e| e.phase
            == (if mode == AgentMode::Parallel {
                "parallel"
            } else {
                "opening"
            })
            && e.status == "stopped"));
        assert_eq!(database::usage_report(c, Some(1))?.chat.requests, 2);
        Ok(())
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn independent_seats_share_a_snapshot_and_survive_a_peer_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let db = Database::with_key(tmp.path().join("parallel.db"), &[42; 32]);
    db.initialize().unwrap();
    let mut team = agents(3);
    for a in &mut team {
        a.focus = format!("Assignment for {}", a.name);
    }
    let run = AgentRun {
        mode: AgentMode::Parallel,
        agents: team,
    };
    let roster = run.clone();
    let request = db
        .run(move |c| {
            c.execute("INSERT INTO chats(title) VALUES('Shared chat')", [])?;
            let previous = database::begin(c, 1, "Earlier question".into(), vec![])?;
            c.execute(
                "UPDATE turns SET status='complete',reply='Earlier answer' WHERE id=?",
                [previous.id],
            )?;
            database::set_goal(c, Some(1), Some("A shared goal".into()))?;
            let file = quill_chat::model::Attachment {
                name: "brief.txt".into(),
                mime: "text/plain".into(),
                data: "YQ==".into(),
            };
            database::begin_turn(
                c,
                1,
                "Current question".into(),
                vec![file],
                None,
                Some(roster),
            )
        })
        .await
        .unwrap();
    let snapshot = request.input.clone();
    let turn_id = request.id;
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let endpoint = format!("http://{}/responses", listener.local_addr().unwrap());
    let server = std::thread::spawn(move || {
        let mut sockets = Vec::new();
        // Hold all replies until every seat has sent its full request.
        for _ in 0..3 {
            let socket = listener.accept().unwrap().0;
            let body = read_request(&socket);
            let input = body["input"].as_array().unwrap();
            assert_eq!(&input[2..], snapshot.as_slice());
            assert!(
                input[0]["content"]
                    .as_str()
                    .unwrap()
                    .contains("independently")
            );
            let profile: Value = serde_json::from_str(
                input[1]["content"]
                    .as_str()
                    .unwrap()
                    .split_once(": ")
                    .unwrap()
                    .1,
            )
            .unwrap();
            let name = profile["name"].as_str().unwrap().to_owned();
            assert_eq!(profile["assignment"], format!("Assignment for {name}"));
            sockets.push((name, socket));
        }
        sockets.sort_by(|a, b| b.0.cmp(&a.0));
        for (name, mut socket) in sockets {
            if name == "Seat 1" {
                socket.write_all(b"HTTP/1.1 503 Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            } else {
                answer_stream(&mut socket, &format!("Result from {name}"));
            }
        }
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let answer = debate::run(
        &db,
        &client,
        &endpoint,
        "fixture",
        1,
        request,
        &run,
        &CancellationToken::new(),
        &Channel::<StreamEvent>::new(|_| Ok(())),
    )
    .await
    .unwrap();
    server.join().unwrap();
    assert_eq!(
        answer,
        "Seat 0:\nResult from Seat 0\n\nSeat 2:\nResult from Seat 2"
    );
    db.run(move |c| {
        c.execute(
            "UPDATE turns SET status='complete',reply=? WHERE id=?",
            rusqlite::params![answer, turn_id],
        )?;
        let saved = debate::load(c, turn_id)?.unwrap();
        assert_eq!(saved.run.mode, AgentMode::Parallel);
        assert_eq!(saved.run.agents[2].focus, "Assignment for Seat 2");
        assert_eq!(saved.entries.len(), 3);
        assert!(saved.entries.iter().all(|e| e.phase == "parallel"
            && if e.seat == 1 {
                e.status == "failed"
            } else {
                e.status == "complete" && e.text == format!("Result from Seat {}", e.seat)
            }));
        assert_eq!(database::usage_report(c, Some(1))?.chat.requests, 4);
        let next = database::begin(c, 1, "Follow up".into(), vec![])?;
        assert!(
            next.input
                .iter()
                .any(|m| m["role"] == "assistant" && m["content"] == answer)
        );
        Ok(())
    })
    .await
    .unwrap();
}

#[test]
fn parallel_limits_follow_active_tier_and_only_its_recent_requests() {
    use quill_chat::services::accounts::{host, parallel_capacity};
    let dir = tempfile::tempdir().unwrap();
    let db = Database::with_key(dir.path().join("limits.sqlite"), &[42; 32]);
    db.initialize().unwrap();
    let mut c = db.connect().unwrap();
    host(&c).unwrap();
    assert_eq!(parallel_capacity(&c).unwrap().seats, 3);
    c.execute("INSERT INTO chats(title) VALUES('test')", [])
        .unwrap();
    c.execute("INSERT INTO oauth_accounts(id,subject,label,session,permitted) VALUES('a','s','test',X'00',1)",[]).unwrap();
    c.execute("UPDATE oauth_host SET active='a'", []).unwrap();
    for (tier, expected) in [("free", 3), ("plus", 4), ("pro", 5)] {
        c.execute("INSERT OR REPLACE INTO account_plans VALUES('a',?)", [tier])
            .unwrap();
        assert_eq!(parallel_capacity(&c).unwrap().seats, expected);
    }
    for n in 1..=20 {
        c.execute(
            "INSERT INTO request_times(agent,request,account) VALUES(1,?,'other')",
            [n],
        )
        .unwrap();
    }
    assert_eq!(parallel_capacity(&c).unwrap().seats, 5);
    c.execute("UPDATE request_times SET account='a'", [])
        .unwrap();
    assert_eq!(parallel_capacity(&c).unwrap().seats, 4);
    let oversized = AgentRun {
        mode: AgentMode::Parallel,
        agents: agents(5),
    };
    assert!(database::begin_turn(&mut c, 1, "test".into(), vec![], None, Some(oversized)).is_err());
    assert_eq!(
        c.query_row("SELECT count(*) FROM turns", [], |r| r.get::<_, usize>(0))
            .unwrap(),
        0
    );
    c.execute("UPDATE request_times SET started=unixepoch()-3601", [])
        .unwrap();
    assert_eq!(parallel_capacity(&c).unwrap().seats, 5);
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn immediate_interjection_is_delivered_to_selected_final_speaker_and_persisted() {
    use quill_chat::services::floor::Floor;
    use std::sync::Arc;
    let tmp = tempfile::tempdir().unwrap();
    let db = Database::with_key(tmp.path().join("interactive.db"), &[77; 32]);
    db.initialize().unwrap();
    let run = AgentRun {
        mode: AgentMode::Roundtable,
        agents: agents(2),
    };
    let roster = run.clone();
    let req = db
        .run(move |c| {
            let mut prefs = database::read_preferences(c)?;
            prefs.workspace.roundtable_budget = 2;
            c.execute(
                "INSERT INTO preferences VALUES(1,?)",
                [serde_json::to_string(&prefs)?],
            )?;
            c.execute("INSERT INTO chats(title) VALUES('Live debate')", [])?;
            database::begin_turn(c, 1, "Design?".into(), vec![], None, Some(roster))
        })
        .await
        .unwrap();
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let endpoint = format!("http://{}/responses", listener.local_addr().unwrap());
    let floor = Arc::new(Floor::new(2));
    let controller = floor.clone();
    let mock = std::thread::spawn(move || {
        let (first, _) = listener.accept().unwrap();
        let _ = read_request(&first);
        controller
            .interject(Some(0), "Offline only".into(), true)
            .unwrap();
        let (mut last, _) = listener.accept().unwrap();
        let body = read_request(&last);
        assert!(
            body["input"][1]["content"]
                .as_str()
                .unwrap()
                .contains("Seat 0")
        );
        assert!(
            body["input"].as_array().unwrap().last().unwrap()["content"]
                .as_str()
                .unwrap()
                .contains("Offline only")
        );
        assert!(controller.interject(None, "Late".into(), false).is_err());
        answer_stream(&mut last, "Use local storage.");
    });
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let answer = tokio::time::timeout(
        Duration::from_secs(10),
        debate::run_controlled(
            &db,
            &client,
            &endpoint,
            "test",
            1,
            req,
            &run,
            &CancellationToken::new(),
            &Channel::new(|_| Ok(())),
            Some(&floor),
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(answer, "Use local storage.");
    mock.join().unwrap();
    db.run(|c| {
        let room = database::turns(c, 1)?.pop().unwrap().debate.unwrap();
        assert_eq!(room.entries.len(), 2);
        assert_eq!(room.entries[0].status, "stopped");
        assert_eq!(room.entries[1].status, "complete");
        assert_eq!(room.floor[0].after_entry, Some(room.entries[0].id));
        assert_eq!(room.floor[0].text, "Offline only");
        assert_eq!(database::usage_report(c, Some(1))?.chat.requests, 2);
        Ok(())
    })
    .await
    .unwrap();
}
