import { createRemoteJWKSet, jwtVerify, type JWTVerifyGetKey } from "jose";
import type { AccessConfig } from "./config";

export function createAccessVerifier(config: AccessConfig, keySet?: JWTVerifyGetKey) {
  const issuer = `https://${config.team}.cloudflareaccess.com`;
  const keys = keySet ?? createRemoteJWKSet(new URL(`${issuer}/cdn-cgi/access/certs`));
  return async (assertion: string | string[] | undefined): Promise<string | undefined> => {
    if (typeof assertion !== "string" || !assertion) return "missing jwt";
    try {
      const { payload } = await jwtVerify(assertion, keys, {
        algorithms: ["RS256"], issuer, audience: config.aud, requiredClaims: ["exp", "email"],
      });
      if (typeof payload.email !== "string" || payload.email.toLowerCase() !== config.email.toLowerCase()) return "wrong email";
    } catch { return "invalid jwt"; }
    return undefined;
  };
}
