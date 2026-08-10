//! The colophon: what this is, what you are reading it over, and how.
//!
//! A visitor arriving over gopher or nex has no idea what "usv" is, why
//! it is abbreviated that way, what the protocol they are using *is*, or
//! which clients speak it. Every surface therefore serves a page that
//! says so, in its own words for its own protocol.
//!
//! Generated per request rather than rendered into the content trees,
//! for two reasons: the text must differ per protocol (a nex page has to
//! say "nex"), and the list of addresses must come from live
//! configuration or it goes stale the moment a listener is toggled. It
//! is not the operator's content and never appears in their tree.

/// The protocol a colophon is being written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// Gemini, over TLS.
    Gemini,
    /// Gopher menus.
    Gopher,
    /// Spartan, gemtext without the cryptography.
    Spartan,
    /// Nex, the minimum.
    Nex,
    /// Finger, a person rather than a document.
    Finger,
}

impl Protocol {
    /// The name as a reader would say it.
    pub fn name(self) -> &'static str {
        match self {
            Protocol::Gemini => "Gemini",
            Protocol::Gopher => "Gopher",
            Protocol::Spartan => "Spartan",
            Protocol::Nex => "Nex",
            Protocol::Finger => "Finger",
        }
    }

    /// The URL scheme.
    pub fn scheme(self) -> &'static str {
        match self {
            Protocol::Gemini => "gemini",
            Protocol::Gopher => "gopher",
            Protocol::Spartan => "spartan",
            Protocol::Nex => "nex",
            Protocol::Finger => "finger",
        }
    }

    /// Where the protocol itself is documented.
    ///
    /// Every entry is a real, fetchable URL, because these become
    /// gemtext link lines: prose here emits `=> RFC 1288 ...`, a link
    /// whose target is not a URL. The RFC-defined protocols therefore
    /// point at the RFC's canonical web copy rather than naming it in
    /// passing — a reader who has just learned Finger exists should be
    /// one activation away from its specification, not left to search.
    ///
    /// Spartan's own spec is published only over `spartan://`, which is
    /// circular for someone who has no client yet, so the web proxy is
    /// given instead.
    pub fn references(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Protocol::Gemini => &[("https://geminiprotocol.net/", "The Gemini protocol")],
            Protocol::Gopher => &[
                ("https://gopher.floodgap.com/", "Gopherspace, over the web"),
                (
                    "https://datatracker.ietf.org/doc/html/rfc1436",
                    "RFC 1436 — the Gopher specification (1993)",
                ),
            ],
            Protocol::Spartan => &[(
                "http://portal.mozz.us/spartan/spartan.mozz.us/specification.gmi",
                "The Spartan specification",
            )],
            Protocol::Nex => &[(
                "https://nightfall.city/nex/info/specification.txt",
                "The Nex specification",
            )],
            Protocol::Finger => &[(
                "https://datatracker.ietf.org/doc/html/rfc1288",
                "RFC 1288 — the Finger specification (1991)",
            )],
        }
    }

    /// One paragraph: what it is and what it is for.
    pub fn about(self) -> &'static str {
        match self {
            Protocol::Gemini => {
                "Gemini is a small internet protocol for documents, deliberately capped so \
                 that one person can write a client in a weekend and no vendor can make it \
                 complicated. It has no cookies, no scripting and no tracking — not as \
                 features it declines to use, but as things it cannot do. It is the only \
                 protocol here that can authenticate a reader, which is what makes private \
                 areas and remote editing possible."
            }
            Protocol::Gopher => {
                "Gopher has been serving menus since 1991 and has not changed its mind \
                 since. It treats the internet as a filing cabinet you browse rather than a \
                 library of documents you read. That refusal to evolve is the point: \
                 software written in the nineties still works, and what you publish today \
                 will very likely still be readable by clients nobody has touched in \
                 decades."
            }
            Protocol::Spartan => {
                "Spartan is Gemini's document model with the cryptography removed. Its \
                 argument is that mandatory TLS is ceremony for public documents — clocks \
                 that must be right, libraries that must be maintained, hardware that must \
                 do the maths — and that a plain document deserves a plain protocol. It \
                 reads the same gemtext."
            }
            Protocol::Nex => {
                "Nex is the smallest thing that still works: send a path, get bytes, the \
                 connection closes. No status codes, no content types, no headers. It is \
                 explicitly telnet-compatible, so you can speak it by hand, and the whole \
                 protocol fits in your head — which is the reason people like it."
            }
            Protocol::Finger => {
                "Finger is the internet's oldest status update. It answers \"what is this \
                 person up to?\" — the .plan file, which predates blogging by decades and \
                 does the same job in four lines. It does not serve documents."
            }
        }
    }

    /// Clients known to speak this protocol natively.
    ///
    /// Verified in `docs/recon/smolnet.md` and `docs/recon/ecosystem.md`,
    /// August 2026. Client support drifts, so the date is printed with
    /// the list rather than left implicit.
    pub fn clients(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Protocol::Gemini => &[
                ("Lagrange (graphical)", "https://gmi.skyjake.fi/lagrange/"),
                (
                    "amfora (terminal)",
                    "https://github.com/makeworld-the-better-one/amfora",
                ),
                (
                    "Bombadillo (terminal)",
                    "https://bombadillo.colorfield.space/",
                ),
                ("Offpunk (terminal)", "https://offpunk.net/"),
                ("gelim (terminal)", "https://git.sr.ht/~hedy/gelim"),
            ],
            Protocol::Gopher => &[
                ("Lagrange (graphical)", "https://gmi.skyjake.fi/lagrange/"),
                (
                    "Bombadillo (terminal)",
                    "https://bombadillo.colorfield.space/",
                ),
                ("lynx (terminal)", "https://lynx.invisible-island.net/"),
                ("Offpunk (terminal)", "https://offpunk.net/"),
                ("BFG (terminal)", "https://codeberg.org/luxferre/BFG"),
            ],
            Protocol::Spartan => &[
                ("Lagrange (graphical)", "https://gmi.skyjake.fi/lagrange/"),
                ("Offpunk (terminal)", "https://offpunk.net/"),
                ("gelim (terminal)", "https://git.sr.ht/~hedy/gelim"),
                ("BFG (terminal)", "https://codeberg.org/luxferre/BFG"),
            ],
            Protocol::Nex => &[
                ("gelim (terminal)", "https://git.sr.ht/~hedy/gelim"),
                ("BFG (terminal)", "https://codeberg.org/luxferre/BFG"),
            ],
            Protocol::Finger => &[
                ("Lagrange (graphical)", "https://gmi.skyjake.fi/lagrange/"),
                (
                    "Bombadillo (terminal)",
                    "https://bombadillo.colorfield.space/",
                ),
                ("the finger(1) command", "RFC 1288; ships with many systems"),
            ],
        }
    }
}

/// Every address this capsule actually answers on, from live config.
#[derive(Debug, Clone, Default)]
pub struct Addresses {
    /// The capsule's hostname.
    pub host: String,
    /// Gemini's advertised port, when the listener is on.
    pub gemini_port: Option<u16>,
    /// The web mirror's base URL, when the HTTP surface is on.
    pub web_base_url: Option<String>,
    /// Gopher's advertised port, when on.
    pub gopher_port: Option<u16>,
    /// Spartan's port, when on.
    pub spartan_port: Option<u16>,
    /// Nex's port, when on.
    pub nex_port: Option<u16>,
    /// Finger's port, when on.
    pub finger_port: Option<u16>,
}

impl Addresses {
    /// Derive every address from live configuration.
    ///
    /// The single place this mapping exists. Each surface is present
    /// only when its listener is configured, so a colophon can never
    /// advertise a protocol the capsule does not actually answer on —
    /// the page is true by construction rather than by remembering.
    pub fn from_config(cfg: &crate::config::Config) -> Self {
        let host = cfg
            .advertised_host
            .clone()
            .or_else(|| cfg.hosts.first().map(|h| h.name.clone()))
            .unwrap_or_default();
        Self {
            web_base_url: cfg.http_listen.map(|_| format!("https://{host}")),
            gemini_port: Some(cfg.advertised_port),
            gopher_port: cfg.gopher.as_ref().map(|g| g.advertised_port),
            spartan_port: cfg.spartan.as_ref().map(|s| s.listen.port()),
            nex_port: cfg.nex.as_ref().map(|n| n.listen.port()),
            finger_port: cfg.finger.as_ref().map(|f| f.listen.port()),
            host,
        }
    }

    /// Every live address as `(protocol, url)`, in a stable order.
    ///
    /// Ports are always written out for the cleartext protocols, because
    /// their clients assume the canonical one (70, 300, 1900, 79) and a
    /// capsule elsewhere is simply unreachable without it.
    pub fn all(&self) -> Vec<(Protocol, String)> {
        let mut out = Vec::new();
        if let Some(p) = self.gemini_port {
            out.push((
                Protocol::Gemini,
                if p == crate::protocol::GEMINI_DEFAULT_PORT {
                    format!("gemini://{}/", self.host)
                } else {
                    format!("gemini://{}:{p}/", self.host)
                },
            ));
        }
        if let Some(p) = self.gopher_port {
            out.push((Protocol::Gopher, format!("gopher://{}:{p}/", self.host)));
        }
        if let Some(p) = self.spartan_port {
            out.push((Protocol::Spartan, format!("spartan://{}:{p}/", self.host)));
        }
        if let Some(p) = self.nex_port {
            out.push((Protocol::Nex, format!("nex://{}:{p}/", self.host)));
        }
        if let Some(p) = self.finger_port {
            out.push((Protocol::Finger, format!("finger://{}:{p}", self.host)));
        }
        out
    }
}

/// The path the colophon answers on, on every protocol.
///
/// Short and typeable, because on Nex and Gopher a reader may well be
/// entering it by hand. The operator's own file at this path always
/// wins — the colophon fills a gap, it does not occupy a slot.
pub const PATH: &str = "/usv";

/// Whether `path` is asking for the colophon.
///
/// The extension variants are not decoration. Nex has no content types,
/// so clients infer them from the suffix — and gelim 0.13.1 does not
/// merely fall back when there is no extension, it **panics** on
/// `nex://host/usv` (`nex.go:38`, indexing the result of a split that
/// found no dot). `nex://host/usv.gmi` renders correctly.
///
/// So `/usv` stays accepted because it is what a person types, and
/// `/usv.gmi` is what documentation should advertise for the gemtext
/// protocols. Being liberal here costs one match arm and is the
/// difference between a page and a stack trace.
///
/// Gopher is unaffected — it carries the type character in the URL
/// instead (`gopher://host:port/0/usv`).
pub fn matches(path: &str) -> bool {
    let p = path.trim_end_matches('/');
    matches!(p, "/usv" | "/usv.gmi" | "/usv.txt")
}

/// The colophon as gemtext, written for `protocol`.
pub fn gemtext(protocol: Protocol, addrs: &Addresses) -> String {
    let mut s = String::with_capacity(2048);
    let name = protocol.name();
    let scheme = protocol.scheme();

    s.push_str(&format!("# This is a {name} page\n\n"));

    // Where you are, and what served it.
    let self_url = addrs
        .all()
        .into_iter()
        .find(|(p, _)| *p == protocol)
        .map(|(_, u)| u)
        .unwrap_or_else(|| format!("{scheme}://{}/", addrs.host));
    s.push_str(&format!(
        "You are reading this at {self_url} — served to you by the Unseen Servant \
         (usv) {name} server.\n\n"
    ));

    // The name, which nobody can be expected to guess.
    s.push_str("## Unseen Servant, and why \"usv\"\n\n");
    s.push_str(
        "Unseen Servant is a small server that publishes one folder of writing to several \
         internet protocols at once. The short name takes the letters from the long one:\n\n",
    );
    s.push_str("```\nUnSeen serVant  ->  u  s  v\n```\n\n");
    s.push_str(
        "It is abbreviated rather than spelled out because it is also a command you type.\n\n",
    );

    // What this protocol is.
    s.push_str(&format!("## About {name}\n\n{}\n\n", protocol.about()));
    for (url, label) in protocol.references() {
        s.push_str(&format!("=> {url} {label}\n"));
    }
    s.push('\n');

    // How to read it natively.
    s.push_str(&format!("## Reading {name} natively\n\n"));
    s.push_str(&format!(
        "If this page arrived looking strange, you may be reading it through a gateway. \
         These clients speak {name} directly:\n\n"
    ));
    for (client, url) in protocol.clients() {
        if url.starts_with("http") {
            s.push_str(&format!("=> {url} {client}\n"));
        } else {
            s.push_str(&format!("* {client} — {url}\n"));
        }
    }
    s.push_str("\nClient support changes; this list was checked in August 2026.\n\n");

    // The same words, elsewhere.
    let others: Vec<_> = addrs
        .all()
        .into_iter()
        .filter(|(p, _)| *p != protocol)
        .collect();
    if !others.is_empty() || addrs.web_base_url.is_some() {
        s.push_str("## The same capsule, other protocols\n\n");
        s.push_str(
            "This capsule is one folder of writing, rendered to each of these. The hostname \
             never changes — the scheme at the front picks the protocol and the port:\n\n",
        );
        for (p, url) in &others {
            s.push_str(&format!("* {}: {url}\n", p.name()));
        }
        if let Some(web) = &addrs.web_base_url {
            s.push_str(&format!("* Web: {web}/\n"));
        }
        s.push('\n');
    }

    s.push_str("=> / Back to the capsule\n");
    s
}

/// The colophon as plain text, for surfaces with no gemtext renderer.
///
/// Gopher and Finger readers see markup as literal characters, so link
/// lines are flattened to `label — url` and heading markers dropped.
/// The wording is not duplicated: this is the same text, unmarked.
pub fn plain(protocol: Protocol, addrs: &Addresses) -> String {
    let mut out = String::with_capacity(2048);
    for line in gemtext(protocol, addrs).lines() {
        if let Some(rest) = line.strip_prefix("=> ") {
            let (url, label) = rest.split_once(' ').unwrap_or((rest, ""));
            if label.is_empty() {
                out.push_str(&format!("  {url}\n"));
            } else {
                out.push_str(&format!("  {label} — {url}\n"));
            }
        } else if line == "```" {
            // The fence itself carries nothing; its contents do.
            continue;
        } else if let Some(rest) = line.strip_prefix("## ") {
            out.push_str(&format!("{rest}\n{}\n", "-".repeat(rest.chars().count())));
        } else if let Some(rest) = line.strip_prefix("# ") {
            out.push_str(&format!("{rest}\n{}\n", "=".repeat(rest.chars().count())));
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addrs() -> Addresses {
        Addresses {
            host: "example.org".into(),
            gemini_port: Some(1965),
            web_base_url: Some("https://example.org".into()),
            gopher_port: Some(70),
            spartan_port: Some(300),
            nex_port: Some(1900),
            finger_port: Some(79),
        }
    }

    #[test]
    fn every_protocol_names_itself_and_its_own_address() {
        for (p, expect) in [
            (Protocol::Gopher, "gopher://example.org:70/"),
            (Protocol::Nex, "nex://example.org:1900/"),
            (Protocol::Spartan, "spartan://example.org:300/"),
        ] {
            let out = gemtext(p, &addrs());
            assert!(
                out.starts_with(&format!("# This is a {} page", p.name())),
                "{out}"
            );
            assert!(out.contains(expect), "{p:?} missing own address: {out}");
        }
    }

    #[test]
    fn the_name_is_always_explained() {
        // The whole reason this page exists: nobody can guess "usv".
        for p in [Protocol::Gemini, Protocol::Gopher, Protocol::Nex] {
            let out = gemtext(p, &addrs());
            assert!(out.contains("UnSeen serVant"), "{p:?}: {out}");
            assert!(out.contains("Unseen Servant"), "{p:?}");
        }
    }

    #[test]
    fn native_clients_are_listed_for_the_protocol_in_hand() {
        let nex = gemtext(Protocol::Nex, &addrs());
        assert!(nex.contains("gelim"), "{nex}");
        // Lagrange does not speak Nex; it must not be suggested here.
        assert!(!nex.contains("Lagrange"), "wrong client suggested: {nex}");

        let gopher = gemtext(Protocol::Gopher, &addrs());
        assert!(
            gopher.contains("Lagrange") && gopher.contains("lynx"),
            "{gopher}"
        );
    }

    #[test]
    fn other_protocols_are_listed_but_never_this_one_twice() {
        let out = gemtext(Protocol::Gopher, &addrs());
        let section = out.split("other protocols").nth(1).unwrap_or("");
        assert!(section.contains("nex://"), "{out}");
        assert!(section.contains("gemini://"), "{out}");
        assert!(
            !section.contains("gopher://"),
            "listed itself as an alternative: {out}"
        );
    }

    #[test]
    fn only_live_surfaces_are_advertised() {
        // A capsule with just Gemini and Nex must not invent the rest.
        let a = Addresses {
            host: "example.org".into(),
            gemini_port: Some(1965),
            nex_port: Some(1900),
            ..Default::default()
        };
        let out = gemtext(Protocol::Nex, &a);
        assert!(out.contains("gemini://example.org/"), "{out}");
        assert!(!out.contains("gopher://"), "{out}");
        assert!(!out.contains("spartan://"), "{out}");
        assert!(!out.contains("https://example.org"), "{out}");
    }

    #[test]
    fn a_non_default_gemini_port_is_written_out() {
        let a = Addresses {
            host: "example.org".into(),
            gemini_port: Some(11965),
            ..Default::default()
        };
        assert!(gemtext(Protocol::Gemini, &a).contains("gemini://example.org:11965/"));
    }

    #[test]
    fn cleartext_addresses_always_carry_their_port() {
        // Their clients assume 70/300/1900/79; without the port the
        // address is simply wrong for a capsule hosted anywhere else.
        let out = gemtext(Protocol::Gemini, &addrs());
        assert!(out.contains("gopher://example.org:70/"));
        assert!(out.contains("nex://example.org:1900/"));
        assert!(out.contains("finger://example.org:79"));
    }

    #[test]
    fn the_plain_form_leaves_no_markup_for_gopher_readers() {
        let out = plain(Protocol::Gopher, &addrs());
        assert!(!out.contains("=> "), "link markup survived: {out}");
        assert!(!out.contains("```"), "fence survived: {out}");
        assert!(!out.starts_with('#'), "heading marker survived: {out}");
        // The content itself must still be all there.
        assert!(out.contains("UnSeen serVant"), "{out}");
        assert!(out.contains("https://gmi.skyjake.fi/lagrange/"), "{out}");
        assert!(out.contains("gopher://example.org:70/"), "{out}");
    }

    #[test]
    fn link_lines_never_carry_prose_as_their_target() {
        // Finger has no home page and Gopher's reference is an RFC, not
        // a URL. Emitting either as a link target produces a link that
        // points at prose — which is what this caught.
        for p in [
            Protocol::Gemini,
            Protocol::Gopher,
            Protocol::Spartan,
            Protocol::Nex,
            Protocol::Finger,
        ] {
            for line in gemtext(p, &addrs()).lines() {
                if let Some(rest) = line.strip_prefix("=> ") {
                    let target = rest.split(' ').next().unwrap_or("");
                    assert!(
                        target.contains("://") || target.starts_with('/'),
                        "{p:?} link target is not a URL: {line}"
                    );
                }
            }
        }
    }

    #[test]
    fn every_protocol_links_to_its_own_specification() {
        // Finger in particular: it has no home page, and an earlier
        // version of this simply omitted the link rather than reaching
        // for the RFC's canonical copy.
        for p in [
            Protocol::Gemini,
            Protocol::Gopher,
            Protocol::Spartan,
            Protocol::Nex,
            Protocol::Finger,
        ] {
            assert!(!p.references().is_empty(), "{p:?} links to nothing");
            let out = gemtext(p, &addrs());
            for (url, _) in p.references() {
                assert!(out.contains(url), "{p:?} dropped {url}");
            }
        }
        assert!(gemtext(Protocol::Finger, &addrs()).contains("rfc1288"));
    }
}
