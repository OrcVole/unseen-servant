//! Gopher wire layer (RFC 1436 + 35 years of practice), pure and fuzzed.
//!
//! The whole protocol is: the client writes a selector line, the server
//! writes a body, the connection closes. There is no status code, no
//! header, no content type on the wire, and no keep-alive. What a body
//! *means* is decided entirely by the item type in the **menu line that
//! linked to it** — which is why menus are the load-bearing structure in
//! gopherspace and why the render target (`crate::render::gopher`) is
//! where this protocol's real work happens.
//!
//! Deliberate deviation from this crate's Gemini strictness: a request
//! terminated by a bare LF is **accepted** here, where the Gemini
//! listener drops it. The reasons that make strictness right there do
//! not apply: there is no equivalent of gemini-diagnostics'
//! `RequestMissingCR` expectation, real gopher clients across three
//! decades are inconsistent about the terminator, and nothing about a
//! selector's framing is security-relevant the way a Gemini request
//! line's is (no query, no userinfo, no scheme confusion). Being liberal
//! costs nothing and buys interoperability with clients nobody maintains
//! any more.

/// Longest selector accepted, in bytes, before the request is refused.
///
/// RFC 1436 sets no limit — it predates the idea that anyone might send
/// a hostile one. This matches the Gemini request cap for consistency,
/// and exists so a peer cannot make the server buffer without bound.
pub const MAX_SELECTOR_BYTES: usize = 1024;

/// A gopher item type: one leading character on a menu line, which tells
/// the client what it will get if the selector is followed.
///
/// Only the types usv actually emits are modelled. The registry is much
/// larger (RFC 1436 §3.8 plus later convention); an unmodelled type is
/// not an error we can encounter, because we never parse menus — we only
/// write them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemType {
    /// `0` — a plain text file, dot-stuffed and terminated with a lastline.
    Text,
    /// `1` — a menu (directory). What a gemtext page becomes.
    Menu,
    /// `3` — an error. Gopher has no status codes; an error *is* a menu
    /// line, which is why a failed request still returns a valid menu.
    Error,
    /// `7` — a search server. Reserved: usv has no search (ADR 0005 —
    /// nothing executes), but the type is modelled so a future
    /// static-index search cannot be added without thinking about it.
    Search,
    /// `9` — binary. Anything that is not text and has no better type.
    Binary,
    /// `g` — GIF, and `I` — other images. Kept distinct because some
    /// long-lived clients only special-case `g`.
    Gif,
    /// `I` — an image that is not a GIF.
    Image,
    /// `h` — an HTML link, conventionally with a `URL:` selector. How a
    /// gopher menu points at the web mirror or an external `https://`
    /// link that has no gopher equivalent.
    Html,
    /// `i` — an informational line: text shown in a menu that is not a
    /// link. Not in RFC 1436, universally supported, and the reason a
    /// gemtext paragraph can appear in a menu at all.
    Info,
}

impl ItemType {
    /// The single character this type occupies at the start of a menu line.
    pub fn as_char(self) -> char {
        match self {
            ItemType::Text => '0',
            ItemType::Menu => '1',
            ItemType::Error => '3',
            ItemType::Search => '7',
            ItemType::Binary => '9',
            ItemType::Gif => 'g',
            ItemType::Image => 'I',
            ItemType::Html => 'h',
            ItemType::Info => 'i',
        }
    }

    /// The type to advertise for a file usv is serving, chosen from its
    /// extension. Deliberately coarse: gopher's type registry cannot
    /// express most MIME types, and guessing wrong is worse than
    /// `Binary`, which every client can at least download.
    pub fn for_path(path: &str) -> Self {
        let lower = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
        let ext = lower.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
        match ext {
            "gmi" | "gemini" | "txt" | "text" | "md" | "asc" => ItemType::Text,
            "gif" => ItemType::Gif,
            "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tif" | "tiff" | "svg" => ItemType::Image,
            "html" | "htm" => ItemType::Html,
            _ => ItemType::Binary,
        }
    }
}

/// A parsed gopher request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The selector, with its terminator stripped. Empty means the root
    /// menu — the convention every client relies on when given a bare
    /// `gopher://host/`.
    pub selector: String,
    /// Search terms, present only when the client sent
    /// `selector<TAB>terms` against a type-7 item. usv never emits a
    /// type-7 item today, so this is always `None` in practice; it is
    /// parsed rather than ignored so the terms can never be mistaken for
    /// part of the selector, which would otherwise turn a search request
    /// into a bizarre path lookup.
    pub search: Option<String>,
}

/// Why a selector line was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestError {
    /// No terminator within [`MAX_SELECTOR_BYTES`]. The peer is refused.
    TooLong,
    /// No terminator *yet*, and the cap is not reached: the caller should
    /// read more and try again. Distinct from [`Self::TooLong`] because a
    /// listener reading incrementally must be able to tell "wait" from
    /// "refuse", and conflating them either truncates honest requests or
    /// buffers hostile ones forever.
    Incomplete,
    /// The selector contained a NUL or a control character other than
    /// the tab that separates search terms. Nothing legitimate sends
    /// these, and they are a classic way to smuggle something past a
    /// later layer that treats the string differently.
    ControlByte,
    /// Not valid UTF-8. RFC 1436 predates Unicode and says nothing about
    /// encoding, but a selector maps onto a path here, and paths that
    /// are not UTF-8 have no representation this server can safely use.
    NotUtf8,
}

/// Parse one selector line from bytes already read from the socket.
///
/// Accepts `CRLF` or a bare `LF` (see the module docs for why the Gemini
/// listener's strictness is deliberately not copied here). Returns the
/// request and the number of bytes consumed, so a caller that over-read
/// can tell what was left behind.
pub fn parse_selector_line(raw: &[u8]) -> Result<(Request, usize), RequestError> {
    let line_end = raw.iter().position(|&b| b == b'\n');
    let Some(nl) = line_end else {
        return if raw.len() >= MAX_SELECTOR_BYTES {
            Err(RequestError::TooLong)
        } else {
            Err(RequestError::Incomplete)
        };
    };
    if nl > MAX_SELECTOR_BYTES {
        return Err(RequestError::TooLong);
    }
    // Strip the LF, then a CR if this was a proper CRLF.
    let mut line = &raw[..nl];
    if line.last() == Some(&b'\r') {
        line = &line[..line.len() - 1];
    }

    // Split search terms before validating, so a tab is only ever
    // legitimate in the one position that means something.
    let (sel_bytes, search_bytes) = match line.iter().position(|&b| b == b'\t') {
        Some(i) => (&line[..i], Some(&line[i + 1..])),
        None => (line, None),
    };

    for &b in sel_bytes {
        if b < 0x20 || b == 0x7f {
            return Err(RequestError::ControlByte);
        }
    }
    if let Some(s) = search_bytes {
        for &b in s {
            if b < 0x20 || b == 0x7f {
                return Err(RequestError::ControlByte);
            }
        }
    }

    let selector = std::str::from_utf8(sel_bytes)
        .map_err(|_| RequestError::NotUtf8)?
        .to_string();
    let search = match search_bytes {
        Some(s) => Some(
            std::str::from_utf8(s)
                .map_err(|_| RequestError::NotUtf8)?
                .to_string(),
        ),
        None => None,
    };

    Ok((Request { selector, search }, nl + 1))
}

/// One line of a gopher menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuLine {
    /// What the client will get if this line is followed.
    pub item: ItemType,
    /// What the user sees. Tabs and newlines are stripped when written,
    /// because either would forge a new field or a new line.
    pub display: String,
    /// The selector the client sends back to retrieve this item.
    pub selector: String,
    /// The host serving it — gopher menus are absolute by construction,
    /// which is why a menu can link straight into another server.
    pub host: String,
    /// The port on that host.
    pub port: u16,
}

impl MenuLine {
    /// An informational line — text in a menu that is not a link.
    ///
    /// The selector/host/port are conventional filler: clients ignore
    /// them for type `i`, but omitting the fields entirely breaks
    /// parsers that split on a fixed field count.
    pub fn info(display: impl Into<String>) -> Self {
        Self {
            item: ItemType::Info,
            display: display.into(),
            selector: String::new(),
            host: "error.host".to_string(),
            port: 1,
        }
    }

    /// Serialise to the wire, with the field separators the display text
    /// could otherwise forge stripped out.
    pub fn to_wire(&self) -> String {
        format!(
            "{}{}\t{}\t{}\t{}\r\n",
            self.item.as_char(),
            scrub_field(&self.display),
            scrub_field(&self.selector),
            scrub_field(&self.host),
            self.port
        )
    }
}

/// Remove the bytes that are structural on a menu line.
///
/// A tab would forge a field boundary and a CR/LF would forge a whole
/// line — either lets attacker-influenced text (a filename, a heading
/// from a page someone uploaded over Titan) invent menu entries that
/// point wherever it likes. Stripping is preferred to escaping because
/// gopher has no escape syntax to use.
fn scrub_field(s: &str) -> String {
    s.chars()
        .filter(|c| *c != '\t' && *c != '\r' && *c != '\n')
        .collect()
}

/// The terminator every menu and text body ends with.
pub const LASTLINE: &str = ".\r\n";

/// Dot-stuff a text body and append the lastline.
///
/// A line consisting of a single `.` terminates the response, so any
/// content line that begins with `.` must be doubled or it truncates the
/// document at that point — the oldest bug in the protocol.
pub fn text_body(content: &str) -> String {
    let mut out = String::with_capacity(content.len() + 16);
    for line in content.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.starts_with('.') {
            out.push('.');
        }
        out.push_str(line);
        out.push_str("\r\n");
    }
    out.push_str(LASTLINE);
    out
}

/// A complete error response: gopher has no status codes, so an error is
/// a one-line menu the client renders like any other.
pub fn error_menu(message: &str) -> String {
    let mut out = MenuLine {
        item: ItemType::Error,
        display: message.to_string(),
        selector: String::new(),
        host: "error.host".to_string(),
        port: 1,
    }
    .to_wire();
    out.push_str(LASTLINE);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_selector_is_the_root_menu() {
        let (req, n) = parse_selector_line(b"\r\n").expect("valid");
        assert_eq!(req.selector, "");
        assert_eq!(req.search, None);
        assert_eq!(n, 2);
    }

    #[test]
    fn a_bare_lf_is_accepted_unlike_gemini() {
        // Deliberate asymmetry; see the module docs.
        let (req, _) = parse_selector_line(b"/about\n").expect("valid");
        assert_eq!(req.selector, "/about");
    }

    #[test]
    fn search_terms_are_split_from_the_selector() {
        let (req, _) = parse_selector_line(b"/search\tsome terms\r\n").expect("valid");
        assert_eq!(req.selector, "/search");
        assert_eq!(req.search.as_deref(), Some("some terms"));
    }

    #[test]
    fn control_bytes_are_refused() {
        assert_eq!(
            parse_selector_line(b"/a\x00b\r\n"),
            Err(RequestError::ControlByte)
        );
    }

    #[test]
    fn a_partial_line_asks_for_more_rather_than_refusing() {
        // The listener reads incrementally; conflating this with
        // TooLong would truncate honest requests mid-flight.
        assert_eq!(parse_selector_line(b"/abo"), Err(RequestError::Incomplete));
    }

    #[test]
    fn an_unterminated_flood_is_refused_not_awaited() {
        let raw = vec![b'a'; MAX_SELECTOR_BYTES + 1];
        assert_eq!(parse_selector_line(&raw), Err(RequestError::TooLong));
    }

    #[test]
    fn an_overlong_selector_is_refused() {
        let mut raw = vec![b'a'; MAX_SELECTOR_BYTES + 10];
        raw.extend_from_slice(b"\r\n");
        assert_eq!(parse_selector_line(&raw), Err(RequestError::TooLong));
    }

    #[test]
    fn a_display_string_cannot_forge_menu_structure() {
        // The nightmare case: a page title (or an uploaded filename)
        // containing a tab invents a selector field pointing elsewhere.
        let line = MenuLine {
            item: ItemType::Menu,
            display: "evil\tfake\there".to_string(),
            selector: "/real".to_string(),
            host: "example.org".to_string(),
            port: 70,
        };
        let wire = line.to_wire();
        assert_eq!(wire.matches('\t').count(), 3, "exactly the real fields");
        assert!(wire.starts_with("1evilfakehere\t/real\t"));
    }

    #[test]
    fn a_display_string_cannot_forge_a_new_line() {
        let line = MenuLine::info("first\r\n1evil\t/x\tevil.host\t70");
        let wire = line.to_wire();
        assert_eq!(wire.matches("\r\n").count(), 1, "only its own terminator");
    }

    #[test]
    fn leading_dots_are_stuffed_so_a_body_cannot_self_terminate() {
        let body = text_body("safe\n.\n.hidden\n");
        assert!(body.contains("..\r\n"), "a lone dot is doubled: {body:?}");
        assert!(body.contains("..hidden\r\n"), "{body:?}");
        assert!(body.ends_with(LASTLINE));
    }

    #[test]
    fn text_bodies_end_with_the_lastline() {
        assert!(text_body("hello").ends_with("\r\n.\r\n"));
    }

    #[test]
    fn item_types_come_from_the_extension() {
        assert_eq!(ItemType::for_path("/a/b.gmi"), ItemType::Text);
        assert_eq!(ItemType::for_path("/a/b.PNG"), ItemType::Image);
        assert_eq!(ItemType::for_path("/a/b.gif"), ItemType::Gif);
        assert_eq!(ItemType::for_path("/a/b.html"), ItemType::Html);
        assert_eq!(ItemType::for_path("/a/b.tar.gz"), ItemType::Binary);
        assert_eq!(ItemType::for_path("/noext"), ItemType::Binary);
    }

    #[test]
    fn an_error_is_a_valid_one_line_menu() {
        let e = error_menu("not found");
        assert!(e.starts_with('3'));
        assert!(e.ends_with(LASTLINE));
        assert_eq!(e.lines().count(), 2, "the error line and the lastline");
    }
}
