import { createContext, useContext, useEffect, useRef, type ReactNode } from 'react';
import { X, Check, Circle, type LucideIcon } from 'lucide-react';
import type { AppSettings, CostGuardPolicy, DashboardSnapshot } from '../lib/types';
export type Page = 'studio' | 'models' | 'api' | 'accounts' | 'plugins' | 'settings';
export interface Workspace {
  data: DashboardSnapshot;
  t: (key: string) => string;
  busy: boolean;
  page: Page;
  navigate: (p: Page) => void;
  run: <T>(job: () => Promise<T>, onSuccess?: (value: T) => void, message?: string) => Promise<void>;
  settings: (patch: Partial<AppSettings>) => void;
  notify: (message: string, error?: boolean) => void;
  refresh: () => Promise<void>;
}
export const WorkContext = createContext<Workspace | null>(null);
export function useWork() {
  const ctx = useContext(WorkContext);
  if (!ctx) throw Error('Workspace unavailable');
  return ctx;
}
export function Header({
  title,
  subtitle,
  actions,
}: {
  title: string;
  subtitle: string;
  actions?: ReactNode;
}) {
  const { t } = useWork();
  return (
    <header className="page-head">
      <div>
        <h1>{t(title)}</h1>
        <p>{t(subtitle)}</p>
      </div>
      {actions}
    </header>
  );
}
export function Panel({
  title,
  icon: Icon,
  actions,
  children,
  className = '',
}: {
  title: string;
  icon?: LucideIcon;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  const { t } = useWork();
  return (
    <section className={'panel ' + className}>
      <div className="section-head">
        <h2>
          {Icon && <Icon size={17} />} {t(title)}
        </h2>
        {actions}
      </div>
      {children}
    </section>
  );
}
export function Field({
  label,
  children,
  wide = false,
}: {
  label: string;
  children: ReactNode;
  wide?: boolean;
}) {
  const { t } = useWork();
  return (
    <label className={'field ' + (wide ? 'wide' : '')}>
      <span>{t(label)}</span>
      {children}
    </label>
  );
}
export function Toggle({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  const { t } = useWork();
  return (
    <label className="toggle">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span className="switch" />
      {t(label)}
    </label>
  );
}
export function Badge({
  label,
  tone = 'neutral',
  dot = false,
}: {
  label: string;
  tone?: string;
  dot?: boolean;
}) {
  const { t } = useWork();
  return (
    <span className={'badge ' + tone}>
      {dot && <i className="dot" />}
      {t(label)}
    </span>
  );
}
export function Modal({
  title,
  close,
  children,
}: {
  title: string;
  close: () => void;
  children: ReactNode;
}) {
  const { t } = useWork();
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const el = ref.current;
    el?.showModal();
    return () => el?.close();
  }, []);
  return (
    <dialog
      ref={ref}
      aria-label={t(title)}
      onCancel={(e) => {
        e.preventDefault();
        close();
      }}
      onClick={(e) => {
        if (e.target === ref.current) {
          const r = ref.current.getBoundingClientRect();
          if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom)
            close();
        }
      }}
    >
      <div className="modal-head">
        <h2>{t(title)}</h2>
        <button type="button" className="icon-btn" aria-label={t('Close')} onClick={close}>
          <X size={17} />
        </button>
      </div>
      {children}
    </dialog>
  );
}
export function GuardFields({
  value,
  change,
}: {
  value: CostGuardPolicy;
  change: (value: CostGuardPolicy) => void;
}) {
  const fields = [
    ['autoStopMinutes', 'Scheduled stop (min)'],
    ['idleStopMinutes', 'Idle stop (min)'],
    ['spendingCapCcu', 'Spending cap (CCU)'],
    ['balanceFloorCcu', 'Balance floor (CCU)'],
  ] as const;
  return (
    <>
      <div className="form-grid">
        {fields.map(([key, label]) => (
          <Field key={key} label={label}>
            <input
              type="number"
              min="0"
              step={key.endsWith('Ccu') ? 'any' : 1}
              value={value[key] ?? ''}
              onChange={(e) =>
                change({ ...value, [key]: e.target.value === '' ? null : Number(e.target.value) })
              }
            />
          </Field>
        ))}
      </div>
      <Toggle
        label="Back up before shutdown"
        checked={value.safeBackupBeforeStop}
        onChange={(v) => change({ ...value, safeBackupBeforeStop: v })}
      />
    </>
  );
}
export function External({ url, children }: { url: string; children: ReactNode }) {
  let valid = false;
  try {
    valid = ['http:', 'https:'].includes(new URL(url).protocol);
  } catch {
    /* malformed endpoint */
  }
  return valid ? (
    <a className="button" target="_blank" rel="noopener noreferrer" href={url}>
      {children}
    </a>
  ) : (
    <span>{children}</span>
  );
}
export function FileImport({
  label,
  load,
}: {
  label: string;
  load: (raw: string, name: string) => void;
}) {
  const { t, notify } = useWork();
  return (
    <label className="button upload">
      {t(label)}
      <input
        type="file"
        accept=".json,application/json"
        onChange={async (e) => {
          const el = e.currentTarget;
          const f = el.files?.[0];
          if (!f) return;
          try {
            load(await f.text(), f.name);
          } catch (err) {
            notify(String(err), true);
          }
          el.value = '';
        }}
      />
    </label>
  );
}
export function Amount({ value }: { value: number | null | undefined }) {
  const { data } = useWork();
  return (
    <>{value == null ? '—' : value.toLocaleString(data.settings.locale, { maximumFractionDigits: 1 })}</>
  );
}
export function Verified() {
  const { t } = useWork();
  return (
    <div role="status" className="notice good">
      <Check size={16} />
      {t('Google instance release verified · no further charges')}
    </div>
  );
}
export function Empty({ label }: { label: string }) {
  const { t } = useWork();
  return (
    <p className="empty">
      <Circle size={12} />
      {t(label)}
    </p>
  );
}
