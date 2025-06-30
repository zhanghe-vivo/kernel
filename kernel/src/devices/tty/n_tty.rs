use crate::devices::{
    tty::{
        serial,
        termios::{CcIndex, Iflags},
    },
    Device, DeviceClass, DeviceId,
};
use alloc::{collections::VecDeque, format, string::String, sync::Arc};
use core::sync::atomic::{AtomicUsize, Ordering};
use embedded_io::ErrorKind;
use serial::Serial;
use spin::{Mutex, Once};

static TTY: Once<Arc<Tty>> = Once::new();

enum SpecKey {
    Up,
    Down,
}

pub struct Tty {
    serial: Arc<Serial>,
    line_buf: Mutex<[u8; 512]>,
    cursor: AtomicUsize,
    history: Mutex<VecDeque<String>>,
    history_cursor: AtomicUsize,
    spec_key: Mutex<Option<SpecKey>>,
}

impl Tty {
    pub fn init(serial: Arc<Serial>) -> &'static Arc<Tty> {
        TTY.call_once(|| {
            Arc::new(Self {
                serial: serial,
                line_buf: Mutex::new([0u8; 512]),
                cursor: AtomicUsize::new(0),
                history: Mutex::new(VecDeque::with_capacity(5)),
                history_cursor: AtomicUsize::new(0),
                spec_key: Mutex::new(None),
            })
        })
    }

    fn add_history(&self, command: &str) {
        let mut history = self.history.lock();
        if history.front().map(|s| s.as_str()) != Some(command) {
            if history.len() == 5 {
                history.pop_back();
            }
            history.push_front(String::from(command));
        }
        self.history_cursor.store(0, Ordering::Relaxed);
    }

    fn get_history(&self, index: usize) -> Option<String> {
        self.history.lock().get(index).cloned()
    }

    fn clear_line(&self, pos: u64, is_blocking: bool) -> Result<(), ErrorKind> {
        self.serial.write(pos, b"\r", is_blocking)?;
        self.serial.write(pos, b"\x1b[2K", is_blocking)?;
        Ok(())
    }
}

impl Device for Tty {
    fn name(&self) -> String {
        format!("n_tty")
    }

    fn class(&self) -> DeviceClass {
        DeviceClass::Char
    }

    fn id(&self) -> DeviceId {
        DeviceId::new(5, 0)
    }

    fn open(&self) -> Result<(), ErrorKind> {
        self.serial.open()
    }

    fn close(&self) -> Result<(), ErrorKind> {
        self.serial.close()
    }

    fn read(&self, _pos: u64, buf: &mut [u8], is_blocking: bool) -> Result<usize, ErrorKind> {
        let mut line_buf = self.line_buf.lock();
        // handle special characters
        if let Some(key) = &*self.spec_key.lock() {
            let history_cursor = self.history_cursor.load(Ordering::Relaxed);
            match key {
                SpecKey::Up => {
                    if history_cursor < self.history.lock().len() {
                        if let Some(hist_cmd) = self.get_history(history_cursor) {
                            line_buf[..hist_cmd.len()].copy_from_slice(hist_cmd.as_bytes());
                            self.cursor.store(hist_cmd.len(), Ordering::Relaxed);
                            self.serial.write(_pos, hist_cmd.as_bytes(), false)?;
                            self.history_cursor
                                .store(history_cursor + 1, Ordering::Relaxed);
                        }
                    }
                }
                SpecKey::Down => {
                    if history_cursor != 0 {
                        if let Some(hist_cmd) = self.get_history(history_cursor - 1) {
                            line_buf[..hist_cmd.len()].copy_from_slice(hist_cmd.as_bytes());
                            self.cursor.store(hist_cmd.len(), Ordering::Relaxed);
                            self.serial.write(_pos, hist_cmd.as_bytes(), false)?;
                        }
                        self.history_cursor
                            .store(history_cursor - 1, Ordering::Relaxed);
                    } else {
                        line_buf.fill(0);
                        self.cursor.store(0, Ordering::Relaxed);
                        self.history_cursor.store(0, Ordering::Relaxed);
                    }
                }
            }
        }
        *self.spec_key.lock() = None;
        // normal character
        loop {
            let mut temp_buf = [0u8; 512];
            let nbytes = self.serial.read(_pos, &mut temp_buf, is_blocking).unwrap();
            let mut i = 0;
            while i < nbytes {
                let ch = temp_buf[i];
                let cursor = self.cursor.load(Ordering::Relaxed);
                if self.serial.termios.iflag.contains(Iflags::ICRNL) && ch == b'\r' {
                    let _ = self.serial.write(_pos, &[b'\n'], false);
                    line_buf[cursor] = b'\n';
                    buf[..cursor + 1].copy_from_slice(&line_buf[..cursor + 1]);
                    let command = String::from_utf8_lossy(&line_buf[..cursor]).into_owned();
                    if !command.is_empty() {
                        self.add_history(&command);
                    }
                    line_buf.fill(0);
                    self.cursor.store(0, Ordering::Relaxed);
                    return Ok(cursor + 1);
                }
                if self.serial.termios.cc[CcIndex::VERASE as usize] == ch as u8 {
                    if cursor > 0 {
                        let backspace_seq = [8u8, b' ', 8u8];
                        let _ = self.serial.write(_pos, &backspace_seq, false);
                        let _ = self.cursor.fetch_sub(1, Ordering::Relaxed);
                        let _ = line_buf[cursor - 1] = 0;
                    }
                    i += 1;
                    continue;
                }

                if self.serial.termios.cc[CcIndex::VKILL as usize] == ch as u8 {
                    line_buf.fill(0);
                    self.cursor.store(0, Ordering::Relaxed);
                    i += 1;
                    continue;
                }

                // get commandline history
                // up key  : 0x1b 0x5b 0x41
                // down key: 0x1b 0x5b 0x42
                if ch == 0x1b && i <= temp_buf.len() - 3 && temp_buf[i + 1] == 0x5b {
                    match temp_buf[i + 2] {
                        0x41 => {
                            *self.spec_key.lock() = Some(SpecKey::Up);
                            self.clear_line(_pos, false)?;
                            buf[0] = b'\n';
                            return Ok(1);
                        }
                        0x42 => {
                            *self.spec_key.lock() = Some(SpecKey::Down);
                            self.clear_line(_pos, false)?;
                            buf[0] = b'\n';
                            return Ok(1);
                        }
                        _ => {
                            i = i + 3;
                            continue;
                        }
                    }
                }
                i += 1;
                line_buf[cursor] = ch;
                let _ = self.cursor.fetch_add(1, Ordering::Relaxed);
                let _ = self.serial.write(_pos, &[ch], false);
            }
        }
    }

    fn write(&self, _pos: u64, buf: &[u8], is_blocking: bool) -> Result<usize, ErrorKind> {
        self.serial.write(_pos, buf, is_blocking)
    }

    fn ioctl(&self, request: u32, arg: usize) -> Result<(), ErrorKind> {
        self.serial.ioctl(request, arg)
    }
}
