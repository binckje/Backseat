import React, { useState, useEffect } from 'react';
import { useStore } from '../../store/index';
import { useIpc } from '../../hooks/useIpc';
import type { AppSettings, CloudProviderType } from '../../types/index';

type Tab = 'steam' | 'cloud' | 'debug';

export function SettingsModal({ onClose }: { onClose: () => void }) {
  const { settings, setSettings, setCloudStatus, addToast, cloudStatus } = useStore();
  const ipc = useIpc();
  const [tab, setTab] = useState<Tab>('steam');
  const [local, setLocal] = useState<AppSettings | null>(settings ? { ...settings, backupPath: settings.backupPath ?? '' } : null);
  const [saving, setSaving] = useState(false);
  const [connecting, setConnecting] = useState<CloudProviderType | null>(null);
  const [gdriveStep, setGdriveStep] = useState<'form'|'code'>('form');
  const [gdriveAuthUrl, setGdriveAuthUrl] = useState('');
  const [gdriveCode, setGdriveCode] = useState('');

  useEffect(() => { if (settings) setLocal({ ...settings, backupPath: settings.backupPath ?? '' }); }, [settings]);
  if (!local) return null;

  const set = (p: Partial<AppSettings>) => setLocal((prev) => ({ ...prev!, ...p }));
  const setProvider = (type: CloudProviderType, patch: object) => {
    const providers = { ...(local.cloudProviders || {}) };
    providers[type] = { ...(providers[type] || { type, active: false }), ...patch } as typeof providers[typeof type];
    set({ cloudProviders: providers });
  };

  const handleSave = async () => {
    setSaving(true);
    await ipc.settings.save(local);
    const r = await ipc.settings.get();
    if (r.success && r.data) setSettings(r.data as AppSettings);
    addToast({ type: 'success', message: 'Paramètres sauvegardés.' });
    setSaving(false);
  };

  const handleConnect = async (type: CloudProviderType) => {
    if (type === 'google_drive') {
      await ipc.settings.save(local);
      const r = await ipc.cloud.gdriveGetAuthUrl();
      if (!r.success) { addToast({ type: 'error', message: r.error || 'Erreur' }); return; }
      setGdriveAuthUrl(r.data as string); setGdriveStep('code'); return;
    }
    setConnecting(type);
    await ipc.settings.save(local);
    const r = await ipc.cloud.connect(type);
    if (r.success && r.data) {
      const info = r.data as { name: string; email?: string };
      setCloudStatus({ connected: true, providerType: type, userInfo: info });
      addToast({ type: 'success', message: `Connecté : ${info.name}` });
      const sr = await ipc.settings.get();
      if (sr.success && sr.data) setSettings(sr.data as AppSettings);
    } else { addToast({ type: 'error', message: r.error || 'Connexion échouée' }); }
    setConnecting(null);
  };

  const handleGdriveCode = async () => {
    setConnecting('google_drive');
    const r = await ipc.cloud.gdriveExchangeCode(gdriveCode.trim());
    if (!r.success) { addToast({ type: 'error', message: r.error || 'Code invalide' }); setConnecting(null); return; }
    const cr = await ipc.cloud.connect('google_drive');
    if (cr.success && cr.data) {
      const info = cr.data as { name: string; email?: string };
      setCloudStatus({ connected: true, providerType: 'google_drive', userInfo: info });
      addToast({ type: 'success', message: `Connecté : ${info.name}` });
      setGdriveStep('form'); setGdriveCode('');
      const sr = await ipc.settings.get(); if (sr.success && sr.data) setSettings(sr.data as AppSettings);
    } else { addToast({ type: 'error', message: cr.error || 'Erreur' }); }
    setConnecting(null);
  };

  const handleDisconnect = async () => {
    await ipc.cloud.disconnect();
    setCloudStatus({ connected: false, providerType: null });
    const r = await ipc.settings.get(); if (r.success && r.data) setSettings(r.data as AppSettings);
    addToast({ type: 'info', message: 'Déconnecté.' });
  };

  const gd = local.cloudProviders?.google_drive?.googleDrive;
  const nc = local.cloudProviders?.nextcloud?.nextcloud;
  const kd = local.cloudProviders?.kdrive?.kdrive;

  return (
    <div className="modal-overlay" onClick={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal modal--wide">
        <div className="modal__header"><span className="modal__title">⚙️ Paramètres</span><button className="modal__close" onClick={onClose}>×</button></div>
        <div className="modal__body">
          <div className="settings-tabs">
            <div className={`settings-tab${tab==='steam'?' settings-tab--active':''}`} onClick={()=>setTab('steam')}>🎮 Steam</div>
            <div className={`settings-tab${tab==='cloud'?' settings-tab--active':''}`} onClick={()=>setTab('cloud')}>☁️ Cloud Drive</div>
          <div className={`settings-tab${tab==='debug'?' settings-tab--active':''}`} onClick={()=>setTab('debug')}>🔧 Debug</div>
          </div>

          {tab === 'steam' && <>
            <div className="field"><label className="label">Steam Web API Key</label>
              <input className="input" type="password" placeholder="steamcommunity.com/dev/apikey" value={local.steamApiKey} onChange={(e)=>set({steamApiKey:e.target.value})} />
            </div>
            <div className="field"><label className="label">Steam ID (64-bit)</label>
              <input className="input" placeholder="Ex : 76561198000000000" value={local.steamId} onChange={(e)=>set({steamId:e.target.value})} />
            </div>
            <div className="field"><label className="label">Dossier des backups locaux</label>
              <div className="input-row">
                <input className="input" value={local.backupPath||''} onChange={(e)=>set({backupPath:e.target.value})} placeholder="Par défaut : ~/.backseat/backups" />
                <button className="btn btn--secondary" onClick={async()=>{ const r=await ipc.dialog.openFolder(); if(r.success&&r.data) set({backupPath:r.data as string}); }}>Parcourir</button>
              </div>
            </div>
            <hr className="divider" />
            <div className="field"><label className="label">Dossier Steam</label>
              <div className="input-row">
                <input className="input" value={local.steamPath} onChange={(e)=>set({steamPath:e.target.value})} placeholder="Chemin vers Steam..." />
                <button className="btn btn--secondary" onClick={async()=>{ const r=await ipc.settings.detectSteamPath(); if(r.success&&r.data) set({steamPath:r.data as string}); }}>Détecter</button>
              </div>
            </div>
          </>}

          {tab === 'debug' && <DebugTab ipc={ipc} local={local} setLocal={setLocal} />}
          {tab === 'cloud' && <>
            <p className="text-muted text-sm" style={{marginBottom:16}}>Un seul drive peut être actif à la fois.</p>
            {cloudStatus.connected && (
              <div style={{ padding:'10px 14px',background:'rgba(74,222,128,.08)',border:'1px solid rgba(74,222,128,.2)',borderRadius:8,marginBottom:16,display:'flex',alignItems:'center',gap:10 }}>
                <span style={{color:'var(--green)'}}>● Connecté</span>
                <span className="text-muted text-sm" style={{flex:1}}>{cloudStatus.userInfo?.name}{cloudStatus.userInfo?.email?` — ${cloudStatus.userInfo.email}`:''}</span>
                <button className="btn btn--sm btn--danger" onClick={handleDisconnect}>Déconnecter</button>
              </div>
            )}

            {/* Nextcloud */}
            <div className={`provider-card${local.activeCloudProvider==='nextcloud'?' provider-card--active':''}`}>
              <div className="provider-card__header"><span className="provider-card__icon">🟠</span><span className="provider-card__name">Nextcloud</span>{local.activeCloudProvider==='nextcloud'&&<span className="provider-card__badge">Actif</span>}</div>
              <div className="field"><label className="label">URL du serveur</label><input className="input" placeholder="https://nextcloud.example.com" value={nc?.serverUrl||''} onChange={(e)=>setProvider('nextcloud',{nextcloud:{...(nc||{}),serverUrl:e.target.value}})} /></div>
              <div className="field"><label className="label">Nom d'utilisateur</label><input className="input" value={nc?.username||''} onChange={(e)=>setProvider('nextcloud',{nextcloud:{...(nc||{}),username:e.target.value}})} /></div>
              <div className="field"><label className="label">Mot de passe / App password</label><input className="input" type="password" value={nc?.password||''} onChange={(e)=>setProvider('nextcloud',{nextcloud:{...(nc||{}),password:e.target.value}})} /></div>
              <button className="btn btn--primary btn--sm" disabled={!nc?.serverUrl||!nc?.username||!nc?.password||connecting==='nextcloud'} onClick={()=>handleConnect('nextcloud')}>
                {connecting==='nextcloud'&&<span className="btn__spinner"/>}Connecter
              </button>
            </div>

            {/* kDrive */}
            <div className={`provider-card${local.activeCloudProvider==='kdrive'?' provider-card--active':''}`}>
              <div className="provider-card__header"><span className="provider-card__icon">🟣</span><span className="provider-card__name">kDrive (Infomaniak)</span>{local.activeCloudProvider==='kdrive'&&<span className="provider-card__badge">Actif</span>}</div>
              <div className="field"><label className="label">Email Infomaniak</label><input className="input" type="email" placeholder="user@example.com" value={kd?.username||''} onChange={(e)=>setProvider('kdrive',{kdrive:{...(kd||{}),username:e.target.value}})} /></div>
              <div className="field"><label className="label">App password kDrive</label><input className="input" type="password" placeholder="Générez-en un dans votre espace Infomaniak" value={kd?.appPassword||''} onChange={(e)=>setProvider('kdrive',{kdrive:{...(kd||{}),appPassword:e.target.value}})} /></div>
              <button className="btn btn--primary btn--sm" disabled={!kd?.username||!kd?.appPassword||connecting==='kdrive'} onClick={()=>handleConnect('kdrive')}>
                {connecting==='kdrive'&&<span className="btn__spinner"/>}Connecter
              </button>
            </div>

            {/* Google Drive */}
            <div className={`provider-card${local.activeCloudProvider==='google_drive'?' provider-card--active':''}`}>
              <div className="provider-card__header"><span className="provider-card__icon">🔵</span><span className="provider-card__name">Google Drive</span>{local.activeCloudProvider==='google_drive'&&<span className="provider-card__badge">Actif</span>}</div>
              <div className="field"><label className="label">Client ID</label><input className="input" placeholder="xxx.apps.googleusercontent.com" value={gd?.clientId||''} onChange={(e)=>setProvider('google_drive',{googleDrive:{...(gd||{}),clientId:e.target.value}})} /></div>
              <div className="field"><label className="label">Client Secret</label><input className="input" type="password" placeholder="GOCSPX-..." value={gd?.clientSecret||''} onChange={(e)=>setProvider('google_drive',{googleDrive:{...(gd||{}),clientSecret:e.target.value}})} /></div>
              {gdriveStep==='code' ? (
                <div>
                  <p className="text-sm text-muted" style={{marginBottom:10}}>Ouvrez ce lien, autorisez, copiez le code :</p>
                  <a href={gdriveAuthUrl} onClick={(e)=>{e.preventDefault();}} style={{color:'var(--accent)',fontSize:12,wordBreak:'break-all',display:'block',marginBottom:10}}>{gdriveAuthUrl.slice(0,80)}…</a>
                  <div className="input-row" style={{marginBottom:8}}>
                    <input className="input" placeholder="Code d'autorisation" value={gdriveCode} onChange={(e)=>setGdriveCode(e.target.value)} />
                    <button className="btn btn--primary" onClick={handleGdriveCode} disabled={connecting==='google_drive'}>{connecting==='google_drive'?<span className="btn__spinner"/>:null}Valider</button>
                  </div>
                  <button className="btn btn--ghost btn--sm" onClick={()=>setGdriveStep('form')}>Annuler</button>
                </div>
              ) : (
                <button className="btn btn--primary btn--sm" disabled={!gd?.clientId||!gd?.clientSecret||connecting==='google_drive'} onClick={()=>handleConnect('google_drive')}>
                  {connecting==='google_drive'&&<span className="btn__spinner"/>}{gd?.accessToken?'Reconnecter':'Autoriser via OAuth2'}
                </button>
              )}
            </div>
          </>}
        </div>
        <div className="modal__footer">
          <VersionBadge />
          <button className="btn btn--ghost" onClick={onClose}>Fermer</button>
          <button className="btn btn--primary" onClick={handleSave} disabled={saving}>{saving&&<span className="btn__spinner"/>}Enregistrer</button>
        </div>
      </div>
    </div>
  );
}

function VersionBadge() {
  const ipc = useIpc();
  const [v, setV] = React.useState('');
  React.useEffect(() => { ipc.app.getVersion().then((r) => { if (r.success && r.data) setV(String(r.data)); }); }, []);
  return <span style={{ fontSize:11, color:'var(--text3)', marginRight:'auto' }}>Backseat{v?` v${v}`:''}</span>;
}

// ─── Debug Tab ────────────────────────────────────────────────────────────────

function DebugTab({ ipc, local, setLocal }: {
  ipc: ReturnType<typeof useIpc>;
  local: any;
  setLocal: (p: any) => void;
}) {
  const [logPath, setLogPath] = React.useState('');
  const [restarting, setRestarting] = React.useState(false);

  React.useEffect(() => {
    ipc.debug.getLogPath().then((r) => { if (r.success && r.data) setLogPath(r.data as string); });
  }, []);

  const handleToggleDebug = async (enabled: boolean) => {
    setLocal({ ...local, debugLogging: enabled });
    await ipc.debug.setDebugLogging(enabled);
  };

  return (
    <div>
      <div style={{ padding: '12px 14px', background: 'var(--surface3)', borderRadius: 8, marginBottom: 16, fontSize: 13, color: 'var(--text2)', lineHeight: 1.6 }}>
        Le mode debug active des logs détaillés : appels réseau, API Steam, opérations cloud, détection des chemins, permissions. Ces logs peuvent aider à diagnostiquer les problèmes.
      </div>

      <div className="field">
        <label className="label">Mode debug</label>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12, padding: '10px 14px', background: 'var(--surface3)', borderRadius: 8 }}>
          <div style={{ flex: 1 }}>
            <div style={{ fontSize: 13, fontWeight: 500 }}>Logs détaillés</div>
            <div style={{ fontSize: 12, color: 'var(--text2)', marginTop: 2 }}>
              Prend effet au prochain lancement · peut aussi être activé avec <code style={{ background: 'var(--bg)', padding: '1px 5px', borderRadius: 4 }}>--verbose</code> ou <code style={{ background: 'var(--bg)', padding: '1px 5px', borderRadius: 4 }}>BACKSEAT_LOG=debug</code>
            </div>
          </div>
          <label style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer' }}>
            <input type="checkbox"
              checked={!!(local as any).debugLogging}
              onChange={(e) => handleToggleDebug(e.target.checked)}
              style={{ width: 16, height: 16, accentColor: 'var(--accent)' }}
            />
            <span style={{ fontSize: 13 }}>{(local as any).debugLogging ? 'Activé' : 'Désactivé'}</span>
          </label>
        </div>
      </div>

      <div className="field">
        <label className="label">Fichier de log</label>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <code style={{ flex: 1, fontSize: 11, color: 'var(--text2)', wordBreak: 'break-all', padding: '8px 10px', background: 'var(--surface3)', borderRadius: 6 }}>
            {logPath || 'Chargement…'}
          </code>
          <button className="btn btn--secondary btn--sm" onClick={() => ipc.debug.openLogFolder()} title="Ouvrir le dossier">
            📂
          </button>
        </div>
      </div>

      <div style={{ marginTop: 16, padding: '10px 14px', background: 'rgba(251,191,36,.08)', border: '1px solid rgba(251,191,36,.25)', borderRadius: 8, fontSize: 12, color: 'var(--yellow)', lineHeight: 1.6 }}>
        ⚠️ Le fichier log est écrasé à chaque lancement (l'ancien est renommé <code>.old.log</code>). Récupérez-le avant de relancer Backseat si nécessaire.
      </div>
    </div>
  );
}
