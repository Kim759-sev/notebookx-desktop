import { useState } from 'react';
import {
  Activity,
  Cable,
  Clock3,
  ExternalLink,
  LogIn,
  Plus,
  RefreshCw,
  ShieldCheck,
  Square,
  Trash2,
  Users,
} from 'lucide-react';
import { api } from '../lib/api';
import type { CostGuardPolicy } from '../lib/types';
import {
  Amount,
  Badge,
  Empty,
  External,
  Field,
  GuardFields,
  Header,
  Modal,
  Panel,
  useWork,
  Verified,
} from '../components/workspace';

function Login({ close }: { close: () => void }) {
  const { t, run, data } = useWork();
  const [url, setUrl] = useState(''),
    [code, setCode] = useState('');
  return (
    <Modal title="Add Google account" close={close}>
      <div className="modal-body stack">
        {!url ? (
          <div className="actions">
            <button
              className="primary"
              onClick={() => void run(() => api('start_account_login'), setUrl)}
            >
              <LogIn />
              {t('Authorization')}
            </button>
            <button onClick={() => void run(() => api('adopt_cli_account'), close)}>
              {t('Adopt CLI account')}
            </button>
          </div>
        ) : (
          <form
            className="stack"
            onSubmit={(e) => {
              e.preventDefault();
              void run(() => api('complete_account_login', { authCode: code.trim() }), close);
            }}
          >
            {data.settings.backendMode === 'mock' ? (
              <p className="notice">{t('Sandbox authorization: enter a demo code.')}</p>
            ) : (
              <External url={url}>
                {t('Open authorization page')}
                <ExternalLink size={14} />
              </External>
            )}
            <Field label="Authorization code">
              <input
                autoFocus
                required
                value={code}
                onChange={(e) => setCode(e.target.value)}
                autoComplete="off"
              />
            </Field>
            <div>
              <button className="primary">{t('Complete sign-in')}</button>
            </div>
          </form>
        )}
      </div>
    </Modal>
  );
}
function Guards({ initial, save }: { initial: CostGuardPolicy; save: (v: CostGuardPolicy) => void }) {
  const { t } = useWork();
  const [guard, setGuard] = useState(initial);
  return (
    <form
      className="stack"
      onSubmit={(e) => {
        e.preventDefault();
        save(guard);
      }}
    >
      <GuardFields value={guard} change={setGuard} />
      <div>
        <button>
          <ShieldCheck />
          {t('Save')}
        </button>
      </div>
    </form>
  );
}
export default function Accounts() {
  const { data, t, run, settings } = useWork();
  const [login, setLogin] = useState(false),
    [add, setAdd] = useState(false),
    [guardScope, setGuardScope] = useState(''),
    [choice, setChoice] = useState('');
  const [local, setLocal] = useState(8188),
    [remote, setRemote] = useState(8188),
    [label, setLabel] = useState(''),
    [health, setHealth] = useState('/');
  const active = data.sessions.filter((s) => !['stopped', 'error', 'stopping'].includes(s.state));
  const target = active.find((s) => s.name === choice) ?? active[0];
  const guardProfile = data.profiles.find((p) => p.id === guardScope);
  const guard = guardProfile?.costGuard ?? data.settings.defaultCostGuard;
  return (
    <>
      <Header
        title="Accounts, sessions & ports"
        subtitle="Manage Google accounts, monitor spending, and forward local ports."
      />
      <Panel
        title="Google accounts"
        icon={Users}
        actions={
          <div className="actions">
            <button
              className="icon-btn"
              aria-label={t('Refresh')}
              onClick={() => void run(() => api('refresh_accounts_usage'))}
            >
              <RefreshCw />
            </button>
            <button className="primary" onClick={() => setLogin(true)}>
              <Plus />
              {t('Add Google account')}
            </button>
          </div>
        }
      >
        <p>{t('Future launches use the selected account. Existing instances retain their owner.')}</p>
        <div className="table-wrap">
          <table>
            <thead>
              <tr>
                {['Google accounts', 'Balance', 'Burn', 'Remaining', ''].map((l, i) => (
                  <th key={i}>{t(l)}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {data.accounts.map((a) => (
                <tr key={a.id}>
                  <td>
                    <strong>{a.email}</strong> <span className="tier">{a.planTier}</span>
                    {a.usageStale && <Badge label="Stale usage" tone="amber" />}
                  </td>
                  <td>
                    <Amount value={a.balanceCcu} /> CCU
                  </td>
                  <td>
                    <Amount value={a.burnRateHourly} />
                    /h
                  </td>
                  <td>
                    <Amount value={a.hoursRemaining} /> h
                  </td>
                  <td>
                    <div className="actions">
                      {a.isActive ? (
                        <Badge dot label="Active" tone="green" />
                      ) : (
                        <button
                          onClick={() => void run(() => api('switch_account', { accountId: a.id }))}
                        >
                          {t('Switch account')}
                        </button>
                      )}
                      <button
                        className="icon-btn"
                        aria-label={t('Delete') + ' ' + a.email}
                        onClick={() => {
                          if (window.confirm(t('Remove this item?') + ' ' + a.email))
                            void run(() => api('remove_account', { accountId: a.id }));
                        }}
                      >
                        <Trash2 />
                      </button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {!data.accounts.length && <Empty label="No account" />}
      </Panel>
      <Panel title="Sessions" icon={Activity} actions={<span className="count">{active.length}</span>}>
        {!data.sessions.length ? (
          <Empty label="No active sessions" />
        ) : (
          data.sessions.map((s) => (
            <div className="session-row" key={s.id}>
              <div className="split">
                <div>
                  <strong>
                    {s.state === 'orphan' ? '[?] ' : ''}
                    {s.name}
                  </strong>
                  <small>
                    {s.accountEmail} · {s.gpuName ?? s.accelerator} · {s.spentCcu.toFixed(1)} CCU
                  </small>
                </div>
                <Badge
                  dot
                  label={s.state}
                  tone={s.state === 'orphan' ? 'amber' : s.state === 'ready' ? 'green' : 'neutral'}
                />
              </div>
              {s.state !== 'stopped' && (
                <div className="actions">
                  <span className="muted">
                    {t('VM watchdog')} · {t(s.watchdogActive ? 'Active' : 'Disabled')}
                  </span>
                  {s.autoStopAt && (
                    <code>{new Date(s.autoStopAt).toLocaleString(data.settings.locale)}</code>
                  )}
                  <button
                    disabled={s.state === 'stopping'}
                    onClick={() =>
                      void run(() =>
                        api('extend_session_deadline', { sessionName: s.name, addMinutes: 120 }),
                      )
                    }
                  >
                    <Clock3 />
                    {t('Extend by 2 hours')}
                  </button>
                  <button
                    className="danger"
                    disabled={s.state === 'stopping'}
                    onClick={() =>
                      void run(() => api('stop_session', { sessionName: s.name, force: false }))
                    }
                  >
                    <Square />
                    {t('Back up and stop billing')}
                  </button>
                </div>
              )}
              {s.lastError && <p className="notice bad">{s.lastError}</p>}
              {s.state === 'stopped' && s.verifiedUnassigned && <Verified />}
            </div>
          ))
        )}
      </Panel>
      <Panel
        title="Cost guards"
        icon={ShieldCheck}
        actions={
          <select
            aria-label={t('Cost guards')}
            value={guardScope}
            onChange={(e) => setGuardScope(e.target.value)}
          >
            <option value="">{t('Default for new profiles')}</option>
            {data.profiles.map((p) => (
              <option key={p.id} value={p.id}>
                {t(p.name)}
              </option>
            ))}
          </select>
        }
      >
        <p className="notice">
          <ShieldCheck size={16} />
          {t(
            'The VM watchdog can shut down the instance on schedule even after the desktop app closes.',
          )}
        </p>
        <Guards
          key={guardScope + JSON.stringify(guard)}
          initial={guard}
          save={(v) =>
            guardProfile
              ? void run(
                  () => api('save_workflow_profile', { profile: { ...guardProfile, costGuard: v } }),
                  undefined,
                  'Saved',
                )
              : settings({ defaultCostGuard: v })
          }
        />
      </Panel>
      <Panel
        title="Port forwarding"
        icon={Cable}
        actions={
          <button disabled={!active.length} onClick={() => setAdd(true)}>
            <Plus />
            {t('New tunnel')}
          </button>
        }
      >
        {!data.tunnels.length ? (
          <Empty label="No tunnels yet" />
        ) : (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  {['Connection', 'Local port', 'Remote port', 'Session', 'Status', ''].map((l, i) => (
                    <th key={i}>{t(l)}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {data.tunnels.map((x) => (
                  <tr key={x.id}>
                    <td>{x.label}</td>
                    <td>
                      <code>127.0.0.1:{x.localPort}</code>
                    </td>
                    <td>
                      <code>VM:{x.remotePort}</code>
                    </td>
                    <td>{x.sessionName}</td>
                    <td>
                      <Badge
                        dot
                        label={x.status}
                        tone={x.healthy ? 'green' : x.status === 'failed' ? 'red' : 'neutral'}
                      />
                      <small>
                        {t(x.healthy === null ? 'Unknown' : x.healthy ? 'Healthy' : 'Unhealthy')}
                      </small>
                    </td>
                    <td>
                      <button
                        disabled={x.status === 'stopped'}
                        onClick={() => void run(() => api('stop_port_tunnel', { tunnelId: x.id }))}
                      >
                        {t('Stop')}
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Panel>
      {login && <Login close={() => setLogin(false)} />}
      {add && (
        <Modal title="New tunnel" close={() => setAdd(false)}>
          <form
            onSubmit={(e) => {
              e.preventDefault();
              if (target)
                void run(
                  () =>
                    api('create_port_tunnel', {
                      sessionName: target.name,
                      remotePort: remote,
                      localPort: local,
                      label,
                      healthPath: health || null,
                    }),
                  () => setAdd(false),
                  'Saved',
                );
            }}
          >
            <div className="modal-body form-grid">
              <Field label="Name">
                <input required value={label} onChange={(e) => setLabel(e.target.value)} />
              </Field>
              <Field label="Session">
                <select required value={target?.name ?? ''} onChange={(e) => setChoice(e.target.value)}>
                  {active.map((s) => (
                    <option key={s.id}>{s.name}</option>
                  ))}
                </select>
              </Field>
              <Field label="Local port">
                <input
                  required
                  type="number"
                  min={1}
                  max={65535}
                  value={local}
                  onChange={(e) => setLocal(+e.target.value)}
                />
              </Field>
              <Field label="Remote port">
                <input
                  required
                  type="number"
                  min={1}
                  max={65535}
                  value={remote}
                  onChange={(e) => setRemote(+e.target.value)}
                />
              </Field>
              <Field label="Health path">
                <input value={health} onChange={(e) => setHealth(e.target.value)} />
              </Field>
            </div>
            <div className="modal-foot">
              <button type="button" onClick={() => setAdd(false)}>
                {t('Cancel')}
              </button>
              <button className="primary" disabled={!target}>
                {t('New tunnel')}
              </button>
            </div>
          </form>
        </Modal>
      )}
    </>
  );
}
