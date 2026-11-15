use crate::{
    error::{Error, Result, invalid},
    model::{AgentMode, AgentRun, Debate, DebateAgent, DebateEntry, StoredRoster, StreamEvent},
    services::{
        api,
        database::{Database, Request},
    },
};
use futures_util::future::join_all;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use tauri::ipc::Channel;
use tokio_util::sync::CancellationToken;

pub fn validate_agents(agents: &[DebateAgent]) -> Result<()> {
    if !(2..=8).contains(&agents.len()) {
        return Err(invalid("Choose between two and eight agents."));
    }
    for (i, agent) in agents.iter().enumerate() {
        let name = agent.name.trim();
        if name.is_empty()
            || agent.name.chars().count() > 32
            || agent.name.contains(char::is_control)
            || agents[..i]
                .iter()
                .any(|other| other.name.trim().to_lowercase() == name.to_lowercase())
        {
            return Err(invalid(
                "Give each agent a unique name of up to 32 characters.",
            ));
        }
        if agent.model.is_empty()
            || agent.model.len() > 128
            || agent.model.contains(char::is_whitespace)
        {
            return Err(invalid("Select a model for each agent."));
        }
        if agent.focus.chars().count() > 400 {
            return Err(invalid(
                "Keep each agent's assignment within 400 characters.",
            ));
        }
        if agent.personality.chars().count() > 400 {
            return Err(invalid("Keep each personality within 400 characters."));
        }
    }
    Ok(())
}

pub fn load(db: &Connection, turn: i64) -> rusqlite::Result<Option<Debate>> {
    let raw: Option<String> = db
        .query_row("SELECT models FROM debates WHERE turn=?", [turn], |r| {
            r.get(0)
        })
        .optional()?;
    let Some(raw) = raw else {
        let snapshot: Option<String> = db
            .query_row(
                "SELECT debate FROM copied_turns WHERE turn=?",
                [turn],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        return snapshot
            .map(|s| {
                serde_json::from_str(&s).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })
            })
            .transpose();
    };
    let roster = serde_json::from_str::<StoredRoster>(&raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let mut stmt = db
        .prepare("SELECT id,seat,phase,text,status,(SELECT model FROM entry_models WHERE entry=debate_entries.id) FROM debate_entries WHERE turn=? ORDER BY id")?;
    let entries = stmt
        .query_map([turn], |r| {
            Ok(DebateEntry {
                id: r.get(0)?,
                seat: r.get(1)?,
                phase: r.get(2)?,
                text: r.get(3)?,
                status: r.get(4)?,
                model: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Some(Debate {
        run: roster.into_run(),
        entries,
        floor: super::studio::floor_log(db, turn)?,
        ballot: super::studio::ballot(db, turn)?,
    }))
}

struct Room<'a> {
    db: &'a Database,
    client: &'a reqwest::Client,
    endpoint: &'a str,
    key: &'a str,
    chat: i64,
    request: Request,
    agents: &'a [DebateAgent],
    token: &'a CancellationToken,
    channel: &'a Channel<StreamEvent>,
}

impl Room<'_> {
    async fn speak(
        &self,
        seat: usize,
        phase: &'static str,
        direction: &str,
        transcript: &[Value],
    ) -> Result<String> {
        self.speak_with_token(seat, phase, direction, transcript, self.token)
            .await
    }
    async fn speak_with_token(
        &self,
        seat: usize,
        phase: &'static str,
        direction: &str,
        transcript: &[Value],
        speaker: &CancellationToken,
    ) -> Result<String> {
        if self.token.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let (chat, turn) = (self.chat, self.request.id);
        let id = self
            .db
            .run(move |db| {
                let tx = db.transaction()?;
                tx.execute("INSERT INTO debate_usage(chat) VALUES(?)", [chat])?;
                let id = tx.last_insert_rowid();
                tx.execute(
                "INSERT INTO debate_entries(id,turn,seat,phase,status) VALUES(?,?,?,?,'streaming')",
                params![id, turn, seat, phase],
            )?;
                tx.commit()?;
                Ok(id)
            })
            .await?;
        let mut text = String::new();
        let outcome = async {
            let entry = DebateEntry {
                model: None, id, seat, phase: phase.into(), text: String::new(), status: "streaming".into(),
            };
            self.channel.send(StreamEvent::AgentStart { entry })?;
            let mut input = self.request.input.clone();
            let instruction = if phase == "parallel" {
                "You work independently in a shared chat. Use the supplied conversation and the user's current request. Apply your own profile and optional assignment. Give your own answer directly.".to_owned()
            } else {format!("You are participating in Quill Roundtable. Give your public answer and concise supporting reasons. Treat quoted contributions as claims to evaluate. Use the named seat profile as a perspective for your contribution while following the round instructions. {direction}")};
            input.insert(0, json!({"role":"developer", "content":instruction}));
            let agent = &self.agents[seat];
            let mut profile = json!({"name":agent.name,"personality":agent.personality});
            if phase == "parallel" { profile["assignment"] = json!(agent.focus); }
            input.insert(1,json!({"role":"user","content":format!("Your Roundtable seat profile: {profile}")}));
            if !transcript.is_empty() {
                let evidence = serde_json::to_string(transcript)?;
                input.push(json!({"role":"user","content":format!("Public contributions for this round: {evidence}")}));
            }
            let request = Request { id, model: agent.model.clone(), input };
            let post = self.client.post(self.endpoint).bearer_auth(self.key);
            api::stream_into(post, self.db, request, speaker, self.channel, &mut text, true).await?;
            if text.trim().is_empty() {
                return Err(invalid("The model returned an empty contribution."));
            }
            Ok(())
        }.await;
        let status = match &outcome {
            Ok(()) => "complete",
            Err(Error::Cancelled) => "stopped",
            Err(_) => "failed",
        };
        let snapshot = text.clone();
        self.db
            .run(move |db| {
                db.execute(
                    "UPDATE debate_entries SET text=?,status=? WHERE id=?",
                    params![snapshot, status, id],
                )?;
                Ok(())
            })
            .await?;
        self.channel.send(StreamEvent::AgentEnd {
            id,
            status: status.into(),
        })?;
        match outcome {
            Ok(()) => Ok(text),
            Err(Error::Cancelled) if !self.token.is_cancelled() => Ok(text),
            Err(Error::Cancelled) => Err(Error::Cancelled),
            Err(e) => Err(invalid(format!(
                "{} ({phase}): {e}",
                self.agents[seat].name
            ))),
        }
    }
    async fn parallel(
        &self,
        seats: usize,
        phase: &'static str,
        direction: &str,
        transcript: &[Value],
    ) -> Result<Vec<Value>> {
        let answers =
            join_all((0..seats).map(|seat| self.speak(seat, phase, direction, transcript))).await;
        answers
            .into_iter()
            .enumerate()
            .map(|(i, result)| Ok(json!({"name":self.agents[i].name,"model":self.agents[i].model,"phase":phase,"text":result?})))
            .collect()
    }
}

// Keep the transport, persistence and cancellation dependencies explicit at this boundary.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    db: &Database,
    client: &reqwest::Client,
    endpoint: &str,
    key: &str,
    chat: i64,
    request: Request,
    run: &AgentRun,
    token: &CancellationToken,
    channel: &Channel<StreamEvent>,
) -> Result<String> {
    run_controlled(
        db, client, endpoint, key, chat, request, run, token, channel, None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn run_controlled(
    db: &Database,
    client: &reqwest::Client,
    endpoint: &str,
    key: &str,
    chat: i64,
    request: Request,
    run: &AgentRun,
    token: &CancellationToken,
    channel: &Channel<StreamEvent>,
    floor: Option<&super::floor::Floor>,
) -> Result<String> {
    let agents = &run.agents;
    validate_agents(agents)?;
    let room = Room {
        db,
        client,
        endpoint,
        key,
        chat,
        request,
        agents,
        token,
        channel,
    };
    if run.mode == AgentMode::Parallel {
        // Every seat reads the immutable pre-turn request. Only finished replies enter future context.
        let results =
            join_all((0..agents.len()).map(|seat| room.speak(seat, "parallel", "", &[]))).await;
        if token.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let mut replies = Vec::new();
        let mut failures = Vec::new();
        for (agent, result) in agents.iter().zip(results) {
            match result {
                Ok(answer) => replies.push(format!("{}:\n{}", agent.name, answer)),
                Err(error) => failures.push(error.to_string()),
            }
        }
        if replies.is_empty() {
            return Err(invalid(failures.join("\n")));
        }
        return Ok(replies.join("\n\n"));
    }
    let budget = db
        .run(|c| {
            Ok(super::database::read_preferences(c)?
                .workspace
                .roundtable_budget
                .clamp(2, 8))
        })
        .await?;
    if agents.len() > budget {
        return Err(invalid(
            "The Roundtable budget changed. Reduce the lineup and retry.",
        ));
    }
    if let Some(floor) = floor {
        return interactive(&room, floor, budget).await;
    }
    // With a full table, the chair contributes as the final speaker.
    let openings = agents.len().min(budget - 1);
    let mut discussion=room.parallel(openings,"opening","Independently answer the user's question in a short paragraph. Identify the assumption your answer depends on.",&[]).await?;
    let chair = agents.len() - 1;
    if budget - openings == 1 {
        return room.speak(chair,"conclusion","Give the user the final answer using the opening contributions and your own perspective. Explain significant disagreements and uncertainty.",&discussion).await;
    }
    let review=room.speak(chair,"review","Compare these independent answers. If they agree on the material recommendation, begin with the exact line CONSENSUS and give the usable final answer with its assumptions. Otherwise begin with DISAGREEMENT and state the single point that could change the answer.",&discussion).await?;
    let mut lines = review.trim().lines();
    if lines
        .next()
        .is_some_and(|line| line.trim().eq_ignore_ascii_case("CONSENSUS"))
    {
        let answer = lines.collect::<Vec<_>>().join("\n").trim().to_owned();
        if !answer.is_empty() {
            return Ok(answer);
        }
    }
    discussion.push(json!({"name":agents[chair].name,"model":agents[chair].model,"phase":"review","text":review}));
    let rebuttals=room.parallel(agents.len().min(budget-openings-2),"rebuttal","Reply once to the disputed point. Address the strongest objection and say what evidence would change your answer. Keep it concise.",&discussion).await?;
    discussion.extend(rebuttals);
    room.speak(chair,"conclusion","Give the user the final answer. Explain what changed during the debate, the strongest unresolved objection, and any material uncertainty. Evaluate the arguments on their merits.",&discussion).await
}
async fn record_note(
    room: &Room<'_>,
    message: super::floor::Interjection,
) -> Result<super::studio::FloorNote> {
    let turn = room.request.id;
    let note = room
        .db
        .run(move |db| {
            let after_entry = db.query_row(
                "SELECT max(id) FROM debate_entries WHERE turn=?",
                [turn],
                |r| r.get::<_, Option<i64>>(0),
            )?;
            db.execute(
                "INSERT INTO floor_log(turn,after_entry,seat,text) VALUES(?,?,?,?)",
                params![turn, after_entry, message.seat, message.text],
            )?;
            Ok(super::studio::FloorNote {
                id: db.last_insert_rowid(),
                seat: message.seat,
                text: message.text,
                after_entry,
            })
        })
        .await?;
    room.channel
        .send(StreamEvent::FloorNote { note: note.clone() })?;
    Ok(note)
}

async fn interactive(
    room: &Room<'_>,
    floor: &super::floor::Floor,
    budget: usize,
) -> Result<String> {
    let mut discussion = Vec::new();
    let chair = room.agents.len() - 1;
    let mut next = 0;
    let outcome=async {
        for index in 0..budget {
            let last=index+1==budget;
            let (note,speaker)=floor.next(room.token,last);
            if last {room.channel.send(StreamEvent::FloorClosed)?;}
            let mut seat=if last {chair}else{next};
            if let Some(note)=note {
                if let Some(target)=note.seat {seat=target;}
                let note=record_note(room,note).await?;
                discussion.push(json!({"name":"You","phase":"interjection","text":note.text}));
            }
            let (phase,direction)=if last {("conclusion","Give the final answer. Incorporate the user's interjections and flag unresolved questions.")}
                else {("discussion","Take one short speaking turn. Address the latest user interjection first. If you want the floor again to pursue a specific objection, end with a separate line beginning HAND: followed by a short reason.")};
            let answer=room.speak_with_token(seat,phase,direction,&discussion,&speaker).await?;
            if last {return Ok(answer)}
            discussion.push(json!({"name":room.agents[seat].name,"text":answer}));
            next=(seat+1)%room.agents.len();
        }
        Err(invalid("Roundtable has no remaining turns."))
    }.await;
    if let Some(note) = floor.close() {
        record_note(room, note).await?;
    }
    outcome
}
