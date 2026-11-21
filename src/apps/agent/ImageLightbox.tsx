import React, { useEffect, useRef } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';

interface Props {
  src: string;
  alt: string;
  onClose: () => void;
}

/** Full-size image viewer. Esc closes; Tab stays on the close button. */
export const ImageLightbox: React.FC<Props> = ({ src, alt, onClose }) => {
  const closeRef = useRef<HTMLButtonElement>(null);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });

  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    closeRef.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onCloseRef.current();
      } else if (e.key === 'Tab') {
        e.preventDefault();
        closeRef.current?.focus();
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
      previous?.focus?.();
    };
  }, []);

  return createPortal(
    <div
      role="dialog"
      aria-modal="true"
      aria-label={alt}
      data-agent-lightbox
      className="fixed inset-0 flex items-center justify-center p-6"
      style={{ zIndex: 100000, background: 'rgba(0,0,0,0.75)' }}
      onClick={onClose}
    >
      <button
        ref={closeRef}
        type="button"
        onClick={onClose}
        aria-label="Close image preview"
        className="absolute top-4 right-4 p-2 rounded-[6px] border focus:outline-none focus-visible:ring-2 focus-visible:ring-geist-link/60"
        style={{ background: 'var(--canvas-elevated, #ffffff)', borderColor: 'var(--hairline, #ebebeb)', color: 'var(--ink, #171717)' }}
      >
        <X className="w-4 h-4" />
      </button>
      <img
        src={src}
        alt={alt}
        className="max-w-full max-h-full rounded-[6px] border"
        style={{ borderColor: 'var(--hairline, #ebebeb)', background: 'var(--canvas, #fafafa)' }}
        onClick={(e) => e.stopPropagation()}
      />
    </div>,
    document.body
  );
};
