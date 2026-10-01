// dist/safe.js — extracted from Fox Stars Lab.app/Contents/Resources/mcp/server.mjs (1.4.0-fox.1 bundle)
import path from "node:path";
var MAX_EXCERPT = 800;
var MAX_RESPONSE = 32 * 1024;
var MAX_QUERY = 500;
var SECRET_KEY = /^(?:api[_-]?key|token(?:_ref)?|authorization|cookie|password|secret|headers|command|args|env)$/i;
var CREDENTIAL_ASSIGNMENT = /\b(api[_-]?key|access[_-]?token|refresh[_-]?token|token|password|passwd|pwd|cookie|authorization)\s*[:=]\s*(?:Basic\s+|Bearer\s+)?[^\s,;]+/gi;
var AUTH_HEADER = /\b(?:Authorization|Cookie)\s*:\s*[^\r\n]+/gi;
var PREFIXED_CREDENTIAL = /\b(?:sk-[A-Za-z0-9_-]{8,}|ghp_[A-Za-z0-9_-]{8,}|github_pat_[A-Za-z0-9_-]{8,}|xox[baprs]-[A-Za-z0-9_-]{8,})\b/gi;
var POSIX_ABSOLUTE_PATH = /(^|[\s('"=])\/(?:Users|home|private|var|tmp|Volumes|opt|etc)\/(?:[^\s)'",;]+\/?)+/g;
var WINDOWS_ABSOLUTE_PATH = /\b[A-Za-z]:\\(?:[^\\\s]+\\)+[^\s,;]+/g;
var CONTROL = /[\u0000-\u0008\u000b\u000c\u000e-\u001f]/g;
var PublicError = class extends Error {
  code;
  constructor(code, message) {
    super(message);
    this.code = code;
  }
};
function codePointSlice(value, max) {
  return Array.from(value).slice(0, max).join("");
}
function redactText(value) {
  return String(value ?? "").replace(AUTH_HEADER, "[credential redacted]").replace(CREDENTIAL_ASSIGNMENT, "$1=[redacted]").replace(PREFIXED_CREDENTIAL, "[credential redacted]").replace(POSIX_ABSOLUTE_PATH, "$1[local-path]").replace(WINDOWS_ABSOLUTE_PATH, "[local-path]").replace(CONTROL, " ");
}
function text(value, max = MAX_EXCERPT) {
  return codePointSlice(redactText(value), max);
}
function safeObject(value) {
  if (Array.isArray(value))
    return value.map(safeObject);
  if (value && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).filter(([key]) => !SECRET_KEY.test(key)).map(([key, item]) => [key, safeObject(item)]));
  }
  return typeof value === "string" ? text(value, 4e3) : value;
}
function safeUrl(value) {
  try {
    const url = new URL(String(value));
    if (!["http:", "https:"].includes(url.protocol))
      return void 0;
    url.username = "";
    url.password = "";
    for (const key of [...url.searchParams.keys()]) {
      if (/token|key|secret|auth|password|signature|cookie/i.test(key)) {
        url.searchParams.delete(key);
      }
    }
    url.hash = "";
    if (/\b(?:token|secret|password|api[_-]?key)\b/i.test(decodeURIComponent(url.pathname))) {
      url.pathname = "/[sensitive-path]";
    }
    return url.toString();
  } catch {
    return void 0;
  }
}
function localLabel(value) {
  const name = path.basename(String(value ?? "").replace(/\\/g, "/"));
  return text(name || "local-document", 200);
}
function query(value) {
  const result = String(value ?? "").trim();
  if (!result || Array.from(result).length > MAX_QUERY) {
    throw new PublicError("invalid_input", "query \u5FC5\u987B\u4E3A 1..500 \u4E2A\u5B57\u7B26");
  }
  return result;
}
function limit(value, max) {
  const result = Number(value ?? Math.min(10, max));
  if (!Number.isInteger(result) || result < 1 || result > max) {
    throw new PublicError("invalid_input", `limit \u5FC5\u987B\u4E3A 1..${max}`);
  }
  return result;
}
function like(value) {
  return `%${value.replace(/\\/g, "\\\\").replace(/%/g, "\\%").replace(/_/g, "\\_")}%`;
}
function excerptAroundMatch(body, needle) {
  const clean = redactText(body);
  const points = Array.from(clean);
  const index = clean.toLocaleLowerCase().indexOf(needle.toLocaleLowerCase());
  if (index < 0)
    return codePointSlice(clean, MAX_EXCERPT);
  const before = Array.from(clean.slice(0, index)).length;
  const start = Math.max(0, before - 240);
  return points.slice(start, start + MAX_EXCERPT).join("");
}
function budget(value) {
  const clean = safeObject(value);
  if (Buffer.byteLength(JSON.stringify(clean)) <= MAX_RESPONSE)
    return clean;
  const arrays = [];
  const visit = (item) => {
    if (Array.isArray(item)) {
      arrays.push(item);
      item.forEach(visit);
    } else if (item && typeof item === "object") {
      Object.values(item).forEach(visit);
    }
  };
  visit(clean);
  while (Buffer.byteLength(JSON.stringify(clean)) > MAX_RESPONSE) {
    const target = arrays.find((array2) => array2.length > 0);
    if (!target)
      break;
    target.pop();
    clean.truncated = true;
  }
  return clean;
}

