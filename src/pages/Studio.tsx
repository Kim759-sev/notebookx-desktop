import { DialogPanel, DialogFooter } from '../components/ui/dialog';
import { Input } from '../components/ui/input';
import { Select } from '../components/ui/select';
import { Textarea } from '../components/ui/textarea';
import { Button } from '../components/ui/button';
import { Table } from '../components/ui/table';
import { useState } from 'react';
import {
  Activity,
  ArrowRight,
  ArrowUpRight,
  Box,
  Cable,
  Check,
  CheckCircle2,
  ChevronDown,
  Circle,
  CircuitBoard,
  Clock3,
  Cpu,
  ExternalLink,
  Film,
  Folder,
  HardDrive,
  Image as ImageIcon,
  Layers3,
  LoaderCircle,
  Monitor,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Save,
  Square,
  Terminal,
  Video,
  Workflow,
  XCircle,
} from 'lucide-react';
import { api, isDesktop } from '../lib/api';
import { runtimePreviewUrl } from './RuntimePreview';
import { accelerators, newProfile, stageLabels } from '../lib/fixtures';
import type { WorkflowProfile, ColabSession } from '../lib/types';
import {
  Badge,
  External,
  Field,
  GuardFields,
  Header,
  Modal,
  Panel,
  Toggle,
  useWork,
  Verified,
} from '../components/workspace';
import terrain from '../assets/terrain.webp';

function ProfileEditor({
  profile,
  existing,
  close,
  saved,
}: {
  profile: WorkflowProfile;
  existing: boolean;
  close: () => void;
  saved: (id: string) => void;
}) {
  const { t, run, busy } = useWork(),
    [p, set] = useState(profile);
  function patch<K extends keyof WorkflowProfile>(k: K, v: WorkflowProfile[K]) {
    set((prev) => ({ ...prev, [k]: v }));
  }
  return (
    <Modal title={existing ? 'Edit profile' : 'New profile'} close={close}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void run(
            () => api('save_workflow_profile', { profile: p }),
            () => {
              saved(p.id);
              close();
            },
            'Saved',
          );
        }}
      >
        <DialogPanel className="stack">
          <div className="form-grid">
            <Field label="Profile name">
              <Input autoFocus required value={p.name} onChange={(e) => patch('name', e.target.value)} />
            </Field>
            <Field label="Description">
              <Input value={p.description} onChange={(e) => patch('description', e.target.value)} />
            </Field>
            <Field label="Accelerator">
              <Select
                value={p.accelerator}
                onChange={(e) => patch('accelerator', e.target.value as WorkflowProfile['accelerator'])}
              >
                {accelerators.map((x) => (
                  <option key={x}>{x}</option>
                ))}
              </Select>
            </Field>
            <Field label="Minimum VRAM (GB)">
              <Input
                type="number"
                min="0"
                required
                value={p.minVramGb}
                onChange={(e) => patch('minVramGb', +e.target.value)}
              />
            </Field>
            <Field label="VRAM retries">
              <Input
                type="number"
                min="0"
                required
                value={p.maxRetries}
                onChange={(e) => patch('maxRetries', +e.target.value)}
              />
            </Field>
            <Toggle label="High-RAM" checked={p.highMem} onChange={(v) => patch('highMem', v)} />
            <Field label="Drive root" wide>
              <Input
                required
                value={p.driveRootPath}
                onChange={(e) => patch('driveRootPath', e.target.value)}
              />
            </Field>
            <Field label="Drive mount">
              <Select
                value={p.driveMountMode}
                onChange={(e) =>
                  patch('driveMountMode', e.target.value as WorkflowProfile['driveMountMode'])
                }
              >
                {[
                  ['colabNative', 'Native mount'],
                  ['rcloneUnattended', 'Unattended rclone'],
                  ['disabled', 'Disabled'],
                ].map(([v, l]) => (
                  <option key={v} value={v}>
                    {t(l)}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label="Startup mode">
              <Select
                value={p.startupMode}
                onChange={(e) => patch('startupMode', e.target.value as WorkflowProfile['startupMode'])}
              >
                {[
                  ['managedScript', 'Managed script'],
                  ['comfySnapshot', 'ComfyUI snapshot'],
                  ['cpuPreDownload', 'CPU pre-download'],
                ].map(([v, l]) => (
                  <option key={v} value={v}>
                    {t(l)}
                  </option>
                ))}
              </Select>
            </Field>
            {p.startupMode === 'managedScript' && (
              <Field label="Script path" wide>
                <Input
                  value={p.managedScriptRelPath ?? ''}
                  onChange={(e) => patch('managedScriptRelPath', e.target.value || null)}
                />
              </Field>
            )}
            <Field label="Required nodes" wide>
              <Textarea
                rows={2}
                defaultValue={p.requiredNodes.join('\n')}
                onChange={(e) =>
                  patch(
                    'requiredNodes',
                    e.target.value
                      .split(/[,\n]/)
                      .map((s) => s.trim())
                      .filter(Boolean),
                  )
                }
              />
            </Field>
          </div>
          <h3>{t('Cost guards')}</h3>
          <GuardFields value={p.costGuard} change={(v) => patch('costGuard', v)} />
        </DialogPanel>
        <DialogFooter>
          {existing && (
            <Button variant="destructive-outline"
              type="button"
              className="danger push-left"
              disabled={busy}
              onClick={() => {
                if (window.confirm(t('Remove this item?')))
                  void run(() => api('delete_workflow_profile', { profileId: p.id }), close);
              }}
            >
              {t('Delete')}
            </Button>
          )}
          <Button variant="outline" type="button" onClick={close}>
            {t('Cancel')}
          </Button>
          <Button variant="default" type="submit" disabled={busy}>
            {t('Save')}
          </Button>
        </DialogFooter>
      </form>
    </Modal>
  );
}
function SessionTools({ s }: { s: ColabSession }) {
  const { t, run, data } = useWork();
  const live = !['stopped', 'error', 'stopping'].includes(s.state);
  const url = s.comfyLocalUrl ?? 'http://127.0.0.1:8188';
  return (
    <>
      <Panel
        title="Runtime pipeline"
        icon={Workflow}
        actions={<Badge label={s.state} tone={s.state === 'error' ? 'red' : 'green'} dot />}
      >
        <div className="pipeline">
          {s.stages.map((st, i) => {
            const Icon =
              st.status === 'completed'
                ? CheckCircle2
                : st.status === 'running'
                  ? LoaderCircle
                  : st.status === 'failed'
                    ? XCircle
                    : Circle;
            return (
              <div key={st.id} className={'step ' + st.status}>
                <Icon size={19} className={st.status === 'running' ? 'spin' : ''} />
                <span>{t(stageLabels[i] ?? st.label)}</span>
                <small>{st.detail || t(st.status)}</small>
              </div>
            );
          })}
        </div>
        {s.driveAuthUrl && (
          <External url={s.driveAuthUrl}>
            {t('Authorize Drive')} <ExternalLink size={14} />
          </External>
        )}
        {s.lastError && <p className="notice bad">{s.lastError}</p>}
        {live && (
          <div className="actions line-top">
            <Button variant="outline"
              onClick={() =>
                void run(
                  () => api('extend_session_deadline', { sessionName: s.name, addMinutes: 120 }),
                  undefined,
                  'Saved',
                )
              }
            >
              <Clock3 />
              {t('Extend by 2 hours')}
            </Button>
            <Button variant="destructive-outline"
              onClick={() => void run(() => api('stop_session', { sessionName: s.name, force: false }))}
            >
              <Square />
              {t('Back up and stop billing')}
            </Button>
            {s.autoStopAt && (
              <small>{new Date(s.autoStopAt).toLocaleTimeString(data.settings.locale)}</small>
            )}
          </div>
        )}
      </Panel>
      {s.comfyReady && (
        <div className="two-col">
          <Panel title="ComfyUI snapshot" icon={Save}>
            <p>{t('Snapshots preserve custom nodes and settings in Google Drive.')}</p>
            <Button variant="outline"
              onClick={() =>
                void run(
                  () => api('save_comfy_environment_snapshot', { sessionName: s.name }),
                  (path) => window.alert(path),
                )
              }
            >
              <HardDrive />
              {t('Save environment snapshot')}
            </Button>
          </Panel>
          <Panel
            title="DramaSmith"
            icon={Cable}
            actions={
              <Badge
                dot
                label={data.env.dramaSmithReachable ? 'Online' : 'Offline'}
                tone={data.env.dramaSmithReachable ? 'green' : 'neutral'}
              />
            }
          >
            <div className="endpoint-lines">
              <code>{data.env.dramaSmithUrl}</code>
              <code>{s.managementLocalUrl ?? '—'}</code>
              {s.cloudflareUrl && <External url={s.cloudflareUrl}>{s.cloudflareUrl}</External>}
            </div>
            <Button variant="outline"
              onClick={() =>
                void run(
                  () =>
                    api('sync_dramasmith_endpoint', {
                      endpointUrl: data.env.gatewayRunning
                        ? data.env.gatewayUrl
                        : (s.cloudflareUrl ?? url),
                    }),
                  undefined,
                  'Saved',
                )
              }
            >
              {t('Sync endpoint to DramaSmith')}
              <ArrowUpRight />
            </Button>
          </Panel>
        </div>
      )}
      <details className="panel">
        <summary>
          <Terminal size={16} />
          {t('Runtime logs')}
          <ChevronDown size={15} />
        </summary>
        <pre className="logs">{s.logs.map(t).join('\n') || t('No logs yet')}</pre>
      </details>
    </>
  );
}
export default function Studio() {
  const { data, t, run, navigate } = useWork();
  const [choice, select] = useState('');
  const [editor, setEditor] = useState<{ p: WorkflowProfile; existing: boolean } | null>(null);
  const p = data.profiles.find((p) => p.id === choice) ?? data.profiles[0];
  const s = data.sessions
    .filter(
      (s) =>
        s.profileId === p?.id &&
        (!data.accounts.some((a) => a.isActive) ||
          s.accountId === data.accounts.find((a) => a.isActive)?.id),
    )
    .sort((a, b) => Date.parse(b.startedAt) - Date.parse(a.startedAt))[0];
  const live = s && !['error', 'stopped'].includes(s.state);
  const comfy = s?.comfyLocalUrl ?? 'http://127.0.0.1:' + (p?.comfyPort ?? 8188);
  function add() {
    const profile = newProfile('');
    profile.driveRootPath = data.settings.defaultDriveRoot;
    profile.costGuard = { ...data.settings.defaultCostGuard };
    setEditor({ p: profile, existing: false });
  }
  return (
    <>
      <Header
        title="Cloud ComfyUI Studio"
        subtitle="Launch a workflow, connect to ComfyUI, and manage your cloud runtime."
        actions={
          <Button variant="outline" onClick={() => void run(() => api('refresh_accounts_usage'))}>
            <RefreshCw />
            {t('Refresh')}
          </Button>
        }
      />
      <section className="hero">
        <img className="terrain" src={terrain} alt="" />
        <div className="hero-info">
          <div className="runtime-state">
            <i className="dot" />
            {t(live ? s.state : 'Standby')}
          </div>
          <h2>{t(live ? (s.profileName ?? s.name) : 'Ready to launch')}</h2>
          <p>
            {p ? (
              <>
                <Cpu size={15} />
                {t(p.name)} <span className="hero-separator">/</span>{' '}
                <span className="mono">
                  {p.accelerator}
                  {p.minVramGb ? ' · ' + p.minVramGb + ' GB' : ''}
                </span>
              </>
            ) : (
              t('New profile')
            )}
          </p>
          <div className="actions">
            {!live ? (
              <Button variant="default"
                className="primary launch"
                onClick={() =>
                  p ? void run(() => api('launch_profile_session', { profileId: p.id })) : add()
                }
              >
                <Play size={16} fill="currentColor" />
                {t(
                  !p
                    ? 'New profile'
                    : p.startupMode === 'cpuPreDownload'
                      ? 'Launch CPU runtime'
                      : 'Launch cloud ComfyUI',
                )}
              </Button>
            ) : s.comfyReady ? (
              <>
                <Button variant="default"
                  onClick={() =>
                    void run(() =>
                      api('open_comfy_desktop_window', { url: comfy, title: 'ComfyUI · ' + s.name }),
                    )
                  }
                >
                  <Monitor />
                  {t('Open desktop ComfyUI')}
                </Button>
                <External url={isDesktop() ? comfy : runtimePreviewUrl(s.name)}>
                  {t('Open in browser')}
                  <ArrowUpRight size={14} />
                </External>
              </>
            ) : (
              <span className="hero-stage">
                <Activity size={16} />
                {t(s.stages.find((st) => st.status === 'running')?.label ?? s.state)}
              </span>
            )}
          </div>
        </div>
        <div className="terrain-label">
          <span className="crosshair" /> {p?.accelerator ?? 'GPU'} <span>/</span> {p?.minVramGb ?? 0} GB
        </div>
      </section>
      {s?.state === 'stopped' && s.verifiedUnassigned && <Verified />}
      {s && s.state !== 'stopped' && <SessionTools s={s} />}
      <section>
        <div className="section-head profiles-title">
          <h2>
            <Layers3 size={19} />
            {t('Workflow profiles')} <span className="count">{data.profiles.length}</span>
          </h2>
          <Button variant="outline" onClick={add}>
            <Plus />
            {t('New profile')}
          </Button>
        </div>
        <div className="profiles">
          {data.profiles.map((item, i) => {
            const Icon = item.accelerator === 'CPU' ? Cpu : [Film, ImageIcon, Video, Box][i % 4];
            const selected = item.id === p?.id;
            return (
              <article
                key={item.id}
                className={'profile ' + (selected ? 'selected ' : '') + 'color-' + (i % 4)}
              >
                <Button variant="ghost"
                  className="pick"
                  aria-pressed={selected}
                  aria-label={t('Select') + ' ' + t(item.name)}
                  onClick={() => select(item.id)}
                ></Button>
                <div className="profile-top">
                  <span className="profile-icon">
                    <Icon size={20} />
                  </span>
                  <span className="profile-select">{selected && <Check size={11} />}</span>
                </div>
                <h3>{t(item.name)}</h3>
                <p>{t(item.description) || t('Workflow profiles')}</p>
                <div className="chips">
                  <span>
                    <Cpu size={12} />
                    {item.accelerator}
                    {item.minVramGb ? ' · ' + item.minVramGb + ' GB' : ''}
                  </span>
                  <span>
                    <Clock3 size={12} />
                    {item.costGuard.idleStopMinutes ?? '—'} {t('min')}
                  </span>
                </div>
                <div className="profile-detail">
                  <span title={item.driveRootPath}>
                    <Folder size={12} />
                    {item.driveRootPath}
                  </span>
                  <span>
                    {item.minVramGb ? '≥ ' + item.minVramGb + ' GB · ' : ''}
                    {item.maxRetries} {t('retries')}
                  </span>
                </div>
                <Button variant="ghost" size="icon"
                  className="edit-icon"
                  title={t('Edit profile')}
                  aria-label={t('Edit profile') + ' ' + item.name}
                  onClick={() => setEditor({ p: structuredClone(item), existing: true })}
                >
                  <Pencil size={13} />
                </Button>
              </article>
            );
          })}
        </div>
      </section>
      <Panel
        title="Runtime connections"
        icon={Cable}
        className="connections"
        actions={
          <Button variant="ghost" onClick={() => navigate('accounts')}>
            {t('View sessions')}
            <ArrowRight />
          </Button>
        }
      >
        <div className="table-wrap">
          <Table>
            <thead>
              <tr>
                <th>{t('Connection')}</th>
                <th>{t('Endpoint')}</th>
                <th>{t('Status')}</th>
              </tr>
            </thead>
            <tbody>
              {[
                {
                  name: 'Local gateway',
                  Icon: CircuitBoard,
                  url: data.env.gatewayUrl,
                  on: data.env.gatewayRunning,
                  off: 'Offline',
                },
                {
                  name: 'ComfyUI',
                  Icon: Workflow,
                  url: comfy,
                  on: !!s?.comfyReady,
                  off: 'Awaiting runtime',
                },
                {
                  name: 'DramaSmith',
                  Icon: Film,
                  url: data.env.dramaSmithUrl,
                  on: data.env.dramaSmithReachable,
                  off: 'Offline',
                },
              ].map(({ name, Icon, url, on, off }) => (
                <tr key={name}>
                  <td>
                    <span className="cell-icon">
                      <Icon size={15} />
                      {t(name)}
                    </span>
                  </td>
                  <td>
                    <code>{url.replace(/^https?:\/\//, '')}</code>
                  </td>
                  <td>
                    <Badge label={on ? 'Online' : off} dot tone={on ? 'green' : 'neutral'} />
                  </td>
                </tr>
              ))}
            </tbody>
          </Table>
        </div>
      </Panel>
      {s?.state === 'stopped' && (
        <details className="panel">
          <summary>
            <Terminal size={16} />
            {t('Runtime logs')}
            <ChevronDown />
          </summary>
          <pre className="logs">{s.logs.map(t).join('\n')}</pre>
        </details>
      )}
      {editor && (
        <ProfileEditor
          existing={editor.existing}
          profile={editor.p}
          saved={select}
          close={() => setEditor(null)}
        />
      )}
    </>
  );
}
