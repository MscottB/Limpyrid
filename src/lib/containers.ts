import type { UsageRow } from './api';

// Las filas de `df` que la limpieza puede liberar: los volumenes nunca se tocan, asi que no se listan.
export const prunableRows = (rows: UsageRow[]): UsageRow[] => rows.filter((r) => !/volum/i.test(r.kind));
