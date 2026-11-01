//! Bluetooth Classic discovery and RFCOMM transport (Windows WinRT + macOS IOBluetooth).

#[cfg(windows)]
pub mod discovery;
#[cfg(windows)]
pub mod rfcomm;

#[cfg(windows)]
pub use discovery::{DiscoveryEvent, PairedDevice, Watcher, paired_devices};
#[cfg(windows)]
pub use rfcomm::{LinkError, LinkEvent, RfcommLink};

#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "macos")]
pub use macos::{DiscoveryEvent, LinkError, LinkEvent, PairedDevice, RfcommLink, Watcher, paired_devices};
