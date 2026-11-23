import { useState } from 'react';
import { ChevronDown, Code2, Copy, FileJson, Network, Pencil, Play, Plus, Trash2 } from 'lucide-react';
import { api } from '../lib/api';
import type { JsonValue, SavedApiWorkflow, WorkflowRunRequest, WorkflowRunResult } from '../lib/types';
import {
  Badge,
  Empty,
  Field,
  FileImport,
  Header,
  Modal,
  Panel,
  Toggle,
  useWork,
} from '../components/workspace';

const boundKeys = ['prompt', 'seed', 'frames'];
function readGraph(raw: string) {
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw Error('Invalid workflow JSON');
  }
  const graph = parsed?.prompt ?? parsed;
  if (
    !graph ||
    typeof graph !== 'object' ||
    Array.isArray(graph) ||
    !Object.values(graph).some((n) => typeof n === 'object' && n !== null && 'class_type' in n)
  )
    throw Error('Invalid workflow JSON');
  return graph as Record<string, { class_type: string; inputs: Record<string, JsonValue> }>;
}
function inferBindings(graph: ReturnType<typeof readGraph>) {
  const seen = new Set<string>();
  return Object.entries(graph).flatMap(([nodeId, node]) =>
    Object.entries(node?.inputs ?? {}).flatMap(([inputField, v]) => {
      const key =
        inputField === 'text'
          ? 'prompt'
          : inputField === 'noise_seed'
            ? 'seed'
            : inputField === 'length'
              ? 'frames'
              : inputField;
      if (!boundKeys.includes(key) || seen.has(key) || !['number', 'string'].includes(typeof v))
        return [];
      seen.add(key);
      return [{ key, label: key, nodeId, inputField, valueType: typeof v, defaultValue: v }];
    }),
  );
}
function TemplateEditor({
  initial,
  close,
  saved,
}: {
  initial: SavedApiWorkflow;
  close: () => void;
  saved: (id: string) => void;
}) {
  const { data, t, run, notify } = useWork();
  const [w, set] = useState(initial),
    [raw, setRaw] = useState(initial.apiGraphJson ? JSON.stringify(initial.apiGraphJson, null, 2) : '');
  function load(text: string, file: string) {
    try {
      const graph = readGraph(text);
      setRaw(JSON.stringify(graph, null, 2));
      set({ ...w, name: w.name || file.replace(/\.json$/i, ''), bindings: inferBindings(graph) });
    } catch {
      notify(t('Invalid workflow JSON'), true);
    }
  }
  return (
    <Modal title="Import API workflow" close={close}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void run(
            async () => {
              const graph = readGraph(raw);
              const keys = new Set();
              for (const b of w.bindings) {
                if (
                  !graph[b.nodeId] ||
                  !(
                    b.inputField in (graph[b.nodeId].inputs ?? {}) ||
                    (b.inputField.startsWith('timeline.boxes.0.') &&
                      typeof graph[b.nodeId].inputs?.timeline === 'string')
                  ) ||
                  keys.has(b.key)
                )
                  throw Error('Invalid parameter binding');
                keys.add(b.key);
              }
              return api('save_api_workflow', {
                workflow: { ...w, apiGraphJson: graph, updatedAt: new Date().toISOString() },
              });
            },
            () => {
              saved(w.id);
              close();
            },
            'Saved',
          );
        }}
      >
        <div className="modal-body stack">
          <div className="actions">
            <FileImport label="Import JSON" load={load} />
            <code>workflow_api.json</code>
          </div>
          <Field label="Paste workflow JSON">
            <textarea
              className="code-input"
              required
              rows={5}
              value={raw}
              onChange={(e) => setRaw(e.target.value)}
              onBlur={() => {
                if (!w.bindings.length && raw)
                  try {
                    set({ ...w, bindings: inferBindings(readGraph(raw)) });
                  } catch {
                    /* validation on save */
                  }
              }}
            />
          </Field>
          <div className="form-grid">
            <Field label="Name">
              <input required value={w.name} onChange={(e) => set({ ...w, name: e.target.value })} />
            </Field>
            <Field label="Bound profile">
              <select
                required
                value={w.profileId}
                onChange={(e) => set({ ...w, profileId: e.target.value })}
              >
                {data.profiles.map((p) => (
                  <option key={p.id} value={p.id}>
                    {t(p.name)}
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Output type">
              <select value={w.outputKind} onChange={(e) => set({ ...w, outputKind: e.target.value })}>
                {['video', 'image', 'audio', 'json'].map((x) => (
                  <option key={x}>{x}</option>
                ))}
              </select>
            </Field>
            <Toggle
              label="Allow continuous shots"
              checked={w.supportsContinuity}
              onChange={(v) => set({ ...w, supportsContinuity: v })}
            />
          </div>
          <div className="section-head">
            <h3>{t('Parameter bindings')}</h3>
            <button
              type="button"
              className="icon-btn"
              aria-label={t('Parameter bindings')}
              onClick={() =>
                set({
                  ...w,
                  bindings: [
                    ...w.bindings,
                    {
                      key: '',
                      label: '',
                      nodeId: '',
                      inputField: '',
                      valueType: 'string',
                      defaultValue: '',
                    },
                  ],
                })
              }
            >
              <Plus />
            </button>
          </div>
          {w.bindings.map((b, i) => (
            <div className="binding" key={i}>
              {(['key', 'nodeId', 'inputField'] as const).map((k, j) => (
                <Field key={k} label={['Parameter', 'Node', 'Input'][j]}>
                  <input
                    required
                    value={b[k]}
                    onChange={(e) =>
                      set({
                        ...w,
                        bindings: w.bindings.map((v, n) =>
                          n === i
                            ? {
                                ...v,
                                [k]: e.target.value,
                                ...(k === 'key' ? { label: e.target.value } : {}),
                              }
                            : v,
                        ),
                      })
                    }
                  />
                </Field>
              ))}
              <button
                type="button"
                className="icon-btn"
                aria-label={t('Delete')}
                onClick={() => set({ ...w, bindings: w.bindings.filter((_, n) => n !== i) })}
              >
                <Trash2 />
              </button>
            </div>
          ))}
        </div>
        <div className="modal-foot">
          <button type="button" onClick={close}>
            {t('Cancel')}
          </button>
          <button className="primary">{t('Save')}</button>
        </div>
      </form>
    </Modal>
  );
}
export function Runner({ w }: { w: SavedApiWorkflow }) {
  const { data, t, run, notify } = useWork();
  const defaults = Object.fromEntries(w.bindings.map((b) => [b.key, b.defaultValue]));
  const [params, setParams] = useState<Record<string, JsonValue>>({
      prompt: '',
      seed: 42,
      frames: 49,
      ...defaults,
    }),
    [mode, setMode] = useState('start'),
    [parent, setParent] = useState(''),
    [sequence, setSequence] = useState(''),
    [result, setResult] = useState<WorkflowRunResult | null>(null),
    [lang, setLang] = useState('curl');
  const keys = Array.from(new Set([...boundKeys, ...w.bindings.map((b) => b.key)]));
  const request: WorkflowRunRequest = {
    workflowId: w.id,
    profileId: w.profileId,
    rawPromptGraph: null,
    params,
    continuityMode: w.supportsContinuity ? mode : null,
    parentTaskId: w.supportsContinuity && mode === 'continue' ? parent : null,
    sequenceName: w.supportsContinuity ? sequence || null : null,
    autoWake: data.settings.autoWakeOnApiCall,
    waitForCompletion: true,
  };
  const url = data.env.gatewayUrl.replace(/\/$/, '') + '/api/v1/comfy/run';
  const payload = JSON.stringify(request, null, 2);
  const shellQuote = (s: string) => "'" + s.replaceAll("'", "'\\''") + "'";
  const snippet =
    lang === 'curl'
      ? 'curl -X POST ' +
        shellQuote(url) +
        ' \\\n  -H ' +
        shellQuote('Content-Type: application/json') +
        ' \\\n  --data-binary ' +
        shellQuote(payload)
      : 'import json\nimport requests\n\npayload = json.loads(' +
        JSON.stringify(payload) +
        ')\nresponse = requests.post(' +
        JSON.stringify(url) +
        ', json=payload, timeout=1800)\nresponse.raise_for_status()\nprint(response.json())';
  return (
    <>
      <Panel title="Workflow runner" icon={Play}>
        <form
          className="stack"
          onSubmit={(e) => {
            e.preventDefault();
            setResult(null);
            void run(() => api('run_api_workflow', { request }), setResult);
          }}
        >
          <div className="form-grid">
            {keys.map((k) => {
              const number = typeof params[k] === 'number' || k === 'frames' || k === 'seed';
              const label = boundKeys.includes(k)
                ? k[0].toUpperCase() + k.slice(1)
                : w.bindings.find((b) => b.key === k)?.label || k;
              return (
                <Field key={k} wide={k === 'prompt'} label={label}>
                  {k === 'prompt' ? (
                    <textarea
                      rows={3}
                      value={String(params[k] ?? '')}
                      onChange={(e) => setParams({ ...params, [k]: e.target.value })}
                    />
                  ) : (
                    <input
                      type={number ? 'number' : 'text'}
                      min={k === 'frames' ? 1 : undefined}
                      value={String(params[k] ?? '')}
                      onChange={(e) =>
                        setParams({ ...params, [k]: number ? Number(e.target.value) : e.target.value })
                      }
                    />
                  )}
                </Field>
              );
            })}
            {w.supportsContinuity && (
              <>
                <Field label="Continuity">
                  <select value={mode} onChange={(e) => setMode(e.target.value)}>
                    <option value="start">start · {t('First shot')}</option>
                    <option value="continue">continue · {t('Continue sequence')}</option>
                  </select>
                </Field>
                <Field label="Sequence name">
                  <input
                    required={mode === 'continue'}
                    value={sequence}
                    onChange={(e) => setSequence(e.target.value)}
                  />
                </Field>
                {mode === 'continue' && (
                  <Field label="Parent task ID" wide>
                    <input required value={parent} onChange={(e) => setParent(e.target.value)} />
                  </Field>
                )}
              </>
            )}
          </div>
          <div>
            <button className="primary">
              <Play />
              {t(data.settings.autoWakeOnApiCall ? 'Run test with auto-wake' : 'Run test')}
            </button>
          </div>
        </form>
        {result && (
          <div className="line-top">
            <h3>
              {t('Run result')} <Badge label={result.status} tone={result.error ? 'red' : 'green'} />
            </h3>
            <pre>{JSON.stringify(result, null, 2)}</pre>
          </div>
        )}
      </Panel>
      <Panel
        title="Request examples"
        icon={Code2}
        actions={
          <div className="actions">
            <select
              aria-label={t('Request examples')}
              value={lang}
              onChange={(e) => setLang(e.target.value)}
            >
              <option>curl</option>
              <option>Python</option>
            </select>
            <button
              onClick={() =>
                void run(
                  () => navigator.clipboard.writeText(snippet),
                  () => notify(t('Copied')),
                )
              }
            >
              <Copy />
              {t('Copy')}
            </button>
          </div>
        }
      >
        <pre>{snippet}</pre>
      </Panel>
    </>
  );
}
export default function WorkflowApi() {
  const { t, data, settings, run } = useWork();
  const [selected, setSelected] = useState(''),
    [editing, setEditing] = useState<SavedApiWorkflow | null>(null);
  const w = data.apiWorkflows.find((w) => w.id === selected) ?? data.apiWorkflows[0];
  return (
    <>
      <Header
        title="Serverless Workflow API"
        subtitle="Calls to the local gateway can wake a GPU, mount Drive, run the workflow and return its outputs. The profile controls idle shutdown."
      />
      <Panel
        title="Local gateway"
        icon={Network}
        actions={
          <Badge
            dot
            tone={data.env.gatewayRunning ? 'green' : 'neutral'}
            label={data.env.gatewayRunning ? 'Online' : 'Offline'}
          />
        }
      >
        <div className="split">
          <code>{data.env.gatewayUrl}</code>
          <Toggle
            label="Auto-wake on API call"
            checked={data.settings.autoWakeOnApiCall}
            onChange={(v) => settings({ autoWakeOnApiCall: v })}
          />
        </div>
      </Panel>
      <Panel
        title="Workflow templates"
        icon={FileJson}
        actions={
          <button
            disabled={!data.profiles.length}
            onClick={() =>
              setEditing({
                id: crypto.randomUUID(),
                name: '',
                description: '',
                profileId: data.profiles[0]?.id ?? '',
                outputKind: 'image',
                supportsContinuity: false,
                bindings: [],
                apiGraphJson: null,
                updatedAt: '',
              })
            }
          >
            <Plus />
            {t('Import API workflow')}
          </button>
        }
      >
        {w ? (
          <>
            <div className="actions">
              <select
                aria-label={t('Workflow templates')}
                value={w.id}
                onChange={(e) => setSelected(e.target.value)}
              >
                {data.apiWorkflows.map((x) => (
                  <option key={x.id} value={x.id}>
                    {t(x.name)}
                  </option>
                ))}
              </select>
              <button onClick={() => setEditing(structuredClone(w))}>
                <Pencil />
                {t('Edit')}
              </button>
              <button
                className="icon-btn"
                aria-label={t('Delete')}
                onClick={() => {
                  if (window.confirm(t('Remove this item?')))
                    void run(() => api('delete_api_workflow', { workflowId: w.id }));
                }}
              >
                <Trash2 />
              </button>
            </div>
            <div className="chips line-top">
              <span>
                {t('Bound profile')} ·{' '}
                {t(data.profiles.find((p) => p.id === w.profileId)?.name ?? w.profileId)}
              </span>
              <span>{w.outputKind}</span>
            </div>
            <details>
              <summary>
                {t('Parameter bindings')} · {w.bindings.length}
                <ChevronDown size={14} />
              </summary>
              {w.bindings.map((b) => (
                <div className="list-row" key={b.key}>
                  <strong>{b.key}</strong>
                  <code>
                    #{b.nodeId} / {b.inputField}
                  </code>
                  <small>{b.valueType}</small>
                </div>
              ))}
              {!w.bindings.length && <Empty label="No parameter bindings" />}
            </details>
          </>
        ) : (
          <Empty label="Import an API workflow to run a test." />
        )}
      </Panel>
      {w && <Runner key={w.id + '-' + w.updatedAt} w={w} />}{' '}
      {editing && (
        <TemplateEditor initial={editing} close={() => setEditing(null)} saved={setSelected} />
      )}
    </>
  );
}
