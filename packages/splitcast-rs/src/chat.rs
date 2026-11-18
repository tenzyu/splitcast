//! Session-local chat history and the bounded finite-HTTP delta.
use serde_json::{json, Map, Value};
use std::collections::VecDeque;
use std::io;

const RETAIN: usize = 128;
const PAGE: usize = 32;
const MAX_CURSOR: u64 = (1 << 53) - 1;

#[derive(Clone, Debug, PartialEq)]
pub struct Cursor {
    epoch: String,
    after: u64,
}

impl Cursor {
    pub(crate) fn parse(args: &Map<String, Value>) -> io::Result<Option<Self>> {
        match (args.get("epoch"), args.get("after")) {
            (None, None) => Ok(None),
            (Some(epoch), Some(after)) => {
                let epoch = epoch.as_str().filter(|epoch| {
                    epoch.len() == 32 && epoch.bytes().all(|byte| byte.is_ascii_hexdigit())
                });
                let after = after.as_u64().filter(|after| *after <= MAX_CURSOR);
                match (epoch, after) {
                    (Some(epoch), Some(after)) => Ok(Some(Self {
                        epoch: epoch.into(),
                        after,
                    })),
                    _ => Err(invalid_cursor()),
                }
            }
            _ => Err(invalid_cursor()),
        }
    }

    pub(crate) fn args(&self) -> Value {
        json!({"epoch": self.epoch, "after": self.after})
    }
}

fn invalid_cursor() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "invalid chat cursor")
}

pub(crate) struct History {
    epoch: String,
    last: u64,
    messages: VecDeque<Entry>,
}

struct Entry {
    cursor: u64,
    id: String,
    display_name: String,
    text: String,
}

fn display(text: &str, limit: usize) -> String {
    let mut bounded = String::new();
    for ch in text.chars().filter(|ch| !ch.is_control()) {
        if bounded.len() + ch.len_utf8() > limit {
            break;
        }
        bounded.push(ch);
    }
    bounded
}

impl History {
    pub(crate) fn new(epoch: String) -> Self {
        Self {
            epoch,
            last: 0,
            messages: VecDeque::with_capacity(RETAIN),
        }
    }

    pub(crate) fn push(&mut self, id: &str, name: &str, text: &str) -> io::Result<()> {
        if self.last == MAX_CURSOR {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "chat cursor exhausted; prepare a new session",
            ));
        }
        self.last += 1;
        if self.messages.len() == RETAIN {
            self.messages.pop_front();
        }
        self.messages.push_back(Entry {
            cursor: self.last,
            id: id.into(),
            display_name: display(name, 128),
            text: display(text, 512),
        });
        Ok(())
    }

    pub(crate) fn page(&self, requested: Option<&Cursor>, subscription: Value) -> Value {
        let oldest = self
            .messages
            .front()
            .map_or(self.last, |entry| entry.cursor - 1);
        let reset =
            requested.is_some_and(|cursor| cursor.epoch != self.epoch || cursor.after > self.last);
        let overrun = !reset && requested.is_some_and(|cursor| cursor.after < oldest);
        let after = requested
            .filter(|_| !reset && !overrun)
            .map_or(oldest, |cursor| cursor.after);
        let mut cursor = after;
        let messages: Vec<_> = self.messages.iter().filter(|entry| entry.cursor > after).take(PAGE).map(|entry| {
            cursor = entry.cursor;
            json!({"cursor":entry.cursor,"id":entry.id,"display_name":entry.display_name,"text":entry.text})
        }).collect();
        json!({
            "enabled":true,"epoch":self.epoch,"cursor":cursor,"more":cursor < self.last,
            "reset":reset,"overrun":overrun,"subscription":subscription,"messages":messages
        })
    }
}

pub(crate) fn inactive(enabled: bool, requested: Option<&Cursor>) -> String {
    json!({
        "enabled":enabled,"epoch":null,"cursor":0,"more":false,
        "reset":requested.is_some(),"overrun":false,"subscription":null,"messages":[]
    })
    .to_string()
}
