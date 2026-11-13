import type { PaletteName } from './palettes';
import { Channel, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface Chat {
  touched: number;
  id: number;
  title: string;
  goal: string | null;
}
export interface Usage {
  requests: number;
  reported: number;
  inputTokens: number;
  outputTokens: number;
}
export interface UsageReport {
  today?: Usage;
  chat: Usage;
  device: Usage;
}
export type AgentMode = 'roundtable' | 'parallel';
export interface DebateEntry {
  model?: string | null;
  id: number;
  seat: number;
  phase: 'parallel' | 'opening' | 'review' | 'rebuttal' | 'conclusion';
  text: string;
  status: 'streaming' | 'complete' | 'failed' | 'interrupted' | 'stopped';
}
export interface DebateAgent {
  name: string;
  model: string;
  personality: string;
  focus?: string;
}
export interface Debate {
  mode: AgentMode;
  agents: DebateAgent[];
  entries: DebateEntry[];
}
export interface Turn {
  debate?: Debate;
  id: number;
  prompt: string;
  reply: string;
  files: string[];
  model: string;
  status: 'complete' | 'streaming' | 'stopped' | 'failed' | 'interrupted';
  error: string | null;
}
export interface WorkspaceOptions {
  resume_chat: boolean;
  restore_drafts: boolean;
  quit_on_close: boolean;
  reply_notifications: boolean;
  preview_notifications: boolean;
  follow_replies: boolean;
  reply_size: number;
  code_size: number;
  roundtable_budget: number;
  composer_usage: 'hidden' | 'chat' | 'today';
}
export const workspaceDefaults: WorkspaceOptions = {
  resume_chat: true,
  restore_drafts: false,
  quit_on_close: false,
  reply_notifications: false,
  preview_notifications: false,
  follow_replies: true,
  reply_size: 16,
  code_size: 13,
  roundtable_budget: 8,
  composer_usage: 'hidden',
};
export interface Preferences {
  workspace?: WorkspaceOptions;
  compact?: boolean;
  wide_reading?: boolean;
  hide_inspector?: boolean;
  modifier_send?: boolean;
  palette: PaletteName;
  sidebar_logo: boolean;
  model: string;
  theme: 'light' | 'dark' | 'system';
  motion: 'system' | 'smooth' | 'reduced';
  font: 'quill' | 'system' | 'serif' | 'mono';
  shortcut: string;
}
export interface Attachment {
  name: string;
  mime: string;
  data: string;
}
export type StreamEvent =
  | { type: 'started'; id: number; model: string }
  | { type: 'model'; id: number; model: string; agent: boolean }
  | { type: 'delta'; text: string }
  | { type: 'agentStart'; entry: DebateEntry }
  | { type: 'agentDelta'; id: number; text: string }
  | { type: 'agentEnd'; id: number; status: DebateEntry['status'] };
export type UpdateStatus =
  | { type: 'current' }
  | { type: 'unconfigured' }
  | { type: 'available'; version: string };
type None = Record<string, never>;
export interface AuthStatus {
  parallel?: { seats: number; tier: string; recent: number };
  active: string | null;
  accounts: {
    id: string;
    label: string;
    connected: boolean;
    permitted: boolean;
  }[];
  models: { slug: string; display_name: string }[];
  model: string | null;
  welcome: boolean;
}
export const manageUsage = () =>
  call('open_link', { url: 'https://chatgpt.com/settings/usage' });
type Commands = {
  auth_status: [None, AuthStatus];
  chatgpt_login: [{ id: string | null }, void];
  cancel_login: [None, void];
  chatgpt_logout: [{ id: string }, boolean];
  use_api_key: [None, void];
  plan_models: [None, void];
  select_plan_model: [{ id: string; model: string }, void];
  welcome_seen: [{ id: string }, void];
  set_chat_goal: [{ chat: number | null; goal: string | null }, Chat];
  get_usage: [{ chat: number | null }, UsageReport];
  show_chat_window: [None, void];
  open_link: [{ url: string }, void];
  list_chats: [None, Chat[]];
  create_chat: [None, Chat];
  rename_chat: [{ chat: number; title: string }, void];
  delete_chat: [{ chat: number }, void];
  read_attachment: [
    { chat: number; turn: number; position: number },
    Attachment,
  ];
  list_turns: [{ chat: number }, Turn[]];
  debate_models: [{ account: string | null }, string[]];
  send_message: [
    {
      chat: number;
      prompt: string;
      files: Attachment[];
      channel: Channel<StreamEvent>;
      debate?: {
        account: string | null;
        mode: AgentMode;
        agents: DebateAgent[];
      };
    },
    void,
  ];
  stop_reply: [{ id: number }, void];
  get_preferences: [None, Preferences];
  save_preferences: [{ preferences: Preferences }, void];
  shortcut_active: [None, boolean];
  key_present: [None, boolean];
  save_key: [{ key: string }, void];
  remove_key: [None, void];
  check_update: [None, UpdateStatus];
  install_update: [{ version: string }, void];
};
export function call<K extends keyof Commands>(
  name: K,
  args: Commands[K][0],
): Promise<Commands[K][1]> {
  return invoke<Commands[K][1]>(name, args);
}
export function onNewChat(handler: () => void) {
  return listen<null>('new-chat', handler);
}
export function streamChannel(handler: (event: StreamEvent) => void) {
  const channel = new Channel<StreamEvent>();
  channel.onmessage = handler;
  return channel;
}
