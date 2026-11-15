use crate::error::{Result, invalid};
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

pub struct Interjection {
    pub seat: Option<usize>,
    pub text: String,
}
struct State {
    closed: bool,
    pending: Option<Interjection>,
    speaker: Option<CancellationToken>,
}
pub struct Floor {
    seats: usize,
    state: Mutex<State>,
}
impl Floor {
    pub fn new(seats: usize) -> Self {
        Self {
            seats,
            state: Mutex::new(State {
                closed: false,
                pending: None,
                speaker: None,
            }),
        }
    }
    pub fn interject(&self, seat: Option<usize>, text: String, now: bool) -> Result<()> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > 2000 || seat.is_some_and(|s| s >= self.seats) {
            return Err(invalid(
                "Choose an available seat and enter 1–2,000 characters.",
            ));
        }
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if s.closed {
            return Err(invalid("The final speaker has the floor."));
        }
        if s.pending.is_some() {
            return Err(invalid("An interjection is already queued."));
        }
        s.pending = Some(Interjection {
            seat,
            text: text.into(),
        });
        if now && let Some(token) = &s.speaker {
            token.cancel()
        }
        Ok(())
    }
    pub fn next(
        &self,
        token: &CancellationToken,
        last: bool,
    ) -> (Option<Interjection>, CancellationToken) {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let speaker = token.child_token();
        s.speaker = Some(speaker.clone());
        s.closed = last;
        (s.pending.take(), speaker)
    }
    pub fn close(&self) -> Option<Interjection> {
        let mut s = self.state.lock().unwrap_or_else(|e| e.into_inner());
        s.closed = true;
        s.pending.take()
    }
}
