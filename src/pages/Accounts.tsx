import { DialogPanel, DialogFooter } from '../components/ui/dialog';
import { Button } from '../components/ui/button';
import { Input } from '../components/ui/input';
import { Table } from '../components/ui/table';
import { Select } from '../components/ui/select';
import { useEffect, useRef, useState } from 'react';
import {
  Activity,
  Cable,
  Clock3,
  ExternalLink,
  Copy,
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
  const { t, run, data, busy, notify } = useWork();
  const [url, setUrl] = useState('');
  const [code, setCode] = useState('');
  const codeInput = useRef<HTMLInputElement>(null);
  const needsFocus = useRef(false);
  const mock = data.settings.backendMode === 'mock';
  const begin = () => void run(() => api('start_account_login'), (next) => {
    needsFocus.current = true;
    setCode('');
    setUrl(next);
  });
  useEffect(() => {
    if (url && !busy && needsFocus.current) {
      codeInput.current?.focus();
      needsFocus.current = false;
    }
  }, [url, busy]);
  async function copyLink() {
    try {
      await navigator.clipboard.writeText(url);
      notify(t('Authorization link copied'));
    } catch {
      notify(t('Could not copy the authorization link'), true);
    }
  }
  return (
    <Modal title="Add Google account" description="Link an account to sync its credit balance." close={close}>
      {!url ? (
        <DialogPanel className="space-y-4">
          <Button variant="default" onClick={begin}>
            <LogIn />{t(mock ? 'Start sandbox authorization' : 'Open Google authorization in browser')}
          </Button>
          <div className="space-y-2 border-t border-border pt-4">
            <p className="text-sm">{t('Already signed in with colab in your terminal? Import that account instead.')}</p>
            <Button variant="outline" onClick={() => void run(() => api('adopt_cli_account'), close)}>{t('Import local CLI account directly')}</Button>
          </div>
        </DialogPanel>
      ) : (
        <form onSubmit={(event) => {
          event.preventDefault();
          if (code.trim() && !busy) void run(() => api('complete_account_login', { authCode: code.trim() }), close);
        }}>
          <DialogPanel className="space-y-4">
            {mock ? <p className="notice">{t('Sandbox authorization: enter a demo code.')}</p> : (
              <>
                <ol className="list-decimal space-y-2 ps-5 text-sm text-foreground marker:font-semibold">
                  <li>{t('Google authorization opened in your default browser. If it did not open, reopen the page or copy the link.')}</li>
                  <li>{t('Choose your Google account (for example, your 20x Pro+ account) and click Allow.')}</li>
                  <li>{t('Paste the final Authorization Code below and finish linking to sync your credit balance.')}</li>
                </ol>
                <div className="actions">
                  <External url={url}><ExternalLink />{t('Reopen page')}</External>
                  <Button variant="outline" onClick={() => void copyLink()}><Copy />{t('Copy authorization link')}</Button>
                </div>
              </>
            )}
            <Field label="Authorization code">
              <Input ref={codeInput} autoFocus required autoComplete="off" spellCheck={false} value={code}
                placeholder={t(mock ? 'Authorization code' : 'Paste the Google authorization code starting with 4/0A...')}
                onChange={(event) => setCode(event.target.value)} />
            </Field>
          </DialogPanel>
          <DialogFooter>
            <Button variant="outline" onClick={begin}><RefreshCw />{t('Get a new link')}</Button>
            <Button variant="default" type="submit" disabled={!code.trim() || busy}>{t('Finish linking and sync credits')}</Button>
          </DialogFooter>
        </form>
      )}
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
        <Button variant="outline" type="submit">
          <ShieldCheck />
          {t('Save')}
        </Button>
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
            <Button variant="outline" size="icon"
              aria-label={t('Refresh')}
              onClick={() => void run(() => api('refresh_accounts_usage'))}
            >
              <RefreshCw />
            </Button>
            <Button variant="default" onClick={() => setLogin(true)}>
              <Plus />
              {t('Add Google account')}
            </Button>
          </div>
        }
      >
        <p>{t('Future launches use the selected account. Existing instances retain their owner.')}</p>
        <div className="table-wrap">
          <Table>
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
                        <Button variant="outline"
                          onClick={() => void run(() => api('switch_account', { accountId: a.id }))}
                        >
                          {t('Switch account')}
                        </Button>
                      )}
                      <Button variant="outline" size="icon"
                        aria-label={t('Delete') + ' ' + a.email}
                        onClick={() => {
                          if (window.confirm(t('Remove this item?') + ' ' + a.email))
                            void run(() => api('remove_account', { accountId: a.id }));
                        }}
                      >
                        <Trash2 />
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </Table>
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
                  <Button variant="outline"
                    disabled={s.state === 'stopping'}
                    onClick={() =>
                      void run(() =>
                        api('extend_session_deadline', { sessionName: s.name, addMinutes: 120 }),
                      )
                    }
                  >
                    <Clock3 />
                    {t('Extend by 2 hours')}
                  </Button>
                  <Button variant="destructive-outline"
                    disabled={s.state === 'stopping'}
                    onClick={() =>
                      void run(() => api('stop_session', { sessionName: s.name, force: false }))
                    }
                  >
                    <Square />
                    {t('Back up and stop billing')}
                  </Button>
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
          <Select
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
          </Select>
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
          <Button variant="outline" disabled={!active.length} onClick={() => setAdd(true)}>
            <Plus />
            {t('New tunnel')}
          </Button>
        }
      >
        {!data.tunnels.length ? (
          <Empty label="No tunnels yet" />
        ) : (
          <div className="table-wrap">
            <Table>
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
                      <Button variant="outline"
                        disabled={x.status === 'stopped'}
                        onClick={() => void run(() => api('stop_port_tunnel', { tunnelId: x.id }))}
                      >
                        {t('Stop')}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </Table>
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
            <DialogPanel className="form-grid">
              <Field label="Name">
                <Input required value={label} onChange={(e) => setLabel(e.target.value)} />
              </Field>
              <Field label="Session">
                <Select required value={target?.name ?? ''} onChange={(e) => setChoice(e.target.value)}>
                  {active.map((s) => (
                    <option key={s.id}>{s.name}</option>
                  ))}
                </Select>
              </Field>
              <Field label="Local port">
                <Input
                  required
                  type="number"
                  min={1}
                  max={65535}
                  value={local}
                  onChange={(e) => setLocal(+e.target.value)}
                />
              </Field>
              <Field label="Remote port">
                <Input
                  required
                  type="number"
                  min={1}
                  max={65535}
                  value={remote}
                  onChange={(e) => setRemote(+e.target.value)}
                />
              </Field>
              <Field label="Health path">
                <Input value={health} onChange={(e) => setHealth(e.target.value)} />
              </Field>
            </DialogPanel>
            <DialogFooter>
              <Button variant="outline" type="button" onClick={() => setAdd(false)}>
                {t('Cancel')}
              </Button>
              <Button variant="default" type="submit" disabled={!target}>
                {t('New tunnel')}
              </Button>
            </DialogFooter>
          </form>
        </Modal>
      )}
    </>
  );
}
