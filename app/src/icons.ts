// Small line icons (Fluent-like, 16 px), as SVG markup.

const wrap = (d: string) =>
  `<svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${d}</svg>`;

export const pinIcon = wrap(
  '<path d="M9.8 2.2 13.8 6.2 11.4 7.4 9.2 9.6 8.9 12.4 3.6 7.1 6.4 6.8 8.6 4.6Z"/><path d="M6.2 9.8 2.5 13.5"/>',
);
export const gearIcon = wrap(
  '<circle cx="8" cy="8" r="2.1"/><path d="M8 1.8v1.6M8 12.6v1.6M1.8 8h1.6M12.6 8h1.6M3.6 3.6l1.1 1.1M11.3 11.3l1.1 1.1M3.6 12.4l1.1-1.1M11.3 4.7l1.1-1.1"/>',
);
export const closeIcon = wrap('<path d="M4 4l8 8M12 4l-8 8"/>');
export const expandIcon = wrap('<path d="M9.5 2.5h4v4M13.5 2.5 8.5 7.5M6.5 13.5h-4v-4M2.5 13.5l5-5"/>');
