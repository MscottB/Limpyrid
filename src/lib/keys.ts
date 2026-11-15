// Atajos de teclado de la lista del grupo y de la navegacion. Puro: check-frontend.mjs lo carga desde Node.
// `target`: 'input' si se escribe en un campo (o en una casilla, que maneja su propia tecla), 'row' si el foco esta en una
// fila de categoria, 'nav' si esta en la navegacion de grupos, 'other' en lo demas. En un campo nunca actua nada; espacio
// y Enter solo en una fila, para no quitarle su efecto a un boton.
export type KeyAction = 'focus-filter' | 'toggle' | 'expand' | 'next' | 'prev' | 'to-nav' | 'to-list' | 'close';

const ON_ROW: Record<string, KeyAction> = { ' ': 'toggle', Enter: 'expand', ArrowDown: 'next', ArrowUp: 'prev', ArrowLeft: 'to-nav' };
const ON_NAV: Record<string, KeyAction> = { ArrowDown: 'next', ArrowUp: 'prev', ArrowRight: 'to-list' };

export function keyAction(e: { key: string; target: 'input' | 'row' | 'nav' | 'other' }): KeyAction | null {
  if (e.target === 'input') return null;
  if (e.key === '/') return 'focus-filter';
  if (e.key === 'Escape') return 'close';
  return (e.target === 'row' ? ON_ROW[e.key] : e.target === 'nav' ? ON_NAV[e.key] : undefined) ?? null;
}
