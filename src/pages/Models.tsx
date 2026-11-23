import { useState, type FormEvent } from 'react';
import {
  ArrowDownToLine,
  CloudDownload,
  Database,
  FileJson,
  ListPlus,
  Plus,
  ScanLine,
  Search,
  Trash2,
} from 'lucide-react';
import { Badge, Empty, Field, FileImport, Header, Panel, useWork } from '../components/workspace';
import { api } from '../lib/api';
import type { ModelCategory, WorkflowScanResult } from '../lib/types';

const sample = JSON.stringify(
  {
    '12': {
      class_type: 'DualCLIPLoader',
      inputs: { clip_name1: 'clip_l.safetensors', clip_name2: 't5xxl_fp16.safetensors' },
    },
    '15': { class_type: 'VAELoader', inputs: { vae_name: 'ae.safetensors' } },
    '20': { class_type: 'H3C', inputs: {} },
  },
  null,
  2,
);
const categories: ModelCategory[] = [
  'diffusion_models',
  'checkpoints',
  'loras',
  'text_encoders',
  'vae',
  'controlnet',
  'semantic_bridge',
  'clip',
  'clip_vision',
  'upscale_models',
  'custom_nodes',
  'other',
];
export default function Models() {
  const { data, t, run } = useWork();
  const [raw, setRaw] = useState(''),
    [name, setName] = useState('workflow.json'),
    [scan, setScan] = useState<WorkflowScanResult | null>(null),
    [sources, setSources] = useState<Record<string, string>>({});
  const [filename, setFilename] = useState(''),
    [url, setUrl] = useState(''),
    [cat, setCat] = useState<ModelCategory>('diffusion_models'),
    [dest, setDest] = useState(''),
    [q, setQ] = useState('');
  const sessions = data.sessions.filter((s) => ['ready', 'busy'].includes(s.state));
  const [selected, setSelected] = useState('');
  const session = sessions.find((s) => s.name === selected) ?? sessions[0];
  function check(text: string, file: string) {
    setRaw(text);
    setName(file);
    setScan(null);
    void run(
      () => api('scan_workflow_models', { workflowName: file, workflowJson: text }),
      (r) => {
        setScan(r);
        setSources(Object.fromEntries(r.models.map((m) => [m.filename, m.suggestedUrl ?? ''])));
      },
    );
  }
  const missing = scan?.models.filter((m) => !m.presentOnDrive) ?? [];
  const folder = (category: ModelCategory) =>
    category === 'custom_nodes' ? 'plugins' : 'models/' + category;
  function enqueue(e: FormEvent) {
    e.preventDefault();
    void run(
      () =>
        api('enqueue_model_downloads', {
          items: missing.map((m) => ({
            filename: m.filename,
            category: m.category,
            sourceUrl: sources[m.filename],
            targetDriveDir: folder(m.category),
          })),
        }),
      undefined,
      'Saved',
    );
  }
  const queued = data.downloads.some((d) => ['queued', 'failed'].includes(d.status));
  const inventory = data.driveModels.filter((m) =>
    (m.filename + ' ' + m.relativePath).toLowerCase().includes(q.toLowerCase()),
  );
  return (
    <>
      <Header
        title="Model library & pre-downloader"
        subtitle="Inspect workflow dependencies and download models to Google Drive."
      />
      <Panel
        title="Workflow scanner"
        icon={ScanLine}
        actions={<FileImport label="Import JSON" load={check} />}
      >
        <textarea
          className="code-input"
          aria-label={t('Paste workflow JSON')}
          placeholder={t('Paste workflow JSON')}
          rows={4}
          value={raw}
          onChange={(e) => {
            setRaw(e.target.value);
            setScan(null);
          }}
        />
        <div className="actions">
          <button className="primary" disabled={!raw.trim()} onClick={() => check(raw, name)}>
            <ScanLine />
            {t('Scan workflow')}
          </button>
          <button onClick={() => check(sample, 'flux-example.json')}>
            <FileJson />
            {t('Load and scan example')}
          </button>
        </div>
        {scan && (
          <form className="stack line-top" onSubmit={enqueue}>
            <div className="chips">
              <span>{scan.workflowName}</span>
              <span>
                {t('Nodes')} · {scan.nodeCount}
              </span>
              <span>
                {t('Missing')} · {scan.missingCount}
              </span>
            </div>
            <p>
              {t('Required nodes')} · {scan.requiredCustomNodes.join(', ') || '—'}
            </p>
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    {['Filename', 'Category', 'Status', 'Source URL'].map((l) => (
                      <th key={l}>{t(l)}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {scan.models.map((m) => (
                    <tr key={m.filename}>
                      <td>{m.filename}</td>
                      <td>
                        <code>{m.category}</code>
                      </td>
                      <td>
                        <Badge
                          label={m.presentOnDrive ? 'On Drive' : 'Missing'}
                          tone={m.presentOnDrive ? 'green' : 'amber'}
                        />
                      </td>
                      <td>
                        {m.presentOnDrive ? (
                          <code>{m.matchedDrivePath ?? '—'}</code>
                        ) : (
                          <input
                            required
                            type="url"
                            aria-label={m.filename + ' URL'}
                            pattern="https?://.+"
                            placeholder="https://…"
                            value={sources[m.filename] ?? ''}
                            onChange={(e) => setSources({ ...sources, [m.filename]: e.target.value })}
                          />
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {missing.some((m) => !sources[m.filename]) && (
              <small>{t('Supply a source URL for each missing model.')}</small>
            )}
            <div>
              <button disabled={!missing.length}>
                <ListPlus />
                {t('Queue all missing models')}
              </button>
            </div>
          </form>
        )}
      </Panel>
      <Panel title="Custom download" icon={Plus}>
        <form
          className="stack"
          onSubmit={(e) => {
            e.preventDefault();
            void run(
              () =>
                api('enqueue_model_downloads', {
                  items: [
                    {
                      filename: filename.trim(),
                      sourceUrl: url.trim(),
                      category: cat,
                      targetDriveDir: dest || folder(cat),
                    },
                  ],
                }),
              () => {
                setFilename('');
                setUrl('');
              },
              'Saved',
            );
          }}
        >
          <div className="form-grid">
            <Field label="Filename">
              <input
                required
                pattern={'[^/\\\\]+'}
                value={filename}
                placeholder="model.safetensors"
                onChange={(e) => setFilename(e.target.value)}
              />
            </Field>
            <Field label="Source URL">
              <input
                required
                pattern="https?://.+"
                type="url"
                value={url}
                placeholder="https://huggingface.co/…"
                onChange={(e) => setUrl(e.target.value)}
              />
            </Field>
            <Field label="Category">
              <select value={cat} onChange={(e) => setCat(e.target.value as ModelCategory)}>
                {categories.map((c) => (
                  <option key={c}>{c}</option>
                ))}
              </select>
            </Field>
            <Field label="Drive folder (relative to root)">
              <input
                required
                pattern="(?!/)(?!.*\\.\\.(?:/|$)).+"
                title={data.settings.defaultDriveRoot}
                value={dest || folder(cat)}
                onChange={(e) => setDest(e.target.value)}
              />
            </Field>
          </div>
          <div>
            <button>
              <Plus />
              {t('Add to queue')}
            </button>
          </div>
        </form>
      </Panel>
      <Panel
        title="Download queue"
        icon={CloudDownload}
        actions={<span className="count">{data.downloads.length}</span>}
      >
        <div className="saver">
          <CloudDownload size={24} />
          <div className="actions">
            <button
              className="primary"
              disabled={!queued}
              onClick={() =>
                void run(() => api('start_model_downloads', { useCpuMode: true, sessionName: null }))
              }
            >
              {t('Start CPU downloads to 30 TB Drive')}
            </button>
            {sessions.length > 1 && (
              <select
                aria-label={t('Session')}
                value={session?.name ?? ''}
                onChange={(e) => setSelected(e.target.value)}
              >
                {sessions.map((s) => (
                  <option key={s.id}>{s.name}</option>
                ))}
              </select>
            )}
            <button
              disabled={!session || !queued}
              onClick={() =>
                void run(() =>
                  api('start_model_downloads', {
                    useCpuMode: false,
                    sessionName: session?.name ?? null,
                  }),
                )
              }
            >
              {t('Download on current instance')}
            </button>
          </div>
        </div>
        {!data.downloads.length ? (
          <Empty label="Queue models to start a download." />
        ) : (
          data.downloads.map((d) => (
            <div className="list-row" key={d.id}>
              <ArrowDownToLine size={16} />
              <div className="grow">
                <strong>{d.filename}</strong>
                <small>{d.error ?? d.targetDriveDir}</small>
                <progress max={100} value={d.progressPercent} />
              </div>
              <span className="mono">{d.progressPercent}%</span>
              <Badge
                label={d.status}
                tone={d.status === 'failed' ? 'red' : d.status === 'completed' ? 'green' : 'neutral'}
              />
              <button
                className="icon-btn"
                aria-label={t('Delete') + ' ' + d.filename}
                onClick={() => void run(() => api('remove_model_download', { taskId: d.id }))}
              >
                <Trash2 />
              </button>
            </div>
          ))
        )}
      </Panel>
      <Panel
        title="Drive inventory"
        icon={Database}
        actions={
          <label className="search">
            <Search size={15} />
            <input
              aria-label={t('Search models')}
              placeholder={t('Search models')}
              value={q}
              onChange={(e) => setQ(e.target.value)}
            />
          </label>
        }
      >
        {inventory.length ? (
          <div className="table-wrap">
            <table>
              <thead>
                <tr>
                  {['Filename', 'Category', 'Drive destination', 'Size'].map((l) => (
                    <th key={l}>{t(l)}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {inventory.map((m) => (
                  <tr key={m.relativePath}>
                    <td>{m.filename}</td>
                    <td>{m.category}</td>
                    <td>
                      <code>{m.relativePath}</code>
                    </td>
                    <td>{(m.sizeBytes / 2 ** 20).toFixed(1)} MiB</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <Empty label="No matching models" />
        )}
      </Panel>
    </>
  );
}
