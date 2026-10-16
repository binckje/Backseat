import React, { useState } from 'react';
import { LocalImage } from './LocalImage';
export function ImageViewerModal({ images, onClose }: { images: string[]; onClose: () => void }) {
  const [idx, setIdx] = useState(0);
  const prev = () => setIdx((i) => (i - 1 + images.length) % images.length);
  const next = () => setIdx((i) => (i + 1) % images.length);
  return (
    <div className="modal-overlay" onClick={(e) => e.target === e.currentTarget && onClose()} style={{ alignItems:'center',justifyContent:'center' }}>
      <div style={{ background:'var(--surface)',borderRadius:12,border:'1px solid var(--border2)',padding:16,maxWidth:'90vw',maxHeight:'90vh',display:'flex',flexDirection:'column',gap:12 }}>
        <div style={{ display:'flex',alignItems:'center',gap:10 }}>
          <span style={{ flex:1,fontSize:13,color:'var(--text2)' }}>Image {idx+1} / {images.length}</span>
          <button className="modal__close" onClick={onClose}>×</button>
        </div>
        <div style={{ overflow:'auto',maxWidth:'calc(90vw - 32px)',maxHeight:'calc(90vh - 140px)',minHeight:0,flexShrink:1 }}>
          {/* Taille réelle ; défilement si l'image dépasse l'écran */}
          <LocalImage path={images[idx]} alt="" style={{ display:'block',maxWidth:'none',flexShrink:0,borderRadius:8 }} />
        </div>
        {images.length > 1 && (
          <div style={{ display:'flex',justifyContent:'center',gap:12 }}>
            <button className="btn btn--secondary btn--sm" onClick={prev}>← Précédente</button>
            <button className="btn btn--secondary btn--sm" onClick={next}>Suivante →</button>
          </div>
        )}
        {images.length > 1 && (
          <div className="img-gallery" style={{ justifyContent:'center' }}>
            {images.map((img, i) => (
              <LocalImage key={img} className="img-thumb" path={img} alt="" onClick={() => setIdx(i)}
                style={{ border: i === idx ? '2px solid var(--accent)' : '2px solid transparent', opacity: i === idx ? 1 : 0.6 }} />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
