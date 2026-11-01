//! macOS Bluetooth Classic discovery and RFCOMM link via `IOBluetooth.framework`.

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char, c_int, c_uchar, c_ushort, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Once};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{Receiver, Sender, bounded, unbounded};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairedDevice {
    /// Bluetooth address string (e.g. `00:1b:66:xx:xx:xx`), usable with `IOBluetoothDevice::deviceWithAddressString:`.
    pub id: String,
    pub name: String,
    pub address: String,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoveryEvent {
    Added(PairedDevice),
    Updated {
        id: String,
        name: Option<String>,
        connected: Option<bool>,
    },
    Removed {
        id: String,
    },
    EnumerationCompleted,
}

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("IOBluetooth error: 0x{0:08x}")]
    IoBluetooth(i32),
    #[error("the headset does not expose the control service (is it connected?)")]
    ServiceNotFound,
    #[error("connection timed out")]
    Timeout,
    #[error("link closed")]
    Closed,
}

impl LinkError {
    pub fn message(&self) -> String {
        self.to_string()
    }
}

#[derive(Debug)]
pub enum LinkEvent {
    Data(Vec<u8>),
    Closed(Option<String>),
}

// ----------------------------------------------------------------------------------------------- Objective-C & CoreFoundation FFI

type Id = *mut c_void;
type Class = *mut c_void;
type Sel = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFStringRef = *const c_void;

const NIL: Id = std::ptr::null_mut();
const OBJC_ASSOCIATION_ASSIGN: usize = 0;

#[link(name = "objc", kind = "dylib")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Class;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
    fn objc_allocateClassPair(superclass: Class, name: *const c_char, extra_bytes: usize) -> Class;
    fn objc_registerClassPair(cls: Class);
    fn class_addMethod(cls: Class, name: Sel, imp: *const c_void, types: *const c_char) -> bool;
    fn objc_setAssociatedObject(object: Id, key: *const c_void, value: Id, policy: usize);
    fn objc_getAssociatedObject(object: Id, key: *const c_void) -> Id;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFRunLoopDefaultMode: CFStringRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopRunInMode(
        mode: CFStringRef,
        seconds: f64,
        return_after_source_handled: bool,
    ) -> i32;
    fn CFRunLoopStop(rl: CFRunLoopRef);
}

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {}

#[link(name = "IOBluetooth", kind = "framework")]
unsafe extern "C" {}

fn cls(name: &str) -> Class {
    let c = CString::new(name).unwrap();
    unsafe { objc_getClass(c.as_ptr()) }
}

fn sel(name: &str) -> Sel {
    let c = CString::new(name).unwrap();
    unsafe { sel_registerName(c.as_ptr()) }
}

unsafe fn msg0_id(recv: Id, s: Sel) -> Id {
    let f: unsafe extern "C" fn(Id, Sel) -> Id = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s) }
}

unsafe fn msg0_void(recv: Id, s: Sel) {
    let f: unsafe extern "C" fn(Id, Sel) = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s) }
}

unsafe fn msg0_usize(recv: Id, s: Sel) -> usize {
    let f: unsafe extern "C" fn(Id, Sel) -> usize = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s) }
}

unsafe fn msg0_bool(recv: Id, s: Sel) -> bool {
    let f: unsafe extern "C" fn(Id, Sel) -> c_uchar = unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s) != 0 }
}

unsafe fn msg0_cstr(recv: Id, s: Sel) -> *const c_char {
    let f: unsafe extern "C" fn(Id, Sel) -> *const c_char =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s) }
}

unsafe fn msg1_usize_id(recv: Id, s: Sel, a: usize) -> Id {
    let f: unsafe extern "C" fn(Id, Sel, usize) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s, a) }
}

unsafe fn msg1_ptr_id(recv: Id, s: Sel, a: *const c_char) -> Id {
    let f: unsafe extern "C" fn(Id, Sel, *const c_char) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s, a) }
}

unsafe fn msg1_id_id(recv: Id, s: Sel, a: Id) -> Id {
    let f: unsafe extern "C" fn(Id, Sel, Id) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s, a) }
}

unsafe fn msg1_id_i32(recv: Id, s: Sel, a: Id) -> c_int {
    let f: unsafe extern "C" fn(Id, Sel, Id) -> c_int =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s, a) }
}

unsafe fn msg1_mut_u8_i32(recv: Id, s: Sel, a: *mut u8) -> c_int {
    let f: unsafe extern "C" fn(Id, Sel, *mut u8) -> c_int =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s, a) }
}

unsafe fn msg2_bytes_len_id(recv: Id, s: Sel, bytes: *const u8, len: usize) -> Id {
    let f: unsafe extern "C" fn(Id, Sel, *const u8, usize) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s, bytes, len) }
}

unsafe fn msg2_write_sync(recv: Id, s: Sel, data: *const c_void, len: c_ushort) -> c_int {
    let f: unsafe extern "C" fn(Id, Sel, *const c_void, c_ushort) -> c_int =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s, data, len) }
}

unsafe fn msg3_open_rfcomm(recv: Id, s: Sel, out_ch: *mut Id, ch_id: u8, delegate: Id) -> c_int {
    let f: unsafe extern "C" fn(Id, Sel, *mut Id, u8, Id) -> c_int =
        unsafe { std::mem::transmute(objc_msgSend as *const c_void) };
    unsafe { f(recv, s, out_ch, ch_id, delegate) }
}

unsafe fn nsstring_to_rust(ns: Id) -> String {
    if ns.is_null() {
        return String::new();
    }
    let ptr = unsafe { msg0_cstr(ns, sel("UTF8String")) };
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr).to_string_lossy().into_owned() }
}

unsafe fn rust_to_nsstring(s: &str) -> Id {
    let c = CString::new(s).unwrap_or_default();
    unsafe { msg1_ptr_id(cls("NSString"), sel("stringWithUTF8String:"), c.as_ptr()) }
}

fn with_autorelease_pool<R>(f: impl FnOnce() -> R) -> R {
    unsafe {
        let pool = msg0_id(msg0_id(cls("NSAutoreleasePool"), sel("alloc")), sel("init"));
        let out = f();
        msg0_void(pool, sel("drain"));
        out
    }
}

// ----------------------------------------------------------------------------------------------- Discovery

/// One-shot snapshot of paired Bluetooth Classic devices on macOS.
pub fn paired_devices() -> Result<Vec<PairedDevice>, LinkError> {
    Ok(with_autorelease_pool(|| unsafe {
        let dev_cls = cls("IOBluetoothDevice");
        if dev_cls.is_null() {
            return Vec::new();
        }
        let arr = msg0_id(dev_cls, sel("pairedDevices"));
        if arr.is_null() {
            return Vec::new();
        }
        let count = msg0_usize(arr, sel("count"));
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let dev = msg1_usize_id(arr, sel("objectAtIndex:"), i);
            if dev.is_null() {
                continue;
            }
            let raw_addr = nsstring_to_rust(msg0_id(dev, sel("addressString")));
            let address = raw_addr.replace('-', ":").to_lowercase();
            let name_ns = msg0_id(dev, sel("name"));
            let name = if name_ns.is_null() {
                address.clone()
            } else {
                nsstring_to_rust(name_ns)
            };
            // Like the Windows build, only list headsets exposing the GAIA control service.
            // The SDP cache may be empty for a not-yet-queried device, so fall back to the model name.
            let is_gaia = {
                let bytes = protocol::GAIA3_SPP_UUID.to_be_bytes();
                let uuid = msg2_bytes_len_id(
                    cls("IOBluetoothSDPUUID"),
                    sel("uuidWithBytes:length:"),
                    bytes.as_ptr(),
                    bytes.len(),
                );
                (!uuid.is_null() && !msg1_id_id(dev, sel("getServiceRecordForUUID:"), uuid).is_null())
                    || name.to_lowercase().contains("momentum")
            };
            if !is_gaia {
                continue;
            }
            let connected = msg0_bool(dev, sel("isConnected"));
            out.push(PairedDevice {
                id: address.clone(),
                name,
                address,
                connected,
            });
        }
        out
    }))
}

/// Live watcher over paired devices on macOS. Polls `IOBluetoothDevice::pairedDevices` and emits diffs.
pub struct Watcher {
    stop: Arc<AtomicBool>,
}

impl Watcher {
    pub fn start() -> Result<(Self, Receiver<DiscoveryEvent>), LinkError> {
        let (tx, rx) = unbounded();
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = stop.clone();
        thread::Builder::new()
            .name("bt-watcher-macos".into())
            .spawn(move || watcher_loop(tx, stop_flag))
            .ok();
        Ok((Self { stop }, rx))
    }
}

fn watcher_loop(tx: Sender<DiscoveryEvent>, stop: Arc<AtomicBool>) {
    let mut known: HashMap<String, PairedDevice> = HashMap::new();
    if let Ok(initial) = paired_devices() {
        for d in initial {
            known.insert(d.id.clone(), d.clone());
            let _ = tx.send(DiscoveryEvent::Added(d));
        }
    }
    let _ = tx.send(DiscoveryEvent::EnumerationCompleted);

    while !stop.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(1200));
        if stop.load(Ordering::SeqCst) {
            break;
        }
        let Ok(current) = paired_devices() else {
            continue;
        };
        let mut seen = HashMap::with_capacity(current.len());
        for d in current {
            seen.insert(d.id.clone(), d.clone());
            match known.get(&d.id) {
                None => {
                    if tx.send(DiscoveryEvent::Added(d)).is_err() {
                        return;
                    }
                }
                Some(prev) if prev != &d => {
                    let ev = DiscoveryEvent::Updated {
                        id: d.id.clone(),
                        name: (prev.name != d.name).then(|| d.name.clone()),
                        connected: (prev.connected != d.connected).then_some(d.connected),
                    };
                    if tx.send(ev).is_err() {
                        return;
                    }
                }
                Some(_) => {}
            }
        }
        for old_id in known.keys() {
            if !seen.contains_key(old_id)
                && tx.send(DiscoveryEvent::Removed { id: old_id.clone() }).is_err()
            {
                return;
            }
        }
        known = seen;
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

// ----------------------------------------------------------------------------------------------- RFCOMM Link

static DELEGATE_KEY: u8 = 0;
static REGISTER_DELEGATE: Once = Once::new();

struct DelegateCtx {
    tx: Sender<LinkEvent>,
}

unsafe extern "C" fn on_rfcomm_data(
    this: Id,
    _cmd: Sel,
    _channel: Id,
    data_ptr: *const u8,
    data_len: usize,
) {
    unsafe {
        let raw = objc_getAssociatedObject(this, &DELEGATE_KEY as *const u8 as *const c_void);
        if raw.is_null() || data_ptr.is_null() || data_len == 0 {
            return;
        }
        let ctx = &*(raw as *const DelegateCtx);
        let bytes = std::slice::from_raw_parts(data_ptr, data_len).to_vec();
        let _ = ctx.tx.send(LinkEvent::Data(bytes));
    }
}

unsafe extern "C" fn on_rfcomm_closed(this: Id, _cmd: Sel, _channel: Id) {
    unsafe {
        let raw = objc_getAssociatedObject(this, &DELEGATE_KEY as *const u8 as *const c_void);
        if raw.is_null() {
            return;
        }
        let ctx = &*(raw as *const DelegateCtx);
        let _ = ctx.tx.send(LinkEvent::Closed(None));
    }
}

fn delegate_class() -> Class {
    REGISTER_DELEGATE.call_once(|| unsafe {
        let superclass = cls("NSObject");
        let name = CString::new("SennheiserControlRfcommDelegate").unwrap();
        let decl = objc_allocateClassPair(superclass, name.as_ptr(), 0);
        if !decl.is_null() {
            let data_types = CString::new("v@:@^vQ").unwrap();
            class_addMethod(
                decl,
                sel("rfcommChannelData:data:length:"),
                on_rfcomm_data as *const c_void,
                data_types.as_ptr(),
            );
            let close_types = CString::new("v@:@").unwrap();
            class_addMethod(
                decl,
                sel("rfcommChannelClosed:"),
                on_rfcomm_closed as *const c_void,
                close_types.as_ptr(),
            );
            objc_registerClassPair(decl);
        }
    });
    cls("SennheiserControlRfcommDelegate")
}

enum Outgoing {
    Send(Vec<u8>, Sender<Result<(), LinkError>>),
    Close,
}

pub struct RfcommLink {
    out_tx: Sender<Outgoing>,
    runloop: usize,
}

impl RfcommLink {
    /// Resolve `service` via SDP on the paired device and open the RFCOMM channel on a dedicated CFRunLoop thread.
    pub fn connect(
        device_id: &str,
        service: u128,
        timeout: Duration,
    ) -> Result<(Arc<Self>, Receiver<LinkEvent>), LinkError> {
        let (ev_tx, ev_rx) = unbounded();
        let (out_tx, out_rx) = unbounded::<Outgoing>();
        let (ready_tx, ready_rx) = bounded::<Result<usize, LinkError>>(1);
        let addr = device_id.to_string();

        thread::Builder::new()
            .name("rfcomm-macos".into())
            .spawn(move || {
                rfcomm_thread(&addr, service, timeout, ev_tx, out_rx, ready_tx);
            })
            .map_err(|_| LinkError::Closed)?;

        match ready_rx.recv_timeout(timeout + Duration::from_secs(2)) {
            Ok(Ok(runloop)) => Ok((Arc::new(Self { out_tx, runloop }), ev_rx)),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(LinkError::Timeout),
        }
    }

    pub fn send(&self, bytes: &[u8]) -> Result<(), LinkError> {
        let (reply_tx, reply_rx) = bounded(1);
        self.out_tx
            .send(Outgoing::Send(bytes.to_vec(), reply_tx))
            .map_err(|_| LinkError::Closed)?;
        if self.runloop != 0 {
            unsafe { CFRunLoopStop(self.runloop as CFRunLoopRef) };
        }
        reply_rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| LinkError::Timeout)?
    }

    pub fn close(&self) {
        let _ = self.out_tx.send(Outgoing::Close);
        if self.runloop != 0 {
            unsafe { CFRunLoopStop(self.runloop as CFRunLoopRef) };
        }
    }
}

impl Drop for RfcommLink {
    fn drop(&mut self) {
        self.close();
    }
}

fn rfcomm_thread(
    device_id: &str,
    service: u128,
    timeout: Duration,
    ev_tx: Sender<LinkEvent>,
    out_rx: Receiver<Outgoing>,
    ready_tx: Sender<Result<usize, LinkError>>,
) {
    unsafe {
        let started = Instant::now();
        let (channel, delegate, ctx_box) = match with_autorelease_pool(|| -> Result<(Id, Id, Box<DelegateCtx>), LinkError> {
            let ns_addr = rust_to_nsstring(device_id);
            let device = msg1_id_id(cls("IOBluetoothDevice"), sel("deviceWithAddressString:"), ns_addr);
            if device.is_null() {
                return Err(LinkError::ServiceNotFound);
            }
            let uuid_bytes = service.to_be_bytes();
            let sdp_uuid = msg2_bytes_len_id(
                cls("IOBluetoothSDPUUID"),
                sel("uuidWithBytes:length:"),
                uuid_bytes.as_ptr(),
                uuid_bytes.len(),
            );
            if sdp_uuid.is_null() {
                return Err(LinkError::ServiceNotFound);
            }
            let mut record = msg1_id_id(device, sel("getServiceRecordForUUID:"), sdp_uuid);
            if record.is_null() {
                let _ = msg1_id_i32(device, sel("performSDPQuery:"), NIL);
                // Wait briefly for SDP query cache to populate
                while record.is_null() && started.elapsed() < timeout {
                    CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.1, false);
                    record = msg1_id_id(device, sel("getServiceRecordForUUID:"), sdp_uuid);
                }
            }
            if record.is_null() {
                return Err(LinkError::ServiceNotFound);
            }
            let mut channel_id: u8 = 0;
            let r = msg1_mut_u8_i32(record, sel("getRFCOMMChannelID:"), &mut channel_id);
            if r != 0 || channel_id == 0 {
                return Err(LinkError::ServiceNotFound);
            }

            let d_cls = delegate_class();
            let delegate = msg0_id(msg0_id(d_cls, sel("alloc")), sel("init"));
            let ctx_box = Box::new(DelegateCtx { tx: ev_tx.clone() });
            objc_setAssociatedObject(
                delegate,
                &DELEGATE_KEY as *const u8 as *const c_void,
                (&*ctx_box as *const DelegateCtx) as Id,
                OBJC_ASSOCIATION_ASSIGN,
            );

            let mut ch: Id = NIL;
            let status = msg3_open_rfcomm(
                device,
                sel("openRFCOMMChannelSync:withChannelID:delegate:"),
                &mut ch,
                channel_id,
                delegate,
            );
            if status != 0 || ch.is_null() {
                msg0_void(delegate, sel("release"));
                return Err(LinkError::IoBluetooth(status));
            }
            let _ = msg0_id(ch, sel("retain"));
            Ok((ch, delegate, ctx_box))
        }) {
            Ok(ok) => ok,
            Err(e) => {
                let _ = ready_tx.send(Err(e));
                return;
            }
        };

        let rl = CFRunLoopGetCurrent() as usize;
        let _ = ready_tx.send(Ok(rl));

        // Pump CFRunLoop for incoming delegate callbacks and drain outgoing writes
        'run: loop {
            with_autorelease_pool(|| {
                CFRunLoopRunInMode(kCFRunLoopDefaultMode, 0.05, true);
            });
            while let Ok(cmd) = out_rx.try_recv() {
                match cmd {
                    Outgoing::Send(bytes, reply) => {
                        let res = with_autorelease_pool(|| {
                            let r = msg2_write_sync(
                                channel,
                                sel("writeSync:length:"),
                                bytes.as_ptr() as *const c_void,
                                bytes.len().min(u16::MAX as usize) as c_ushort,
                            );
                            if r == 0 {
                                Ok(())
                            } else {
                                Err(LinkError::IoBluetooth(r))
                            }
                        });
                        let _ = reply.send(res);
                    }
                    Outgoing::Close => break 'run,
                }
            }
        }

        with_autorelease_pool(|| {
            let _ = msg1_id_i32(channel, sel("setDelegate:"), NIL);
            msg0_void(channel, sel("closeChannel"));
            msg0_void(channel, sel("release"));
            objc_setAssociatedObject(
                delegate,
                &DELEGATE_KEY as *const u8 as *const c_void,
                NIL,
                OBJC_ASSOCIATION_ASSIGN,
            );
            msg0_void(delegate, sel("release"));
        });
        drop(ctx_box);
    }
}
