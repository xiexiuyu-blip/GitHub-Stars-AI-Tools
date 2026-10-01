// dist/server.js — extracted from Fox Stars Lab.app/Contents/Resources/mcp/server.mjs (1.4.0-fox.1 bundle)
async function main() {
  try {
    const db = process.env.FOX_STARS_LAB_DB, account = process.env.FOX_STARS_LAB_ACCOUNT_ID;
    if (!db || !account)
      throw new PublicError("invalid_config", "\u5FC5\u987B\u914D\u7F6E FOX_STARS_LAB_DB \u548C FOX_STARS_LAB_ACCOUNT_ID");
    const ctx = openDb(db, account);
    await createServer(ctx).connect(new StdioServerTransport());
  } catch (e) {
    const x = e instanceof PublicError ? e : new PublicError("startup_failed", "\u542F\u52A8\u5931\u8D25");
    process.stderr.write(`[fox-stars-lab] ${x.code}: ${x.message}
`);
    process.exitCode = 1;
  }
}
void main();
