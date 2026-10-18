import { writeFileSync } from 'node:fs';

const plot = { left: 230, right: 1080, top: 120, bottom: 380 };
const points = [
  [4, 2200], [5, 3400], [6, 3400], [7, 6500], [18, 6500],
  [19, 3400], [24, 3400], [25, 2200], [27, 2200],
];
const levels = [['Daytime', 6500], ['Morning/Evening', 3400], ['Sleep', 2200]];
const anchors = [
  { name: 'Wake', hour: 5, kelvin: 3400, offset: 0, labelY: 250, align: 'middle' },
  { name: 'Sunrise', hour: 7, kelvin: 6500, offset: 0, labelY: 78, align: 'middle' },
  { name: 'Sunset', hour: 19, kelvin: 3400, offset: 0, labelY: 343, align: 'middle' },
  { name: 'Bedtime', hour: 25, kelvin: 2200, offset: 16, labelY: 334, align: 'start' },
];
const firstHour = points[0][0];
const lastHour = points.at(-1)[0];
const highestKelvin = levels[0][1];
const lowestKelvin = levels.at(-1)[1];

function x(hour) {
  return Number((plot.left + (hour - firstHour) / (lastHour - firstHour) * (plot.right - plot.left)).toFixed(2));
}

function y(kelvin) {
  return Number((plot.bottom - (kelvin - lowestKelvin) / (highestKelvin - lowestKelvin) * (plot.bottom - plot.top)).toFixed(2));
}

function clock(hour) {
  return String(hour % 24).padStart(2, '0') + ':00';
}

const transitions = points.flatMap(([hour, kelvin], index) => {
  const next = points[index + 1];
  return next && next[1] !== kelvin ? [[hour, next[0]]] : [];
});
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="1120" height="560" viewBox="0 0 1120 560" role="img" aria-labelledby="curve-title curve-description">
  <title id="curve-title">Scheduled color temperature across midnight</title>
  <desc id="curve-description">Example of a display color temperature schedule on a day without a clock change. Sleep is 2200 K, Morning/Evening is 3400 K, and Daytime is 6500 K. One-hour transitions end at Wake 05:00, Sunrise 07:00, Sunset 19:00, and Bedtime 01:00 the next date. The curve continues through midnight without resetting.</desc>
  <style>
    text { font-family: "Segoe UI", Arial, sans-serif; fill: #1f2937; }
    .heading { font-size: 26px; font-weight: 600; }
    .label { font-size: 18px; font-weight: 600; }
    .detail { font-size: 17px; fill: #526171; }
    .axis { font-size: 16px; fill: #526171; font-variant-numeric: tabular-nums; }
    .guide { stroke: #d7dfe8; stroke-width: 1; }
    .bracket { fill: none; stroke: #526171; stroke-width: 1.5; }
  </style>
  <rect width="1120" height="560" fill="#ffffff"/>
  <text x="28" y="36" class="heading">Scheduled color temperature</text>
  <rect x="${x(24)}" y="104" width="${plot.right - x(24)}" height="316" fill="#f2f6fb"/>
  <text x="${x(24) - 12}" y="92" class="axis" text-anchor="end">Midnight</text>
  <text x="${x(24) + 12}" y="92" class="axis">Next date</text>
${levels.map(([name, kelvin]) => `  <g>
    <line x1="210" y1="${y(kelvin)}" x2="${plot.right}" y2="${y(kelvin)}" class="guide"/>
    <text x="28" y="${y(kelvin) - 6}" class="label">${name}</text>
    <text x="28" y="${y(kelvin) + 19}" class="detail">${kelvin} K</text>
  </g>`).join('\n')}
  <line x1="${x(24)}" y1="104" x2="${x(24)}" y2="420" stroke="#8b9bad" stroke-width="1.5" stroke-dasharray="5 5"/>
  <polyline points="${points.map(([hour, kelvin]) => `${x(hour)},${y(kelvin)}`).join(' ')}" fill="none" stroke="#3975b4" stroke-width="3" stroke-linejoin="round"/>
${anchors.map(anchor => `  <g data-hour="${anchor.hour}" data-kelvin="${anchor.kelvin}">
    <circle cx="${x(anchor.hour)}" cy="${y(anchor.kelvin)}" r="5" fill="#3975b4" stroke="#ffffff" stroke-width="2"/>
    <text x="${x(anchor.hour) + anchor.offset}" y="${anchor.labelY}" class="label" text-anchor="${anchor.align}">${anchor.name}</text>
    <text x="${x(anchor.hour) + anchor.offset}" y="${anchor.labelY + 23}" class="detail" text-anchor="${anchor.align}">${clock(anchor.hour)}</text>
  </g>`).join('\n')}
  <line x1="${plot.left}" y1="420" x2="${plot.right}" y2="420" class="guide"/>
${[4, 6, 12, 18, 24, 27].map(hour => `  <line x1="${x(hour)}" y1="420" x2="${x(hour)}" y2="426" stroke="#8b9bad"/>
  <text x="${x(hour)}" y="446" class="axis" text-anchor="middle">${clock(hour)}</text>`).join('\n')}
${transitions.map(([start, end]) => `  <path d="M ${x(start)} 462 V 470 H ${x(end)} V 462" class="bracket"/>
  <text x="${(x(start) + x(end)) / 2}" y="496" class="axis" text-anchor="middle">${(end - start) * 60} min</text>`).join('\n')}
  <text x="${(plot.left + plot.right) / 2}" y="536" class="detail" text-anchor="middle">Local time</text>
</svg>
`;

writeFileSync(new URL('./daily-temperature-curve.svg', import.meta.url), svg);
