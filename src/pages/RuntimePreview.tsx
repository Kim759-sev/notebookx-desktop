import { Select } from '../components/ui/select';
import { useState } from 'react';
import { ArrowUpRight, Braces, Cpu, Workflow } from 'lucide-react';
import { Badge, Empty, External, Header, Panel, useWork } from '../components/workspace';
import { Runner } from './WorkflowApi';

export function runtimePreviewUrl(sessionName: string) {
  const url = new URL(window.location.href);
  url.hash = '';
  url.searchParams.set('runtimePreview', sessionName);
  return url.toString();
}

export default function RuntimePreview({ name }: { name: string }) {
  const { data, t, busy } = useWork();
  const [selected, setSelected] = useState('');
  const s = data.sessions.find((s) => s.name === name);
  const workflows = data.apiWorkflows.filter((w) => w.profileId === s?.profileId);
  const w = workflows.find((w) => w.id === selected) ?? workflows[0];
  const graph = w?.apiGraphJson;
  const nodes = graph && typeof graph === 'object' && !Array.isArray(graph) ? Object.entries(graph) : [];
  return (
    <main className="content runtime-preview">
      <div className="crumb-row">
        <span className="crumb">
          <Workflow size={16} />
          NotebookX Desktop / ComfyUI
        </span>
        <Badge dot label="Mock sandbox" tone="green" />
      </div>
      <fieldset className="page" disabled={busy}>
        <Header
          title="Mock ComfyUI runtime"
          subtitle="Sandbox workflow runs simulate inference and output paths."
        />
        {s?.comfyReady ? (
          <>
            <Panel title="Runtime" icon={Cpu} actions={<Badge label={s.state} tone="green" />}>
              <div className="split">
                <div>
                  <strong>{s.name}</strong>
                  <p>
                    {t(s.profileName ?? '')} · {s.gpuName} · {Math.round((s.vramTotalMib ?? 0) / 1024)}{' '}
                    GB
                  </p>
                </div>
                {s.comfyLocalUrl && (
                  <External url={s.comfyLocalUrl}>
                    {t('Open the live endpoint')}
                    <ArrowUpRight size={14} />
                  </External>
                )}
              </div>
            </Panel>
            <Panel
              title="Workflow graph"
              icon={Braces}
              actions={
                w && (
                  <Select
                    aria-label={t('Workflow templates')}
                    value={w.id}
                    onChange={(e) => setSelected(e.target.value)}
                  >
                    {workflows.map((v) => (
                      <option key={v.id} value={v.id}>
                        {t(v.name)}
                      </option>
                    ))}
                  </Select>
                )
              }
            >
              {nodes.length ? (
                <div className="node-grid">
                  {nodes.map(([id, raw]) => {
                    const node = raw && typeof raw === 'object' && !Array.isArray(raw) ? raw : {};
                    return (
                      <div className="graph-node" key={id}>
                        <small>#{id}</small>
                        <strong>{String(node.class_type ?? 'Node')}</strong>
                      </div>
                    );
                  })}
                </div>
              ) : (
                <Empty label="Import an API workflow to run a test." />
              )}
            </Panel>
            {w && <Runner key={w.id} w={w} />}
          </>
        ) : (
          <Empty label="Awaiting runtime" />
        )}
      </fieldset>
    </main>
  );
}
