//! Deliver macOS Apple Events for opened files and links to the application.
//!
//! The system launches `iwe://` links and associated documents into this
//! process. Winit registers no application delegate, so this module installs
//! its own before the event loop starts.

use super::send;
use crate::domain::external::{Link, Request};
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{MainThreadMarker, MainThreadOnly, define_class, extern_methods};
use objc2_app_kit::{NSApplication, NSApplicationDelegate};
use objc2_foundation::{NSArray, NSURL};

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "IWEExternalDelegate"]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl NSApplicationDelegate for Delegate {
        /// Handles both file URLs and `iwe://` links. Implementing this
        /// selector replaces the deprecated openFile and openFiles variants.
        #[unsafe(method(application:openURLs:))]
        fn open_urls(&self, _application: &NSApplication, urls: &NSArray<NSURL>) {
            for url in urls.iter() {
                if let Some(request) = request_from_url(&url) {
                    send(request);
                }
            }
        }
    }
);

impl Delegate {
    extern_methods!(
        // SAFETY: The subclass adds no instance variables, so the NSObject
        // initializer fully initializes it.
        #[unsafe(method(new))]
        fn new() -> Retained<Self>;
    );
}

/// Install the application delegate for the lifetime of the process.
pub(crate) fn install() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let delegate = Delegate::new();
    let application = NSApplication::sharedApplication(mtm);
    application.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    // AppKit keeps an unretained reference, so the delegate must outlive it.
    std::mem::forget(delegate);
}

/// Turn an operating-system URL into a request.
///
/// File URLs become file requests. Application links parse as deep links.
fn request_from_url(url: &NSURL) -> Option<Request> {
    let text = url.absoluteString()?.to_string();
    if text.starts_with("file:") {
        let path = url.path()?.to_string();
        return Some(Request::File { path: path.into() });
    }
    match Link::parse(&text)? {
        Link::Open(path) => Some(Request::File { path }),
        Link::Project(directory) => Some(Request::Project { directory }),
    }
}
