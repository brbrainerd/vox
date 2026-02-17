// Auto-generated API client for Vox server functions
// Do not edit manually — regenerated on each build.

const API_BASE = '';

export async function greet(name: string): Promise<Greeting> {
  const response = await fetch(`${API_BASE}/api/greet`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ name }),
  });
  if (!response.ok) throw new Error(`Server error: ${response.status}`);
  return response.json();
}

export async function add(a: number, b: number): Promise<number> {
  const response = await fetch(`${API_BASE}/api/add`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ a, b }),
  });
  if (!response.ok) throw new Error(`Server error: ${response.status}`);
  return response.json();
}

