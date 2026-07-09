use serde::Serialize;
use ton_language_server_core::languages::tlb::LANGUAGE_ID;
use ton_language_server_core::{
    DocumentUri, LanguageService, Location, Position, default_language_service,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct TlbLanguageServer {
    service: LanguageService,
}

#[wasm_bindgen]
impl TlbLanguageServer {
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        install_tree_sitter_allocator();
        console_error_panic_hook::set_once();
        Self {
            service: default_language_service(),
        }
    }

    #[wasm_bindgen(js_name = openDocument)]
    pub fn open_document(
        &mut self,
        uri: String,
        version: i32,
        text: String,
    ) -> Result<(), JsValue> {
        self.service
            .open_document(DocumentUri::from(uri), LANGUAGE_ID, version, text)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = changeDocument)]
    pub fn change_document(
        &mut self,
        uri: String,
        version: i32,
        text: String,
    ) -> Result<(), JsValue> {
        self.service
            .change_document(&DocumentUri::from(uri), version, text)
            .map_err(js_error)
    }

    #[wasm_bindgen(js_name = definition)]
    pub fn definition(
        &mut self,
        uri: String,
        line: u32,
        character: u32,
    ) -> Result<JsValue, JsValue> {
        let locations = self
            .service
            .definition(&DocumentUri::from(uri), Position::new(line, character))
            .map_err(js_error)?;
        serde_wasm_bindgen::to_value(&locations_to_lsp(locations)).map_err(js_error)
    }
}

impl Default for TlbLanguageServer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Serialize)]
struct LspLocation {
    uri: String,
    range: LspRange,
}

#[derive(Serialize)]
struct LspRange {
    start: LspPosition,
    end: LspPosition,
}

#[derive(Serialize)]
struct LspPosition {
    line: u32,
    character: u32,
}

fn locations_to_lsp(locations: Vec<Location>) -> Vec<LspLocation> {
    locations
        .into_iter()
        .map(|location| LspLocation {
            uri: location.uri.as_str().to_owned(),
            range: LspRange {
                start: position_to_lsp(location.range.start),
                end: position_to_lsp(location.range.end),
            },
        })
        .collect()
}

const fn position_to_lsp(position: Position) -> LspPosition {
    LspPosition {
        line: position.line,
        character: position.character,
    }
}

fn js_error(error: impl ToString) -> JsValue {
    JsValue::from_str(&error.to_string())
}

#[cfg(target_arch = "wasm32")]
fn install_tree_sitter_allocator() {
    tree_sitter_allocator::install();
}

#[cfg(not(target_arch = "wasm32"))]
const fn install_tree_sitter_allocator() {}

#[cfg(target_arch = "wasm32")]
#[allow(unsafe_code)]
mod tree_sitter_allocator {
    use std::alloc::{Layout, alloc, alloc_zeroed, dealloc};
    use std::ffi::c_void;
    use std::mem::size_of;
    use std::ptr;
    use std::sync::Once;

    const ALLOCATION_ALIGN: usize = 16;
    const HEADER_SIZE: usize = size_of::<AllocationHeader>();

    static INSTALL: Once = Once::new();

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct AllocationHeader {
        layout_size: usize,
        offset: usize,
    }

    pub(crate) fn install() {
        INSTALL.call_once(|| {
            // SAFETY: Tree-sitter stores these process-global callbacks and calls them with
            // ordinary C allocator semantics. The functions below return pointers allocated by
            // Rust's global allocator and retain enough header metadata to free them with the
            // same allocator.
            unsafe {
                tree_sitter::set_allocator(Some(malloc), Some(calloc), Some(realloc), Some(free));
            }
        });
    }

    unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
        // SAFETY: `allocate` implements C `malloc` semantics for arbitrary sizes.
        unsafe { allocate(size, false) }
    }

    unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
        let Some(total_size) = count.checked_mul(size) else {
            return ptr::null_mut();
        };

        // SAFETY: `allocate` implements C `calloc` semantics when `zeroed` is true.
        unsafe { allocate(total_size, true) }
    }

    unsafe extern "C" fn realloc(ptr: *mut c_void, new_size: usize) -> *mut c_void {
        if ptr.is_null() {
            // SAFETY: null `realloc` is equivalent to `malloc`.
            return unsafe { allocate(new_size, false) };
        }
        if new_size == 0 {
            // SAFETY: zero-sized `realloc` frees the original pointer.
            unsafe { free(ptr) };
            return ptr::null_mut();
        }

        // SAFETY: `ptr` was allocated by this allocator because Tree-sitter receives these
        // callbacks as a matched allocator set.
        let header = unsafe { read_header(ptr) };
        // SAFETY: Allocate a new block before freeing the old one, preserving C `realloc`
        // behavior on allocation failure.
        let new_ptr = unsafe { allocate(new_size, false) };
        if new_ptr.is_null() {
            return ptr::null_mut();
        }

        // SAFETY: Both blocks are valid and non-overlapping allocations from this allocator.
        unsafe {
            ptr::copy_nonoverlapping(
                ptr.cast::<u8>(),
                new_ptr.cast::<u8>(),
                header.requested_size().min(new_size),
            );
            free(ptr);
        }

        new_ptr
    }

    unsafe extern "C" fn free(ptr: *mut c_void) {
        if ptr.is_null() {
            return;
        }

        // SAFETY: `ptr` was returned by `allocate`; the header records the original allocation.
        let header = unsafe { read_header(ptr) };
        let user_ptr = ptr.cast::<u8>();
        // SAFETY: `offset` was computed from `base` to `user_ptr` in `allocate`.
        let base_ptr = unsafe { user_ptr.sub(header.offset) };
        // SAFETY: the layout size was accepted by `allocate` and persisted in the header.
        let layout = unsafe { layout_from_size_unchecked(header.layout_size) };
        // SAFETY: `base_ptr` and `layout` match the allocation created in `allocate`.
        unsafe { dealloc(base_ptr, layout) };
    }

    unsafe fn allocate(size: usize, zeroed: bool) -> *mut c_void {
        if size == 0 {
            return ptr::null_mut();
        }

        let Some(layout_size) = size
            .checked_add(HEADER_SIZE)
            .and_then(|size| size.checked_add(ALLOCATION_ALIGN - 1))
        else {
            return ptr::null_mut();
        };

        let Ok(layout) = Layout::from_size_align(layout_size, ALLOCATION_ALIGN) else {
            return ptr::null_mut();
        };
        let base_ptr = if zeroed {
            // SAFETY: `layout` is valid and non-zero sized.
            unsafe { alloc_zeroed(layout) }
        } else {
            // SAFETY: `layout` is valid and non-zero sized.
            unsafe { alloc(layout) }
        };
        if base_ptr.is_null() {
            return ptr::null_mut();
        }

        let unaligned_user_addr = base_ptr as usize + HEADER_SIZE;
        let user_addr = (unaligned_user_addr + ALLOCATION_ALIGN - 1) & !(ALLOCATION_ALIGN - 1);
        let user_ptr = user_addr as *mut u8;
        let offset = user_addr - base_ptr as usize;
        let header_ptr = user_ptr
            .wrapping_sub(HEADER_SIZE)
            .cast::<AllocationHeader>();

        // SAFETY: `header_ptr` points into the allocation immediately before `user_ptr`.
        unsafe {
            header_ptr.write(AllocationHeader {
                layout_size,
                offset,
            });
        }

        user_ptr.cast()
    }

    unsafe fn read_header(ptr: *mut c_void) -> AllocationHeader {
        let header_ptr = ptr
            .cast::<u8>()
            .wrapping_sub(HEADER_SIZE)
            .cast::<AllocationHeader>();
        // SAFETY: All allocator entry points only receive pointers previously returned by
        // `allocate`, so the header immediately before the user pointer is initialized.
        unsafe { header_ptr.read() }
    }

    unsafe fn layout_from_size_unchecked(size: usize) -> Layout {
        // SAFETY: callers only pass sizes that were returned by `Layout::from_size_align`.
        unsafe { Layout::from_size_align_unchecked(size, ALLOCATION_ALIGN) }
    }

    impl AllocationHeader {
        fn requested_size(self) -> usize {
            self.layout_size - HEADER_SIZE - (ALLOCATION_ALIGN - 1)
        }
    }
}
