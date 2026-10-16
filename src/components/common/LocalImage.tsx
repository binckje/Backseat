import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

// Images locales lues côté Rust et passées en data URL : le protocole asset
// ne s'affiche pas de façon fiable dans la webview Linux.
const cache = new Map<string, string>();

export function LocalImage({ path, ...props }: { path: string } & Omit<React.ImgHTMLAttributes<HTMLImageElement>, 'src'>) {
  const [src, setSrc] = useState(() => cache.get(path));
  useEffect(() => {
    const cached = cache.get(path);
    if (cached) { setSrc(cached); return; }
    let alive = true;
    invoke<string>('read_image', { path })
      .then((url) => { cache.set(path, url); if (alive) setSrc(url); })
      .catch((err) => console.error('[LocalImage]', path, err));
    return () => { alive = false; };
  }, [path]);
  return src ? <img src={src} {...props} /> : null;
}
