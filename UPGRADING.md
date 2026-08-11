# Upgrading, and what survives what

Under Gemini's TOFU model (trust-on-first-use: clients pin your
certificate's fingerprint the way SSH pins host keys), **the keypair is your
capsule's identity**. usv therefore treats it as sacred: generated once,
never silently regenerated, never rotated behind your back (ADR 0003). This
document is the promise, spelled out per operation.

## The survival table (Cloudron)

Verified against Cloudron's own documentation, 2026-08-09
(`docs/internal/recon/cloudron-fit.md` §3), and, 2026-08-10, against a real running
Cloudron install: every row below was actually exercised (a real package
update, a real backup and restore, a real clone to a new domain), with the
certificate fingerprint checked byte-for-byte before and after each one
(`docs/internal/BUILD-PLAN.md` C6's E1: E10 protocol). This table is a tested claim,
not a read of the platform's own promises.

| Operation | Your identity (keypair) | Your content |
|---|---|---|
| Restart / crash | survives | survives |
| App update (new package version) | survives | survives |
| Restore from backup | as backed up | as backed up (note: app code also reverts to backup-time version) |
| Clone to another domain | copied: usv detects the hostname change, mints a FRESH keypair for the new name, and leaves the old one untouched on disk | copied |
| Move / relocate domain | survives (same hostname-change handling) | survives |
| Migrate to another server (via backup) | survives | survives |
| **Uninstall** | **destroyed**: recoverable only from a retained backup | **destroyed** |

Back up before uninstalling. Cloudron's app backups include `/app/data`,
which is everything usv needs to be reborn identical.

## Standalone

Everything lives under the state directory (`certs/`, `content/`, config).
Copy that directory and you have copied the capsule: identity, content,
settings. A cron'd tarball of it is a complete disaster-recovery plan.

## When usv itself updates

The maintenance posture is "finished software, actively watched"
(`docs/internal/ROADMAP.md`): most releases are dependency bumps. An update will
never change your certificate, rewrite your content, or alter config
semantics without a MAJOR version and an explicit migration note here.
Reserved config sections (`[titan]`, `[responses]`) error helpfully until
their feature ships rather than being silently ignored, so a config written
for a newer usv fails loudly on an older one instead of half-working.
