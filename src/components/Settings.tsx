import { defaultPalette, palettes, type PaletteName } from '../palettes';
import { useEffect, useRef, useState } from 'react';
import {
  call,
  manageUsage,
  type AuthStatus,
  type Preferences,
  type UpdateStatus,
} from '../ipc';
import { WorkspaceSettings } from './WorkspaceSettings';
import { workspaceDefaults } from '../ipc';
import { Icon } from './Icon';

import { useDialogMotion } from './useDialogMotion';
import { modelGroups } from '../models';

interface Props {
  auth: AuthStatus | null;
  syncAuth: () => Promise<AuthStatus>;
  preferences: Preferences;
  saved: (p: Preferences) => void;
  close: () => void;
}
export function Settings({ preferences, saved, close, auth, syncAuth }: Props) {
  const [form, setForm] = useState(preferences);
  const isPreset = Object.values(modelGroups).some((ids) =>
    ids.includes(preferences.model),
  );
  const [custom, setCustom] = useState(!isPreset);
  const [customId, setCustomId] = useState(isPreset ? '' : preferences.model);
  const [key, setKey] = useState('');
  const [present, setPresent] = useState<boolean | null>(null);
  const [hotkey, setHotkey] = useState(true);
  const [status, setStatus] = useState('');
  const [update, setUpdate] = useState<UpdateStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const working = useRef(false);
  const [signing, setSigning] = useState(false);
  const [selected, setSelected] = useState(auth?.active ?? '');
  const current = auth?.accounts.find((a) => a.id === auth.active);
  const usingSelected =
    current?.id === selected && current.connected && current.permitted;
  const { modal, dismiss } = useDialogMotion(close);
  useEffect(() => {
    if (current?.permitted && current.connected) {
      void accountWork(() => call('plan_models', {}));
    }
    void call('key_present', {})
      .then(setPresent)
      .catch((e) => setStatus(String(e)));
    void call('shortcut_active', {})
      .then(setHotkey)
      .catch((e) => setStatus(String(e)));
  }, []);
  async function work(action: () => Promise<void>) {
    if (working.current) return;
    working.current = true;
    setBusy(true);
    setStatus('');
    try {
      await action();
    } catch (e) {
      setStatus(String(e));
    } finally {
      working.current = false;
      setBusy(false);
    }
  }
  async function accountWork(action: () => Promise<unknown>) {
    await work(async () => {
      try {
        await action();
      } finally {
        await syncAuth();
      }
    });
  }
  function login() {
    void work(async () => {
      setSigning(true);
      try {
        await call('chatgpt_login', { id: selected || null });
      } finally {
        setSigning(false);
        const next = await syncAuth();
        setSelected(next.active ?? '');
        if (
          next.active &&
          !next.accounts.find((a) => a.id === next.active)?.permitted
        )
          setStatus(
            'Signed in. Plan access was not granted. Authorize it or choose API key billing.',
          );
      }
    });
  }
  function startChat() {
    void work(async () => {
      if (auth?.welcome && auth.active) {
        await call('welcome_seen', { id: auth.active });
        await syncAuth();
      }
      void dismiss();
    });
  }
  return (
    <dialog
      ref={modal}
      className="scroll-panel"
      data-font={form.font ?? 'quill'}
      aria-labelledby="settings-title"
      onCancel={(e) => {
        e.preventDefault();
        if (!busy) void dismiss();
      }}
    >
      <div className="dialog-head">
        <h2 id="settings-title">Settings</h2>
        <button
          className="icon-btn"
          disabled={busy}
          aria-label="Close settings"
          onClick={dismiss}
        >
          <Icon name="close" />
        </button>
      </div>
      <div className="panel-scroll">
        {signing && (
          <div className="feedback" role="status">
            Waiting for browser sign-in.{' '}
            <button
              onClick={() =>
                void call('cancel_login', {}).catch((e) => setStatus(String(e)))
              }
            >
              Cancel sign-in
            </button>
          </div>
        )}
        <fieldset disabled={busy}>
          <h3>ChatGPT account</h3>
          {auth?.welcome && auth.active && (
            <section className="plan-welcome" role="status">
              <strong>You’re using your ChatGPT plan</strong>
              <p>
                Eligible Quill requests use your plan or available credits.
                Manage usage in ChatGPT settings.
              </p>
            </section>
          )}
          {auth && auth.accounts.length > 0 && (
            <label>
              Account registration
              <select
                value={selected}
                onChange={(e) => setSelected(e.target.value)}
              >
                <option value="">Add another account</option>
                {auth.accounts.map((a) => (
                  <option key={a.id} value={a.id}>
                    {a.label} · {a.id.slice(-8)}
                    {a.id === auth.active ? ' (selected)' : ''}
                  </option>
                ))}
              </select>
            </label>
          )}
          <p className="status">
            {current
              ? current.connected && current.permitted
                ? 'Using the ChatGPT plan for ' + current.label
                : 'ChatGPT plan selected. Sign in and authorize plan use to send messages.'
              : 'Use an eligible ChatGPT subscription or enter an API key below.'}
          </p>
          <div className="row">
            {usingSelected ? (
              <button
                className="primary"
                disabled={!auth?.models.some((m) => m.slug === auth.model)}
                onClick={startChat}
              >
                Start chatting
              </button>
            ) : (
              <button className="chatgpt-signin" onClick={login}>
                Continue with ChatGPT
              </button>
            )}
            {auth?.accounts.some((a) => a.id === selected && a.connected) && (
              <button
                onClick={() =>
                  void accountWork(async () => {
                    const revoked = await call('chatgpt_logout', {
                      id: selected,
                    });
                    setStatus(
                      revoked
                        ? 'Signed out.'
                        : 'Signed out locally. Disconnect Quill in ChatGPT settings to finish revocation.',
                    );
                  })
                }
              >
                Sign out
              </button>
            )}
          </div>
          {usingSelected && (
            <details>
              <summary>Connection options</summary>
              <button onClick={login}>Reconnect this account</button>
            </details>
          )}
          {auth?.active && (
            <div className="row">
              <button
                onClick={() => void accountWork(() => call('use_api_key', {}))}
              >
                Use API key billing
              </button>
              <button onClick={() => void work(manageUsage)}>
                Manage usage
              </button>
            </div>
          )}
          <hr />
          {auth?.active ? (
            <label>
              Plan model
              <select
                value={auth.model ?? ''}
                disabled={!auth.models.length}
                onChange={(e) => {
                  const model = e.target.value;
                  void accountWork(() =>
                    call('select_plan_model', { id: auth.active!, model }),
                  );
                }}
              >
                {!auth.models.length && (
                  <option value="">No plan models loaded</option>
                )}
                {auth.models.map((m) => (
                  <option key={m.slug} value={m.slug}>
                    {m.display_name}
                  </option>
                ))}
              </select>
              <button
                disabled={!current?.permitted}
                onClick={() => void accountWork(() => call('plan_models', {}))}
              >
                Refresh models
              </button>
            </label>
          ) : (
            <>
              <label>
                Model
                <select
                  value={custom ? '' : form.model}
                  onChange={(event) => {
                    const preset = event.target.value;
                    setCustom(preset === '');
                    setForm({ ...form, model: preset || customId });
                  }}
                >
                  {Object.entries(modelGroups).map(([family, ids]) => (
                    <optgroup key={family} label={family}>
                      {ids.map((id) => (
                        <option key={id} value={id}>
                          {id}
                        </option>
                      ))}
                    </optgroup>
                  ))}
                  <option value="">Custom model…</option>
                </select>
              </label>
              {custom && (
                <label>
                  Custom model ID
                  <input
                    value={customId}
                    onChange={(event) => {
                      setCustomId(event.target.value);
                      setForm({ ...form, model: event.target.value });
                    }}
                    spellCheck={false}
                  />
                </label>
              )}
            </>
          )}
          <div className="pair">
            <label>
              Appearance
              <select
                value={form.theme}
                onChange={(e) =>
                  setForm({
                    ...form,
                    theme: e.target.value as Preferences['theme'],
                  })
                }
              >
                <option value="system">System</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
            </label>
            <label>
              Open shortcut
              <input
                value={form.shortcut}
                onChange={(e) => setForm({ ...form, shortcut: e.target.value })}
              />
            </label>
          </div>
          <div className="pair">
            <label>
              Color palette
              <select
                value={form.palette ?? defaultPalette}
                onChange={(e) =>
                  setForm({ ...form, palette: e.target.value as PaletteName })
                }
              >
                {Object.keys(palettes).map((name) => (
                  <option key={name}>{name}</option>
                ))}
              </select>
            </label>
            <label>
              Sidebar logo
              <select
                value={form.sidebar_logo ? 'show' : 'hide'}
                onChange={(e) =>
                  setForm({ ...form, sidebar_logo: e.target.value === 'show' })
                }
              >
                <option value="hide">Hidden</option>
                <option value="show">Show feather and name</option>
              </select>
            </label>
          </div>
          <label>
            Motion
            <select
              value={form.motion ?? 'system'}
              onChange={(e) =>
                setForm({
                  ...form,
                  motion: e.target.value as Preferences['motion'],
                })
              }
            >
              <option value="system">Follow system</option>
              <option value="smooth">Smooth</option>
              <option value="reduced">Reduced</option>
            </select>
          </label>
          <label>
            Interface font
            <select
              value={form.font ?? 'quill'}
              onChange={(e) =>
                setForm({
                  ...form,
                  font: e.target.value as Preferences['font'],
                })
              }
            >
              <option value="quill">Quill · Instrument Sans &amp; Serif</option>
              <option value="system">Segoe UI / system sans</option>
              <option value="serif">Classic serif</option>
              <option value="mono">Monospace</option>
            </select>
          </label>
          <fieldset className="workspace-settings">
            <legend>Workspace</legend>
            {(
              [
                ['compact', 'Compact message spacing'],
                ['wide_reading', 'Wider reading column'],
                ['hide_inspector', 'Keep attachments panel closed by default'],
                ['modifier_send', 'Send with Ctrl+Enter (⌘+Enter on Mac)'],
              ] as const
            ).map(([key, label]) => (
              <label key={key}>
                <input
                  type="checkbox"
                  checked={!!form[key]}
                  onChange={(e) =>
                    setForm({ ...form, [key]: e.target.checked })
                  }
                />
                {label}
              </label>
            ))}
          </fieldset>
          <WorkspaceSettings
            key={auth?.active ?? 'api'}
            account={auth?.active ?? null}
            capacity={auth?.parallel?.seats ?? 3}
            options={form.workspace ?? workspaceDefaults}
            change={(workspace) => setForm({ ...form, workspace })}
          />
          {!hotkey && (
            <p className="status">
              The saved shortcut is unavailable. Choose another combination.
            </p>
          )}
          <button
            className="primary"
            onClick={() =>
              void work(async () => {
                const p = {
                  ...form,
                  model: form.model.trim(),
                  shortcut: form.shortcut.trim(),
                };
                await call('save_preferences', { preferences: p });
                saved(p);
                setHotkey(true);
                setStatus('Preferences saved.');
              })
            }
          >
            Save preferences
          </button>
          <hr />
          <label>
            OpenAI API key{' '}
            <small>
              {present === null
                ? ''
                : present
                  ? 'Saved in system keychain'
                  : 'No key saved'}
            </small>
            <input
              type="password"
              autoComplete="off"
              spellCheck={false}
              value={key}
              placeholder="Enter API key"
              onChange={(e) => setKey(e.target.value)}
            />
          </label>
          <div className="row">
            <button
              disabled={!key.trim()}
              onClick={() =>
                void work(async () => {
                  await call('save_key', { key });
                  setKey('');
                  setPresent(true);
                  setStatus('API key saved.');
                })
              }
            >
              Save key
            </button>
            {present && (
              <button
                onClick={() =>
                  void work(async () => {
                    await call('remove_key', {});
                    setPresent(false);
                    setKey('');
                    setStatus('API key removed.');
                  })
                }
              >
                Remove key
              </button>
            )}
          </div>
          <hr />
          <div className="row">
            <button
              onClick={() =>
                void work(async () => {
                  const value = await call('check_update', {});
                  setUpdate(value);
                  setStatus(
                    value.type === 'unconfigured'
                      ? 'This build has no release feed.'
                      : value.type === 'current'
                        ? 'Quill is up to date.'
                        : 'Update available: ' + value.version,
                  );
                })
              }
            >
              Check for updates
            </button>
            {update?.type === 'available' && (
              <button
                className="primary"
                onClick={() =>
                  void work(async () => {
                    setStatus('Downloading update…');
                    await call('install_update', { version: update.version });
                  })
                }
              >
                Install and restart
              </button>
            )}
          </div>
        </fieldset>
        <div className="feedback" role="status">
          {busy && !status && !signing ? 'Working…' : status}
        </div>
      </div>
    </dialog>
  );
}
