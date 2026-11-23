import { Input } from '../components/ui/input';
import { Button } from '../components/ui/button';
import { Textarea } from '../components/ui/textarea';
import { Tabs, TabsList, TabsTrigger, TabsPanel } from '../components/ui/tabs';
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
          <Input required value={root} onChange={(e) => setRoot(e.target.value)} />
        </Field>
        <Field label="DramaSmith URL">
          <Input
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
        <Button variant="outline" type="submit">
          <Save />
          {t('Save')}
        </Button>
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
          <Textarea
            aria-label={t(label)}
            className="grow"
            value={value}
            autoComplete="off"
            spellCheck={false}
            rows={2}
            onChange={(e) => setValue(e.target.value)}
          />
        ) : (
          <Input
            aria-label={t(label)}
            className="grow"
            type="password"
            autoComplete="new-password"
            placeholder="••••••••"
            value={value}
            onChange={(e) => setValue(e.target.value)}
          />
        )}
        <Button variant="outline" type="submit" disabled={!value.trim()}>{t('Save')}</Button>
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
      <Tabs defaultValue="setup" className="space-y-4">
        <TabsList aria-label={t('Settings')}>
          <TabsTrigger value="setup">{t('Setup checklist')}</TabsTrigger>
          <TabsTrigger value="preferences">{t('Preferences')}</TabsTrigger>
          <TabsTrigger value="secrets">{t('Secret vault')}</TabsTrigger>
        </TabsList>
        <TabsPanel value="setup" keepMounted>
      <Panel
        title="Setup checklist"
        icon={ListChecks}
        actions={
          <Button variant="outline" onClick={() => void run(() => api('check_environment'))}>
            <RefreshCw />
            {t('Check environment')}
          </Button>
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
              <Button variant="outline" onClick={() => void run(() => api('install_colab_cli'))}>
                <Download />
                {t('Install Colab CLI')}
              </Button>
            )}
            {label === 'Google authorization' && !ok && (
              <Button variant="outline" onClick={() => navigate('accounts')}>{t('Add Google account')}</Button>
            )}
          </div>
        ))}
      </Panel>
        </TabsPanel>
        <TabsPanel value="preferences" keepMounted>
      <Panel title="Preferences" icon={FolderCog}>
        <Preferences />
      </Panel>
        </TabsPanel>
        <TabsPanel value="secrets" keepMounted>
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
        </TabsPanel>
      </Tabs>
    </>
  );
}
