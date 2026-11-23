import { useState } from 'react';
import {
  CheckCircle2,
  CircleDashed,
  Download,
  FolderCog,
  KeyRound,
  ListChecks,
  RefreshCw,
  Save,
} from 'lucide-react';
import { api, isDesktop } from '../lib/api';
import { Badge, Field, Header, Panel, Toggle, useWork } from '../components/workspace';
function Preferences() {
  const { data, settings, t } = useWork();
  const [root, setRoot] = useState(data.settings.defaultDriveRoot),
    [drama, setDrama] = useState(data.settings.dramaSmithApiUrl),
    [tray, setTray] = useState(data.settings.minimizeToTray),
    [sync, setSync] = useState(data.settings.autoSyncDramaSmith);
  return (
    <form
      className="stack"
      onSubmit={(e) => {
        e.preventDefault();
        settings({
          defaultDriveRoot: root.trim(),
          dramaSmithApiUrl: drama.trim(),
          minimizeToTray: tray,
          autoSyncDramaSmith: sync,
        });
      }}
    >
      <div className="form-grid">
        <Field label="Drive root">
          <input required value={root} onChange={(e) => setRoot(e.target.value)} />
        </Field>
        <Field label="DramaSmith URL">
          <input
            type="url"
            required
            pattern="https?://.+"
            value={drama}
            onChange={(e) => setDrama(e.target.value)}
          />
        </Field>
      </div>
      <Toggle label="Minimize to tray" checked={tray} onChange={setTray} />
      <Toggle label="Sync DramaSmith automatically" checked={sync} onChange={setSync} />
      <div>
        <button>
          <Save />
          {t('Save')}
        </button>
      </div>
    </form>
  );
}
function Secret({ name, label, exists }: { name: string; label: string; exists: boolean }) {
  const { t, run } = useWork();
  const [value, setValue] = useState('');
  return (
    <form
      className="secret"
      onSubmit={(e) => {
        e.preventDefault();
        void run(
          () => api('save_secret', { name, value }),
          () => setValue(''),
          'Saved',
        );
      }}
    >
      <div className="split">
        <strong>{t(label)}</strong>
        <Badge label={exists ? 'Configured' : 'Not configured'} tone={exists ? 'green' : 'neutral'} />
      </div>
      <div className="actions">
        {name === 'rclone_config' ? (
          <textarea
            aria-label={t(label)}
            className="grow"
            value={value}
            autoComplete="off"
            spellCheck={false}
            rows={2}
            onChange={(e) => setValue(e.target.value)}
          />
        ) : (
          <input
            aria-label={t(label)}
            className="grow"
            type="password"
            autoComplete="new-password"
            placeholder="••••••••"
            value={value}
            onChange={(e) => setValue(e.target.value)}
          />
        )}
        <button disabled={!value.trim()}>{t('Save')}</button>
      </div>
    </form>
  );
}
export default function Settings() {
  const { data, t, run, navigate } = useWork();
  const e = data.env;
  const checks: [string, boolean, string | null][] = [
    ['Python', e.pythonFound, e.pythonPath],
    ['uv', e.uvFound, e.uvPath],
    [
      'google-colab-cli',
      e.colabCliFound,
      [e.colabCliVersion, e.colabCliPath].filter(Boolean).join(' · '),
    ],
    ['Windows OpenSSH', e.sshFound, e.sshPath],
    ['SSH key', e.sshKeyReady, e.sshKeyPath],
    [
      'Google authorization',
      data.accounts.some((a) => a.isActive),
      data.accounts.find((a) => a.isActive)?.email ?? null,
    ],
  ];
  return (
    <>
      <Header
        title="Environment & setup"
        subtitle="Verify local dependencies, configure storage, and manage credentials."
      />
      <Panel
        title="Setup checklist"
        icon={ListChecks}
        actions={
          <button onClick={() => void run(() => api('check_environment'))}>
            <RefreshCw />
            {t('Check environment')}
          </button>
        }
      >
        {checks.map(([label, ok, path]) => (
          <div className="list-row" key={label}>
            {ok ? <CheckCircle2 size={17} className="positive" /> : <CircleDashed size={17} />}
            <div className="grow">
              <strong>{t(label)}</strong>
              {path && <small>{path}</small>}
            </div>
            <Badge label={ok ? 'Available' : 'Action needed'} tone={ok ? 'green' : 'amber'} />
            {label === 'google-colab-cli' && !ok && (
              <button onClick={() => void run(() => api('install_colab_cli'))}>
                <Download />
                {t('Install Colab CLI')}
              </button>
            )}
            {label === 'Google authorization' && !ok && (
              <button onClick={() => navigate('accounts')}>{t('Add Google account')}</button>
            )}
          </div>
        ))}
      </Panel>
      <Panel title="Preferences" icon={FolderCog}>
        <Preferences />
      </Panel>
      <Panel title="Secret vault" icon={KeyRound}>
        {!isDesktop() && <p>{t('Sandbox stores configuration flags only.')}</p>}
        <Secret name="hf_token" label="Hugging Face Token" exists={data.settings.hasHfToken} />
        <Secret name="civitai_token" label="Civitai API Key" exists={data.settings.hasCivitaiToken} />
        <Secret
          name="rclone_config"
          label="rclone unattended config"
          exists={data.settings.hasRcloneConfig}
        />
      </Panel>
    </>
  );
}
