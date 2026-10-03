const MASK = "[REDACTED]";

export function redact(text: string): string {
  text = text.replace(/-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----[\s\S]*?(?:-----END (?:[A-Z]+ )?PRIVATE KEY-----|$)/g, MASK);
  text = text.replace(/\bhttps?:\/\/[^\s/@:]+:[^\s/@]+@/gi, match => `${match.slice(0, match.indexOf("://") + 3)}${MASK}@`);
  text = text.replace(/\bAuthorization[ \t]*:[^\r\n]*/gi, `Authorization: ${MASK}`);
  text = text.replace(/\bBearer[ \t]+[^\s"',;]+/gi, `Bearer ${MASK}`);
  text = text.replace(/\b(?:gh[pousr]_[A-Za-z0-9_]{8,}|github_pat_[A-Za-z0-9_]{8,}|sk-(?:ant-|proj-)?[A-Za-z0-9_-]{8,}|xox[abprs]-[A-Za-z0-9-]{8,}|(?:AKIA|ASIA)[A-Z0-9]{16}|AIza[A-Za-z0-9_-]{8,}|(?:sk_live|rk_live)_[A-Za-z0-9]{8,})\b/g, MASK);
  text = text.replace(/\beyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b/g, MASK);
  // Consume each key token once, including keys with no following assignment.
  const keys = /["']?[\w.-]+["']?/g;
  const chunks: string[] = [];
  let copied = 0;
  for (let match = keys.exec(text); match; match = keys.exec(text)) {
    if (!/token|secret|password|passwd|api_key|apikey|private_key|client_secret/i.test(match[0])) continue;
    let start = keys.lastIndex;
    while (text[start] === " " || text[start] === "\t") start++;
    if (text[start] !== "=" && text[start] !== ":") continue;
    start++;
    while (text[start] === " " || text[start] === "\t") start++;
    const quote = text[start] === '"' || text[start] === "'" ? text[start++]! : "";
    let end = start;
    while (end < text.length && text[end] !== "\r" && text[end] !== "\n") {
      if (quote ? text[end] === quote : /[\s,;}]/.test(text[end]!)) break;
      if (quote && text[end] === "\\" && end + 1 < text.length) end++;
      end++;
    }
    keys.lastIndex = end + (quote && text[end] === quote ? 1 : 0);
    if (end - start < 8) continue;
    chunks.push(text.slice(copied, start), MASK);
    copied = end;
  }
  return chunks.length ? chunks.join("") + text.slice(copied) : text;
}

export function isSecretPath(path: string): boolean {
  const parts = path.replaceAll("\\", "/").toLowerCase().split("/").filter(Boolean);
  const base = parts.at(-1) ?? "";
  if (parts.some(part => [".ssh", ".gnupg", ".aws", ".cloudflared", ".kube"].includes(part))) return true;
  for (let i = 0; i < parts.length - 1; i++) {
    if (parts[i] === ".config" && ["gh", "prism"].includes(parts[i + 1]!)) return true;
    if (parts[i] === ".docker" && parts[i + 1] === "config.json") return true;
  }
  if (base === "hosts.yml" && parts.slice(0, -1).includes("gh")) return true;
  if (base === ".env" || (base.startsWith(".env.") && ![".env.example", ".env.sample", ".env.template"].includes(base))) return true;
  return /\.(?:pem|key|p12|pfx)$/.test(base) || /^id_(?:rsa|ed25519|ecdsa)/.test(base)
    || base.includes(".keychain") || [".netrc", ".npmrc", ".pypirc"].includes(base)
    || base.startsWith("credentials") || base.includes("secret") || /token.*\.json$/.test(base);
}
