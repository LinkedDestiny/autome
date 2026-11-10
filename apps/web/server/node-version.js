export const MIN_NODE_VERSION = '22.22.0';

/** @param {string} version */
export function checkNodeVersion(version) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) return false;
  const parts = version.split('.').map(Number);
  if (!parts.every(Number.isSafeInteger)) return false;
  const minimum = MIN_NODE_VERSION.split('.').map(Number);
  for (let i = 0; i < minimum.length; i++) {
    if (parts[i] !== minimum[i]) return parts[i] > minimum[i];
  }
  return true;
}
