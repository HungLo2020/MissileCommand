//! X11 window identity, required for window managers to apply rules correctly.
//!
//! minifb creates a normal opaque visual but doesn't set WM_CLASS or _NET_WM_PID.
//! An empty WM_CLASS matches empty-class KWin rules, including opacity rules.
//! Set metadata only on our own client window; no desktop configuration changes.
#[cfg(target_os = "linux")]
use x11rb::{
    connection::Connection,
    protocol::xproto::{AtomEnum, ConnectionExt, PropMode},
    wrapper::ConnectionExt as _,
};

pub struct Desktop {
    #[cfg(target_os = "linux")]
    connection: x11rb::rust_connection::RustConnection,
    #[cfg(target_os = "linux")]
    window: u32,
    #[cfg(target_os = "linux")]
    opaque_region: u32,
}

impl Desktop {
    #[cfg(target_os = "linux")]
    pub fn attach(window: &minifb::Window) -> Result<Self, Box<dyn std::error::Error>> {
        // Linux builds select minifb's X11 backend explicitly, so this is an XID,
        // never a Wayland pointer. Obtaining an integer handle requires no dereference.
        let window = u32::try_from(window.get_window_handle() as usize)?;
        let (connection, _) = x11rb::connect(None)?;
        let atom = |name: &[u8]| -> Result<u32, Box<dyn std::error::Error>> {
            Ok(connection.intern_atom(false, name)?.reply()?.atom)
        };
        connection
            .change_property8(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_CLASS,
                AtomEnum::STRING,
                b"missile-command\0MissileCommand\0",
            )?
            .check()?;
        connection
            .change_property32(
                PropMode::REPLACE,
                window,
                atom(b"_NET_WM_PID")?,
                AtomEnum::CARDINAL,
                &[std::process::id()],
            )?
            .check()?;
        connection
            .change_property32(
                PropMode::REPLACE,
                window,
                atom(b"_NET_WM_WINDOW_TYPE")?,
                AtomEnum::ATOM,
                &[atom(b"_NET_WM_WINDOW_TYPE_NORMAL")?],
            )?
            .check()?;
        connection
            .change_property32(
                PropMode::REPLACE,
                window,
                atom(b"_NET_WM_WINDOW_OPACITY")?,
                AtomEnum::CARDINAL,
                &[u32::MAX],
            )?
            .check()?;
        let opaque_region = atom(b"_NET_WM_OPAQUE_REGION")?;
        connection.flush()?;
        Ok(Self {
            connection,
            window,
            opaque_region,
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn attach(_window: &minifb::Window) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {})
    }

    pub fn resize(&self, width: usize, height: usize) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(target_os = "linux")]
        {
            self.connection
                .change_property32(
                    PropMode::REPLACE,
                    self.window,
                    self.opaque_region,
                    AtomEnum::CARDINAL,
                    &[0, 0, u32::try_from(width)?, u32::try_from(height)?],
                )?
                .check()?;
            self.connection.flush()?;
        }
        #[cfg(not(target_os = "linux"))]
        let _ = (width, height);
        Ok(())
    }
}
