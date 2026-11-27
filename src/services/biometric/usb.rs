use std::{ffi::c_void, mem, ptr, slice};

use async_channel::Sender;
use windows_sys::Win32::Devices::{
    DeviceAndDriverInstallation::{
        CM_NOTIFY_ACTION, CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL,
        CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL, CM_NOTIFY_EVENT_DATA, CM_NOTIFY_EVENT_DATA_0_0,
        CM_NOTIFY_FILTER, CM_NOTIFY_FILTER_0, CM_NOTIFY_FILTER_0_0,
        CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE, CM_Register_Notification,
        CM_Unregister_Notification, CR_SUCCESS, HCMNOTIFICATION,
    },
    Usb::GUID_DEVINTERFACE_USB_DEVICE,
};

#[derive(Clone, Copy)]
pub enum UsbReaderEvent {
    Attached,
    Detached,
}

pub struct UsbReaderNotifications {
    handle: HCMNOTIFICATION,
    sender: *mut Sender<UsbReaderEvent>,
}

impl UsbReaderNotifications {
    pub fn register(sender: Sender<UsbReaderEvent>) -> Result<Self, u32> {
        let mut handle = ptr::null_mut();
        let sender = Box::into_raw(Box::new(sender));
        let filter = CM_NOTIFY_FILTER {
            cbSize: mem::size_of::<CM_NOTIFY_FILTER>() as u32,
            FilterType: CM_NOTIFY_FILTER_TYPE_DEVICEINTERFACE,
            u: CM_NOTIFY_FILTER_0 {
                DeviceInterface: CM_NOTIFY_FILTER_0_0 {
                    ClassGuid: GUID_DEVINTERFACE_USB_DEVICE,
                },
            },
            ..Default::default()
        };

        // The boxed callback context stays at the same address until unregistration.
        let result = unsafe {
            CM_Register_Notification(
                &filter,
                sender.cast(),
                Some(on_usb_notification),
                &mut handle,
            )
        };
        if result != CR_SUCCESS {
            // Registration failed, so Windows has no active callback for this context.
            unsafe { drop(Box::from_raw(sender)) };
            return Err(result);
        }
        Ok(Self { handle, sender })
    }
}

impl Drop for UsbReaderNotifications {
    fn drop(&mut self) {
        // Windows waits for in-flight callbacks. If unregistration fails, their context must live on.
        unsafe {
            if CM_Unregister_Notification(self.handle) == CR_SUCCESS {
                drop(Box::from_raw(self.sender));
            }
        }
    }
}

unsafe extern "system" fn on_usb_notification(
    _: HCMNOTIFICATION,
    context: *const c_void,
    action: CM_NOTIFY_ACTION,
    data: *const CM_NOTIFY_EVENT_DATA,
    size: u32,
) -> u32 {
    const PATH_OFFSET: usize = mem::offset_of!(CM_NOTIFY_EVENT_DATA, u)
        + mem::offset_of!(CM_NOTIFY_EVENT_DATA_0_0, SymbolicLink);

    let event = match action {
        CM_NOTIFY_ACTION_DEVICEINTERFACEARRIVAL => UsbReaderEvent::Attached,
        CM_NOTIFY_ACTION_DEVICEINTERFACEREMOVAL => UsbReaderEvent::Detached,
        _ => return 0,
    };
    let size = size as usize;
    if context.is_null() || data.is_null() || size < PATH_OFFSET + mem::size_of::<u16>() {
        return 0;
    }

    // Windows supplies an aligned buffer of `size` bytes matching the device-interface filter.
    let path = unsafe {
        slice::from_raw_parts(
            data.cast::<u8>().add(PATH_OFFSET).cast::<u16>(),
            (size - PATH_OFFSET) / mem::size_of::<u16>(),
        )
    };
    if is_secugen_usb(path) {
        // The context survives every callback. Queue the event without calling the SDK here.
        let sender = unsafe { &*context.cast::<Sender<UsbReaderEvent>>() };
        let _ = sender.try_send(event);
    }
    0
}

fn is_secugen_usb(path: &[u16]) -> bool {
    const PREFIX: &[u8] = br"\\?\USB#VID_1162&PID_";
    path.len() >= PREFIX.len()
        && path.iter().zip(PREFIX).all(|(&unit, expected)| {
            u8::try_from(unit).is_ok_and(|actual| actual.eq_ignore_ascii_case(expected))
        })
}

#[cfg(test)]
mod tests {
    use super::{UsbReaderNotifications, is_secugen_usb};

    #[test]
    fn accepts_secugen_usb_paths_without_matching_other_vendors() {
        for (path, expected) in [
            (r"\\?\USB#VID_1162&PID_2200#1", true),
            (r"\\?\usb#vid_1162&pid_2203#2", true),
            (r"\\?\USB#VID_046D&PID_1162#3", false),
            (r"\\?\HID#VID_1162&PID_2200#1", false),
            (r"\\?\USB#VID_116", false),
        ] {
            assert_eq!(
                is_secugen_usb(&path.encode_utf16().collect::<Vec<_>>()),
                expected
            );
        }
    }

    #[test]
    fn registers_and_releases_windows_notifications_without_a_reader() {
        let (sender, receiver) = async_channel::unbounded();
        let notifications = UsbReaderNotifications::register(sender).unwrap();
        drop(notifications);
        assert!(receiver.is_closed());
    }
}
