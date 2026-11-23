import { useState } from 'react';
import { Bot, Boxes, Cpu, Package, Play, ScanSearch, Send } from 'lucide-react';
import { api, type Message } from '../lib/api';
import type { AcceleratorKind, GgufRepoInspection, LlamaDeployment } from '../lib/types';
import { Badge, Empty, Field, Header, Panel, useWork } from '../components/workspace';

const repos = ['bartowski/Qwen2.5-32B-Instruct-GGUF', 'unsloth/DeepSeek-R1-Distill-Qwen-32B-GGUF'];
const targets: [AcceleratorKind, number][] = [
  ['T4', 15],
  ['L4', 24],
  ['A100', 40],
  ['A100', 80],
  ['G4', 96],
];
function Chat({ deployment: d }: { deployment: LlamaDeployment }) {
  const { run, t } = useWork();
  const [messages, setMessages] = useState<Message[]>([]),
    [draft, setDraft] = useState('');
  return (
    <div className="stack">
      <div className="split">
        <strong>{d.repoId}</strong>
        <Badge label={d.status} tone={d.status === 'ready' ? 'green' : 'neutral'} />
      </div>
      <div className="chips">
        <span>{d.chosenQuant}</span>
        <span>{d.ctxSize} ctx</span>
      </div>
      <div className="endpoint">
        <span>OpenAI API</span>
        <code>{d.apiBaseUrl}</code>
      </div>
      <h3>{t('Chat')}</h3>
      {messages.length > 0 && (
        <div className="chat-history" aria-live="polite">
          {messages.map((m, i) => (
            <div className={'chat-message ' + m.role} key={i}>
              <strong>{m.role === 'user' ? t('You') : 'llama.cpp'}</strong>
              <p>{m.content}</p>
            </div>
          ))}
        </div>
      )}
      <form
        className="actions"
        onSubmit={(e) => {
          e.preventDefault();
          if (!draft.trim()) return;
          const next: Message[] = [...messages, { role: 'user', content: draft.trim() }];
          void run(
            () =>
              api('chat_completion_proxy', {
                apiBaseUrl: d.apiBaseUrl,
                messages: next,
                model: d.repoId,
              }),
            (reply) => {
              setMessages([...next, { role: 'assistant', content: reply }]);
              setDraft('');
            },
          );
        }}
      >
        <input
          className="grow"
          aria-label={t('Message')}
          placeholder={t('Message')}
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
        />
        <button className="primary" disabled={!draft.trim()}>
          <Send />
          {t('Send')}
        </button>
      </form>
    </div>
  );
}
export default function Plugins() {
  const { data, t, run } = useWork();
  const [repo, setRepo] = useState(repos[0]),
    [target, setTarget] = useState(1),
    [vram, setVram] = useState(24),
    [port, setPort] = useState(8000),
    [inspect, setInspect] = useState<GgufRepoInspection | null>(null),
    [quant, setQuant] = useState(''),
    [active, setActive] = useState('');
  const d = data.llamaDeployments.find((x) => x.id === active) ?? data.llamaDeployments[0];
  return (
    <>
      <Header
        title="GGUF deployment & plugins"
        subtitle="Inspect GGUF files, deploy llama.cpp, and test an OpenAI-compatible endpoint."
      />
      <Panel title="Model repository" icon={Boxes}>
        <form
          className="stack"
          onSubmit={(e) => {
            e.preventDefault();
            void run(
              () => api('inspect_gguf_repo', { repoId: repo.trim(), targetVramMib: vram * 1024 }),
              (r) => {
                setInspect(r);
                setQuant(r.autoPickedQuant ?? '');
              },
            );
          }}
        >
          <Field label="Hugging Face GGUF">
            <input
              required
              pattern="[^ /]+/[^ /]+"
              value={repo}
              onChange={(e) => {
                setRepo(e.target.value);
                setInspect(null);
              }}
            />
          </Field>
          <div className="actions">
            {repos.map((r) => (
              <button
                type="button"
                className="text-btn"
                key={r}
                onClick={() => {
                  setRepo(r);
                  setInspect(null);
                }}
              >
                {r.split('/')[1]}
              </button>
            ))}
          </div>
          <div className="form-grid">
            <Field label="Target GPU">
              <select
                value={target}
                onChange={(e) => {
                  const i = Number(e.target.value);
                  setTarget(i);
                  setVram(targets[i][1]);
                  setInspect(null);
                }}
              >
                {targets.map(([gpu, gb], i) => (
                  <option value={i} key={i}>
                    {gpu} · {gb} GB
                  </option>
                ))}
              </select>
            </Field>
            <Field label="Minimum VRAM (GB)">
              <input
                required
                type="number"
                min="1"
                max="1024"
                value={vram}
                onChange={(e) => {
                  setVram(+e.target.value);
                  setInspect(null);
                }}
              />
            </Field>
          </div>
          <div>
            <button className="primary">
              <ScanSearch />
              {t('Inspect and select quantization')}
            </button>
          </div>
        </form>
      </Panel>
      {inspect && (
        <Panel title="Quantization" icon={Cpu}>
          <form
            className="stack"
            onSubmit={(e) => {
              e.preventDefault();
              void run(
                () =>
                  api('deploy_gguf_model', {
                    repoId: inspect.repoId,
                    accelerator: targets[target][0],
                    minVramGb: vram,
                    quant,
                    localPort: port,
                  }),
                (v) => setActive(v.id),
              );
            }}
          >
            <div className="table-wrap">
              <table>
                <thead>
                  <tr>
                    {['Quantization', 'Size', 'Estimated VRAM', 'Status'].map((l) => (
                      <th key={l}>{t(l)}</th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {inspect.variants.map((v) => (
                    <tr key={v.quant} className={v.isAutoSelected ? 'recommended' : ''}>
                      <td>
                        <label className="radio">
                          <input
                            type="radio"
                            name="quant"
                            checked={v.quant === quant}
                            disabled={!v.fitsCurrentTarget}
                            onChange={() => setQuant(v.quant)}
                          />
                          {v.quant}
                        </label>
                      </td>
                      <td>{(v.sizeBytes / 2 ** 30).toFixed(1)} GiB</td>
                      <td>{(v.estimatedVramMib / 1024).toFixed(1)} GiB</td>
                      <td>
                        <Badge
                          label={
                            v.isAutoSelected
                              ? 'Recommended'
                              : v.fitsCurrentTarget
                                ? 'Fits'
                                : 'Insufficient VRAM'
                          }
                          tone={v.isAutoSelected ? 'green' : 'neutral'}
                        />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div className="actions">
              <Field label="Local port">
                <input
                  type="number"
                  min={1}
                  max={65535}
                  required
                  value={port}
                  onChange={(e) => setPort(+e.target.value)}
                />
              </Field>
              <button className="primary" disabled={!quant}>
                <Play />
                {t('Deploy llama.cpp')}
              </button>
            </div>
          </form>
        </Panel>
      )}
      {d && (
        <Panel
          title="Deployments"
          icon={Bot}
          actions={
            <select
              aria-label={t('Deployments')}
              value={d.id}
              onChange={(e) => setActive(e.target.value)}
            >
              {data.llamaDeployments.map((d) => (
                <option key={d.id} value={d.id}>
                  {d.repoId} · {d.localPort}
                </option>
              ))}
            </select>
          }
        >
          <Chat key={d.id} deployment={d} />
        </Panel>
      )}
      <Panel title="Installed plugins" icon={Package} actions={<code>plugin.toml</code>}>
        {data.plugins.map((p) => (
          <div className="list-row" key={p.id}>
            <span className="plugin-icon">
              <Package size={19} />
            </span>
            <div className="grow">
              <strong>{p.name[data.settings.locale] ?? p.name.en ?? p.id}</strong>
              <small>{p.description[data.settings.locale] ?? p.description.en}</small>
              <code>{p.scriptPath}</code>
            </div>
            <span className="chip">{p.defaultAccelerator}</span>
            <code>v{p.version}</code>
          </div>
        ))}
        {!data.plugins.length && <Empty label="No plugins installed" />}
      </Panel>
    </>
  );
}
