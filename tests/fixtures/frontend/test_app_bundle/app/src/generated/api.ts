// Auto-generated API client for Vox server functions
// @query → GET + JSON query values; @server / @mutation → POST + JSON body.
// Do not edit manually — regenerated on each build.

const API_BASE = '';

export async function list_items(): Promise<Item[]> {
  const response = await fetch(`${API_BASE}/api/query/list_items`, { method: 'GET' });
  if (!response.ok) throw new Error(`Server error: ${response.status}`);
  return response.json();
}

export async function add_item(name: string, value: number): Promise<Result<string>> {
  const response = await fetch(`${API_BASE}/api/mutation/add_item`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name, value }),
  });
  if (!response.ok) throw new Error(`Server error: ${response.status}`);
  return response.json();
}

