use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
pub struct Chat {
    pub touched: i64,
    pub id: i64,
    pub title: String,
    pub goal: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub requests: i64,
    pub reported: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
}

#[derive(Serialize)]
pub struct UsageReport {
    pub today: Usage,
    pub chat: Usage,
    pub device: Usage,
}

#[derive(Serialize)]
pub struct Turn {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debate: Option<Debate>,
    pub id: i64,
    pub prompt: String,
    pub reply: String,
    pub files: Vec<String>,
    pub model: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DebateAgent {
    pub name: String,
    pub model: String,
    pub personality: String,
    #[serde(default)]
    pub focus: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentMode {
    #[default]
    Roundtable,
    Parallel,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct AgentRun {
    #[serde(default)]
    pub mode: AgentMode,
    pub agents: Vec<DebateAgent>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum StoredRoster {
    Run(AgentRun),
    Agents(Vec<DebateAgent>),
    Models(Vec<String>),
}

impl StoredRoster {
    pub fn into_run(self) -> AgentRun {
        let agents = match self {
            Self::Run(run) => return run,
            Self::Agents(agents) => agents,
            Self::Models(models) => models
                .into_iter()
                .enumerate()
                .map(|(i, model)| DebateAgent {
                    name: ["Finch", "Moss", "Pip"]
                        .get(i)
                        .map_or_else(|| format!("Agent {}", i + 1), |n| n.to_string()),
                    model,
                    personality: String::new(),
                    focus: String::new(),
                })
                .collect(),
        };
        AgentRun {
            mode: AgentMode::Roundtable,
            agents,
        }
    }
}

#[derive(Deserialize)]
pub struct DebateOptions {
    #[serde(default)]
    pub interactive: bool,
    #[serde(default)]
    pub blind: bool,
    #[serde(default)]
    pub category: Option<String>,
    pub account: Option<String>,
    #[serde(flatten)]
    pub run: AgentRun,
}

#[derive(Deserialize, Serialize)]
pub struct Debate {
    #[serde(flatten)]
    pub run: AgentRun,
    pub entries: Vec<DebateEntry>,
    #[serde(default)]
    pub floor: Vec<crate::services::studio::FloorNote>,
    #[serde(default)]
    pub ballot: Option<crate::services::studio::Ballot>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct DebateEntry {
    pub model: Option<String>,
    pub id: i64,
    pub seat: usize,
    pub phase: String,
    pub text: String,
    pub status: String,
}

#[derive(Deserialize, Serialize)]
pub struct Attachment {
    pub name: String,
    pub mime: String,
    pub data: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}
impl Theme {
    pub fn native(&self) -> Option<tauri::Theme> {
        match self {
            Self::System => None,
            Self::Light => Some(tauri::Theme::Light),
            Self::Dark => Some(tauri::Theme::Dark),
        }
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Motion {
    #[default]
    System,
    Smooth,
    Reduced,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceFont {
    #[default]
    Quill,
    System,
    Serif,
    Mono,
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub enum Palette {
    #[default]
    #[serde(rename = "Violet / clay")]
    VioletClay,
    #[serde(rename = "Paper / ink / violet")]
    PaperInkViolet,
    #[serde(rename = "Cobalt / apricot")]
    CobaltApricot,
    #[serde(rename = "Moss / parchment")]
    MossParchment,
    #[serde(rename = "Rosewood / oat")]
    RosewoodOat,
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ComposerUsage {
    #[default]
    Hidden,
    Chat,
    Today,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceOptions {
    pub resume_chat: bool,
    pub restore_drafts: bool,
    pub reply_notifications: bool,
    pub preview_notifications: bool,
    pub quit_on_close: bool,
    pub follow_replies: bool,
    pub reply_size: u8,
    pub code_size: u8,
    pub roundtable_budget: usize,
    pub composer_usage: ComposerUsage,
}
impl Default for WorkspaceOptions {
    fn default() -> Self {
        Self {
            resume_chat: true,
            restore_drafts: false,
            reply_notifications: false,
            preview_notifications: false,
            quit_on_close: false,
            follow_replies: true,
            reply_size: 16,
            code_size: 13,
            roundtable_budget: 8,
            composer_usage: ComposerUsage::Hidden,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(default)]
    pub workspace: WorkspaceOptions,
    #[serde(default)]
    pub compact: bool,
    #[serde(default)]
    pub wide_reading: bool,
    #[serde(default)]
    pub hide_inspector: bool,
    #[serde(default)]
    pub modifier_send: bool,
    pub model: String,
    pub theme: Theme,
    #[serde(default)]
    pub palette: Palette,
    #[serde(default)]
    pub sidebar_logo: bool,
    #[serde(default)]
    pub motion: Motion,
    #[serde(default)]
    pub font: InterfaceFont,
    pub shortcut: String,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            workspace: WorkspaceOptions::default(),
            model: "gpt-4.1-mini".into(),
            theme: Theme::System,
            palette: Palette::default(),
            sidebar_logo: false,
            motion: Motion::System,
            font: InterfaceFont::System,
            compact: false,
            wide_reading: false,
            hide_inspector: false,
            modifier_send: false,
            shortcut: "Ctrl+Alt+Space".into(),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StreamEvent {
    Model {
        id: i64,
        model: String,
        agent: bool,
    },
    Started {
        id: i64,
        model: String,
        ballot: Option<crate::services::studio::Ballot>,
    },
    #[serde(rename = "floorNote")]
    FloorNote {
        note: crate::services::studio::FloorNote,
    },
    #[serde(rename = "floorClosed")]
    FloorClosed,
    Delta {
        text: String,
    },
    #[serde(rename = "agentStart")]
    AgentStart {
        entry: DebateEntry,
    },
    #[serde(rename = "agentDelta")]
    AgentDelta {
        id: i64,
        text: String,
    },
    #[serde(rename = "agentEnd")]
    AgentEnd {
        id: i64,
        status: String,
    },
}
