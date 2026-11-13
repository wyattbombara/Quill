import { useState } from 'react';
import type { WorkspaceOptions } from '../ipc';
import { readTeams, writeTeams } from '../workspaceStorage';

export function WorkspaceSettings({
  options,
  change,
  account,
  capacity,
}: {
  options: WorkspaceOptions;
  change: (next: WorkspaceOptions) => void;
  account: string | null;
  capacity: number;
}) {
  const [teams, setTeams] = useState(() => readTeams(account));
  const [notice, setNotice] = useState('');
  const update = <K extends keyof WorkspaceOptions>(
    key: K,
    value: WorkspaceOptions[K],
  ) => change({ ...options, [key]: value });
  const checks = [
    ['resume_chat', 'Reopen the last chat'],
    ['restore_drafts', 'Restore unsent text and attachments'],
    ['quit_on_close', 'Quit when the window closes'],
    ['follow_replies', 'Follow streaming replies until I scroll up'],
  ] as const;
  return (
    <div className="extra-settings">
      <fieldset className="workspace-settings">
        <legend>Desktop behavior</legend>
        {checks.map(([key, label]) => (
          <label key={key}>
            <input
              type="checkbox"
              checked={options[key]}
              onChange={(e) => update(key, e.target.checked)}
            />
            {label}
          </label>
        ))}
        <label>
          <input
            type="checkbox"
            checked={options.reply_notifications}
            onChange={async (e) => {
              if (!e.target.checked) {
                update('reply_notifications', false);
                return;
              }
              try {
                if (
                  typeof Notification === 'undefined' ||
                  (await Notification.requestPermission()) !== 'granted'
                ) {
                  setNotice(
                    'Allow notifications for Quill in your system or WebView settings.',
                  );
                  return;
                }
                setNotice('');
                update('reply_notifications', true);
              } catch {
                setNotice('Notifications are unavailable in this WebView.');
              }
            }}
          />
          Notify when a background reply finishes
        </label>
        <label>
          <input
            type="checkbox"
            disabled={!options.reply_notifications}
            checked={options.preview_notifications}
            onChange={(e) => update('preview_notifications', e.target.checked)}
          />
          Include a reply preview in notifications
        </label>
      </fieldset>
      <fieldset>
        <legend>Reading and usage</legend>
        <div className="pair">
          {(
            [
              ['reply_size', 'Reply text', 12, 24],
              ['code_size', 'Code text', 10, 22],
            ] as const
          ).map(([key, label, min, max]) => (
            <label key={key}>
              {label} · {options[key]} px
              <input
                type="range"
                min={min}
                max={max}
                value={options[key]}
                onChange={(e) => update(key, Number(e.target.value))}
              />
            </label>
          ))}
        </div>
        <div className="size-preview" style={{ fontSize: options.reply_size }}>
          A reply looks like this.
          <br />
          <code style={{ fontSize: options.code_size }}>let ready = true;</code>
        </div>
        <label>
          Usage beside the composer
          <select
            value={options.composer_usage}
            onChange={(e) =>
              update(
                'composer_usage',
                e.target.value as WorkspaceOptions['composer_usage'],
              )
            }
          >
            <option value="hidden">Hidden</option>
            <option value="chat">This chat</option>
            <option value="today">Today on this device</option>
          </select>
        </label>
      </fieldset>
      <fieldset>
        <legend>Agents</legend>
        <label>
          Roundtable request limit
          <select
            value={options.roundtable_budget}
            onChange={(e) =>
              update('roundtable_budget', Number(e.target.value))
            }
          >
            {[2, 3, 4, 5, 6, 7, 8].map((n) => (
              <option key={n} value={n}>
                {n} requests
              </option>
            ))}
          </select>
        </label>
        <small>
          Parallel currently allows {capacity} agents. Save a named team in the
          agent picker.
        </small>
        {teams.map((t) => (
          <div className="saved-team-row" key={t.mode + t.name}>
            <span>
              {t.name} · {t.mode} · {t.agents.length} seats
            </span>
            <button
              type="button"
              aria-label={'Remove ' + t.name + ' team'}
              onClick={() => {
                const next = teams.filter((item) => item !== t);
                try {
                  writeTeams(account, next);
                  setTeams(next);
                } catch {
                  setNotice('Could not remove this saved team.');
                }
              }}
            >
              Remove
            </button>
          </div>
        ))}
      </fieldset>
      {notice && <small role="status">{notice}</small>}
    </div>
  );
}
