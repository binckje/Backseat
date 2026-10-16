import React, { useState, useEffect } from 'react';
import { useStore } from '../../store/index';
import { SaveTreeModal } from '../SaveTreeModal/index';
import { useIpc } from '../../hooks/useIpc';
import { Icon } from '../common/Icon';
import type { SharedGame, GamePermissions, CloudSaveInfo, ProgressEvent, SteamFriend } from '../../types/index';
import { convertFileSrc } from '@tauri-apps/api/core';

export function GameCard() {
  const { sharedGames, selectedAppId } = useStore();
  const game = sharedGames.find((g) => g.appId === selectedAppId);
  if (!game) return (
    <div className="welcome">
      <div className="welcome__icon">🎮</div>
      <div className="welcome__title">Backseat</div>
      <div className="welcome__sub">Sélectionnez un jeu dans la liste, ou ajoutez-en un nouveau pour commencer.</div>
    </div>
  );
  return <GameCardInner game={game} />;
}

function GameCardInner({ game }: { game: SharedGame }) {
  const ipc = useIpc();
  const { settings, addToast, updateSharedGame, openModal, cloudStatus, selectGame, setSharedGames } = useStore();
  const [perms, setPerms] = useState<GamePermissions | null>(null);
  const [cloudInfo, setCloudInfo] = useState<{ description: string; uploaderName: string; uploadedAt: string } | null>(null);
  const [imageCount, setImageCount] = useState(0);
  const [localImages, setLocalImages] = useState<string[]>([]);
  const [loadingInfo, setLoadingInfo] = useState(false);
  const [uploading, setUploading] = useState(false);
  const [downloading, setDownloading] = useState(false);
  const [uploadProgress, setUploadProgress] = useState<ProgressEvent | null>(null);
  const [downloadProgress, setDownloadProgress] = useState<ProgressEvent | null>(null);
  const [description, setDescription] = useState('');
  const [selectedImages, setSelectedImages] = useState<string[]>([]);
  const [friends, setFriends] = useState<SteamFriend[]>([]);
  const [showFriendSearch, setShowFriendSearch] = useState(false);
  const [friendQuery, setFriendQuery] = useState('');
  const [friendsError, setFriendsError] = useState('');
  const [lookingUpId, setLookingUpId] = useState(false);
  const [backups, setBackups] = useState<Array<{ timestamp: string; path: string }>>([]);
  const [removing, setRemoving] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [treeModal, setTreeModal] = useState<'upload'|'download'|null>(null);
  const [uploadDesc, setUploadDesc] = useState('');
  const [uploadImages, setUploadImages] = useState<string[]>([]);

  const myId = settings?.steamId;
  const isOwner = perms ? perms.owner === myId : cloudStatus.connected;
  const hasPermission = isOwner || (perms?.allowedUsers || []).includes(myId || '');
  const saveStatus = perms?.status ?? 'not_uploaded';
  const isLockedByMe = perms?.lockedBy === myId;
  const isLockedByOther = (perms?.lockedBy ?? null) !== null && perms?.lockedBy !== myId;



  useEffect(() => {
    if (!cloudStatus.connected) return;
    setPerms(null); setCloudInfo(null); setImageCount(0); setLocalImages([]);
    setLoadingInfo(true);
    Promise.all([ipc.permissions.get(game.appId), ipc.saves.getCloudInfo(game.appId)])
      .then(([pRes, iRes]) => {
        if (pRes.success && pRes.data) setPerms(pRes.data as GamePermissions);
        if (iRes.success && iRes.data) {
          const m = iRes.data as { notes: { description: string; uploaderName: string; uploadedAt: string } | null; imageCount: number };
          if (m.notes) setCloudInfo(m.notes);
          setImageCount(m.imageCount || 0);
        }
      }).finally(() => setLoadingInfo(false));
  }, [game.appId, cloudStatus.connected]);

  useEffect(() => {
    ipc.backup.list(game.appId).then((r) => { if (r.success && r.data) setBackups(r.data as Array<{ timestamp: string; path: string }>); });
  }, [game.appId]);

  const refreshPerms = async () => {
    const r = await ipc.permissions.get(game.appId);
    if (r.success && r.data) {
      const p = r.data as GamePermissions;
      setPerms(p);
      updateSharedGame(game.appId, { cloudInfo: { ...(game.cloudInfo || {} as CloudSaveInfo), status: p.status, lockedBy: p.lockedBy, lockedAt: p.lockedAt } });
    }
  };

  const handleUpload = async () => {
    if (uploading) return;
    setUploading(true); setUploadProgress(null);
    try {
      const r = await ipc.saves.upload(
      { appId: game.appId, description, imagePaths: selectedImages },
      (ev) => setUploadProgress(ev)
    );
    if (r.success) {
        addToast({ type: 'success', message: 'Sauvegarde uploadée !' });
        setDescription(''); setSelectedImages([]);
        const ir = await ipc.saves.getCloudInfo(game.appId);
        if (ir.success && ir.data) {
          const m = ir.data as { notes: { description: string; uploaderName: string; uploadedAt: string } | null; imageCount: number };
          if (m.notes) setCloudInfo(m.notes); setImageCount(m.imageCount || 0);
        }
        // Refresh sidebar badge for this game
        updateSharedGame(game.appId, { cloudInfo: { ...(game.cloudInfo as any || {}), status: 'available', lockedBy: null } });
      } else { addToast({ type: 'error', message: r.error || 'Erreur upload' }); }
    } finally { await refreshPerms(); setUploading(false); setUploadProgress(null); }
  };

  const handleDownload = async () => {
    if (downloading) return;
    setDownloading(true); setDownloadProgress(null);
    try {
      const r = await ipc.saves.download(game.appId, (ev) => setDownloadProgress(ev));
      if (r.success) {
        const d = r.data as { backupTimestamp?: string };
        addToast({ type: 'success', message: 'Sauvegarde téléchargée ! Backup local créé.' });
        if (d?.backupTimestamp) updateSharedGame(game.appId, { hasLocalBackup: true, backupTimestamp: d.backupTimestamp });
        const br = await ipc.backup.list(game.appId);
        if (br.success && br.data) setBackups(br.data as Array<{ timestamp: string; path: string }>);
      } else { addToast({ type: 'error', message: r.error || 'Erreur téléchargement' }); }
    } finally { await refreshPerms(); setDownloading(false); setDownloadProgress(null); }
  };

  const handleReleaseLock = async () => {
    const r = await ipc.permissions.releaseLock(game.appId);
    if (r.success) { addToast({ type: 'success', message: 'Verrou libéré.' }); await refreshPerms(); }
    else addToast({ type: 'error', message: r.error || 'Erreur' });
  };

  const handleRestore = async (ts?: string) => {
    const r = await ipc.backup.restore(game.appId, ts);
    r.success ? addToast({ type: 'success', message: 'Backup restauré !' }) : addToast({ type: 'error', message: r.error || 'Erreur restauration' });
  };

  const handleDeleteBackup = async (ts: string) => {
    const r = await ipc.backup.delete(game.appId, ts);
    if (r.success) { setBackups((p) => p.filter((b) => b.timestamp !== ts)); addToast({ type: 'success', message: 'Backup supprimé.' }); }
    else addToast({ type: 'error', message: r.error || 'Erreur' });
  };

  const handleRemoveGame = async () => {
    setRemoving(true);
    const r = await ipc.games.remove(game.appId);
    if (r.success) {
      addToast({ type: 'success', message: `${game.name} retiré.` });
      selectGame(null);
      const gr = await ipc.games.getShared();
      if (gr.success && gr.data) setSharedGames(gr.data as SharedGame[]);
    } else addToast({ type: 'error', message: r.error || 'Erreur' });
    setRemoving(false); setConfirmRemove(false);
  };

  const loadFriends = async () => {
    if (friends.length > 0) { setShowFriendSearch(true); return; }
    setFriendsError('');
    const r = await ipc.steam.getFriends();
    if (r.success && r.data) setFriends(r.data as SteamFriend[]);
    else setFriendsError(r.error || 'Impossible de charger les amis. Vérifiez que votre profil Steam est public.');
    setShowFriendSearch(true);
  };

  const handleAddBySteamId = async () => {
    if (!friendQuery.trim()) return;
    setLookingUpId(true);
    try {
      const r = await ipc.permissions.grant(game.appId, friendQuery.trim());
      if (r.success) { addToast({ type: 'success', message: `Accès accordé à ${friendQuery}` }); setShowFriendSearch(false); setFriendQuery(''); await refreshPerms(); }
      else addToast({ type: 'error', message: r.error || 'Erreur' });
    } finally { setLookingUpId(false); }
  };

  const handleAddFriend = async (f: SteamFriend) => {
    const r = await ipc.permissions.grant(game.appId, f.steamId);
    if (r.success) { addToast({ type: 'success', message: `Accès accordé à ${f.personaName}` }); setShowFriendSearch(false); await refreshPerms(); }
    else addToast({ type: 'error', message: r.error || 'Erreur' });
  };

  const handleViewImages = async () => {
    if (localImages.length > 0) { openModal('image_viewer', localImages); return; }
    const r = await ipc.saves.downloadImages(game.appId);
    if (r.success && r.data) {
      const paths = r.data as string[];
      setLocalImages(paths);
      if (paths.length > 0) openModal('image_viewer', paths);
      else addToast({ type: 'info', message: 'Aucune image disponible.' });
    }
  };

  const pct = (ev: ProgressEvent | null) => ev && ev.total > 0 ? Math.round((ev.current / ev.total) * 100) : 0;
  const filteredFriends = friends.filter((f) => f.personaName.toLowerCase().includes(friendQuery.toLowerCase()));

  return (
    <div className="game-card">
      <div className="game-card__hero">
        <img className="game-card__hero-img"
          src={game.launcher === 'heroic' && game.backgroundUrl
            ? game.backgroundUrl
            : `https://cdn.cloudflare.steamstatic.com/steam/apps/${game.appId}/header.jpg`
          }
          alt={game.name}
          onError={(e) => { (e.target as HTMLImageElement).style.display = 'none'; }} />
        <div className="game-card__hero-overlay">
          <div>
            <div className="game-card__name">{game.name}</div>
            <div className="game-card__appid" style={{display:'flex',alignItems:'center',gap:8}}>
              {game.launcher === 'heroic'
                ? <span style={{fontSize:11,padding:'1px 7px',borderRadius:10,background:'rgba(255,255,255,.12)',color:'#fff'}}>
                    {game.heroPlatform === 'epic' ? '🟣 Epic' : '🔵 GOG'} via Heroic
                  </span>
                : <span style={{fontSize:11,padding:'1px 7px',borderRadius:10,background:'rgba(255,255,255,.12)',color:'#fff'}}>🎮 Steam</span>
              }
              <span>AppID {game.appId}</span>
            </div>
          </div>
        </div>
      </div>

      {/* Status bar */}
      <div className="game-card__status-bar">
        <span className="game-card__status-text" style={{ display:'flex',alignItems:'center',gap:6 }}>
          {saveStatus==='locked' && <Icon name="lock" size={13} color="var(--yellow)" />}
          {saveStatus==='available' && 'Sauvegarde disponible sur le cloud'}
          {saveStatus==='locked' && (isLockedByMe ? 'Verrouillé par vous — libérez quand vous avez fini.' : `Verrouillé par ${perms?.lockedBy}`)}
          {saveStatus==='not_uploaded' && 'Aucune sauvegarde sur le cloud'}
        </span>
        <span className={`status-badge status-badge--${saveStatus==='available'?'available':saveStatus==='locked'?'locked':'none'}`}>
          {saveStatus==='available'?'Disponible':saveStatus==='locked'?'Verrouillé':'Non uploadé'}
        </span>
        {isLockedByMe && <button className="btn btn--sm btn--secondary" onClick={handleReleaseLock} style={{display:'flex',alignItems:'center',gap:5}}><Icon name="unlock" size={13}/>Libérer</button>}
        {isOwner && isLockedByOther && <button className="btn btn--sm btn--danger" onClick={handleReleaseLock} style={{display:'flex',alignItems:'center',gap:5}}><Icon name="release" size={13}/>Forcer libération</button>}
      </div>

      {!cloudStatus.connected && <div className="section"><div className="section__body" style={{display:'flex',alignItems:'center',gap:8,color:'var(--yellow)',padding:'20px',justifyContent:'center'}}><Icon name="warning" size={16} color="var(--yellow)"/>Non connecté au cloud. Configurez votre drive dans les paramètres.</div></div>}

      {cloudStatus.connected && <>
        {cloudInfo && <div className="section">
          <div className="section__header"><Icon name="notes" size={15} color="var(--accentEmerald)"/><span className="section__title">Sauvegarde actuelle sur le cloud</span></div>
          <div className="section__body">
            <p className="text-muted text-sm" style={{marginBottom:8}}>Uploadée par <strong style={{color:'var(--text)'}}>{cloudInfo.uploaderName}</strong> le {new Date(cloudInfo.uploadedAt).toLocaleString('fr-FR')}</p>
            {cloudInfo.description && <p style={{marginTop:8,lineHeight:1.6,fontSize:13}}>{cloudInfo.description}</p>}
            {imageCount > 0 && <div className="mt-8"><button className="btn btn--sm btn--secondary" onClick={handleViewImages} style={{display:'flex',alignItems:'center',gap:5}}><Icon name="image" size={13}/>Voir les {imageCount} image{imageCount>1?'s':''}</button></div>}
          </div>
        </div>}

        {hasPermission && saveStatus!=='not_uploaded' && <div className="section">
          <div className="section__header"><Icon name="download" size={15} color="var(--accentEmerald)"/><span className="section__title">Télécharger la sauvegarde</span></div>
          <div className="section__body">
            {isLockedByOther ? <p className="text-muted text-sm">Cette sauvegarde est utilisée par un autre joueur.</p> : <>
              <p className="text-muted text-sm" style={{marginBottom:12}}>Un backup local sera créé automatiquement avant le téléchargement.</p>
              {downloading
                ? <InlineProgress event={downloadProgress} pct={pct(downloadProgress)} label="Téléchargement" color="var(--accent)" />
                : <button className="btn btn--primary" onClick={() => setTreeModal('download')} style={{display:'flex',alignItems:'center',gap:6}}>
                    <Icon name="download" size={14}/>Télécharger depuis le cloud
                  </button>
              }
            </>}
          </div>
        </div>}

        {hasPermission && <div className="section">
          <div className="section__header"><Icon name="upload" size={15} color="var(--accentEmerald)"/><span className="section__title">Uploader ma sauvegarde</span></div>
          <div className="section__body">
            {isLockedByOther ? <div style={{display:'flex',alignItems:'center',gap:8,color:'var(--yellow)',fontSize:13}}><Icon name="lock" size={14} color="var(--yellow)"/>Sauvegarde verrouillée par un autre joueur.</div> : <>
              <p className="text-muted text-sm" style={{marginBottom:12}}><span className="tag tag--method">{game.savePath}</span></p>
              <div className="field"><label className="label">Description (optionnel)</label>
                <textarea className="textarea" placeholder="Ex : après le boss final, niveau 42…" value={description} onChange={(e)=>setDescription(e.target.value)}/>
              </div>
              <div className="field"><label className="label">Screenshots ({selectedImages.length} sélectionné{selectedImages.length>1?'s':''})</label>
                <button className="btn btn--secondary btn--sm" onClick={async()=>{ const r=await ipc.dialog.openImages(); if(r.success&&r.data) setSelectedImages(r.data as string[]); }} style={{display:'flex',alignItems:'center',gap:5}}><Icon name="image" size={13}/>Choisir des images</button>
                {selectedImages.length>0 && <div className="img-gallery" style={{marginTop:8}}>{selectedImages.map((img)=><img key={img} className="img-thumb" src={convertFileSrc(img)} alt=""/>)}</div>}
              </div>
              {uploading
                ? <InlineProgress event={uploadProgress} pct={pct(uploadProgress)} label="Upload" color="var(--accent)" />
                : <button className="btn btn--primary"
                    onClick={() => { setUploadDesc(description); setUploadImages(selectedImages); setTreeModal('upload'); }}
                    style={{display:'flex',alignItems:'center',gap:6}}>
                    <Icon name="upload" size={14}/>Uploader sur le cloud
                  </button>
              }
            </>}
          </div>
        </div>}

        {backups.length>0 && <div className="section">
          <div className="section__header"><Icon name="backup" size={15} color="var(--accentEmerald)"/><span className="section__title">Backups locaux</span></div>
          <div className="section__body">
            <p className="text-muted text-sm" style={{marginBottom:10}}>{backups.length} backup{backups.length>1?'s':''} disponible{backups.length>1?'s':''}. Les 3 derniers sont conservés.</p>
            {backups.map((bk)=>(
              <div key={bk.timestamp} style={{display:'flex',alignItems:'center',gap:8,marginBottom:6}}>
                <span className="text-sm text-muted" style={{flex:1}}>{formatTs(bk.timestamp)}</span>
                <button className="btn btn--sm btn--secondary" onClick={()=>handleRestore(bk.timestamp)} style={{display:'flex',alignItems:'center',gap:5}}><Icon name="restore" size={13}/>Restaurer</button>
                <button className="btn btn--sm btn--danger" onClick={()=>handleDeleteBackup(bk.timestamp)} style={{display:'flex',alignItems:'center',gap:4}}><Icon name="trash" size={13}/></button>
              </div>
            ))}
          </div>
        </div>}

        {isOwner && perms && <div className="section">
          <div className="section__header"><Icon name="users" size={15} color="var(--accentEmerald)"/><span className="section__title">Permissions d'accès</span></div>
          <div className="section__body">
            <div className="perm-list" style={{marginBottom:14}}>
              <div className="perm-item"><div className="perm-item__name" style={{color:'var(--txtEmerald)',fontSize:13}}>{settings?.cachedPlayerInfo?.personaName||perms.owner}</div><span className="perm-item__role" style={{color:'var(--txtEmerald)'}}>Propriétaire</span></div>
              {perms.allowedUsers.map((uid)=>(
                <div key={uid} className="perm-item">
                  <div className="perm-item__name" style={{color:'var(--accentEmerald)',fontSize:11}}>{uid}</div>
                  <span className="perm-item__role" style={{color:'var(--accentEmerald)',fontWeight:400}}>Invité</span>
                  <button className="perm-item__btn" onClick={async()=>{ const r=await ipc.permissions.revoke(game.appId,uid); if(r.success){addToast({type:'success',message:'Accès révoqué.'});await refreshPerms();}else addToast({type:'error',message:r.error||'Erreur'}); }}><Icon name="x" size={14}/></button>
                </div>
              ))}
            </div>
            {!showFriendSearch ? (
              <button className="btn btn--secondary btn--sm" onClick={loadFriends} style={{display:'flex',alignItems:'center',gap:5}}><Icon name="plus" size={13}/>Ajouter un ami Steam</button>
            ) : (
              <div>
                {friendsError && <div style={{padding:'8px 12px',marginBottom:10,background:'rgba(251,191,36,.08)',border:'1px solid rgba(251,191,36,.25)',borderRadius:6,fontSize:12,color:'var(--yellow)',lineHeight:1.5,display:'flex',gap:6,alignItems:'flex-start'}}><Icon name="warning" size={13} color="var(--yellow)" style={{marginTop:1,flexShrink:0}}/>{friendsError}</div>}
                <div style={{display:'flex',gap:8,marginBottom:8}}>
                  <input className="input search-input" style={{marginBottom:0,flex:1}} placeholder={friends.length>0?'Rechercher un ami…':'Steam ID (ex: 76561198…)'} value={friendQuery} onChange={(e)=>setFriendQuery(e.target.value)} onKeyDown={(e)=>e.key==='Enter'&&friends.length===0&&handleAddBySteamId()} autoFocus/>
                  {(friends.length===0||/^7656\d{13}$/.test(friendQuery.trim()))&&friendQuery.trim()&&<button className="btn btn--primary btn--sm" onClick={handleAddBySteamId} disabled={lookingUpId}>{lookingUpId?<span className="btn__spinner"/>:null}Ajouter</button>}
                </div>
                {friends.length>0&&<div className="friend-list">{filteredFriends.filter((f)=>!perms.allowedUsers.includes(f.steamId)&&f.steamId!==myId).map((f)=>(
                  <div key={f.steamId} className="friend-row" onClick={()=>handleAddFriend(f)}>
                    {f.avatar?<img className="friend-row__avatar" src={f.avatar} alt=""/>:<div className="friend-row__avatar"/>}
                    <div className="friend-row__name">{f.personaName}</div>
                    <div className="friend-row__id">{f.steamId}</div>
                  </div>
                ))}</div>}
                {friends.length===0&&!friendsError&&<p className="text-muted text-sm" style={{marginBottom:8}}>Entre le Steam ID 64-bit du joueur (commence par 76561198…)</p>}
                <button className="btn btn--ghost btn--sm" onClick={()=>{setShowFriendSearch(false);setFriendQuery('');setFriendsError('');}} style={{marginTop:4}}>Annuler</button>
              </div>
            )}
          </div>
        </div>}

        <SavePathSection game={game} ipc={ipc} updateSharedGame={updateSharedGame} addToast={addToast} />

        {isOwner && <div className="section" style={{borderColor:'rgba(248,113,113,.25)'}}>
          <div className="section__header" style={{borderColor:'rgba(248,113,113,.25)'}}><Icon name="trash" size={15} color="var(--red)"/><span className="section__title" style={{color:'var(--red)'}}>Retirer ce jeu</span></div>
          <div className="section__body">
            {!confirmRemove ? <>
              <p className="text-muted text-sm" style={{marginBottom:12}}>Retire ce jeu et supprime définitivement son dossier sur le cloud.</p>
              <button className="btn btn--danger btn--sm" onClick={()=>setConfirmRemove(true)} style={{display:'flex',alignItems:'center',gap:5}}><Icon name="trash" size={13}/>Retirer ce jeu</button>
            </> : (
              <div style={{display:'flex',alignItems:'center',gap:10}}>
                <span style={{fontSize:13,color:'var(--red)',flex:1}}>Confirmer la suppression de "{game.name}" ?</span>
                <button className="btn btn--ghost btn--sm" onClick={()=>setConfirmRemove(false)} disabled={removing}>Annuler</button>
                <button className="btn btn--danger btn--sm" onClick={handleRemoveGame} disabled={removing}>{removing&&<span className="btn__spinner"/>}Confirmer</button>
              </div>
            )}
          </div>
        </div>}
      </>}

      {/* Save tree modal */}
      {treeModal && (
        <SaveTreeModal
          appId={game.appId}
          mode={treeModal}
          description={uploadDesc}
          imagePaths={uploadImages}
          addToast={addToast}
          onClose={() => setTreeModal(null)}
          onDone={async (result) => {
            setTreeModal(null);
            if (treeModal === 'upload') {
              setDescription(''); setSelectedImages([]);
              updateSharedGame(game.appId, { cloudInfo: { ...(game.cloudInfo as any || {}), status: 'available', lockedBy: null } });
            } else {
              if (result?.backupTimestamp) updateSharedGame(game.appId, { hasLocalBackup: true, backupTimestamp: result.backupTimestamp });
              const br = await ipc.backup.list(game.appId);
              if (br.success && br.data) setBackups(br.data as Array<{ timestamp: string; path: string }>);
            }
            await refreshPerms();
          }}
        />
      )}
    </div>
  );
}

function InlineProgress({ event, pct, label, color }: {
  event: ProgressEvent | null; pct: number; label: string; color: string;
}) {
  const phases: Record<string, string> = {
    scanning: 'Analyse…', backing_up: 'Backup local…',
    uploading: 'Upload…', downloading: 'Téléchargement…', finalizing: 'Finalisation…',
  };
  const phase = event ? (phases[event.phase] || event.phase) : label + '…';
  const currentFile = event?.currentFile;

  return (
    <div style={{
      border: '1px solid var(--border2)', borderRadius: 8,
      padding: '12px 16px', background: 'var(--surface3)',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', marginBottom: 8, fontSize: 13 }}>
        <span style={{ color: 'var(--text)', fontWeight: 500 }}>{phase}</span>
        <span style={{ color, fontWeight: 700 }}>{pct}%</span>
      </div>
      <div style={{ height: 6, background: 'var(--border)', borderRadius: 3, overflow: 'hidden' }}>
        <div style={{
          height: '100%', width: `${pct}%`,
          background: color, borderRadius: 3,
          transition: 'width 0.25s ease',
          boxShadow: `0 0 8px ${color}66`,
        }} />
      </div>
      {currentFile && (
        <div style={{ marginTop: 6, fontSize: 11, color: 'var(--text3)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
          title={currentFile}>
          {currentFile}
        </div>
      )}
    </div>
  );
}

function formatTs(ts: string): string {
  try {
    const iso = ts.replace(/T(\d{2})-(\d{2})-(\d{2})-(\d+)Z$/, 'T$1:$2:$3.$4Z');
    const d = new Date(iso);
    if (isNaN(d.getTime())) return ts;
    return d.toLocaleString('fr-FR', { day:'2-digit', month:'2-digit', year:'numeric', hour:'2-digit', minute:'2-digit' });
  } catch { return ts; }
}

// ─── Save Path Section ────────────────────────────────────────────────────────

function SavePathSection({ game, ipc, updateSharedGame, addToast }: {
  game: SharedGame;
  ipc: ReturnType<typeof useIpc>;
  updateSharedGame: (id: string, p: Partial<SharedGame>) => void;
  addToast: (m: { type: 'success'|'error'|'info'|'warning'; message: string }) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [path, setPath] = useState(game.savePath);
  const [saving, setSaving] = useState(false);

  const methodLabel: Record<string, string> = {
    steam_userdata: 'Steam userdata',
    pcgw: 'PCGamingWiki',
    proton: 'Proton',
    heroic: 'Heroic',
    manual: 'Manuel',
  };

  const handleBrowse = async () => {
    const r = await ipc.dialog.openFolder();
    if (r.success && r.data) setPath(r.data as string);
  };

  const handleSave = async () => {
    if (!path || path === game.savePath) { setEditing(false); return; }
    setSaving(true);
    const r = await ipc.games.updateSavePath(game.appId, path);
    if (r.success) {
      updateSharedGame(game.appId, { savePath: path, savePathDetectionMethod: 'manual' });
      addToast({ type: 'success', message: 'Chemin de sauvegarde mis à jour.' });
      setEditing(false);
    } else {
      addToast({ type: 'error', message: r.error || 'Erreur' });
    }
    setSaving(false);
  };

  const handleCancel = () => {
    setPath(game.savePath);
    setEditing(false);
  };

  return (
    <div className="section">
      <div className="section__header">
        <Icon name="folder" size={15} color="var(--accentEmerald)" />
        <span className="section__title">Dossier de sauvegarde</span>
        {!editing && (
          <button
            className="btn btn--sm btn--secondary"
            style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 5 }}
            onClick={() => { setPath(game.savePath); setEditing(true); }}
          >
            <Icon name="settings" size={12} />Modifier
          </button>
        )}
      </div>
      <div className="section__body">
        {editing ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
            <div className="input-row">
              <input
                className="input"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                placeholder="Chemin vers les sauvegardes"
                autoFocus
                style={{ fontSize: 12, fontFamily: 'monospace' }}
              />
              <button className="btn btn--secondary" onClick={handleBrowse} title="Parcourir">
                <Icon name="folder" size={14} />
              </button>
            </div>
            <div style={{ display: 'flex', gap: 8 }}>
              <button className="btn btn--ghost btn--sm" onClick={handleCancel} disabled={saving}>
                Annuler
              </button>
              <button className="btn btn--primary btn--sm" onClick={handleSave} disabled={saving || !path}>
                {saving && <span className="btn__spinner" />}Enregistrer
              </button>
            </div>
          </div>
        ) : (
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <code style={{ flex: 1, fontSize: 11, color: 'var(--txtEmerald-dim)', wordBreak: 'break-all' }}>
              {game.savePath}
            </code>
            <span className="tag tag--accent tag--method">
              {methodLabel[game.savePathDetectionMethod] || game.savePathDetectionMethod}
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
