import { createPortal } from 'react-dom';
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  Boxes,
  Braces,
  CheckCircle2,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  Database,
  Globe2,
  Layers3,
  Menu,
  Monitor,
  Moon,
  PanelTop,
  RefreshCw,
  Router,
  SlidersHorizontal,
  Sun,
  UsersRound,
  X,
} from 'lucide-react';
import { api, isDesktop } from './lib/api';
import { translate } from './i18n/translations';
import type { AppSettings, DashboardSnapshot, LocaleCode, ThemeMode } from './lib/types';
import { Amount, WorkContext, type Page } from './components/workspace';
import Studio from './pages/Studio';
import RuntimePreview from './pages/RuntimePreview';
import Models from './pages/Models';
import WorkflowApi from './pages/WorkflowApi';
import Accounts from './pages/Accounts';
import Plugins from './pages/Plugins';
import Settings from './pages/Settings';

const navigation = [
  { id: 'studio', label: 'Cloud studio', icon: PanelTop, view: Studio },
  { id: 'models', label: 'Model library', icon: Database, view: Models },
  { id: 'api', label: 'Workflow API', icon: Braces, view: WorkflowApi },
  { id: 'accounts', label: 'Accounts & sessions', icon: UsersRound, view: Accounts },
  { id: 'plugins', label: 'Models & plugins', icon: Boxes, view: Plugins },
  { id: 'settings', label: 'Environment & settings', icon: SlidersHorizontal, view: Settings },
] as const;
function currentPage(): Page {
  return navigation.find((n) => n.id === location.hash.slice(1))?.id ?? 'studio';
}
function Brand() {
  return (
    <div className="brand">
      <svg width="35" height="35" viewBox="0 0 36 36" fill="none" aria-hidden="true">
        <rect width="36" height="36" rx="9" fill="#91E9BE" />
        <path fill="#10392E" d="M7 25.5V11l4.9-2.8 11.4 13V10.7l5.7-3.2v17l-5.1 2.9-11.3-13v8z" />
      </svg>
      <span>
        Notebook<span className="brand-x">X</span>
        <small>Desktop</small>
      </span>
    </div>
  );
}
export default function App() {
  const [data, setData] = useState<DashboardSnapshot | null>(null),
    [loadError, setLoadError] = useState(''),
    [page, setPage] = useState<Page>(currentPage),
    [open, setOpen] = useState(false),
    [busy, setBusy] = useState(false),
    [toast, setToast] = useState<{ id: number; text: string; error: boolean } | null>(null);
  const latest = useRef(0),
    locked = useRef(false),
    scrollRef = useRef<HTMLElement>(null);
  const locale = data?.settings.locale ?? 'zh-CN';
  const t = useCallback((key: string) => translate(locale, key), [locale]);
  const notify = useCallback(
    (text: string, error = false) => setToast({ text, error, id: Date.now() }),
    [],
  );
  const refresh = useCallback(async () => {
    const version = ++latest.current;
    const snapshot = await api('get_dashboard_snapshot');
    if (version === latest.current) {
      setData(snapshot);
      setLoadError('');
    }
  }, []);
  const failed = useCallback((e: unknown) => {
    const message = e instanceof Error ? e.message : String(e);
    setLoadError(message);
  }, []);

  const liveState = useRef(data);
  liveState.current = data;
  useEffect(() => {
    let closed = false,
      pending = false,
      timer: ReturnType<typeof setTimeout>;
    async function poll() {
      if (closed || pending) return;
      clearTimeout(timer);
      pending = true;
      try {
        if (document.visibilityState === 'visible') await refresh();
      } catch (error) {
        failed(error);
      } finally {
        pending = false;
        const state = liveState.current;
        const active =
          !state ||
          locked.current ||
          state.sessions.some((s) => !['stopped', 'error'].includes(s.state)) ||
          state.downloads.some((d) => d.status === 'downloading');
        if (!closed) timer = setTimeout(poll, isDesktop() ? (active ? 2500 : 15000) : 1050);
      }
    }
    void poll();
    window.addEventListener('focus', poll);
    return () => {
      closed = true;
      clearTimeout(timer);
      window.removeEventListener('focus', poll);
    };
  }, [refresh, failed]);
  const accountId = data?.accounts.find((a) => a.isActive)?.id;
  useEffect(() => {
    if (!isDesktop() || data?.settings.backendMode !== 'real' || !accountId) return;
    let closed = false;
    let timer: ReturnType<typeof setTimeout>;
    async function updateUsage() {
      try {
        if (document.visibilityState === 'visible' && !locked.current) {
          await api('refresh_accounts_usage');
          if (!closed) await refresh();
        }
      } catch (error) {
        if (!closed) failed(error);
      } finally {
        if (!closed) timer = setTimeout(updateUsage, 60000);
      }
    }
    timer = setTimeout(updateUsage, 2000);
    return () => {
      closed = true;
      clearTimeout(timer);
    };
  }, [data?.settings.backendMode, accountId, refresh, failed]);
  useEffect(() => {
    const cb = () => {
      setPage(currentPage());
      setOpen(false);
      scrollRef.current?.scrollTo({ top: 0 });
      window.scrollTo(0, 0);
    };
    window.addEventListener('hashchange', cb);
    return () => window.removeEventListener('hashchange', cb);
  }, []);
  useEffect(() => {
    if (!toast) return;
    const timeout = setTimeout(() => setToast(null), toast.error ? 10000 : 3600);
    return () => clearTimeout(timeout);
  }, [toast]);
  const theme = data?.settings.theme ?? 'dark';
  useEffect(() => {
    const mq = matchMedia('(prefers-color-scheme: dark)');
    const apply = () => {
      document.documentElement.dataset.theme =
        theme === 'system' ? (mq.matches ? 'dark' : 'light') : theme;
    };
    apply();
    mq.addEventListener('change', apply);
    return () => mq.removeEventListener('change', apply);
  }, [theme]);
  useEffect(() => {
    document.documentElement.lang = locale;
    document.title = 'NotebookX Desktop · ' + t(navigation.find((x) => x.id === page)!.label);
  }, [locale, page, t]);
  function navigate(id: Page) {
    location.hash = id;
    setPage(id);
    setOpen(false);
  }
  async function run<T>(job: () => Promise<T>, onSuccess?: (v: T) => void, message?: string) {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    try {
      const result = await job();
      onSuccess?.(result);
      if (message) notify(t(message));
      await refresh();
    } catch (e) {
      notify(t(e instanceof Error ? e.message : String(e)), true);
    } finally {
      locked.current = false;
      setBusy(false);
    }
  }
  function settings(patch: Partial<AppSettings>) {
    if (!data) return;
    void run(
      () => api('update_settings', { settings: { ...data.settings, ...patch } }),
      undefined,
      'Saved',
    );
  }
  if (!data)
    return (
      <div className="initial">
        <Brand />
        {loadError ? (
          <>
            <p role="alert">{loadError}</p>
            <button onClick={() => void refresh().catch(failed)}>{t('Retry')}</button>
          </>
        ) : (
          <p>{t('Loading workspace…')}</p>
        )}
      </div>
    );
  const account = data.accounts.find((a) => a.isActive),
    section = navigation.find((n) => n.id === page)!,
    View = section.view,
    ThemeIcon = theme === 'system' ? Monitor : theme === 'dark' ? Moon : Sun;
  const addr = data.env.gatewayUrl.replace(/^https?:\/\//, '');
  const online = data.env.gatewayRunning;
  const plan = account?.planTier?.toUpperCase();
  const navItem = (item: (typeof navigation)[number]) => (
    <button
      className={'nav-item ' + (item.id === page ? 'current' : '')}
      key={item.id}
      aria-current={item.id === page ? 'page' : undefined}
      title={t(item.label)}
      onClick={() => navigate(item.id)}
    >
      <item.icon size={18} />
      <span>{t(item.label)}</span>
      {item.id === page && <i className="nav-dot" />}
    </button>
  );
  const previewName = !isDesktop() && new URLSearchParams(location.search).get('runtimePreview');
  return (
    <WorkContext.Provider value={{ data, t, busy, page, run, navigate, notify, settings, refresh }}>
      <div className={previewName ? 'preview-shell' : 'shell'}>
        {previewName ? (
          <RuntimePreview name={previewName} />
        ) : (
          <>
            {open && <button className="scrim" aria-label={t('Close')} onClick={() => setOpen(false)} />}
            <aside className={'sidebar ' + (open ? 'shown' : '')}>
              <Brand />
              <div className="sidebar-nav">
                <div className="nav-label">{t('Workspace')}</div>
                <nav>{navigation.slice(0, 3).map(navItem)}</nav>
                <div className="nav-label">{t('System')}</div>
                <nav>{navigation.slice(3).map(navItem)}</nav>
              </div>
              <div className="side-foot">
                <div className="side-gateway">
                  <span className={'gateway-symbol ' + (online ? 'positive' : '')}>
                    <Router size={19} />
                  </span>
                  <div>
                    <strong>{t('Local gateway')}</strong>
                    <code>{addr}</code>
                  </div>
                  <i className={'dot ' + (online ? 'positive' : '')} />
                </div>
                <div className="side-version">
                  <span>NotebookX Desktop</span>
                  <span>v0.1.0</span>
                </div>
              </div>
            </aside>
            <div className="main-shell">
              <header className="topbar">
                <button
                  className="mobile-menu icon-btn"
                  aria-label={t('Menu')}
                  onClick={() => setOpen(!open)}
                >
                  <Menu />
                </button>
                <button
                  className="account-btn"
                  onClick={() => navigate('accounts')}
                  title={account?.email ?? t('No account')}
                >
                  <span className="avatar">
                    {account?.email[0]?.toUpperCase() ?? <UsersRound size={14} />}
                  </span>
                  <span className="account-name">{account?.email ?? t('No account')}</span>
                  {plan && <span className="tier">{plan.startsWith('CO ') ? plan : 'CO ' + plan}</span>}
                </button>
                <div className="usage-metrics">
                  {[
                    ['Balance', account?.balanceCcu, 'CCU'],
                    ['Burn', account?.burnRateHourly, '/h'],
                    ['Remaining', account?.hoursRemaining, 'h'],
                  ].map(([l, v, u]) => (
                    <div key={String(l)} className="metric">
                      <small>{t(String(l))}</small>
                      <strong>
                        <Amount value={v as number | undefined} /> <em>{u}</em>
                      </strong>
                    </div>
                  ))}
                  <span className={'api-status ' + (online ? 'up' : '')}>
                    <i className="dot" /> API {addr} <span>{t(online ? 'Online' : 'Offline')}</span>
                  </span>
                </div>
                <div className="top-controls">
                  <label
                    className={
                      'select-pill mode ' + (data.settings.backendMode === 'mock' ? 'mock' : 'live')
                    }
                  >
                    <span className="dot" />
                    <select
                      aria-label={t('Backend mode')}
                      disabled={busy}
                      value={data.settings.backendMode}
                      onChange={(e) => {
                        const mode = e.target.value as 'real' | 'mock';
                        if (
                          isDesktop() &&
                          data.sessions.some((s) => s.state !== 'stopped' && !s.verifiedUnassigned)
                        ) {
                          notify(t('Release existing instances before switching backend modes.'), true);
                          return;
                        }
                        if (
                          mode === 'mock' ||
                          !isDesktop() ||
                          window.confirm(
                            t('Switch to live Colab? Launching an instance may consume compute units.'),
                          )
                        )
                          settings({ backendMode: mode });
                      }}
                    >
                      <option value="real">{t('Live Colab')}</option>
                      <option value="mock">{t('Mock sandbox')}</option>
                    </select>
                    <ChevronDown size={11} />
                  </label>
                  <label className="select-pill lang">
                    <Globe2 size={14} />
                    <select
                      aria-label={t('Language')}
                      value={locale}
                      disabled={busy}
                      onChange={(e) => settings({ locale: e.target.value as LocaleCode })}
                    >
                      <option value="zh-CN">简</option>
                      <option value="zh-TW">繁</option>
                      <option value="en">EN</option>
                    </select>
                    <ChevronDown size={10} />
                  </label>
                  <label
                    className="theme-control"
                    title={
                      t('Theme') +
                      ' · ' +
                      t(theme === 'system' ? 'System theme' : theme === 'light' ? 'Light' : 'Dark')
                    }
                  >
                    <ThemeIcon size={17} />
                    <select
                      aria-label={t('Theme')}
                      disabled={busy}
                      value={theme}
                      onChange={(e) => settings({ theme: e.target.value as ThemeMode })}
                    >
                      <option value="dark">{t('Dark')}</option>
                      <option value="light">{t('Light')}</option>
                      <option value="system">{t('System theme')}</option>
                    </select>
                  </label>
                </div>
              </header>
              <main ref={scrollRef}>
                <div className="content">
                  <div className="crumb-row">
                    <div className="crumb">
                      <Layers3 size={14} />
                      {t('Workspace')}
                      <ChevronRight size={12} />
                      <span>{t(section.label)}</span>
                    </div>
                  </div>
                  {loadError && (
                    <div role="alert" className="notice bad">
                      {loadError}
                      <button onClick={() => void refresh().catch(failed)}>{t('Retry')}</button>
                    </div>
                  )}
                  <fieldset className="page" disabled={busy}>
                    <View />
                  </fieldset>
                </div>
              </main>
            </div>
          </>
        )}
        {busy && (
          <div className="working" aria-live="polite">
            <RefreshCw className="spin" size={14} />
            {t('Working…')}
          </div>
        )}
        {toast &&
          createPortal(
            <div
              role={toast.error ? 'alert' : 'status'}
              className={'toast ' + (toast.error ? 'bad' : '')}
            >
              <span>{toast.error ? <CircleAlert size={18} /> : <CheckCircle2 size={18} />}</span>
              {toast.text}
              <button className="icon-btn" aria-label={t('Close')} onClick={() => setToast(null)}>
                <X size={15} />
              </button>
            </div>,
            document.querySelector('dialog[open]') ?? document.body,
          )}
      </div>
    </WorkContext.Provider>
  );
}
