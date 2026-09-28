use std::cell::RefCell;

use quote::ToTokens;

pub struct Ctxt {
    errors: RefCell<Vec<syn::Error>>,
}

impl Ctxt {
    pub fn new() -> Self {
        Self {
            errors: RefCell::new(Vec::new()),
        }
    }

    pub fn error_spanned_by<T, M>(&self, object: T, message: M)
    where
        T: ToTokens,
        M: std::fmt::Display,
    {
        self.errors
            .borrow_mut()
            .push(syn::Error::new_spanned(object.into_token_stream(), message));
    }

    pub fn syn_error(&self, error: syn::Error) {
        self.errors.borrow_mut().push(error)
    }

    pub fn check(self) -> Result<(), Vec<syn::Error>> {
        let errors = std::mem::take(&mut *self.errors.borrow_mut());
        match errors.len() {
            0 => Ok(()),
            _ => Err(errors),
        }
    }
}
