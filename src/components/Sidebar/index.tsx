import React, { useState, useMemo } from 'react';
import { useStore } from '../../store/index';
import { Icon } from '../common/Icon';
import type { SharedGame } from '../../types/index';

type SortMode = 'az' | 'za' | 'recent';

const SORT_LABELS: Record<SortMode, string> = {
  az:     'A → Z',
  za:     'Z → A',
  recent: 'Récent',
};
const SORT_NEXT: Record<SortMode, SortMode> = {
  az: 'za', za: 'recent', recent: 'az',
};
const SORT_ICON: Record<SortMode, string> = {
  az: '↑', za: '↓', recent: '🕐',
};

export function Sidebar() {
  const { sharedGames, selectedAppId, selectGame, openModal } = useStore();
  const [sort, setSort] = useState<SortMode>('az');

  const sorted = useMemo(() => {
    const games = [...sharedGames];
    switch (sort) {
      case 'az':
        return games.sort((a, b) => a.name.localeCompare(b.name));
      case 'za':
        return games.sort((a, b) => b.name.localeCompare(a.name));
      case 'recent':
        return games.reverse(); // last added = first shown
    }
  }, [sharedGames, sort]);

  return (
    <aside className="sidebar">
      <div className="sidebar__header">
        <span className="sidebar__title">Mes jeux</span>
        <div style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          {/* Sort toggle button */}
          <button
            title={`Tri : ${SORT_LABELS[sort]} — cliquer pour changer`}
            onClick={() => setSort(SORT_NEXT[sort])}
            style={{
              background: 'none',
              border: '1px solid var(--borderEmerald)',
              color: 'var(--txtEmerald)',
              borderRadius: 'var(--radius-sm)',
              padding: '2px 6px',
              cursor: 'pointer',
              fontSize: 11,
              display: 'flex',
              alignItems: 'center',
              gap: 3,
              transition: 'background .15s',
            }}
            onMouseEnter={e => (e.currentTarget.style.background = 'var(--surfaceEmeraldBlueHover)')}
            onMouseLeave={e => (e.currentTarget.style.background = 'none')}
          >
            <span>{SORT_ICON[sort]}</span>
            <span>{SORT_LABELS[sort]}</span>
          </button>

          {/* Add game button */}
          <button
            className="sidebar__add-btn"
            title="Ajouter un jeu"
            onClick={() => openModal('add_game')}
          >
            <Icon name="plus" size={14} color="var(--txtEmerald)" />
          </button>
        </div>
      </div>

      <div className="sidebar__list">
        {sorted.length === 0
          ? <div className="sidebar__empty">
              Ajoutez un jeu Steam ou Heroic pour commencer à partager vos sauvegardes.
            </div>
          : sorted.map((g) => (
              <GameItem
                key={g.appId}
                game={g}
                active={g.appId === selectedAppId}
                onClick={() => selectGame(g.appId)}
              />
            ))
        }
      </div>
    </aside>
  );
}

function GameItem({ game, active, onClick }: {
  game: SharedGame;
  active: boolean;
  onClick: () => void;
}) {
  const s = game.cloudInfo?.status ?? 'not_uploaded';
  const badge =
    s === 'available' ? 'game-item__badge--available' :
    s === 'locked'    ? 'game-item__badge--locked'    :
    'game-item__badge--none';
  const label =
    s === 'available' ? 'Disponible' :
    s === 'locked'    ? 'Verrouillé' :
    'Non uploadé';

  const iconSrc = game.launcher === 'heroic' && game.coverUrl
    ? game.coverUrl
    : `https://cdn.cloudflare.steamstatic.com/steam/apps/${game.appId}/capsule_sm_120.jpg`;

  return (
    <div
      className={`game-item${active ? ' game-item--active' : ''}`}
      onClick={onClick}
    >
      <img
        className="game-item__icon"
        src={iconSrc}
        alt={game.name}
        onError={(e) => {
          (e.target as HTMLImageElement).src =
            'data:image/svg+xml,<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40">' +
            '<rect width="40" height="40" rx="6" fill="%232a2a3a"/>' +
            '<text x="50%" y="54%" dominant-baseline="middle" text-anchor="middle" fill="%238888a8" font-size="18">🎮</text></svg>';
        }}
      />
      <div className="game-item__info">
        <div className="game-item__name" title={game.name}>{game.name}</div>
        <div className="game-item__status" style={{ display: 'flex', alignItems: 'center', gap: 5 }}>
          {game.launcher === 'heroic' && (
            <span style={{ fontSize: 10 }}>
              {game.heroPlatform === 'epic' ? '🟣' : '🔵'}
            </span>
          )}
          {label}
        </div>
      </div>
      <div className={`game-item__badge ${badge}`} title={label} />
    </div>
  );
}
