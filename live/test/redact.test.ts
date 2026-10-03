import { expect, test } from "bun:test";
import { isSecretPath, redact } from "../src/redact";

const tokens = [
  ...["ghp", "gho", "ghu", "ghs", "ghr"].map(prefix => `${prefix}_SYNTHETIC12345678`), "github_pat_SYNTHETIC12345678",
  "sk-SYNTHETIC12345678", "sk-ant-SYNTHETIC12345678", "sk-proj-SYNTHETIC12345678",
  ...["a", "b", "p", "r", "s"].map(kind => `xox${kind}-12345678-synthetic`),
  "AKIA1234567890ABCDEF", "ASIA1234567890ABCDEF", "AIzaSYNTHETIC1234567890", "sk_" + "live_SYNTHETIC12345678", "rk_" + "live_SYNTHETIC12345678",
  ["eyJhbGciOiJIUzI1NiJ9", "eyJzdWIiOiIxMjMifQ", "syntheticSignature"].join("."),
];
for (const token of tokens) test(`masks synthetic ${token.split(/[_-]/)[0]}`, () => expect(redact(`before ${token} after`)).toBe("before [REDACTED] after"));

for (const key of ["token", "access_token", "secret", "password", "passwd", "api_key", "apikey", "private_key", "client_secret", "aws_secret_access_key"]) {
  test(`masks assignment ${key}`, () => {
    expect(redact(`${key} = synthetic-value-123`)).toBe(`${key} = [REDACTED]`);
    expect(redact(`"${key}": "synthetic value 123"`)).toBe(`"${key}": "[REDACTED]"`);
  });
}
test("masks authorization and URL credentials", () => {
  expect(redact("Bearer synthetic-token-value")).toBe("Bearer [REDACTED]");
  expect(redact("Authorization: Basic synthetic-value\nAccept: application/json")).toBe("Authorization: [REDACTED]\nAccept: application/json");
  expect(redact("https://user:synthetic-password@example.com/repo")).toBe("https://[REDACTED]@example.com/repo");
});
test("masks complete and truncated private key blocks", () => {
  expect(redact("before\n-----BEGIN " + "RSA PRIVATE KEY-----\nsynthetic-key-data\n-----END RSA PRIVATE KEY-----\nafter")).toBe("before\n[REDACTED]\nafter");
  expect(redact("-----BEGIN " + "PRIVATE KEY-----\nsynthetic-key-data")).toBe("[REDACTED]");
});
test("ordinary code and short assignments stay intact", () => {
  const code = 'const result = add(1, 2);\nconst url = "https://example.com";\npassword=short';
  expect(redact(code)).toBe(code);
  expect(redact("a-".repeat(100000))).toBe("a-".repeat(100000));
});

for (const path of [".env", "config/.env.production", "a.pem", "a.key", "a.p12", "a.pfx", "id_rsa.pub", "id_ed25519", "id_ecdsa_old", "login.keychain-db", ".netrc", ".npmrc", ".pypirc", "credentials.json", "my-secrets.txt", "refresh-token.json", "gh/hosts.yml", ".ssh/config", ".gnupg/data", ".aws/config", ".config/gh/config.yml", ".config/prism/config", ".cloudflared/config.yml", ".docker/config.json", ".kube/config"]) {
  test(`blocks secret path ${path}`, () => expect(isSecretPath(path)).toBe(true));
}
for (const path of [".env.example", ".env.sample", ".env.template", "src/key.ts", "src/token.ts", "config.json", "hosts.yml", ".docker/Dockerfile", "README.md"]) {
  test(`allows ordinary path ${path}`, () => expect(isSecretPath(path)).toBe(false));
}
