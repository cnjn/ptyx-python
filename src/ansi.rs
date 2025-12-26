//! ANSI escape sequence helpers

/// Create a CSI (Control Sequence Introducer) sequence
pub fn csi(seq: &str) -> String {
    format!("\x1b[{}", seq)
}

/// Create an SGR (Select Graphic Rendition) sequence
pub fn sgr(codes: &[u32]) -> String {
    if codes.is_empty() {
        return csi("m");
    }
    let codes_str: Vec<String> = codes.iter().map(|c| c.to_string()).collect();
    csi(&format!("{}m", codes_str.join(";")))
}

/// Common ANSI codes
pub mod codes {
    /// Reset all attributes
    pub const RESET: u32 = 0;
    /// Bold
    pub const BOLD: u32 = 1;
    /// Dim
    pub const DIM: u32 = 2;
    /// Italic
    pub const ITALIC: u32 = 3;
    /// Underline
    pub const UNDERLINE: u32 = 4;
    /// Blink
    pub const BLINK: u32 = 5;
    /// Reverse
    pub const REVERSE: u32 = 7;
    /// Hidden
    pub const HIDDEN: u32 = 8;
    /// Strikethrough
    pub const STRIKETHROUGH: u32 = 9;

    /// Foreground colors
    pub mod fg {
        pub const BLACK: u32 = 30;
        pub const RED: u32 = 31;
        pub const GREEN: u32 = 32;
        pub const YELLOW: u32 = 33;
        pub const BLUE: u32 = 34;
        pub const MAGENTA: u32 = 35;
        pub const CYAN: u32 = 36;
        pub const WHITE: u32 = 37;
        pub const DEFAULT: u32 = 39;
    }

    /// Background colors
    pub mod bg {
        pub const BLACK: u32 = 40;
        pub const RED: u32 = 41;
        pub const GREEN: u32 = 42;
        pub const YELLOW: u32 = 43;
        pub const BLUE: u32 = 44;
        pub const MAGENTA: u32 = 45;
        pub const CYAN: u32 = 46;
        pub const WHITE: u32 = 47;
        pub const DEFAULT: u32 = 49;
    }
}

/// Clear screen
pub fn clear_screen() -> &'static str {
    "\x1b[2J"
}

/// Move cursor to home position
pub fn cursor_home() -> &'static str {
    "\x1b[H"
}

/// Move cursor to position
pub fn cursor_to(row: u16, col: u16) -> String {
    csi(&format!("{};{}H", row, col))
}

/// Hide cursor
pub fn cursor_hide() -> &'static str {
    "\x1b[?25l"
}

/// Show cursor
pub fn cursor_show() -> &'static str {
    "\x1b[?25h"
}

/// Save cursor position
pub fn cursor_save() -> &'static str {
    "\x1b[s"
}

/// Restore cursor position
pub fn cursor_restore() -> &'static str {
    "\x1b[u"
}

/// Clear line from cursor to end
pub fn clear_line_to_end() -> &'static str {
    "\x1b[K"
}

/// Clear entire line
pub fn clear_line() -> &'static str {
    "\x1b[2K"
}
