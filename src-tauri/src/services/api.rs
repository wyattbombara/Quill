use crate::{
    error::{Result, invalid},
    model::StreamEvent,
    services::database::{Database, Request},
};
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use reqwest::RequestBuilder;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;

enum Frame {
    Text(String),
    Terminal {
        model: Option<String>,
        usage: Option<(i64, i64)>,
        outcome: Result<()>,
    },
    Skip,
}
fn api_message(code: Option<&str>, detail: &str) -> String {
    match code {
        Some("subscription_sharing_usage_limit_exceeded") => "ChatGPT usage limit reached. Choose Manage usage to review the limit for your plan or Quill.".into(),
        Some("subscription_sharing_user_not_eligible") => "This ChatGPT account or workspace is ineligible for plan usage in Quill.".into(),
        Some("subscription_sharing_usage_unavailable") => "ChatGPT plan usage is temporarily unavailable. Try again later.".into(),
        Some("subscription_sharing_invalid_user") => "This ChatGPT session is no longer valid. Sign in again in Settings.".into(),
        Some(code) => format!("{detail} ({code})"),
        None => detail.to_owned(),
    }
}
fn frame(data: &str) -> Result<Frame> {
    let event: Value = serde_json::from_str(data)?;
    match event["type"].as_str() {
        Some("response.output_text.delta" | "response.refusal.delta") => Ok(Frame::Text(
            event["delta"]
                .as_str()
                .ok_or_else(|| invalid("Missing stream delta."))?
                .to_owned(),
        )),
        Some(
            kind @ ("response.completed" | "error" | "response.failed" | "response.incomplete"),
        ) => {
            let usage = &event["response"]["usage"];
            let tokens = usage["input_tokens"]
                .as_i64()
                .zip(usage["output_tokens"].as_i64())
                .filter(|(input, output)| *input >= 0 && *output >= 0);
            let detail = event
                .pointer("/response/error/message")
                .or_else(|| event.get("message"))
                .or_else(|| event.pointer("/response/incomplete_details/reason"))
                .and_then(Value::as_str)
                .unwrap_or("The API could not complete the response.");
            let outcome = if kind == "response.completed" {
                Ok(())
            } else {
                Err(invalid(api_message(
                    event
                        .pointer("/response/error/code")
                        .or_else(|| event.get("code"))
                        .and_then(Value::as_str),
                    detail,
                )))
            };
            Ok(Frame::Terminal {
                model: event
                    .pointer("/response/model")
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
                usage: tokens,
                outcome,
            })
        }
        Some(_) => Ok(Frame::Skip),
        None => Err(invalid("Stream event is missing its type.")),
    }
}

pub const RESPONSES: &str = "https://api.openai.com/v1/responses";

pub async fn stream(
    post: RequestBuilder,
    db: &Database,
    request: Request,
    token: &tokio_util::sync::CancellationToken,
    channel: &Channel<StreamEvent>,
    reply: &mut String,
) -> Result<()> {
    stream_into(post, db, request, token, channel, reply, false).await
}

pub async fn stream_into(
    post: RequestBuilder,
    db: &Database,
    request: Request,
    token: &tokio_util::sync::CancellationToken,
    channel: &Channel<StreamEvent>,
    reply: &mut String,
    agent: bool,
) -> Result<()> {
    let id = request.id;
    let transfer = async {
        let response = post
            .header("Accept", "text/event-stream")
            .json(&json!({"model":request.model,"input":request.input,"stream":true,"store":false}))
            .send()
            .await?;
        if !response.status().is_success() {
            let status = response.status();
            let body: Value = response.json().await.unwrap_or_default();
            let message = body
                .pointer("/error/message")
                .or_else(|| body.get("detail"))
                .and_then(Value::as_str)
                .unwrap_or("Request rejected.");
            let detail = api_message(body.pointer("/error/code").and_then(Value::as_str), message);
            return Err(invalid(format!("OpenAI {status}: {detail}")));
        }
        let mut events = response.bytes_stream().eventsource();
        let mut saved = Instant::now();
        while let Some(event) = events.next().await {
            let event = event.map_err(|e| invalid(format!("Stream interrupted: {e}")))?;
            match frame(&event.data)? {
                Frame::Text(text) => {
                    reply.push_str(&text);
                    channel.send(if agent {
                        StreamEvent::AgentDelta { id, text }
                    } else {
                        StreamEvent::Delta { text }
                    })?;
                    if saved.elapsed() >= Duration::from_millis(800) {
                        let snapshot = reply.clone();
                        let id = request.id;
                        db.run(move |db| {
                            db.execute(
                                if agent {"UPDATE debate_entries SET text=? WHERE id=? AND status='streaming'"} else {"UPDATE turns SET reply=? WHERE id=? AND status='streaming'"},
                                rusqlite::params![snapshot, id],
                            )?;
                            Ok(())
                        })
                        .await?;
                        saved = Instant::now();
                    }
                }
                Frame::Terminal {
                    model,
                    usage,
                    outcome,
                } => {
                    if let Some(model) = model {
                        let stored = model.clone();
                        db.run(move |db| {
                            if agent {
                                db.execute(
                                    "INSERT OR REPLACE INTO entry_models(entry, model) VALUES(?,?)",
                                    rusqlite::params![id, stored],
                                )?;
                            } else {
                                db.execute(
                                    "UPDATE turns SET model=? WHERE id=?",
                                    rusqlite::params![stored, id],
                                )?;
                            }
                            Ok(())
                        })
                        .await?;
                        channel.send(StreamEvent::Model { id, model, agent })?;
                    }
                    if let Some((input, output)) = usage {
                        let id = request.id;
                        db.run(move |db| {
                            db.execute(if agent {"UPDATE debate_usage SET input_tokens=?, output_tokens=? WHERE id=?"} else {"UPDATE turn_usage SET input_tokens=?, output_tokens=? WHERE turn=?"}, [input, output, id])?;
                            Ok(())
                        }).await?;
                    }
                    return outcome;
                }
                Frame::Skip => {}
            }
        }
        Err(invalid(
            "Connection closed before the API completed the reply.",
        ))
    };
    token
        .run_until_cancelled(transfer)
        .await
        .ok_or(crate::error::Error::Cancelled)?
}
