use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;

use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeTuple;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::cell::{Cell, DynCell};

// === SerdeBoc ===

/// A wrapper type which implements `Serialize` and `Deserialize` for
/// types involving `Cell`.
#[repr(transparent)]
pub struct SerdeBoc<T: ?Sized>(T);

impl<T: ?Sized> SerdeBoc<T> {
    /// Creates a wrapper around a reference to the value.
    #[inline(always)]
    pub const fn wrap(value: &T) -> &Self {
        // SAFETY: SerdeBoc is #[repr(transparent)]
        unsafe { &*(value as *const T as *const Self) }
    }
}

impl<T> SerdeBoc<T> {
    #[inline(always)]
    const fn wrap_slice(value: &[T]) -> &[Self] {
        // SAFETY: SerdeBoc is #[repr(transparent)]
        unsafe { std::slice::from_raw_parts(value.as_ptr() as *const Self, value.len()) }
    }

    /// Consumes self, returning the inner value.
    #[inline(always)]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for SerdeBoc<T> {
    #[inline]
    fn from(val: T) -> SerdeBoc<T> {
        Self(val)
    }
}

impl<T: SerializeAsBoc + ?Sized> Serialize for SerdeBoc<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize_as_boc(serializer)
    }
}

impl<'de, T: DeserializeAsBoc<'de>> Deserialize<'de> for SerdeBoc<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        T::deserialize_as_boc(deserializer).map(Self)
    }
}

// === Serializer stuff ===

trait SerializeAsBoc {
    fn serialize_as_boc<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error>;
}

impl SerializeAsBoc for Cell {
    #[inline]
    fn serialize_as_boc<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_ref().serialize(serializer)
    }
}

impl SerializeAsBoc for DynCell {
    #[inline]
    fn serialize_as_boc<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.serialize(serializer)
    }
}

impl<T: SerializeAsBoc + ?Sized> SerializeAsBoc for &'_ T {
    #[inline]
    fn serialize_as_boc<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        T::serialize_as_boc(self, serializer)
    }
}

impl<T: SerializeAsBoc> SerializeAsBoc for Option<T> {
    #[inline]
    fn serialize_as_boc<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_ref().map(SerdeBoc::<T>::wrap).serialize(serializer)
    }
}

impl<T: SerializeAsBoc + ?Sized> SerializeAsBoc for Box<T> {
    #[inline]
    fn serialize_as_boc<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        T::serialize_as_boc(self.as_ref(), serializer)
    }
}

impl<T: SerializeAsBoc + ?Sized> SerializeAsBoc for Rc<T> {
    #[inline]
    fn serialize_as_boc<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        T::serialize_as_boc(self.as_ref(), serializer)
    }
}

impl<T: SerializeAsBoc + ?Sized> SerializeAsBoc for Arc<T> {
    #[inline]
    fn serialize_as_boc<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        T::serialize_as_boc(self.as_ref(), serializer)
    }
}

impl<T: SerializeAsBoc> SerializeAsBoc for [T] {
    #[inline]
    fn serialize_as_boc<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        SerdeBoc::wrap_slice(self).serialize(serializer)
    }
}

impl<T: SerializeAsBoc> SerializeAsBoc for Vec<T> {
    #[inline]
    fn serialize_as_boc<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_slice().serialize_as_boc(serializer)
    }
}

impl<T> SerializeAsBoc for [T; 0] {
    #[inline]
    fn serialize_as_boc<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        ok!(serializer.serialize_tuple(0)).end()
    }
}

macro_rules! serialize_as_boc_array_impls {
    ($($len:tt)+) => {
        $(
            #[allow(clippy::zero_prefixed_literal)]
            impl<T: SerializeAsBoc> SerializeAsBoc for [T; $len] {
                #[inline]
                fn serialize_as_boc<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: Serializer,
                {
                    let mut seq = ok!(serializer.serialize_tuple($len));
                    for e in self {
                        ok!(seq.serialize_element(SerdeBoc::<T>::wrap(e)));
                    }
                    seq.end()
                }
            }
        )+
    }
}

serialize_as_boc_array_impls! {
    01 02 03 04 05 06 07 08 09 10
    11 12 13 14 15 16 17 18 19 20
    21 22 23 24 25 26 27 28 29 30
    31 32
}

macro_rules! serialize_as_boc_map_impl {
    ($ty:ident<K$(: $kbound1:ident $(+ $kbound2:ident)*)*, V$(, $typaram:ident: $bound:ident)*>) => {
        impl<K, V$(, $typaram)*> SerializeAsBoc for $ty<K, V$(, $typaram)*>
        where
            K: Serialize $(+ $kbound1 $(+ $kbound2)*)*,
            V: SerializeAsBoc,
            $($typaram: $bound,)*
        {
            #[inline]
            fn serialize_as_boc<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.collect_map(self.iter().map(|(k, v)| (k, SerdeBoc::wrap(v))))
            }
        }
    }
}

serialize_as_boc_map_impl! { BTreeMap<K: Ord, V> }
serialize_as_boc_map_impl! { HashMap<K: Eq + Hash, V, H: BuildHasher> }

// === Deserializer stuff ===

trait DeserializeAsBoc<'de>: Sized {
    fn deserialize_as_boc<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error>;
}

impl<'de> DeserializeAsBoc<'de> for Cell {
    fn deserialize_as_boc<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error;

        let is_human_readable = deserializer.is_human_readable();
        let mut boc = ok!(borrow_cow_bytes(deserializer));

        if is_human_readable {
            match crate::util::decode_base64(boc) {
                Ok(bytes) => {
                    boc = Cow::Owned(bytes);
                }
                Err(_) => return Err(Error::custom("invalid base64 string")),
            }
        }

        match crate::boc::Boc::decode(boc) {
            Ok(cell) => Ok(cell),
            Err(e) => Err(Error::custom(e)),
        }
    }
}

impl<'de, T: DeserializeAsBoc<'de>> DeserializeAsBoc<'de> for Box<T> {
    #[inline]
    fn deserialize_as_boc<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = ok!(Box::<SerdeBoc<T>>::deserialize(deserializer));
        // SAFETY: `SerdeBoc<T>` has the same layout as `T`.
        Ok(unsafe { Box::from_raw(Box::into_raw(value).cast::<T>()) })
    }
}

impl<'de, T: DeserializeAsBoc<'de>> DeserializeAsBoc<'de> for Option<T> {
    #[inline]
    fn deserialize_as_boc<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(ok!(Option::<SerdeBoc<T>>::deserialize(deserializer)).map(SerdeBoc::into_inner))
    }
}

impl<'de, T: DeserializeAsBoc<'de>> DeserializeAsBoc<'de> for Vec<T> {
    #[inline]
    fn deserialize_as_boc<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(ok!(Vec::<SerdeBoc<T>>::deserialize(deserializer))
            .into_iter()
            .map(SerdeBoc::into_inner)
            .collect())
    }
}

impl<'de, T: DeserializeAsBoc<'de>, const N: usize> DeserializeAsBoc<'de> for [T; N]
where
    [SerdeBoc<T>; N]: Deserialize<'de>,
{
    fn deserialize_as_boc<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = ok!(<[SerdeBoc<T>; N]>::deserialize(deserializer));
        Ok(value.map(SerdeBoc::into_inner))
    }
}

macro_rules! deserialize_as_boc_map_impl {
    (
        $ty:ident <K $(: $kbound1:ident $(+ $kbound2:ident)*)*, V $(, $typaram:ident : $bound1:ident $(+ $bound2:ident)*)*>,
        $access:ident,
        $with_capacity:expr,
    ) => {
        impl<'de, K, V $(, $typaram)*> DeserializeAsBoc<'de> for $ty<K, V $(, $typaram)*>
        where
            K: Deserialize<'de> $(+ $kbound1 $(+ $kbound2)*)*,
            V: DeserializeAsBoc<'de>,
            $($typaram: $bound1 $(+ $bound2)*),*
        {
            fn deserialize_as_boc<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct MapVisitor<K, V $(, $typaram)*> {
                    marker: PhantomData<$ty<K, V $(, $typaram)*>>,
                }

                impl<'de, K, V $(, $typaram)*> Visitor<'de> for MapVisitor<K, V $(, $typaram)*>
                where
                    K: Deserialize<'de> $(+ $kbound1 $(+ $kbound2)*)*,
                    V: DeserializeAsBoc<'de>,
                    $($typaram: $bound1 $(+ $bound2)*),*
                {
                    type Value = $ty<K, V $(, $typaram)*>;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("a map")
                    }

                    #[inline]
                    fn visit_map<A>(self, mut $access: A) -> Result<Self::Value, A::Error>
                    where
                        A: MapAccess<'de>,
                    {
                        let mut values = $with_capacity;

                        while let Some((key, SerdeBoc(value))) = ok!($access.next_entry()) {
                            values.insert(key, value);
                        }

                        Ok(values)
                    }
                }

                let visitor = MapVisitor { marker: PhantomData };
                deserializer.deserialize_map(visitor)
            }
        }
    }
}

deserialize_as_boc_map_impl! {
    BTreeMap<K: Ord, V>,
    map,
    BTreeMap::new(),
}

deserialize_as_boc_map_impl! {
    HashMap<K: Eq + Hash, V, S: BuildHasher + Default>,
    map,
    HashMap::with_capacity_and_hasher(cautious_size_hint::<(K, V)>(map.size_hint()), S::default()),
}

fn cautious_size_hint<Element>(hint: Option<usize>) -> usize {
    const MAX_PREALLOC_BYTES: usize = 1024 * 1024;

    if std::mem::size_of::<Element>() == 0 {
        0
    } else {
        std::cmp::min(
            hint.unwrap_or(0),
            MAX_PREALLOC_BYTES / std::mem::size_of::<Element>(),
        )
    }
}

// === Other stuff ===

fn borrow_cow_bytes<'de: 'a, 'a, D>(deserializer: D) -> Result<std::borrow::Cow<'a, [u8]>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    struct CowBytesVisitor;

    impl<'a> Visitor<'a> for CowBytesVisitor {
        type Value = Cow<'a, [u8]>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a byte array")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Owned(v.as_bytes().to_vec()))
        }

        fn visit_borrowed_str<E>(self, v: &'a str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Borrowed(v.as_bytes()))
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Owned(v.into_bytes()))
        }

        fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Owned(v.to_vec()))
        }

        fn visit_borrowed_bytes<E>(self, v: &'a [u8]) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Borrowed(v))
        }

        fn visit_byte_buf<E>(self, v: Vec<u8>) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(Cow::Owned(v))
        }
    }

    deserializer.deserialize_bytes(CowBytesVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::CellFamily;

    #[derive(serde::Serialize)]
    struct StructWithDynCell<'a> {
        #[serde(with = "crate::boc::Boc")]
        cell: &'a DynCell,
    }

    #[test]
    fn serde_dyn_cell_works() {
        serde_json::to_string(&StructWithDynCell {
            cell: Cell::empty_cell_ref(),
        })
        .unwrap();
    }
}
