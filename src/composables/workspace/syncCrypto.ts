/// 云同步载荷的 WebCrypto 加解密：PBKDF2-HMAC-SHA256(600k) 派生 AES-256-GCM 密钥，
/// 字节布局与 Rust 加密层完全一致（存量云端密文可直接解密），解密走浏览器原生实现
const PBKDF2_ITERATIONS = 600_000;
/// 新加密对齐 dockpilot 使用 32 字节盐；解密不校验盐长，历史 16 字节盐密文同样可解
const SALT_LENGTH = 32;
const IV_LENGTH = 12;
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder();

function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}

function bytesToBase64(bytes: Uint8Array): string {
  let binary = '';
  bytes.forEach(byte => {
    binary += String.fromCharCode(byte);
  });
  return btoa(binary);
}

function base64ToBytes(value: string): Uint8Array {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i);
  return bytes;
}

async function deriveAesKey(password: string, salt: Uint8Array): Promise<CryptoKey> {
  const passwordKey = await crypto.subtle.importKey('raw', toArrayBuffer(textEncoder.encode(password)), 'PBKDF2', false, [
    'deriveKey',
  ]);
  return crypto.subtle.deriveKey(
    { name: 'PBKDF2', salt: toArrayBuffer(salt), iterations: PBKDF2_ITERATIONS, hash: 'SHA-256' },
    passwordKey,
    { name: 'AES-GCM', length: 256 },
    false,
    ['encrypt', 'decrypt'],
  );
}

export type PasswordSealed = { iv: string; salt: string; payload: string };

export async function encryptWithPassword(plaintext: string, password: string): Promise<PasswordSealed> {
  if (!password) throw new Error('同步密码不能为空');
  const salt = crypto.getRandomValues(new Uint8Array(SALT_LENGTH));
  const iv = crypto.getRandomValues(new Uint8Array(IV_LENGTH));
  const key = await deriveAesKey(password, salt);
  const sealed = new Uint8Array(
    await crypto.subtle.encrypt({ name: 'AES-GCM', iv: toArrayBuffer(iv) }, key, toArrayBuffer(textEncoder.encode(plaintext))),
  );
  return { iv: bytesToBase64(iv), salt: bytesToBase64(salt), payload: bytesToBase64(sealed) };
}

/// 分立式解密（对照 dockpilot：不校验盐长等格式细节，密码或格式错误统一由 GCM 认证失败兜底）
export async function decryptWithPasswordParts(
  payload: string,
  password: string,
  ivB64: string,
  saltB64: string,
): Promise<string> {
  if (!password) throw new Error('同步密码不能为空');
  try {
    const iv = base64ToBytes(ivB64);
    const salt = base64ToBytes(saltB64);
    const sealed = base64ToBytes(payload);
    const key = await deriveAesKey(password, salt);
    const plain = await crypto.subtle.decrypt({ name: 'AES-GCM', iv: toArrayBuffer(iv) }, key, toArrayBuffer(sealed));
    return textDecoder.decode(plain);
  } catch {
    throw new Error('同步密码不正确或云端数据已损坏');
  }
}

/// 旧内嵌格式：base64(salt + iv + 密文)，兼容分立式结构上线前的存量云端数据
export async function decryptWithPasswordEmbedded(payload: string, password: string): Promise<string> {
  if (!password) throw new Error('同步密码不能为空');
  const raw = base64ToBytes(payload);
  if (raw.length < SALT_LENGTH + IV_LENGTH) throw new Error('同步数据格式不正确');
  return decryptWithPasswordParts(
    bytesToBase64(raw.subarray(SALT_LENGTH + IV_LENGTH)),
    password,
    bytesToBase64(raw.subarray(SALT_LENGTH, SALT_LENGTH + IV_LENGTH)),
    bytesToBase64(raw.subarray(0, SALT_LENGTH)),
  );
}
