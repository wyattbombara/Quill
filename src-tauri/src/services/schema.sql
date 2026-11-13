PRAGMA journal_mode = WAL;
CREATE TABLE IF NOT EXISTS chats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    title TEXT NOT NULL,
    touched INTEGER NOT NULL DEFAULT (unixepoch())
);
CREATE TABLE IF NOT EXISTS turns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    chat INTEGER NOT NULL REFERENCES chats(id) ON DELETE CASCADE,
    prompt TEXT NOT NULL,
    reply TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('streaming','complete','stopped','failed','interrupted')),
    error TEXT
);
CREATE INDEX IF NOT EXISTS turns_by_chat ON turns(chat,id);
CREATE UNIQUE INDEX IF NOT EXISTS one_reply ON turns(chat) WHERE status='streaming';
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    turn INTEGER NOT NULL REFERENCES turns(id) ON DELETE CASCADE,
    name TEXT NOT NULL, mime TEXT NOT NULL, data TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS files_by_turn ON files(turn,id);
CREATE TABLE IF NOT EXISTS preferences (id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS chat_goals (
    chat INTEGER PRIMARY KEY REFERENCES chats(id) ON DELETE CASCADE,
    goal TEXT NOT NULL
);
-- Keep only anonymous counters when a conversation is deleted.
CREATE TABLE IF NOT EXISTS turn_usage (
    turn INTEGER PRIMARY KEY, chat INTEGER NOT NULL,
    input_tokens INTEGER, output_tokens INTEGER
);
CREATE INDEX IF NOT EXISTS usage_by_chat ON turn_usage(chat);
CREATE TABLE IF NOT EXISTS debates (
    turn INTEGER PRIMARY KEY REFERENCES turns(id) ON DELETE CASCADE, models TEXT NOT NULL
);
-- Counters survive deletion; the public transcript cascades with the conversation.
CREATE TABLE IF NOT EXISTS debate_usage (
    id INTEGER PRIMARY KEY AUTOINCREMENT, chat INTEGER NOT NULL, input_tokens INTEGER, output_tokens INTEGER
);
CREATE INDEX IF NOT EXISTS debate_usage_by_chat ON debate_usage(chat);
CREATE TABLE IF NOT EXISTS debate_entries (
    id INTEGER PRIMARY KEY, turn INTEGER NOT NULL REFERENCES debates(turn) ON DELETE CASCADE,
    seat INTEGER NOT NULL, phase TEXT NOT NULL, text TEXT NOT NULL DEFAULT '', status TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS entry_models (entry INTEGER PRIMARY KEY REFERENCES debate_entries(id) ON DELETE CASCADE, model TEXT NOT NULL);
CREATE INDEX IF NOT EXISTS debate_entries_by_turn ON debate_entries(turn,id);
UPDATE debate_entries SET status='interrupted' WHERE status='streaming';
INSERT OR IGNORE INTO turn_usage(turn,chat) SELECT id,chat FROM turns WHERE id NOT IN (SELECT turn FROM debates);
UPDATE turns SET status='interrupted' WHERE status='streaming';
CREATE TABLE IF NOT EXISTS oauth_accounts (
    id TEXT PRIMARY KEY, subject TEXT NOT NULL, label TEXT NOT NULL,
    session BLOB, permitted INTEGER NOT NULL, welcomed INTEGER NOT NULL DEFAULT 0,
    models TEXT NOT NULL DEFAULT '[]', model TEXT
);
CREATE TABLE IF NOT EXISTS oauth_host (
    singleton INTEGER PRIMARY KEY CHECK(singleton=1), host TEXT NOT NULL,
    active TEXT REFERENCES oauth_accounts(id)
);

-- Dates and billing registrations for new requests. Legacy counters have no known start time.
CREATE TABLE IF NOT EXISTS account_plans (
    account TEXT PRIMARY KEY REFERENCES oauth_accounts(id), tier TEXT
);
CREATE TABLE IF NOT EXISTS turn_accounts (
    turn INTEGER PRIMARY KEY REFERENCES turns(id) ON DELETE CASCADE, account TEXT
);
CREATE TRIGGER IF NOT EXISTS capture_turn_account AFTER INSERT ON turns BEGIN
    INSERT INTO turn_accounts VALUES(NEW.id,(SELECT active FROM oauth_host WHERE singleton=1));
END;
CREATE TABLE IF NOT EXISTS request_times (
    agent INTEGER NOT NULL, request INTEGER NOT NULL, account TEXT,
    started INTEGER NOT NULL DEFAULT (unixepoch()), PRIMARY KEY(agent,request)
);
CREATE INDEX IF NOT EXISTS times_by_account ON request_times(account,started);
CREATE INDEX IF NOT EXISTS times_by_date ON request_times(started);
CREATE TRIGGER IF NOT EXISTS date_single AFTER INSERT ON turn_usage BEGIN
    INSERT INTO request_times(agent,request,account)
    VALUES(0,NEW.turn,(SELECT account FROM turn_accounts WHERE turn=NEW.turn));
END;
CREATE TRIGGER IF NOT EXISTS date_agent AFTER INSERT ON debate_entries BEGIN
    INSERT INTO request_times(agent,request,account)
    VALUES(1,NEW.id,(SELECT account FROM turn_accounts WHERE turn=NEW.turn));
END;
