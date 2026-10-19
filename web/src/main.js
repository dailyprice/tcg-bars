import * as maplibregl from 'maplibre-gl';
import workerUrl from 'maplibre-gl/dist/maplibre-gl-worker.mjs?worker&url';
import { Geoman } from '@geoman-io/maplibre-geoman-free';

import 'maplibre-gl/dist/maplibre-gl.css';
import '@geoman-io/maplibre-geoman-free/dist/maplibre-geoman.css';
import './style.css';

import * as api from './api.js';

maplibregl.setWorkerUrl(workerUrl);

const COPENHAGEN = [12.5600, 55.6720];
const CATEGORIES = [
  { key: 'lighting', label: 'Lighting', hint: 'Can you read your cards?' },
  { key: 'table_space', label: 'Table space', hint: 'Room for playmats?' },
  { key: 'offers', label: 'Player offers', hint: 'Deals or perks for players' },
];

const map = new maplibregl.Map({
  container: 'map',
  // OpenFreeMap: free vector tiles, no API key.
  style: 'https://tiles.openfreemap.org/styles/positron',
  center: COPENHAGEN,
  zoom: 13,
});
map.addControl(new maplibregl.NavigationControl(), 'bottom-left');
map.addControl(
  new maplibregl.GeolocateControl({ positionOptions: { enableHighAccuracy: false } }),
  'bottom-left',
);

// Geoman handles placing and dragging pins. Its own toolbar stays hidden; the
// app drives it from its own buttons.
const gm = new Geoman(map, { settings: { useControlsUi: false } });

const panel = document.getElementById('panel');
const panelBody = document.getElementById('panel-body');
const toast = document.getElementById('toast');
const addButton = document.getElementById('add-bar');

/** All bars, keyed by id; the map source is rebuilt from this. */
const bars = new Map();
let selectedId = null;
/** The pin Geoman is currently managing: a new bar or one being moved. */
let draft = null; // { feature, mode: 'new' | 'move', barId? }
/** True between clicking "Add a bar" and placing the marker. */
let placing = false;

// --- helpers -------------------------------------------------------------

const escape = (value) =>
  String(value ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

let toastTimer;
function showToast(message) {
  toast.textContent = message;
  toast.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (toast.hidden = true), 3500);
}

function openPanel(html) {
  panelBody.innerHTML = html;
  panel.hidden = false;
}

function closePanel() {
  panel.hidden = true;
  selectedId = null;
  discardDraft();
}

function refreshSource() {
  const source = map.getSource('bars');
  if (!source) return;
  const hidden = draft?.mode === 'move' ? draft.barId : null;
  source.setData({
    type: 'FeatureCollection',
    features: [...bars.values()].filter((f) => f.id !== hidden),
  });
}

function upsert(feature) {
  bars.set(feature.id, feature);
  refreshSource();
}

const draftCoordinates = () => draft.feature.getGeoJson().geometry.coordinates;

async function startDraft(coordinates, mode, barId) {
  await discardDraft();
  const feature = await gm.features.importGeoJsonFeature({
    type: 'Feature',
    geometry: { type: 'Point', coordinates },
    properties: { shape: 'marker' },
  });
  draft = { feature, mode, barId };
  refreshSource();
  await gm.enableGlobalDragMode();
}

async function discardDraft() {
  if (!draft) return;
  const { feature } = draft;
  draft = null;
  if (gm.globalDragModeEnabled()) await gm.disableGlobalDragMode();
  await gm.features.delete(feature);
  refreshSource();
}

// --- rendering -------------------------------------------------------------

function scoreBar(value) {
  if (value == null) return '<span class="muted">Not rated yet</span>';
  const pct = (value / 5) * 100;
  return `<span class="meter"><span style="width:${pct}%"></span></span><span class="score">${value.toFixed(1)}</span>`;
}

function renderBar({ bar, ratings }) {
  const p = bar.properties;
  const place = [p.address, p.city].filter(Boolean).map(escape).join(', ');
  const stars = (name) =>
    [1, 2, 3, 4, 5]
      .map((n) => `<label><input type="radio" name="${name}" value="${n}" required /><span>${n}</span></label>`)
      .join('');

  return `
    <h2>${escape(p.name)}</h2>
    ${place ? `<p class="muted">${place}</p>` : ''}
    ${p.location_verified ? '' : `<p class="notice">This pin is approximate. If you know where it is, move it onto the venue.</p>`}
    ${p.player_offers ? `<p class="offer">🎴 ${escape(p.player_offers)}</p>` : ''}

    <dl class="ratings">
      ${CATEGORIES.map((c) => `<dt>${c.label}</dt><dd>${scoreBar(p[`avg_${c.key}`])}</dd>`).join('')}
    </dl>
    <p class="muted small">${p.rating_count} rating${p.rating_count === 1 ? '' : 's'}</p>

    <div class="actions">
      <button type="button" class="button" data-action="move">Move pin</button>
      ${p.website ? `<a class="button" href="${escape(p.website)}" target="_blank" rel="noopener">Website</a>` : ''}
    </div>

    <form id="rate-form" class="stack">
      <h3>Rate it</h3>
      ${CATEGORIES.map(
        (c) => `<fieldset class="stars"><legend>${c.label} <span class="muted small">${c.hint}</span></legend>${stars(c.key)}</fieldset>`,
      ).join('')}
      <label class="field">Comment <textarea name="comment" rows="2" maxlength="1000" placeholder="Big tables upstairs, quiet before 8pm…"></textarea></label>
      <button class="button primary" type="submit">Submit rating</button>
    </form>

    ${
      ratings.some((r) => r.comment)
        ? `<h3>What players say</h3><ul class="comments">${ratings
            .filter((r) => r.comment)
            .map((r) => `<li>${escape(r.comment)}</li>`)
            .join('')}</ul>`
        : ''
    }
  `;
}

function renderMoveControls(name) {
  return `
    <h2>Move ${escape(name)}</h2>
    <p>Drag the pin onto the venue, then save.</p>
    <div class="actions">
      <button type="button" class="button primary" data-action="save-move">Save location</button>
      <button type="button" class="button" data-action="cancel-move">Cancel</button>
    </div>`;
}

function renderNewBarForm() {
  return `
    <h2>Add a bar</h2>
    <p class="muted">Drag the pin if it's not quite on the venue.</p>
    <form id="new-bar-form" class="stack">
      <label class="field">Name <input name="name" required maxlength="120" autocomplete="off" /></label>
      <label class="field">Address <input name="address" maxlength="200" /></label>
      <label class="field">City <input name="city" maxlength="100" /></label>
      <label class="field">Country <input name="country" maxlength="100" /></label>
      <label class="field">Offers for players <input name="player_offers" maxlength="500" placeholder="e.g. 2-for-1 on Tuesday game night" /></label>
      <label class="field">Website <input name="website" type="url" maxlength="300" placeholder="https://" /></label>
      <div class="actions">
        <button class="button primary" type="submit">Add bar</button>
        <button class="button" type="button" data-action="cancel-new">Cancel</button>
      </div>
    </form>`;
}

async function showBar(id) {
  selectedId = id;
  try {
    openPanel(renderBar(await api.getBar(id)));
  } catch (err) {
    showToast(err.message);
  }
}

// --- map setup -------------------------------------------------------------

map.on('load', async () => {
  map.addSource('bars', { type: 'geojson', data: { type: 'FeatureCollection', features: [] } });
  map.addLayer({
    id: 'bars-circle',
    type: 'circle',
    source: 'bars',
    paint: {
      'circle-radius': ['interpolate', ['linear'], ['zoom'], 3, 4, 14, 9],
      // Unrated bars are grey; rated ones go from red (1) to green (5).
      'circle-color': [
        'case',
        ['==', ['get', 'score'], null],
        '#8a8f98',
        ['interpolate', ['linear'], ['get', 'score'], 1, '#d9534f', 3, '#f0ad4e', 5, '#2e9e5b'],
      ],
      'circle-stroke-width': 2,
      'circle-stroke-color': ['case', ['get', 'location_verified'], '#ffffff', '#1d2330'],
    },
  });
  map.addLayer({
    id: 'bars-label',
    type: 'symbol',
    source: 'bars',
    minzoom: 11,
    layout: {
      'text-field': ['get', 'name'],
      'text-font': ['Noto Sans Regular'],
      'text-size': 12,
      'text-offset': [0, 1.3],
      'text-anchor': 'top',
    },
    paint: { 'text-color': '#1d2330', 'text-halo-color': '#ffffff', 'text-halo-width': 1.5 },
  });

  map.on('click', 'bars-circle', (e) => {
    if (draft) return;
    showBar(e.features[0].properties.id);
  });
  map.on('mouseenter', 'bars-circle', () => (map.getCanvas().style.cursor = 'pointer'));
  map.on('mouseleave', 'bars-circle', () => (map.getCanvas().style.cursor = ''));

  try {
    const collection = await api.listBars();
    for (const feature of collection.features) bars.set(feature.id, feature);
    refreshSource();
  } catch (err) {
    showToast(`Couldn't load bars: ${err.message}`);
  }

  api.subscribe((type, feature) => {
    const isNew = !bars.has(feature.id);
    upsert(feature);
    if (type === 'bar_created' && isNew) showToast(`New bar on the map: ${feature.properties.name}`);
    if (feature.id === selectedId && !draft) showBar(feature.id);
  });
});

// Adding a bar: Geoman draws the marker, then we keep it as a draggable draft.
addButton.addEventListener('click', async () => {
  await discardDraft();
  selectedId = null;
  panel.hidden = true;
  showToast('Click the map where the bar is');
  placing = true;
  await gm.enableDraw('marker');
});

map.on('gm:create', async ({ feature }) => {
  // Only react to the user placing a marker, not to drafts we import.
  if (!placing) return;
  placing = false;
  const coordinates = feature.getGeoJson().geometry.coordinates;
  await gm.disableDraw();
  await gm.features.delete(feature);
  await startDraft(coordinates, 'new');
  openPanel(renderNewBarForm());
  panelBody.querySelector('input[name=name]').focus();
});

// --- panel interactions ----------------------------------------------------

document.getElementById('panel-close').addEventListener('click', closePanel);

panelBody.addEventListener('click', async (e) => {
  const action = e.target.closest('[data-action]')?.dataset.action;
  if (!action) return;

  if (action === 'move' && selectedId) {
    const bar = bars.get(selectedId);
    await startDraft(bar.geometry.coordinates, 'move', selectedId);
    openPanel(renderMoveControls(bar.properties.name));
  } else if (action === 'save-move' && draft) {
    const [lng, lat] = draftCoordinates();
    const id = draft.barId;
    try {
      upsert(await api.moveBar(id, lat, lng));
      await discardDraft();
      showToast('Location saved');
      showBar(id);
    } catch (err) {
      showToast(err.message);
    }
  } else if (action === 'cancel-move') {
    const id = draft?.barId;
    await discardDraft();
    if (id) showBar(id);
  } else if (action === 'cancel-new') {
    closePanel();
  }
});

panelBody.addEventListener('submit', async (e) => {
  e.preventDefault();
  const form = new FormData(e.target);

  if (e.target.id === 'new-bar-form' && draft) {
    const [lng, lat] = draftCoordinates();
    const body = Object.fromEntries(form);
    try {
      const feature = await api.createBar({ ...body, lat, lng });
      upsert(feature);
      await discardDraft();
      showToast(`Added ${feature.properties.name}. Thanks!`);
      showBar(feature.id);
    } catch (err) {
      showToast(err.message);
    }
  } else if (e.target.id === 'rate-form' && selectedId) {
    const rating = {
      lighting: Number(form.get('lighting')),
      table_space: Number(form.get('table_space')),
      offers: Number(form.get('offers')),
      comment: form.get('comment'),
    };
    try {
      upsert(await api.rateBar(selectedId, rating));
      showToast('Thanks for rating!');
      showBar(selectedId);
    } catch (err) {
      showToast(err.message);
    }
  }
});

document.addEventListener('keydown', async (e) => {
  if (e.key !== 'Escape') return;
  if (placing) {
    placing = false;
    await gm.disableDraw();
  }
  closePanel();
});
