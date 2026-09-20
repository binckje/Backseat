import React from 'react';
import { useStore } from '../../store/index';
import { Icon } from './Icon';
import type { ToastMessage } from '../../types/index';
const ICONS: Record<ToastMessage['type'], React.ReactNode> = {
  success: <Icon name="check" size={15} color="var(--green)" />,
  error:   <Icon name="x" size={15} color="var(--red)" />,
  info:    <Icon name="info" size={15} color="var(--accent)" />,
  warning: <Icon name="warning" size={15} color="var(--yellow)" />,
};
export function ToastContainer() {
  const { toasts, removeToast } = useStore();
  if (!toasts.length) return null;
  return (
    <div className="toast-container">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast--${t.type}`}>
          <span className="toast__icon">{ICONS[t.type]}</span>
          <span className="toast__msg">{t.message}</span>
          <button onClick={() => removeToast(t.id)} style={{ background:'none',border:'none',color:'var(--text3)',cursor:'pointer',display:'flex',alignItems:'center' }}>
            <Icon name="x" size={14} />
          </button>
        </div>
      ))}
    </div>
  );
}
