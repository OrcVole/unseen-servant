# What we learned from the other servers

**Unseen Servant**

`usv` is not a first attempt at this problem. Every small network already
has a server that solved something well, and the design started by reading
them rather than by starting from scratch. This page records what each one
taught and: just as usefully: what `usv` declined to copy.

The full research, with dates and sources, is in
[`internal/recon/prior-art.md`](internal/recon/prior-art.md) and
[`internal/recon/smolnet.md`](internal/recon/smolnet.md). Where you would be
better off running one of these instead,
[`../COMPARISON.md`](../COMPARISON.md) says so.

## Agate: certificate lifecycle

Rust, static files only by explicit policy, feature-frozen and still
maintained. Its releases are mostly dependency bumps, which is what a
finished project looks like.

**Taken:** the whole certificate story, which is the best in the field.
Generate a key per hostname on first run with no setup step, set the expiry
far enough out that it never churns, keep certificates in per-hostname
directories so multi-hostname serving needs no configuration syntax, and
accept an operator's own certificate in the same slot. Also its
file-permission hygiene at generation time, and clean handling of the
signals a container stops you with: something Agate had to retrofit.

**Declined:** configuration by command-line flags only. That makes the
Dockerfile the configuration file, which does not survive packaging.

## gmid: configuration semantics, and real tests

C, the most actively developed server in the field: FastCGI, reverse
proxying, virtual host and location blocks, and a four-process
privilege-separation architecture that is the security high-water mark for a
C server.

**Taken:** two things. Its configuration *semantics*: named host blocks,
path matching, sane defaults, and reloading configuration and certificates
on a signal without dropping listeners. And its testing discipline: an
in-tree suite that runs the real binary against real sockets. gmid was the
only server surveyed with serious integration tests, and `usv`'s wire suite
exists because of it.

**Declined:** the multi-process separation and its message-passing plumbing.
The *goals* translate to Rust in a container; the mechanism does not. Memory
safety removes the class of bug that made isolating the parser from the key
material worthwhile, and the container provides the filesystem isolation.
What survives is the shape: one task owns log output, and private key
material lives inside the TLS (Transport Layer Security) configuration where
request-handling code cannot reach it.

Also declined: a custom configuration grammar. gmid's semantics are
excellent; reimplementing a yacc-grade parser for them is not.

## Molly Brown: certificate zones

Go, written by Gemini's creator, aimed at pubnix and shared hosting.

**Taken:** certificate zones, essentially as designed: path-scoped
allowlists of client-certificate fingerprints, which its own documentation
describes as analogous to SSH's `authorized_keys`. It is the simplest
possible client-authentication story and it maps exactly onto Gemini's
status codes. `usv`'s zones are a direct descendant, later extended with a
named roster and capabilities. Also taken: TOML (Tom's Obvious Minimal
Language) as the configuration format, and the principle that a file inside
the content tree may never override a security-relevant setting.

**Declined:** `~username` expansion and the world-readable bit as a
publishing switch. Those are pubnix concerns; `usv` is single-tenant.

## GmCapsule: Titan, done properly

Python, the reference Titan implementation and the extensibility flagship.
Bubble, the small internet's most successful interaction platform, runs as a
GmCapsule module rather than a separate daemon.

**Taken:** its Titan handling as the correctness reference: buffer the
whole upload before dispatching it, require a client certificate by default,
and make the caller's fingerprint available to the code that authorises the
write. `usv`'s Titan design was checked against it.

**Declined:** the module interface itself. `usv` has no extension
programming interface and is not growing one. If what you are building is a
program rather than a capsule, GmCapsule is the right foundation and this is
not.

## Jetforce: the internal shape

Python, self-described as experimental, and effectively the reference
teaching implementation. It is both a server and a framework: routing over
request and response objects, with static file serving implemented as one
application among several rather than as a special case.

**Taken:** exactly that shape. `usv` has a `Handler` trait; static serving
is one handler, and so are redirects, certificate zones, Titan uploads and
the status resource. That is what lets them compose instead of accreting
conditionals.

**Declined:** exposing it as a public extension interface.

## twins: the counterexample

Go, static serving plus per-path reverse proxying to several kinds of
backend. Barely maintained now, and its issue tracker is the evidence file:
response bodies cropped, images loading intermittently, path handling only
partly working, and a failure of the connection-shutdown check in the
community conformance suite.

**Taken:** the idea that a path may map to different kinds of thing, as
internal architecture.

**Declined:** proxying itself. Almost all of twins' defect load came from
the one feature beyond static serving. That single observation is the
strongest argument behind ADR (architecture decision record) 0005, the
decision that `usv` never executes or fetches anything on a visitor's behalf
: no CGI (Common Gateway Interface), no FastCGI, no proxying, permanently.

## The Gopher servers: menu conventions

gophernicus, geomyidae, Bucktooth and pygopherd between them define what
modern gopherspace actually expects, most of which is convention rather than
RFC (Request for Comments) 1436 (where RFC is Request for Comments, the
standards series).

**Taken:** the informational line type that is the backbone of every modern
menu, the `URL:` link convention for pointing at other protocols, `caps.txt`
as the closest thing gopher has to a server identity endpoint, and the hard
formatting rules: display strings under about seventy columns, and never a
tab character inside a field.

**Declined:** Gopher+ entirely, since nothing modern depends on it; and the
type-7 search item, which needs a query handler and so contradicts a static
model.

## Agate+: the closest prior art for the web mirror

The nearest thing to what `usv` does with HTML (HyperText Markup Language):
it converts gemtext to HTML per request.

**Taken:** the idea that one content tree can honestly serve two audiences.

**Declined:** doing the conversion per request. `usv` renders the whole tree
at write time instead, which makes the web mirror trivially cacheable and, 
the part that matters: makes the rendered output a portable folder that
works with no server behind it at all. That is what `usv export` hands you.

## gemini-diagnostics: the gate

Not a server: a torture test, frozen but canonical, and the community's
standard check before a server is exposed publicly. `usv` treats a clean run
as a hard gate rather than as advice.

Its limits are worth knowing, because passing it is not the same as being
correct: it has no client-certificate tests at all, no redirect-chain or
timeout tests, no virtual-host tests, and its traversal check has a known
false negative. Those gaps are exactly where `usv`'s own tests and fuzz
targets have to earn their keep.

## The pattern

Reading all of them together, one thing stands out. The features people have
actually wanted from a small-internet server over the past six years are a
short list: automatic certificates, redirects, per-directory metadata,
client-certificate gating, uploads, and every server that grew an escape
hatch beyond static serving spent most of its maintenance budget on that
escape hatch.

`usv` therefore has the short list, built in, and no escape hatch.
