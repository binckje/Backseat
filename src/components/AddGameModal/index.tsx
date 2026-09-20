import React, { useState, useEffect } from 'react';
import { useStore } from '../../store/index';
import { useIpc } from '../../hooks/useIpc';
import type { SteamGame, HeroicGame, SharedGame } from '../../types/index';

type Tab = 'steam' | 'heroic';

export function AddGameModal({ onClose }: { onClose: () => void }) {
  const [tab, setTab] = useState<Tab>('steam');
  return (
    <div className="modal-overlay" onClick={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal modal--wide">
        <div className="modal__header">
          <span className="modal__title">🎮 Ajouter un jeu</span>
          <button className="modal__close" onClick={onClose}>×</button>
        </div>
        <div className="settings-tabs" style={{ padding: '0 20px', marginBottom: 0 }}>
          <div className={`settings-tab${tab === 'steam' ? ' settings-tab--active' : ''}`} onClick={() => setTab('steam')}>
            🎮 Steam
          </div>
          <div className={`settings-tab${tab === 'heroic' ? ' settings-tab--active' : ''}`} onClick={() => setTab('heroic')}>
            🦸 Heroic
          </div>
        </div>
        {tab === 'steam'  && <SteamTab  onClose={onClose} />}
        {tab === 'heroic' && <HeroicTab onClose={onClose} />}
      </div>
    </div>
  );
}

function SteamTab({ onClose }: { onClose: () => void }) {
  const { addToast, setSharedGames, sharedGames, cloudStatus } = useStore();
  const ipc = useIpc();
  const [games, setGames] = useState<SteamGame[]>([]);
  const [loading, setLoading] = useState(false);
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState<SteamGame | null>(null);
  const [savePath, setSavePath] = useState('');
  const [detectMethod, setDetectMethod] = useState('');
  const [detecting, setDetecting] = useState(false);
  const [adding, setAdding] = useState(false);

  useEffect(() => {
    setLoading(true);
    ipc.steam.getGames().then((res) => {
      if (res.success && res.data)
        setGames((res.data as SteamGame[]).sort((a, b) => a.name.localeCompare(b.name)));
      else addToast({ type: 'error', message: res.error || 'Impossible de charger la bibliothèque Steam.' });
    }).finally(() => setLoading(false));
  }, []);

  const handleSelect = async (game: SteamGame) => {
    setSelected(game); setSavePath(''); setDetectMethod(''); setDetecting(true);
    const res = await ipc.steam.detectSavePath(game.appId);
    if (res.success && res.data) {
      const d = res.data as { path: string; method: string };
      setSavePath(d.path); setDetectMethod(d.method);
    }
    setDetecting(false);
  };

  const handleAdd = async () => {
    if (!selected || !savePath) return;
    if (!cloudStatus.connected) { addToast({ type: 'error', message: "Connectez-vous d'abord à un cloud drive." }); return; }
    setAdding(true);
    const res = await ipc.games.add({
      appId: selected.appId, name: selected.name, savePath,
      savePathDetectionMethod: detectMethod || 'manual',
      iconHash: selected.iconHash, launcher: 'steam',
    });
    if (res.success) {
      const u = await ipc.games.getShared();
      if (u.success && u.data) setSharedGames(u.data as typeof sharedGames);
      addToast({ type: 'success', message: `${selected.name} ajouté.` }); onClose();
    } else { addToast({ type: 'error', message: res.error || 'Erreur' }); }
    setAdding(false);
  };

  const ml: Record<string, string> = {
    steam_userdata: 'Steam userdata', pcgw: 'PCGamingWiki',
    proton: 'Proton (Linux)', manual: 'Manuel',
  };
  const added = new Set(sharedGames.map((g) => g.appId));
  const filtered = games.filter((g) =>
    !added.has(g.appId) && g.name.toLowerCase().includes(query.toLowerCase())
  );

  return (
    <>
      <div className="modal__body">
        <div className="field">
          <label className="label">Rechercher dans votre bibliothèque Steam</label>
          <input className="input" placeholder="Nom du jeu..." value={query}
            onChange={(e) => setQuery(e.target.value)} autoFocus />
        </div>
        {loading
          ? <div style={{ textAlign: 'center', padding: '20px', color: 'var(--text2)' }}>Chargement...</div>
          : <GameListPanel items={filtered} selectedId={selected?.appId}
              onSelect={(g) => handleSelect(g as SteamGame)}
              getId={(g) => (g as SteamGame).appId}
              getName={(g) => (g as SteamGame).name}
              getSub={(g) => { const sg = g as SteamGame; return `AppID ${sg.appId}${sg.playtime > 0 ? ` · ${Math.round(sg.playtime/60)}h` : ''}`; }}
              getIcon={(g) => `https://cdn.cloudflare.steamstatic.com/steam/apps/${(g as SteamGame).appId}/capsule_sm_120.jpg`}
            />
        }
        {selected && (
          <div className="field">
            <label className="label">
              Dossier de sauvegarde
              {detectMethod && <span className="tag tag--accent tag--method" style={{ marginLeft: 8 }}>{ml[detectMethod] || detectMethod}</span>}
            </label>
            {detecting
              ? <div style={{ fontSize: 13, color: 'var(--text2)', padding: '8px 0' }}>Détection...</div>
              : <div className="input-row">
                  <input className="input" value={savePath}
                    onChange={(e) => { setSavePath(e.target.value); setDetectMethod('manual'); }}
                    placeholder="Chemin vers les sauvegardes" />
                  <button className="btn btn--secondary" onClick={async () => {
                    const r = await ipc.dialog.openFolder();
                    if (r.success && r.data) setSavePath(r.data as string);
                  }}>Parcourir</button>
                </div>
            }
          </div>
        )}
      </div>
      <div className="modal__footer">
        <button className="btn btn--ghost" onClick={onClose}>Annuler</button>
        <button className="btn btn--primary" onClick={handleAdd}
          disabled={!selected || !savePath || adding || detecting}>
          {adding && <span className="btn__spinner" />}Ajouter le jeu
        </button>
      </div>
    </>
  );
}

type PlatformFilter = 'all' | 'epic' | 'gog';

function HeroicTab({ onClose }: { onClose: () => void }) {
  const { addToast, setSharedGames, sharedGames, cloudStatus } = useStore();
  const ipc = useIpc();
  const [games, setGames] = useState<HeroicGame[]>([]);
  const [loading, setLoading] = useState(false);
  const [heroicMissing, setHeroicMissing] = useState(false);
  const [query, setQuery] = useState('');
  const [platformFilter, setPlatformFilter] = useState<PlatformFilter>('all');
  const [selected, setSelected] = useState<HeroicGame | null>(null);
  const [savePath, setSavePath] = useState('');
  const [suggestions, setSuggestions] = useState<string[]>([]);
  const [loadingSugg, setLoadingSugg] = useState(false);
  const [adding, setAdding] = useState(false);

  useEffect(() => {
    setLoading(true);
    ipc.heroic.getGames().then((res) => {
      if (res.success && res.data) setGames(res.data as HeroicGame[]);
      else setHeroicMissing(true);
    }).finally(() => setLoading(false));
  }, []);

  const handleSelect = async (game: HeroicGame) => {
    setSelected(game); setSavePath(game.savePath || ''); setSuggestions([]);
    setLoadingSugg(true);
    const res = await ipc.heroic.suggestSavePaths(game.appName, game.platform);
    const sugg = (res.success && res.data) ? res.data as string[] : [];
    setSuggestions(sugg);
    if (!game.savePath && sugg.length > 0) setSavePath(sugg[0]);
    setLoadingSugg(false);
  };

  const handleAdd = async () => {
    if (!selected || !savePath) return;
    if (!cloudStatus.connected) { addToast({ type: 'error', message: "Connectez-vous d'abord à un cloud drive." }); return; }
    setAdding(true);
    const res = await ipc.games.add({
      appId: selected.appName, name: selected.title, savePath,
      savePathDetectionMethod: selected.savePath ? 'heroic' : 'manual',
      launcher: 'heroic', heroPlatform: selected.platform,
      coverUrl: selected.coverUrl,
      backgroundUrl: selected.backgroundUrl,
    });
    if (res.success) {
      const u = await ipc.games.getShared();
      if (u.success && u.data) setSharedGames(u.data as typeof sharedGames);
      addToast({ type: 'success', message: `${selected.title} ajouté.` }); onClose();
    } else { addToast({ type: 'error', message: res.error || 'Erreur' }); }
    setAdding(false);
  };

  if (heroicMissing) return (
    <>
      <div className="modal__body" style={{ textAlign: 'center', padding: '40px 20px', color: 'var(--text2)' }}>
        <div style={{ fontSize: 40, marginBottom: 12 }}>🦸</div>
        <div style={{ fontWeight: 600, marginBottom: 8 }}>Heroic Games Launcher non détecté</div>
        <div style={{ fontSize: 13 }}>Installez Heroic et lancez-le au moins une fois pour qu'il génère ses fichiers de config.</div>
      </div>
      <div className="modal__footer"><button className="btn btn--ghost" onClick={onClose}>Fermer</button></div>
    </>
  );

  const added = new Set(sharedGames.map((g) => g.appId));
  const filtered = games.filter((g) => {
    if (added.has(g.appName)) return false;
    if (platformFilter !== 'all' && g.platform !== platformFilter) return false;
    return g.title.toLowerCase().includes(query.toLowerCase());
  });

  const epicDetected = selected?.platform === 'epic' && !!selected?.savePath;
  const showSuggestions = !epicDetected && suggestions.length > 0;

  return (
    <>
      <div className="modal__body">
        <div style={{ display: 'flex', gap: 6, marginBottom: 14 }}>
          {(['all', 'epic', 'gog'] as PlatformFilter[]).map((f) => (
            <button key={f} onClick={() => setPlatformFilter(f)}
              className={`btn btn--sm ${platformFilter === f ? 'btn--primary' : 'btn--secondary'}`}>
              {f === 'all' ? 'Tous' : f === 'epic' ? '🟣 Epic' : '🔵 GOG'}
            </button>
          ))}
          <span style={{ marginLeft: 'auto', fontSize: 12, color: 'var(--text3)', alignSelf: 'center' }}>
            {filtered.length} jeu{filtered.length !== 1 ? 'x' : ''}
          </span>
        </div>
        <div className="field">
          <label className="label">Rechercher dans Heroic</label>
          <input className="input" placeholder="Nom du jeu..." value={query}
            onChange={(e) => setQuery(e.target.value)} autoFocus />
        </div>
        {loading
          ? <div style={{ textAlign: 'center', padding: '20px', color: 'var(--text2)' }}>Chargement...</div>
          : <GameListPanel items={filtered} selectedId={selected?.appName}
              onSelect={(g) => handleSelect(g as HeroicGame)}
              getId={(g) => (g as HeroicGame).appName}
              getName={(g) => (g as HeroicGame).title}
              getSub={(g) => {
                const hg = g as HeroicGame;
                return `${hg.platform === 'epic' ? '🟣 Epic' : '🔵 GOG'} · ${hg.isNative ? 'Natif Linux' : 'Wine'}`;
              }}
              getIcon={() => ''}
              getBadge={(g) => (g as HeroicGame).platform === 'epic' ? '🟣' : '🔵'}
            />
        }
        {selected && (
          <div className="field" style={{ marginTop: 8 }}>
            <label className="label">
              Dossier de sauvegarde
              {epicDetected && <span className="tag tag--accent tag--method" style={{ marginLeft: 8 }}>Détecté par Heroic</span>}
              {!epicDetected && selected.isNative && <span className="tag tag--method" style={{ marginLeft: 8 }}>Natif Linux</span>}
              {!epicDetected && !selected.isNative && <span className="tag tag--method" style={{ marginLeft: 8 }}>Wine</span>}
            </label>
            {loadingSugg
              ? <div style={{ fontSize: 13, color: 'var(--text2)', padding: '8px 0' }}>Recherche des chemins...</div>
              : <>
                  {showSuggestions && (
                    <div style={{ marginBottom: 8, display: 'flex', flexDirection: 'column', gap: 4 }}>
                      <div style={{ fontSize: 11, color: 'var(--text3)', marginBottom: 4 }}>
                        {selected.isNative ? 'Dossiers détectés :' : 'Dossiers Wine suggérés :'}
                      </div>
                      {suggestions.map((s) => (
                        <button key={s} onClick={() => setSavePath(s)} style={{
                          textAlign: 'left', padding: '6px 10px', borderRadius: 6,
                          fontSize: 11, fontFamily: 'monospace', cursor: 'pointer',
                          background: savePath === s ? 'var(--accent-glow)' : 'var(--surface3)',
                          border: savePath === s ? '1px solid var(--accent)' : '1px solid var(--border)',
                          color: 'var(--text)', wordBreak: 'break-all',
                        }}>{s}</button>
                      ))}
                    </div>
                  )}
                  <div className="input-row">
                    <input className="input" value={savePath}
                      onChange={(e) => setSavePath(e.target.value)}
                      placeholder="Chemin vers les sauvegardes" />
                    <button className="btn btn--secondary" onClick={async () => {
                      const r = await ipc.dialog.openFolder();
                      if (r.success && r.data) setSavePath(r.data as string);
                    }}>Parcourir</button>
                  </div>
                </>
            }
          </div>
        )}
      </div>
      <div className="modal__footer">
        <button className="btn btn--ghost" onClick={onClose}>Annuler</button>
        <button className="btn btn--primary" onClick={handleAdd}
          disabled={!selected || !savePath || adding || loadingSugg}>
          {adding && <span className="btn__spinner" />}Ajouter le jeu
        </button>
      </div>
    </>
  );
}

interface GameListPanelProps<T> {
  items: T[]; selectedId?: string;
  onSelect: (g: T) => void;
  getId: (g: T) => string;
  getName: (g: T) => string;
  getSub: (g: T) => string;
  getIcon: (g: T) => string;
  getBadge?: (g: T) => string;
}

function GameListPanel<T>({ items, selectedId, onSelect, getId, getName, getSub, getIcon, getBadge }: GameListPanelProps<T>) {
  if (items.length === 0) return (
    <div style={{ padding: 20, textAlign: 'center', color: 'var(--text3)', fontSize: 13, border: '1px solid var(--border)', borderRadius: 8, marginBottom: 16 }}>
      Aucun jeu correspondant.
    </div>
  );
  return (
    <div style={{ maxHeight: 220, overflowY: 'auto', border: '1px solid var(--border)', borderRadius: 8, marginBottom: 16 }}>
      {items.map((g) => {
        const id = getId(g);
        const active = id === selectedId;
        const icon = getIcon(g);
        return (
          <div key={id} onClick={() => onSelect(g)} style={{
            display: 'flex', alignItems: 'center', gap: 10, padding: '8px 12px', cursor: 'pointer',
            background: active ? 'var(--surfaceEmerald-select)' : 'transparent',
            borderLeft: active ? '3px solid var(--txtEmerald)' : '3px solid transparent',
          }}>
            {icon
              ? <img src={icon} alt="" style={{ width: 36, height: 36, borderRadius: 4, objectFit: 'cover', background: 'var(--surface3)' }}
                  onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }} />
              : <div style={{ width: 36, height: 36, borderRadius: 4, background: 'var(--surface3)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 20 }}>
                  {getBadge ? getBadge(g) : '🎮'}
                </div>
            }
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 13, fontWeight: 500, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{getName(g)}</div>
              <div style={{ fontSize: 11, color: 'var(--text3)' }}>{getSub(g)}</div>
            </div>
            {active && <span style={{ color: 'var(--accent)', fontSize: 16 }}>✓</span>}
          </div>
        );
      })}
    </div>
  );
}
