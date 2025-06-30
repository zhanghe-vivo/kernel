use bitflags::bitflags;

/// Termios flags, see: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/termios.h.html.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Termios {
    // Input modes, input mode flags for controlling:
    //  - input parity;
    //  - input newline translation;
    //  - modem flow control;
    //  - 8-bit cleanliness;
    //  - response to a (serial port's) "break" condition.
    pub iflag: Iflags,
    // Output mods, output mode flags for controlling:
    // - implementation-defined output postprocessing;
    // - output newline translation;
    // - output delays after various control characters have been sent.
    pub oflag: Oflags,
    // Control mods, terminal hardware control flags for controlling the actual terminal device rather than the line discipline:
    // - the number of bits in a character;
    // - parity type;
    // - hangup control;
    // - serial line flow control.
    pub cflag: Cflags,
    // Local modes, terminal hardware control flags for controlling the actual terminal device rather than the line discipline:
    // - the number of bits in a character;
    // - parity type;
    // - hangup control;
    // - serial line flow control.
    pub lflag: Lflags,
    // Control characters,
    pub cc: [u8; 12],
    // Input baud rates
    pub ispeed: u32,
    /// Onput baud rates
    pub ospeed: u32,
}

// Input modes.
bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug)]
    pub struct Iflags: u32 {
        // Ignore break condition.
        const IGNBRK = 0x001;
        // Signal interrupt on break.
        const BRKINT = 0x002;
        // Ignore characters with parity errors.
        const IGNPAR = 0x004;
        // Mark parity and framing errors.
        const PARMRK = 0x008;
        // Enable input parity check.
        const INPCK = 0x010;
        // Strip character.
        const ISTRIP = 0x020;
        // Map NL to CR on input.
        const INLCR = 0x040;
        // Ignore CR.
        const IGNCR = 0x080;
        // Map CR to NL on input.
        const ICRNL = 0x100;
        // Any character will restart after stop.
        const IXANY = 0x800;
        // Enable start/stop output control.
        const IXON = 0x0200;
        // Enable start/stop input control.
        const IXOFF = 0x1000;
    }
}

impl Iflags {
    pub fn default() -> Self {
        Iflags::ICRNL | Iflags::IXON
    }
}

// Output modes.
bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug)]
    pub struct Oflags: u32 {
        // Post-process output.
        const OPOST = 0x01;
        // Map NL to CR-NL on output.
        const ONLCR = 0x04;
        // Map CR to NL on output.
        const OCRNL = 0x08;
        // No CR output at column 0.
        const ONOCR = 0x10;
        // NL performs CR function.
        const ONLRET = 0x20;
        // Use fill characters for delay.
        const OFILL = 0x40;
        // Fill is DEL.
        const OFDEL = 0x80;
    }
}

impl Oflags {
    pub fn default() -> Self {
        Oflags::OPOST | Oflags::ONLCR
    }
}

// Control modes.
bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug)]
    pub struct Cflags: u32 {
        // Character size, 5 bits.
        const CSIZE_5 = 0x00;
        // Character size, 6 bits.
        const CSIZE_6 = 0x10;
        // Character size, 7 bits.
        const CSIZE_7 = 0x20;
        // Character size, 8 bits.
        const CSIZE_8 = 0x30;
        // Send two stop bits, else one.
        const CSTOPB = 0x40;
        // Enable receiver.
        const CREAD = 0x80;
        // Parity enable.
        const PARENB = 0x100;
        // Odd parity, else even.
        const PARODD = 0x200;
    }
}

impl Cflags {
    pub fn default() -> Self {
        Cflags::CREAD | Cflags::CSIZE_8
    }
}

// Local modes.
bitflags! {
    #[repr(transparent)]
    #[derive(Clone, Copy, Debug)]
    pub struct Lflags: u32 {
        // Enable signals.
        const ISIG = 0x01;
        // Canonical input.
        const ICANON = 0x02;
        // Enable echo.
        const ECHO = 0x08;
        // Echo erase character as error-correcting backspace.
        const ECHOE = 0x10;
        // Echo KILL.
        const ECHOK = 0x20;
        // Echo NL.
        const ECHONL = 0x40;
        // Disable flush after interrupt or quit.
        const NOFLSH = 0x80;
        // Send SIGTTOU for background output.
        const TOSTOP = 0x100;
        // Enable extended input character processing.
        const IEXTEN = 0x800;
    }
}

impl Lflags {
    pub fn default() -> Self {
        Lflags::ISIG
            | Lflags::ICANON
            | Lflags::ECHO
            | Lflags::ECHOE
            | Lflags::ECHOK
            | Lflags::IEXTEN
    }
}

// Define the following symbolic constants for use as subscripts for the array c_cc.
#[repr(u32)]
pub enum CcIndex {
    VINTR = 0,  /* INTR    character */
    VQUIT = 1,  /* QUIT    character */
    VERASE = 2, /* ERASE   character */
    VKILL = 3,  /* KILL    character */
    VEOF = 4,   /* EOF     character */
    VTIME = 5,  /* TIME      value   */
    VMIN = 6,   /* MIN       value   */
    VSWTC = 7,  /* SWTC    character */
    VSTART = 8, /* START   character */
    VSTOP = 9,  /* STOP    character */
    VSUSP = 10, /* SUSP    character */
    VEOL = 11,  /* EOL     character */
}

impl CcIndex {
    pub fn default_value(self) -> u8 {
        match self {
            CcIndex::VINTR => 0x03,  // ^C
            CcIndex::VQUIT => 0x1C,  // ^\
            CcIndex::VERASE => 0x7F, // DEL (Backspace)
            CcIndex::VKILL => 0x15,  // ^U
            CcIndex::VEOF => 0x04,   // ^D
            CcIndex::VTIME => 0,
            CcIndex::VMIN => 1,
            CcIndex::VSWTC => 0,
            CcIndex::VSTART => 0x11, // ^Q (XON)
            CcIndex::VSTOP => 0x13,  // ^S (XOFF)
            CcIndex::VSUSP => 0x1A,  // ^Z
            CcIndex::VEOL => 0,      // 0
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => CcIndex::VINTR,
            1 => CcIndex::VQUIT,
            2 => CcIndex::VERASE,
            3 => CcIndex::VKILL,
            4 => CcIndex::VEOF,
            5 => CcIndex::VTIME,
            6 => CcIndex::VMIN,
            7 => CcIndex::VSWTC,
            8 => CcIndex::VSTART,
            9 => CcIndex::VSTOP,
            10 => CcIndex::VSUSP,
            11 => CcIndex::VEOL,
            _ => unreachable!("Invalid c_cc index"),
        }
    }
}

impl Termios {
    pub fn default() -> Self {
        let mut cc = [0u8; 12];
        for i in 0..cc.len() {
            cc[i] = CcIndex::from_u8(i as u8).default_value();
        }
        Self {
            iflag: Iflags::default(),
            oflag: Oflags::default(),
            cflag: Cflags::default(),
            lflag: Lflags::default(),
            cc,
            ispeed: 115200,
            ospeed: 115200,
        }
    }

    pub fn new(
        iflag: Iflags,
        oflag: Oflags,
        cflag: Cflags,
        lflag: Lflags,
        ispeed: u32,
        ospeed: u32,
    ) -> Self {
        let mut cc = [0u8; 12];
        for i in 0..cc.len() {
            cc[i] = CcIndex::from_u8(i as u8).default_value();
        }
        Self {
            iflag,
            oflag,
            cflag,
            lflag,
            cc,
            ispeed,
            ospeed,
        }
    }

    pub fn getispeed(&self) -> u32 {
        self.ispeed
    }

    pub fn getospeed(&self) -> u32 {
        self.ospeed
    }

    pub fn setispeed(&mut self, baud_rate: u32) {
        self.ispeed = baud_rate;
    }

    pub fn setospeed(&mut self, baud_rate: u32) {
        self.ospeed = baud_rate;
    }

    pub fn getc(&mut self, baud_rate: u32) {
        self.ospeed = baud_rate;
    }
}
