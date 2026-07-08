// Cloudflare Pages Functions middleware: gate the whole site behind a shared passcode
// (HTTP Basic Auth). The passcode is the `PASSWORD` environment secret set on the Pages
// project (`wrangler pages secret put PASSWORD`), never stored in the repo. This file is
// injected into the build output by `taliesin publish`.
export async function onRequest(context) {
  const { request, env, next } = context;
  const expected = env.PASSWORD;
  // Fail closed: if the secret is unset, never serve ungated content.
  if (!expected) {
    return new Response("Site not configured: missing PASSWORD secret.", {
      status: 503,
    });
  }
  const header = request.headers.get("Authorization") || "";
  const [scheme, encoded] = header.split(" ");
  if (scheme === "Basic" && encoded) {
    let decoded = "";
    try {
      decoded = atob(encoded);
    } catch {
      decoded = "";
    }
    // "user:pass"; compare only the password (a shared passcode has no per-user
    // identity). Constant-time compare to avoid a timing oracle.
    const pass = decoded.slice(decoded.indexOf(":") + 1);
    if (timingSafeEqual(pass, expected)) {
      return next();
    }
  }
  return new Response("Authentication required.", {
    status: 401,
    headers: { "WWW-Authenticate": 'Basic realm="draft", charset="UTF-8"' },
  });
}

// Content-constant-time string compare (both encoded to bytes first). The compare
// does not short-circuit on a byte mismatch; the loop length can still reveal the
// expected passcode's length, which is an acceptable leak for a shared draft gate.
function timingSafeEqual(a, b) {
  const enc = new TextEncoder();
  const ab = enc.encode(a);
  const bb = enc.encode(b);
  const len = Math.max(ab.length, bb.length);
  let diff = ab.length ^ bb.length;
  for (let i = 0; i < len; i++) {
    diff |= (ab[i] ?? 0) ^ (bb[i] ?? 0);
  }
  return diff === 0;
}
