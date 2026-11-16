// Aceptacion de los terminos de uso (TERMINOS.md). Puro: check-frontend.mjs lo carga desde Node.
// Subir TERMS_VERSION cuando cambie lo que el aviso dice: la aceptacion anterior deja de valer y se vuelve a pedir.
export const TERMS_VERSION = 2; // 2: las limpiezas pueden ir a la Papelera (terms.point.1)

// Cierto solo si el valor guardado es JSON valido con la version actual.
export function termsAccepted(stored: string | null): boolean {
  return termsDate(stored) != null;
}

export const termsRecord = (nowMs: number): string => JSON.stringify({ version: TERMS_VERSION, at: nowMs });

// Cuando se acepto (ms desde 1970), o null si el registro no vale para esta version.
export function termsDate(stored: string | null): number | null {
  try {
    const v = JSON.parse(stored ?? '');
    return v?.version === TERMS_VERSION && typeof v.at === 'number' ? v.at : null;
  } catch {
    return null;
  }
}
