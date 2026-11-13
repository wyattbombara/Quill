import { readTeams, writeTeams } from '../workspaceStorage';
import { shortModel } from '../models';
import { useLayoutEffect, useRef, useState } from 'react';
import type { AgentMode, Debate, DebateAgent, Turn } from '../ipc';
import { modelGroups } from '../models';

const names = ['Finch', 'Moss', 'Pip', 'Rook', 'Wren', 'Cedar', 'Fern', 'Lark'];
const phases = {
  parallel: 'Independent reply',
  opening: 'Opening',
  review: 'Review',
  rebuttal: 'Rebuttal',
  conclusion: 'Conclusion',
};

export interface Roster {
  mode: AgentMode;
  account: string | null;
  limit: number;
  capacityNote?: string;
  choices: string[];
  agents: DebateAgent[];
}

export function autoRoster(available: string[], preferred: string): string[] {
  const ranked = [
    ...new Set([preferred, ...Object.values(modelGroups).flat(), ...available]),
  ].filter((id) => available.includes(id));
  const family = (id: string) =>
    id
      .replace(/-\d{4}-\d{2}-\d{2}$/, '')
      .replace(/^(gpt-\d+)(?:\.\d+)?-(sol|terra|astra|luna)(?:-.+)?$/, '$1-$2')
      .replace(/-(preview|latest)$/, '');
  const unique = ranked.filter(
    (id, i) => ranked.findIndex((other) => family(other) === family(id)) === i,
  );
  return [...new Set([...unique, ...ranked])].slice(0, 3);
}

const presets = {
  Pragmatist:
    'Favor practical next steps. Weigh effort, cost, and what can be tested today.',
  Skeptic:
    'Challenge the weakest assumption. Say what evidence would change your mind.',
  Mediator:
    'Find common ground and identify the trade-off behind any disagreement.',
  Explorer: 'Suggest an overlooked approach and explain when it would work.',
  Researcher:
    'Separate observations from assumptions and flag missing evidence.',
  Architect:
    'Break the problem into parts, map their dependencies, and explain how the pieces fit together.',
  Tester:
    'Look for edge cases. Propose a concrete test that would distinguish the competing answers.',
  Teacher:
    'Build an explanation from first principles and use one worked example to make it clear.',
};
function newAgent(agents: DebateAgent[], choices: string[]): DebateAgent {
  const used = new Set(agents.map((a) => a.name.trim().toLowerCase()));
  const name = names.find((n) => !used.has(n.toLowerCase()))!;
  return {
    name,
    model:
      choices.find((id) => !agents.some((a) => a.model === id)) ??
      choices[agents.length % choices.length],
    personality:
      Object.values(presets)[agents.length % Object.keys(presets).length],
  };
}
export function requestCount(seats: number, budget: number) {
  const low = Math.min(seats + 1, budget),
    high = Math.min(2 * seats + 2, budget);
  return low === high ? String(high) : low + '–' + high;
}
export function rosterError(agents: DebateAgent[]) {
  if (agents.some((a) => !a.name.trim())) return 'Give each agent a name.';
  if (
    new Set(agents.map((a) => a.name.trim().toLowerCase())).size !==
    agents.length
  )
    return 'Each agent needs a different name.';
  return '';
}
const storageKey = (account: string | null, mode: AgentMode) =>
  'quill.' + mode + '.v1.' + JSON.stringify(account);
export function rememberRoster(roster: Roster) {
  try {
    localStorage.setItem(
      storageKey(roster.account, roster.mode),
      JSON.stringify(roster.agents),
    );
  } catch {
    /* Storage is optional. */
  }
}
export function loadRoster(
  account: string | null,
  choices: string[],
  preferred: string,
  mode: AgentMode,
): DebateAgent[] {
  try {
    const saved: unknown = JSON.parse(
      localStorage.getItem(storageKey(account, mode)) ?? 'null',
    );
    if (
      Array.isArray(saved) &&
      saved.length >= 2 &&
      saved.length <= 8 &&
      saved.every(
        (a) =>
          a &&
          typeof a.name === 'string' &&
          a.name.length <= 32 &&
          typeof a.personality === 'string' &&
          a.personality.length <= 400 &&
          (a.focus === undefined ||
            (typeof a.focus === 'string' && a.focus.length <= 400)) &&
          choices.includes(a.model),
      ) &&
      !rosterError(saved)
    )
      return saved;
  } catch {
    /* Fall back to the current catalog. */
  }
  const ranked = autoRoster(choices, preferred);
  const agents: DebateAgent[] = [];
  while (agents.length < Math.max(2, ranked.length))
    agents.push(newAgent(agents, ranked));
  return agents;
}
export function RosterPicker({
  roster,
  update,
  busy,
}: {
  roster: Roster;
  update: (agents: DebateAgent[]) => void;
  busy: boolean;
}) {
  const [index, select] = useState(0);
  const { agents, choices } = roster;
  const [teams, setTeams] = useState(() => readTeams(roster.account));
  const [teamName, setTeamName] = useState('');
  const [teamMessage, setTeamMessage] = useState('');
  const parallel = roster.mode === 'parallel';
  const seat = Math.min(index, agents.length - 1);
  const agent = agents[seat];
  function edit(change: Partial<DebateAgent>) {
    update(agents.map((a, i) => (i === seat ? { ...a, ...change } : a)));
  }
  const preset =
    Object.entries(presets).find(
      ([, text]) => text === agent.personality,
    )?.[0] ?? '';
  return (
    <fieldset className="rt-setup" disabled={busy}>
      <legend>{parallel ? 'Parallel agents' : 'Roundtable'}</legend>
      <div className="rt-editor">
        <div className="team-controls">
          <label>
            Saved teams
            <select
              aria-label="Load saved team"
              value=""
              onChange={(e) => {
                const team = teams.find(
                  (t) => t.name === e.target.value && t.mode === roster.mode,
                );
                if (!team) return;
                if (
                  team.agents.some((a) => !choices.includes(a.model)) ||
                  team.agents.length > roster.limit
                ) {
                  setTeamMessage(
                    'This team exceeds the current limit or uses an unavailable model.',
                  );
                  return;
                }
                update(team.agents.map((a) => ({ ...a })));
                select(0);
                setTeamName(team.name);
                setTeamMessage('');
              }}
            >
              <option value="">Choose a team…</option>
              {teams
                .filter((t) => t.mode === roster.mode)
                .map((t) => (
                  <option key={t.name}>{t.name}</option>
                ))}
            </select>
          </label>
          <label>
            Team name
            <input
              maxLength={48}
              value={teamName}
              onChange={(e) => setTeamName(e.target.value)}
            />
          </label>
          <button
            type="button"
            disabled={
              !teamName.trim() ||
              !!rosterError(agents) ||
              agents.length > roster.limit
            }
            onClick={() => {
              const name = teamName.trim();
              const next = [
                ...teams.filter(
                  (t) => !(t.mode === roster.mode && t.name === name),
                ),
                { name, mode: roster.mode, agents },
              ];
              try {
                writeTeams(roster.account, next);
                setTeams(next);
                setTeamMessage('Team saved.');
              } catch {
                setTeamMessage('Could not save this team.');
              }
            }}
          >
            {teams.some(
              (t) => t.mode === roster.mode && t.name === teamName.trim(),
            )
              ? 'Update team'
              : 'Save team'}
          </button>
        </div>
        {teamMessage && <small role="status">{teamMessage}</small>}
        <div className="rt-roster" aria-label="Agents">
          {agents.map((a, i) => (
            <button
              type="button"
              key={i}
              className={'rt-chip seat-' + i}
              aria-pressed={i === seat}
              onClick={() => select(i)}
            >
              {a.name || 'Unnamed'}
            </button>
          ))}
          <button
            className="rt-add"
            type="button"
            disabled={agents.length >= roster.limit}
            onClick={() => {
              update([...agents, newAgent(agents, choices)]);
              select(agents.length);
            }}
          >
            + Add agent
          </button>
        </div>
        <div className="rt-picks">
          <label>
            Name
            <input
              maxLength={32}
              value={agent.name}
              onChange={(e) => edit({ name: e.target.value })}
            />
          </label>
          <label>
            Model
            <select
              value={agent.model}
              onChange={(e) => edit({ model: e.target.value })}
            >
              {choices.map((id) => (
                <option key={id}>{id}</option>
              ))}
            </select>
          </label>
          <label>
            Personality preset
            <select
              value={preset}
              onChange={(e) =>
                edit({
                  personality:
                    presets[e.target.value as keyof typeof presets] ?? '',
                })
              }
            >
              <option value="">Custom</option>
              {Object.keys(presets).map((n) => (
                <option key={n}>{n}</option>
              ))}
            </select>
          </label>
        </div>
        <label className="rt-persona">
          Personality
          <textarea
            rows={2}
            maxLength={400}
            placeholder="How should this agent approach the question?"
            value={agent.personality}
            onChange={(e) => edit({ personality: e.target.value })}
          />
        </label>
        {parallel && (
          <label className="rt-persona">
            Assignment (optional)
            <input
              maxLength={400}
              placeholder="What should this agent focus on?"
              value={agent.focus ?? ''}
              onChange={(e) => edit({ focus: e.target.value })}
            />
          </label>
        )}
        <div className="rt-footer">
          <small>
            {parallel
              ? agents.length
              : requestCount(agents.length, roster.limit)}{' '}
            model requests ·{' '}
            {roster.account ? 'ChatGPT plan' : 'API key billing'} ·{' '}
            {parallel
              ? 'Each agent replies independently.'
              : (agents.at(-1)?.name || 'Last seat') +
                ' gives the final answer.'}
          </small>
          <button
            className="rt-remove"
            type="button"
            disabled={agents.length <= 2}
            onClick={() => {
              update(agents.filter((_, i) => i !== seat));
              select(Math.max(0, seat - 1));
            }}
          >
            Remove agent
          </button>
        </div>
        <small>
          {parallel
            ? roster.capacityNote
            : 'Roundtable budget: ' + roster.limit + ' requests.'}
        </small>
        {agents.length > roster.limit && (
          <small role="alert">
            Remove agents to fit the current limit of {roster.limit}.
          </small>
        )}
        {rosterError(agents) && (
          <small role="alert">{rosterError(agents)}</small>
        )}
      </div>
    </fieldset>
  );
}

export function Roundtable({
  debate,
  status,
}: {
  debate: Debate;
  status: Turn['status'];
}) {
  const log = useRef<HTMLDivElement>(null);
  const follow = useRef(true);
  useLayoutEffect(() => {
    const el = log.current;
    if (el && follow.current) el.scrollTop = el.scrollHeight;
  }, [debate.entries]);
  const live = debate.entries
    .filter((entry) => entry.status === 'streaming' && status === 'streaming')
    .map((e) => e.seat);
  const phase = debate.entries.at(-1)?.phase;
  return (
    <section className="rt-room" aria-label="Roundtable debate">
      <header>
        <h2>Roundtable</h2>
        <span role="status">
          {status === 'streaming'
            ? phase
              ? phases[phase]
              : 'Taking seats…'
            : status === 'complete'
              ? 'Debate complete'
              : status === 'failed'
                ? 'Debate failed'
                : 'Debate interrupted'}
        </span>
      </header>
      <div className="rt-body">
        <div
          className={'rt-stage seats-' + debate.agents.length}
          aria-label="Agent seats"
        >
          <div className="rt-table" aria-hidden="true">
            Quill
          </div>
          {debate.agents.map((agent, i) => (
            <div
              key={i}
              className={
                'rt-seat seat-' + i + (live.includes(i) ? ' speaking' : '')
              }
              style={{
                left:
                  50 +
                  35 * Math.sin((i * 2 * Math.PI) / debate.agents.length) +
                  '%',
                top:
                  50 -
                  38 * Math.cos((i * 2 * Math.PI) / debate.agents.length) +
                  '%',
              }}
              title={[agent.name, agent.model, agent.personality]
                .filter(Boolean)
                .join(' · ')}
              aria-label={
                agent.name +
                ' · ' +
                agent.model +
                (live.includes(i) ? ', speaking' : '')
              }
            >
              <i className="rt-face" aria-hidden="true" />
              <b>{agent.name}</b>
            </div>
          ))}
        </div>
        <div
          className="rt-log"
          ref={log}
          tabIndex={0}
          role="region"
          aria-label="Public debate transcript"
          onScroll={(e) => {
            const el = e.currentTarget;
            follow.current =
              el.scrollHeight - el.scrollTop - el.clientHeight < 36;
          }}
        >
          {debate.entries.map((e) => (
            <div key={e.id} className={'rt-line seat-' + e.seat}>
              <strong title={debate.agents[e.seat]?.personality}>
                {debate.agents[e.seat]?.name}
              </strong>
              <small>
                {shortModel(e.model ?? debate.agents[e.seat]?.model ?? '')} ·{' '}
                {phases[e.phase]}
              </small>
              <p>
                {e.text ||
                  (e.status === 'streaming'
                    ? 'Waiting for reply…'
                    : 'No reply received.')}
              </p>
              {e.status !== 'complete' && e.status !== 'streaming' && (
                <small>{e.status}</small>
              )}
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}
