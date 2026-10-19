async function request(method, path, body) {
  const response = await fetch(`/api${path}`, {
    method,
    headers: body ? { 'content-type': 'application/json' } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  const data = await response.json().catch(() => null);
  if (!response.ok) {
    throw new Error(data?.error ?? `Request failed (${response.status})`);
  }
  return data;
}

export const listBars = () => request('GET', '/bars');
export const getBar = (id) => request('GET', `/bars/${id}`);
export const createBar = (bar) => request('POST', '/bars', bar);
export const moveBar = (id, lat, lng) => request('PATCH', `/bars/${id}/location`, { lat, lng });
export const rateBar = (id, rating) => request('POST', `/bars/${id}/ratings`, rating);

/** Calls `onBar(type, feature)` whenever any visitor adds, moves or rates a bar. */
export function subscribe(onBar) {
  const source = new EventSource('/api/events');
  for (const type of ['bar_created', 'bar_moved', 'bar_rated']) {
    source.addEventListener(type, (event) => {
      const payload = JSON.parse(event.data);
      onBar(type, payload.bar);
    });
  }
  return source;
}
