//! USB Driver Subsystem
//!
//! Implements USB Host Controller (xHCI) and HID Class Driver.

pub mod xhci;
pub mod hid;

pub use hid::UsbHid;
pub use xhci::XhciController;

/// Initialize the USB subsystem
pub fn init() {
    xhci::init();
}
