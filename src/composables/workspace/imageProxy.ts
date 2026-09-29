export function isGithubImageURL(value: string) {
  try {
    const host = new URL(value).hostname.toLowerCase();
    return host.includes('githubusercontent.com') || host.includes('github.com');
  } catch {
    return false;
  }
}

export function normalizeProxyURL(value: string) {
  const trimmed = value.trim() || 'https://gh-proxy.com/';
  return trimmed.endsWith('/') ? trimmed : trimmed + '/';
}

export function withGithubProxy(content: string, proxyURL: string) {
  const normalizedProxy = normalizeProxyURL(proxyURL);
  return content.replace(/(https?:\/\/(?:[^\s)"']*githubusercontent\.com|[^\s)"']*github\.com)[^\s)"']*)/gi, url => {
    if (url.startsWith(normalizedProxy)) return url;
    return normalizedProxy + url;
  });
}
