import React, { useEffect, useState } from 'react';
import { useStore } from './store/index';
import { useIpc } from './hooks/useIpc';
import { Sidebar } from './components/Sidebar/index';
import { GameCard } from './components/GameCard/index';
import { SettingsModal } from './components/Settings/index';
import { AddGameModal } from './components/AddGameModal/index';
import { CloudSuggestModal } from './components/CloudSuggestModal/index';
import { ImageViewerModal } from './components/common/ImageViewer';
import { ToastContainer } from './components/common/Toast';
import { Icon } from './components/common/Icon';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { AppSettings, SharedGame, CloudGameSuggestion } from './types/index';

export default function App() {
  const ipc = useIpc();
  const {
    settings, setSettings, setSharedGames, setCloudStatus,
    modal, modalData, openModal, closeModal, cloudStatus, addToast,
  } = useStore();
  const [version, setVersion] = useState('');
  const [cloudSuggestions, setCloudSuggestions] = useState<CloudGameSuggestion[]>([]);
  const [initMessage, setInitMessage] = useState<string>('Démarrage…');
  const [initDone, setInitDone] = useState(false);
  const [cloudBusy, setCloudBusy] = useState<string | null>(null);
  const [showSuggest, setShowSuggest] = useState(false);

  // Refresh sidebar badges for all shared games
  const refreshAllStatuses = async () => {
    const r = await ipc.status.refreshAll();
    if (r.success && r.data) {
      const updates = r.data as Array<{ appId: string; status: string; lockedBy: string | null }>;
      updates.forEach((u) => {
        useStore.getState().updateSharedGame(u.appId, {
          cloudInfo: {
            ...(useStore.getState().sharedGames.find(g => g.appId === u.appId)?.cloudInfo as any || {}),
            status: u.status as any,
            lockedBy: u.lockedBy,
          }
        });
      });
    }
  };

  // Scan cloud for games the user can access but hasn't added yet
  const scanCloud = async () => {
    const r = await ipc.cloud_scan.scanGames();
    if (r.success && r.data) {
      const suggestions = r.data as CloudGameSuggestion[];
      if (suggestions.length > 0) {
        setCloudSuggestions(suggestions);
        setShowSuggest(true);
      }
    }
  };

  useEffect(() => {
    (async () => {
      // Local data first: the UI is shown as soon as settings and games are read
      setInitMessage('Chargement…');
      const [vr, sr, gr] = await Promise.all([ipc.app.getVersion(), ipc.settings.get(), ipc.games.getShared()]);
      if (vr.success && vr.data) setVersion(String(vr.data));
      if (gr.success && gr.data) setSharedGames(gr.data as SharedGame[]);
      const s = sr.success && sr.data ? sr.data as AppSettings : null;
      if (s) setSettings(s);
      setInitDone(true);

      // Background: Heroic artwork (local) and cloud connection
      ipc.heroic.enrichArt().then(async (r) => {
        if (!r.success || !r.data) return;
        const g = await ipc.games.getShared();
        if (g.success && g.data) setSharedGames(g.data as SharedGame[]);
      });
      if (s?.activeCloudProvider) {
        setCloudBusy('Connexion au cloud…');
        const cr = await ipc.cloud.connect(s.activeCloudProvider);
        setCloudBusy(null);
        if (cr.success && cr.data) {
          setCloudStatus({
            connected: true,
            providerType: s.activeCloudProvider,
            userInfo: cr.data as { name: string; email?: string },
          });
        } else {
          addToast({ type: 'error', message: `Connexion au cloud impossible : ${cr.error || 'erreur inconnue'}` });
        }
      }
    })();
  }, []);

  // Refresh badges and scan for new games whenever the cloud connects (in background)
  useEffect(() => {
    if (!cloudStatus.connected) return;
    setCloudBusy('Vérification du cloud…');
    Promise.all([refreshAllStatuses(), scanCloud()]).finally(() => setCloudBusy(null));
  }, [cloudStatus.connected]);

  useEffect(() => {
    if (settings && !settings.steamApiKey && modal === null) openModal('settings');
  }, [settings]);

  const providerLabel: Record<string, string> = {
    google_drive: 'Google Drive',
    nextcloud: 'Nextcloud',
    kdrive: 'kDrive',
  };

  return (
    <div className="app">
      {/* Titlebar */}
      <div className="titlebar" data-tauri-drag-region
        onMouseDown={(e) => {
          // startDragging() for Wayland; data-tauri-drag-region handles X11/Windows
          if (e.button === 0 && !(e.target as HTMLElement).closest('.titlebar__btn, .titlebar__btnVariant')) {
            try { getCurrentWindow().startDragging(); } catch (_) {}
          }
        }}>
        <span className="titlebar__logo">
          <Icon name="game" size={14} style={{ marginRight: 6 }} />
          Backseat
        </span>
        <div className="titlebar__connection">
          <div className={`titlebar__dot${cloudStatus.connected ? ' titlebar__dot--connected' : ''}`} />
          <span>
            {cloudStatus.connected && cloudStatus.providerType
              ? `${providerLabel[cloudStatus.providerType] || cloudStatus.providerType} · ${cloudStatus.userInfo?.name || ''}`
              : cloudBusy ? '' : 'Non connecté'}
          </span>
          {cloudBusy && (
            <span style={{ display: 'flex', alignItems: 'center', gap: 5, color: 'var(--text3)' }}>
              <span className="btn__spinner" style={{ width: 10, height: 10 }} />{cloudBusy}
            </span>
          )}
        </div>
        <div className="titlebar__spacer" />
        {version && (
          <span style={{ fontSize: 11, color: 'var(--text3)', marginRight: 4 }}>v{version}</span>
        )}

        <button
          className="titlebar__btn"
          onClick={() => openModal('settings')}
          title="Paramètres"
          style={{ display: 'flex', alignItems: 'center', gap: 5 }}
        >
          <Icon name="settings" size={14} />Paramètres
        </button>

        <button
          className="titlebar__btnVariant"
          onClick={() => ipc.app.quit()}
          title="Quitter"
          style={{ display: 'flex', alignItems: 'center', gap: 5, color: 'var(--red)' }}
        >
          <Icon name="quit" size={14} />Quitter
        </button>
      </div>

      <div className="app-body">
        <Sidebar />
        <main className="main"><GameCard /></main>
      </div>

      {modal === 'settings'     && <SettingsModal onClose={closeModal} />}
      {modal === 'add_game'     && <AddGameModal onClose={closeModal} />}
      {modal === 'image_viewer' && Array.isArray(modalData) && (
        <ImageViewerModal images={modalData as string[]} onClose={closeModal} />
      )}
      {showSuggest && cloudSuggestions.length > 0 && (
        <CloudSuggestModal
          suggestions={cloudSuggestions}
          onClose={() => setShowSuggest(false)}
        />
      )}

      <ToastContainer />

      {/* Init loading overlay */}
      {!initDone && (
        <div style={{
          position: 'fixed', inset: 0, zIndex: 9999,
          background: 'var(--bg)',
          display: 'flex', flexDirection: 'column',
          alignItems: 'center', justifyContent: 'center', gap: 20,
        }}>
          <div style={{ fontSize: 32, marginBottom: 4 }}>🎮</div>
          <div style={{ fontSize: 22, fontWeight: 700, color: 'var(--accent)', letterSpacing: '0.08em' }}>BACKSEAT</div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginTop: 8 }}>
            <div style={{
              width: 18, height: 18, borderRadius: '50%',
              border: '2px solid var(--border2)', borderTopColor: 'var(--accent)',
              animation: 'spin 0.7s linear infinite',
            }} />
            <span style={{ fontSize: 14, color: 'var(--text2)' }}>{initMessage}</span>
          </div>
        </div>
      )}
    </div>
  );
}
