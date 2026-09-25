// Shared helpers for the source-discovery scripts.
//
// These scripts exist so that nobody (human or agent) ever loads ~/.claude.json or the Claude Desktop
// folder whole: those files can hold MCP server configs with API keys. They print a sanitised view
// of the whitelisted usage fields only.

const MAX_STRING = 40;
const MAX_ARRAY = 4;

/** Looks like a credential or opaque token: long, or key-like prefixes. */
function looksSecret(s) {
  return (
    s.length > MAX_STRING ||
    /^(sk-|gho_|ghp_|xox|eyJ|Bearer\s)/i.test(s) ||
    /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i.test(s) // account/org UUIDs
  );
}

/**
 * Return a copy of `value` safe to print: numbers/booleans/null kept, short strings kept unless they
 * look like secrets, long arrays truncated (first items + last item + a count).
 */
export function sanitize(value, depth = 0) {
  if (depth > 8) return "<max depth>";
  if (value === null || typeof value === "number" || typeof value === "boolean") return value;
  if (typeof value === "string") {
    return looksSecret(value) ? `<string len ${value.length}>` : value;
  }
  if (Array.isArray(value)) {
    const items = value.length > MAX_ARRAY
      ? [...value.slice(0, MAX_ARRAY - 1), `<... ${value.length - MAX_ARRAY} more ...>`, value[value.length - 1]]
      : value;
    return items.map((v) => (typeof v === "string" && v.startsWith("<...") ? v : sanitize(v, depth + 1)));
  }
  if (typeof value === "object") {
    const out = {};
    for (const [k, v] of Object.entries(value)) out[k] = sanitize(v, depth + 1);
    return out;
  }
  return `<${typeof value}>`;
}

/** Describe a file without reading it. */
export async function statInfo(fs, path) {
  try {
    const st = await fs.stat(path);
    return { exists: true, size: st.size, mtime: st.mtime.toISOString() };
  } catch {
    return { exists: false };
  }
}
