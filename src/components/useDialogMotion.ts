import { useLayoutEffect, useRef } from 'react';

export function useDialogMotion(onDismiss: () => void) {
  const modal = useRef<HTMLDialogElement>(null);
  useLayoutEffect(() => {
    modal.current?.showModal();
  }, []);

  async function dismiss() {
    const node = modal.current;
    if (!node || node.dataset.leaving) return;
    node.dataset.leaving = 'true';
    node.inert = true;
    // With reduced motion the list is empty and dismissal is immediate.
    const animations = node.getAnimations?.() ?? [];
    await Promise.allSettled(animations.map((animation) => animation.finished));
    if (!node.isConnected) return;
    node.close?.();
    onDismiss();
  }
  return { modal, dismiss };
}
