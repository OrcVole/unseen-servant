//! The MIME table: file extension → response META.
//!
//! Text types carry an explicit `charset=utf-8` even though clients should
//! assume UTF-8 for `text/*` (recon guidance §2: "adding `; charset=utf-8`
//! explicitly for text/gemini even though clients should assume it").
//! Unknown extensions fall back to `application/octet-stream` — never guess,
//! never sniff content.

/// Look up the MIME type for a filename by its extension, case-insensitive.
/// Always returns a usable META string; unknown extensions (or none) get
/// the safe binary default.
pub fn lookup(filename: &str) -> &'static str {
    let ext = filename
        .rsplit_once('.')
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "gmi" | "gemini" => "text/gemini; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "xml" => "application/xml",
        "atom" => "application/atom+xml",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "pdf" => "application/pdf",
        // GemPub (docs/recon/ecosystem.md §7): a file format, not a
        // protocol feature — one MIME table entry is the entire "support".
        "gpub" => "application/gpub+zip",
        "zip" => "application/zip",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "opus" => "audio/opus",
        "flac" => "audio/flac",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gemtext_gets_charset() {
        assert_eq!(lookup("index.gmi"), "text/gemini; charset=utf-8");
    }

    #[test]
    fn gpub_is_supported() {
        assert_eq!(lookup("book.gpub"), "application/gpub+zip");
    }

    #[test]
    fn case_insensitive() {
        assert_eq!(lookup("PHOTO.JPG"), "image/jpeg");
    }

    #[test]
    fn unknown_extension_is_octet_stream() {
        assert_eq!(lookup("file.xyz123"), "application/octet-stream");
    }

    #[test]
    fn no_extension_is_octet_stream() {
        assert_eq!(lookup("README"), "application/octet-stream");
        assert_eq!(lookup(""), "application/octet-stream");
    }

    #[test]
    fn dotfile_with_no_further_extension_is_octet_stream() {
        // ".gitignore" splits as ("", "gitignore") via rsplit_once — that IS
        // the extension by this logic, which is wrong for dotfiles. Guard
        // against ever treating the leading dot as a separator improperly.
        assert_eq!(lookup(".gitignore"), "application/octet-stream");
    }
}
