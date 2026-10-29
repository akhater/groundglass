// The screen's icons: one line drawing each on a 24-unit grid, drawn in the
// current text colour so a control lights up by changing colour.

const PATHS = {
  zebra: "M3.5 6.5a3 3 0 0 1 3-3h11a3 3 0 0 1 3 3v11a3 3 0 0 1-3 3h-11a3 3 0 0 1-3-3zM3.5 12 12 3.5M3.5 20 20 3.5M12 20.5l8.5-8.5",
  peaking: "M3 8V3.5h4.5M16.5 3.5H21V8M21 16v4.5h-4.5M7.5 20.5H3V16M6.5 16l3.5-5 2.5 3 2-2.5 3 4.5",
  guides: "M5.5 3.5h13a2 2 0 0 1 2 2v13a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2v-13a2 2 0 0 1 2-2zM9.2 3.5v17M14.8 3.5v17M3.5 9.2h17M3.5 14.8h17",
  crop: "M7 2.5V17h14.5M2.5 7H17v14.5",
  level: "M2.5 13h6M15.5 13h6M15 13a3 3 0 1 1-6 0 3 3 0 0 1 6 0zM12 4v3M5 9.5v2M19 9.5v2",
  histogram: "M3 20.5h18M3 20.5c2.5 0 3-9 5.5-9s2.5 5 4.5 5 2-12 4.5-12 2 16 3.5 16",
  magnify: "M17 10.5a6.5 6.5 0 1 1-13 0 6.5 6.5 0 0 1 13 0zM15.3 15.3 21 21",
  dof: "M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0zM12 3l3.6 7.4M21 12l-8 1.6M15.6 19.8 10 13.9M5.2 18.3l3.8-7.3M4.3 7.1l8.2.6M14.8 3.4 9.3 9.5",
  close: "M6 6l12 12M18 6 6 18",
  plus: "M12 5v14M5 12h14",
  minus: "M5 12h14",
  rotate: "M20 11a8 8 0 1 0-2.3 5.7M20 4.5V11h-6.5",
  left: "M14.5 6 8.5 12l6 6",
  right: "M9.5 6l6 6-6 6",
};

export type IconName = keyof typeof PATHS;

export function Icon({ name }: { name: IconName }) {
  return (
    <svg className="icon" viewBox="0 0 24 24" aria-hidden="true">
      <path d={PATHS[name]} />
    </svg>
  );
}
