import { useState } from 'react';
import type { Chat } from '../ipc';
import { Icon } from './Icon';

export function ChatHistory({
  chats,
  active,
  busy,
  select,
}: {
  chats: Chat[];
  active: number | null;
  busy: boolean;
  select: (id: number) => void;
}) {
  const [query, setQuery] = useState('');
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  const groups: Record<string, Chat[]> = {
    Today: [],
    Yesterday: [],
    Earlier: [],
  };
  const needle = query.trim().toLocaleLowerCase();
  for (const chat of chats) {
    if (!chat.title.toLocaleLowerCase().includes(needle)) continue;
    const time = chat.touched * 1000;
    groups[
      time >= today.getTime()
        ? 'Today'
        : time >= yesterday.getTime()
          ? 'Yesterday'
          : 'Earlier'
    ].push(chat);
  }
  return (
    <>
      <label className="history-search">
        <Icon name="search" />
        <input
          type="search"
          aria-label="Find a conversation"
          placeholder="Find a conversation"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </label>
      <nav aria-label="Conversations">
        {Object.entries(groups)
          .filter(([, items]) => items.length)
          .map(([label, items]) => (
            <section key={label}>
              <h2 className="nav-label">{label}</h2>
              {items.map((chat) => (
                <button
                  key={chat.id}
                  disabled={busy}
                  className={chat.id === active ? 'selected' : ''}
                  aria-current={chat.id === active ? 'page' : undefined}
                  title={chat.title}
                  onClick={() => select(chat.id)}
                >
                  <Icon name="chat" />
                  <span>{chat.title}</span>
                </button>
              ))}
            </section>
          ))}
        {Object.values(groups).every((items) => !items.length) && (
          <p className="nav-empty">
            {chats.length ? 'No matching chats' : 'No chats yet'}
          </p>
        )}
      </nav>
    </>
  );
}
