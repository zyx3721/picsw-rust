export function createClientId() {
  if (globalThis.crypto && typeof globalThis.crypto.randomUUID === 'function') {
    return globalThis.crypto.randomUUID();
  }
  const randomPart = Math.random().toString(36).slice(2, 10);
  return `${Date.now().toString(36)}-${randomPart}`;
}
