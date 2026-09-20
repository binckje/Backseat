import React, { useState } from 'react';
import { useStore } from '../../store/index';
import { useIpc } from '../../hooks/useIpc';
import { Icon } from '../common/Icon';
import type { CloudGameSuggestion, SharedGame } from '../../types/index';

interface Props {
  suggestions: CloudGameSuggestion[];
  onClose: () => void;
}

export function CloudSuggestModal({ suggestions, onClose }: Props) {
  const ipc = useIpc();
  const { addToast, setSharedGames, sharedGames } = useStore();
  const [adding, setAdding] = useState<string | null>(null);
  const [added, setAdded] = useState<Set<string>>(new Set());
  const [paths, setPaths] = useState<Record<string, string>>({});
  const [detecting, setDetecting] = useState<string | null>(null);

  const handleDetect = async (appId: string) => {
    setDetecting(appId);
    const r = await ipc.steam.detectSavePath(appId);
    if (r.success && r.data) {
      const d = r.data as { path: string; method: string };
      setPaths((prev) => ({ ...prev, [appId]: d.path }));
    }
    setDetecting(null);
  };

  const handleAdd = async (s: CloudGameSuggestion) => {
    const savePath = paths[s.appId] || '';
    if (!savePath) {
      addToast({ type: 'warning', message: 'Indiquez le dossier de sauvegarde avant d\'ajouter.' });
      return;
    }
    setAdding(s.appId);
    const r = await ipc.games.add({
      appId: s.appId,
      name: s.gameName,
      savePath,
      savePathDetectionMethod: 'manual',
    });
    if (r.success) {
      const gr = await ipc.games.getShared();
      if (gr.success && gr.data) setSharedGames(gr.data as SharedGame[]);
      setAdded((prev) => new Set([...prev, s.appId]));
      addToast({ type: 'success', message: `${s.gameName} ajouté !` });
    } else {
      addToast({ type: 'error', message: r.error || 'Erreur lors de l\'ajout' });
    }
    setAdding(null);
  };

  const handleBrowse = async (appId: string) => {
    const r = await ipc.dialog.openFolder();
    if (r.success && r.data) {
      setPaths((prev) => ({ ...prev, [appId]: r.data as string }));
    }
  };

  const remaining = suggestions.filter((s) => !added.has(s.appId));

  return (
    <div className="modal-overlay" onClick={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal modal--wide">
        <div className="modal__header">
          <span className="modal__title">☁️ Jeux disponibles sur le cloud</span>
          <button className="modal__close" onClick={onClose}>×</button>
        </div>

        <div className="modal__body">
          <p className="text-muted text-sm" style={{ marginBottom: 18, lineHeight: 1.6 }}>
            Ces jeux sont présents sur votre drive et vous avez les permissions pour y accéder,
            mais ils ne sont pas encore dans Backseat. Indiquez le chemin de sauvegarde pour les ajouter.
          </p>

          {remaining.length === 0 ? (
            <div style={{ textAlign: 'center', padding: '24px 0', color: 'var(--green)', display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 8 }}>
              <Icon name="check" size={18} color="var(--green)" />
              Tous les jeux ont été ajoutés !
            </div>
          ) : remaining.map((s) => (
            <div key={s.appId} style={{
              border: '1px solid var(--border)',
              borderRadius: 'var(--radius)',
              padding: 16,
              marginBottom: 12,
              background: 'var(--surface3)',
            }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 12 }}>
                <img
                  src={`https://cdn.cloudflare.steamstatic.com/steam/apps/${s.appId}/capsule_sm_120.jpg`}
                  alt={s.gameName}
                  style={{ width: 48, height: 32, borderRadius: 4, objectFit: 'cover', background: 'var(--surface)' }}
                  onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }}
                />
                <div style={{ flex: 1 }}>
                  <div style={{ fontWeight: 600, fontSize: 14 }}>{s.gameName}</div>
                  <div style={{ fontSize: 11, color: 'var(--text3)', marginTop: 2 }}>
                    AppID {s.appId} · {s.isOwner ? 'Vous êtes propriétaire' : 'Accès partagé'}
                  </div>
                </div>
                {s.isOwner && (
                  <span style={{ fontSize: 11, padding: '2px 8px', borderRadius: 10, background: 'var(--accent-glow)', color: 'var(--accent)', fontWeight: 600 }}>
                    Propriétaire
                  </span>
                )}
              </div>

              <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
                <input
                  className="input"
                  style={{ flex: 1, fontSize: 12 }}
                  placeholder="Dossier de sauvegarde…"
                  value={paths[s.appId] || ''}
                  onChange={(e) => setPaths((prev) => ({ ...prev, [s.appId]: e.target.value }))}
                />
                <button
                  className="btn btn--secondary btn--sm"
                  onClick={() => handleDetect(s.appId)}
                  disabled={detecting === s.appId}
                  title="Détecter automatiquement"
                >
                  {detecting === s.appId ? <span className="btn__spinner" /> : <Icon name="search" size={13} />}
                </button>
                <button
                  className="btn btn--secondary btn--sm"
                  onClick={() => handleBrowse(s.appId)}
                  title="Parcourir"
                >
                  <Icon name="folder" size={13} />
                </button>
                <button
                  className="btn btn--primary btn--sm"
                  onClick={() => handleAdd(s)}
                  disabled={adding === s.appId || !paths[s.appId]}
                  style={{ whiteSpace: 'nowrap' }}
                >
                  {adding === s.appId ? <span className="btn__spinner" /> : <Icon name="plus" size={13} />}
                  Ajouter
                </button>
              </div>
            </div>
          ))}
        </div>

        <div className="modal__footer">
          <button className="btn btn--ghost" onClick={onClose}>
            {remaining.length === 0 ? 'Fermer' : 'Ignorer pour l\'instant'}
          </button>
        </div>
      </div>
    </div>
  );
}
