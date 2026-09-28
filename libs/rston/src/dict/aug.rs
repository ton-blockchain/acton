use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::marker::PhantomData;

use super::raw::*;
use super::typed::*;
use super::{
    DictKey, LoadDictKey, SearchByExtra, SetMode, StoreDictKey, aug_dict_find_by_extra,
    aug_dict_insert, aug_dict_merge_siblings, aug_dict_modify_from_sorted_iter,
    aug_dict_remove_owned, build_aug_dict_from_sorted_iter, read_label,
};
use crate::cell::*;
use crate::error::*;
use crate::util::*;

/// A trait for values that can be used as augmented values in an augmented dictionary.
pub trait AugDictExtra: Default {
    /// Merges two augmented values.
    ///
    /// # Parameters
    /// - `left` - The left branch (should start with `extra`).
    /// - `right` - The right branch (should start with `extra`).
    /// - `b` - The builder to store the result (only `extra`).
    /// - `cx` - The cell context.
    fn comp_add(
        left: &mut CellSlice,
        right: &mut CellSlice,
        b: &mut CellBuilder,
        cx: &dyn CellContext,
    ) -> Result<(), Error>;
}

/// Typed augmented dictionary with fixed length keys.
///
/// # TLB scheme
///
/// ```text
/// ahm_edge#_ {n:#} {V:Type} {A:Type} {l:#} {m:#}
///   label:(HmLabel ~l n) {n = (~m) + l}
///   node:(HashmapAugNode m V A) = HashmapAug n V A;
///
/// ahmn_leaf#_ {V:Type} {A:Type} extra:A value:V = HashmapAugNode 0 V A;
/// ahmn_fork#_ {n:#} {V:Type} {A:Type} left:^(HashmapAug n V A)
///   right:^(HashmapAug n V A) extra:A = HashmapAugNode (n + 1) V A;
///
/// ahme_empty$0 {n:#} {V:Type} {A:Type} extra:A = HashmapAugE n V A;
/// ahme_root$1 {n:#} {V:Type} {A:Type} root:^(HashmapAug n V A) extra:A = HashmapAugE n V A;
/// ```
pub struct AugDict<K, A, V> {
    dict: Dict<K, (A, V)>,
    extra: A,
    _key: PhantomData<K>,
    _value: PhantomData<(A, V)>,
}

impl<K, A: ExactSize, V> ExactSize for AugDict<K, A, V> {
    #[inline]
    fn exact_size(&self) -> Size {
        self.dict.exact_size() + self.extra.exact_size()
    }
}

impl<'a, K, A: Load<'a>, V> Load<'a> for AugDict<K, A, V> {
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        Ok(Self {
            dict: ok!(Dict::load_from(slice)),
            extra: ok!(A::load_from(slice)),
            _key: PhantomData,
            _value: PhantomData,
        })
    }
}

impl<K, A: Store, V> Store for AugDict<K, A, V> {
    #[inline]
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        ok!(self.dict.store_into(builder, context));
        self.extra.store_into(builder, context)
    }
}

impl<K, A: Default, V> Default for AugDict<K, A, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<K, A: Clone, V> Clone for AugDict<K, A, V> {
    fn clone(&self) -> Self {
        Self {
            dict: self.dict.clone(),
            extra: self.extra.clone(),
            _key: PhantomData,
            _value: PhantomData,
        }
    }
}

impl<K, A: Eq, V> Eq for AugDict<K, A, V> {}

impl<K, A: PartialEq, V> PartialEq for AugDict<K, A, V> {
    fn eq(&self, other: &Self) -> bool {
        self.dict.eq(&other.dict) && self.extra.eq(&other.extra)
    }
}

impl<K, A: std::fmt::Debug, V> std::fmt::Debug for AugDict<K, A, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_struct_field2_finish(f, "AugDict", "dict", &self.dict, "extra", &self.extra)
    }
}

impl<K, A: Default, V> AugDict<K, A, V> {
    /// Creates an empty dictionary
    pub fn new() -> Self {
        Self {
            dict: Dict::new(),
            extra: A::default(),
            _key: PhantomData,
            _value: PhantomData,
        }
    }

    /// Manually constructs the dictionary from parts.
    pub const fn from_parts(dict: Dict<K, (A, V)>, extra: A) -> Self {
        Self {
            dict,
            extra,
            _key: PhantomData,
            _value: PhantomData,
        }
    }

    /// Returns an underlying dictionary and the extra value.
    pub fn into_parts(self) -> (Dict<K, (A, V)>, A) {
        (self.dict, self.extra)
    }

    /// Converts into a dictionary with an equivalent value type.
    #[inline]
    pub fn cast_into<Q, T>(self) -> AugDict<Q, A, T>
    where
        Q: EquivalentRepr<K>,
        (A, T): EquivalentRepr<(A, V)>,
    {
        AugDict {
            dict: self.dict.cast_into(),
            extra: self.extra,
            _key: PhantomData,
            _value: PhantomData,
        }
    }
}

impl<K: DictKey, A, V> AugDict<K, A, V> {
    /// Loads a non-empty dictionary from a root cell.
    pub fn load_from_root_ext<'a>(
        slice: &mut CellSlice<'a>,
        context: &dyn CellContext,
    ) -> Result<Self, Error>
    where
        A: Load<'a>,
        V: Load<'a>,
    {
        let (extra, root) = ok!(load_from_root::<A, V>(slice, K::BITS, context));

        Ok(Self {
            dict: Dict::from(Some(root)),
            extra,
            _key: PhantomData,
            _value: PhantomData,
        })
    }
}

impl<K, A, V> AugDict<K, A, V>
where
    K: DictKey,
    for<'a> A: Default + Load<'a>,
{
    /// Recomputes the root extra value.
    pub fn update_root_extra(&mut self) -> Result<(), Error> {
        self.extra = match &self.dict.root {
            Some(root) => {
                let slice = &mut ok!(root.as_slice());
                let prefix = ok!(read_label(slice, K::BITS));
                if prefix.size_bits() != K::BITS {
                    ok!(slice.skip_first(0, 2));
                }
                ok!(A::load_from(slice))
            }
            None => A::default(),
        };
        Ok(())
    }
}

fn load_from_root<'a, A, V>(
    slice: &mut CellSlice<'a>,
    key_bit_len: u16,
    context: &dyn CellContext,
) -> Result<(A, Cell), Error>
where
    A: Load<'a>,
    V: Load<'a>,
{
    let root = *slice;

    let label = ok!(read_label(slice, key_bit_len));
    let extra = if label.size_bits() != key_bit_len {
        ok!(slice.skip_first(0, 2));
        ok!(A::load_from(slice))
    } else {
        let extra = ok!(A::load_from(slice));
        ok!(V::load_from(slice));
        extra
    };

    let root_bits = root.size_bits() - slice.size_bits();
    let root_refs = root.size_refs() - slice.size_refs();

    let mut b = CellBuilder::new();
    ok!(b.store_slice(root.get_prefix(root_bits, root_refs)));
    match b.build_ext(context) {
        Ok(cell) => Ok((extra, cell)),
        Err(e) => Err(e),
    }
}

impl<K, A, V> AugDict<K, A, V> {
    /// Returns `true` if the dictionary contains no elements.
    pub const fn is_empty(&self) -> bool {
        self.dict.is_empty()
    }

    /// Returns the underlying dictionary.
    #[inline]
    pub const fn dict(&self) -> &Dict<K, (A, V)> {
        &self.dict
    }

    /// Returns the root augmented value.
    #[inline]
    pub const fn root_extra(&self) -> &A {
        &self.extra
    }
}

impl<K, A, V> AugDict<K, A, V>
where
    K: StoreDictKey,
{
    /// Returns `true` if the dictionary contains a value for the specified key.
    pub fn contains_key<Q>(&self, key: Q) -> Result<bool, Error>
    where
        Q: Borrow<K>,
    {
        self.dict.contains_key(key)
    }
}

impl<K, A, V> AugDict<K, A, V>
where
    K: StoreDictKey,
{
    /// Returns the value corresponding to the key.
    pub fn get<'a: 'b, 'b, Q>(&'a self, key: Q) -> Result<Option<(A, V)>, Error>
    where
        Q: Borrow<K> + 'b,
        (A, V): Load<'a>,
    {
        self.dict.get(key)
    }
}

impl<K, A, V> AugDict<K, A, V>
where
    K: LoadDictKey,
{
    /// Searches for an item using a predicate on extra values.
    ///
    /// Used as a secondary index.
    pub fn find_by_extra<'a, S>(&'a self, flow: S) -> Result<Option<(K, A, V)>, Error>
    where
        S: SearchByExtra<A>,
        A: Load<'a>,
        V: Load<'a>,
    {
        let Some((key, extra, mut value)) = ok!(aug_dict_find_by_extra::<A, S>(
            self.dict.root.as_ref(),
            K::BITS,
            flow
        )) else {
            return Ok(None);
        };

        let Some(key) = K::load_from_data(&key) else {
            return Err(Error::CellUnderflow);
        };
        let value = ok!(V::load_from(&mut value));
        Ok(Some((key, extra, value)))
    }
}

impl<K, A, V> AugDict<K, A, V>
where
    K: StoreDictKey,
    for<'a> A: AugDictExtra + Store + Load<'a>,
    V: Store,
{
    /// Builds a dictionary from a sorted collection.
    pub fn try_from_btree<Q, E, T>(sorted: &BTreeMap<Q, (E, T)>) -> Result<Self, Error>
    where
        Q: Borrow<K>,
        E: Borrow<A>,
        T: Borrow<V>,
        K: DictKey + Ord,
    {
        let root = ok!(build_aug_dict_from_sorted_iter(
            sorted
                .iter()
                .map(|(k, (a, v))| (k.borrow(), a.borrow(), v.borrow())),
            A::comp_add,
            Cell::empty_context()
        ));

        let mut result = Self {
            dict: Dict::from_raw(root),
            extra: A::default(),
            _key: PhantomData,
            _value: PhantomData,
        };
        ok!(result.update_root_extra());
        Ok(result)
    }

    /// Builds a dictionary from a sorted slice.
    pub fn try_from_sorted_slice<Q, E, T>(sorted: &[(Q, E, T)]) -> Result<Self, Error>
    where
        Q: Borrow<K>,
        E: Borrow<A>,
        T: Borrow<V>,
        K: Ord,
    {
        let root = ok!(build_aug_dict_from_sorted_iter(
            sorted
                .iter()
                .map(|(k, a, v)| (k.borrow(), a.borrow(), v.borrow())),
            A::comp_add,
            Cell::empty_context()
        ));

        let mut result = Self {
            dict: Dict::from_raw(root),
            extra: A::default(),
            _key: PhantomData,
            _value: PhantomData,
        };
        ok!(result.update_root_extra());
        Ok(result)
    }

    /// Applies a sorted list of inserts/removes to the dictionary.
    /// Use this when you have a large set of known changes.
    ///
    /// Uses custom extracts for values.
    pub fn modify_with_sorted_iter<I>(&mut self, entries: I) -> Result<bool, Error>
    where
        I: IntoIterator<Item = (K, Option<(A, V)>)>,
        K: Clone + Ord,
    {
        self.modify_with_sorted_iter_ext(
            entries,
            |(key, _)| key.clone(),
            |(_, value)| Ok(value),
            Cell::empty_context(),
        )
    }

    /// Applies a sorted list of inserts/removes to the dictionary.
    /// Use this when you have a large set of known changes.
    ///
    /// Uses custom extracts for values.
    pub fn modify_with_sorted_iter_ext<T, I, FK, FV>(
        &mut self,
        entries: I,
        extract_key: FK,
        extract_value: FV,
        context: &dyn CellContext,
    ) -> Result<bool, Error>
    where
        I: IntoIterator<Item = T>,
        K: Ord,
        for<'a> FK: FnMut(&'a T) -> K,
        FV: FnMut(T) -> Result<Option<(A, V)>, Error>,
    {
        let modified = ok!(aug_dict_modify_from_sorted_iter(
            &mut self.dict.root,
            entries,
            extract_key,
            extract_value,
            A::comp_add,
            context,
        ));

        if modified {
            ok!(self.update_root_extra());
        }

        Ok(modified)
    }

    /// Sets the augmented value associated with the key in the aug dictionary.
    ///
    /// Use [`set_ext`] if you need to use a custom cell context.
    ///
    /// [`set_ext`]: AugDict::set_ext
    pub fn set<Q, E, T>(&mut self, key: Q, aug: E, value: T) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        E: Borrow<A>,
        T: Borrow<V>,
    {
        self.set_ext(key, aug, value, Cell::empty_context())
    }

    /// Sets the value associated with the key in the dictionary.
    pub fn set_ext<Q, E, T>(
        &mut self,
        key: Q,
        aug: E,
        value: T,
        context: &dyn CellContext,
    ) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        E: Borrow<A>,
        T: Borrow<V>,
    {
        self.insert_impl(
            key.borrow(),
            aug.borrow(),
            value.borrow(),
            SetMode::Set,
            context,
        )
    }

    /// Sets the augmented value associated with the key in the aug dictionary
    /// only if the key was already present in it.
    ///
    /// Use [`replace_ext`] if you need to use a custom cell context.
    ///
    /// [`replace_ext`]: AugDict::replace_ext
    pub fn replace<Q, E, T>(&mut self, key: Q, aug: E, value: T) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        E: Borrow<A>,
        T: Borrow<V>,
    {
        self.replace_ext(key, aug, value, Cell::empty_context())
    }

    /// Sets the value associated with the key in the dictionary
    /// only if the key was already present in it.
    pub fn replace_ext<Q, E, T>(
        &mut self,
        key: Q,
        aug: E,
        value: T,
        context: &dyn CellContext,
    ) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        E: Borrow<A>,
        T: Borrow<V>,
    {
        self.insert_impl(
            key.borrow(),
            aug.borrow(),
            value.borrow(),
            SetMode::Replace,
            context,
        )
    }

    /// Sets the value associated with key in aug dictionary,
    /// but only if it is not already present.
    ///
    /// Use [`add_ext`] if you need to use a custom cell context.
    ///
    /// [`add_ext`]: AugDict::add_ext
    pub fn add<Q, E, T>(&mut self, key: Q, aug: E, value: T) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        E: Borrow<A>,
        T: Borrow<V>,
    {
        self.add_ext(key, aug, value, Cell::empty_context())
    }

    /// Sets the value associated with key in dictionary,
    /// but only if it is not already present.
    pub fn add_ext<Q, E, T>(
        &mut self,
        key: Q,
        aug: E,
        value: T,
        context: &dyn CellContext,
    ) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        E: Borrow<A>,
        T: Borrow<V>,
    {
        self.insert_impl(
            key.borrow(),
            aug.borrow(),
            value.borrow(),
            SetMode::Add,
            context,
        )
    }

    /// Removes the value associated with key in aug dictionary.
    /// Returns an optional removed value as cell slice parts.
    pub fn remove<Q>(&mut self, key: Q) -> Result<Option<(A, V)>, Error>
    where
        Q: Borrow<K>,
        for<'a> A: Load<'a> + 'static,
        for<'a> V: Load<'a> + 'static,
    {
        match ok!(self.remove_raw_ext(key, Cell::empty_context())) {
            Some(parts) => {
                let mut slice = ok!(CellSlice::apply(&parts));
                let extra = ok!(A::load_from(&mut slice));
                let value = ok!(V::load_from(&mut slice));
                Ok(Some((extra, value)))
            }
            None => Ok(None),
        }
    }

    /// Removes the value associated with key in dictionary.
    /// Returns an optional removed value as cell slice parts.
    pub fn remove_raw<Q>(&mut self, key: Q) -> Result<Option<CellSliceParts>, Error>
    where
        Q: Borrow<K>,
    {
        self.remove_impl(key.borrow(), Cell::empty_context())
    }

    /// Removes the value associated with key in dictionary.
    /// Returns an optional removed value as cell slice parts.
    pub fn remove_raw_ext<Q>(
        &mut self,
        key: Q,
        context: &dyn CellContext,
    ) -> Result<Option<CellSliceParts>, Error>
    where
        Q: Borrow<K>,
    {
        self.remove_impl(key.borrow(), context)
    }

    fn insert_impl(
        &mut self,
        key: &K,
        extra: &A,
        value: &V,
        mode: SetMode,
        context: &dyn CellContext,
    ) -> Result<bool, Error> {
        let mut key_builder = CellDataBuilder::new();
        ok!(key.store_into_data(&mut key_builder));
        let inserted = ok!(aug_dict_insert(
            &mut self.dict.root,
            &mut key_builder.as_data_slice(),
            K::BITS,
            extra,
            value,
            mode,
            A::comp_add,
            context,
        ));

        if inserted {
            ok!(self.update_root_extra());
        }

        Ok(inserted)
    }

    fn remove_impl(
        &mut self,
        key: &K,
        context: &dyn CellContext,
    ) -> Result<Option<CellSliceParts>, Error> {
        let mut key_builder = CellDataBuilder::new();
        ok!(key.store_into_data(&mut key_builder));
        let res = ok!(aug_dict_remove_owned(
            &mut self.dict.root,
            &mut key_builder.as_data_slice(),
            K::BITS,
            false,
            A::comp_add,
            context,
        ));

        if res.is_some() {
            ok!(self.update_root_extra());
        }

        Ok(res)
    }

    /// Split dictionary into 2 dictionaries by the first key bit.
    pub fn split(&self) -> Result<(Self, Self), Error> {
        self.split_by_prefix_ext(&Default::default(), Cell::empty_context())
    }

    /// Split dictionary into 2 dictionaries by the first key bit.
    pub fn split_ext(&self, context: &dyn CellContext) -> Result<(Self, Self), Error> {
        self.split_by_prefix_ext(&Default::default(), context)
    }

    /// Split dictionary into 2 dictionaries at the prefix.
    pub fn split_by_prefix(&self, key_prefix: &CellSlice<'_>) -> Result<(Self, Self), Error> {
        self.split_by_prefix_ext(key_prefix, Cell::empty_context())
    }

    /// Split dictionary into 2 dictionaries at the prefix.
    pub fn split_by_prefix_ext(
        &self,
        key_prefix: &CellSlice<'_>,
        context: &dyn CellContext,
    ) -> Result<(Self, Self), Error> {
        let (left, right) = ok!(self.dict.split_by_prefix_ext(key_prefix, context));

        let mut left = Self {
            dict: left,
            extra: A::default(),
            _key: PhantomData,
            _value: PhantomData,
        };
        ok!(left.update_root_extra());

        let mut right = Self {
            dict: right,
            extra: A::default(),
            _key: PhantomData,
            _value: PhantomData,
        };
        ok!(right.update_root_extra());

        Ok((left, right))
    }

    /// Merge dictionary with its right sibling.
    pub fn merge_with_right_sibling(&self, right: &AugDict<K, A, V>) -> Result<Self, Error> {
        let dict = self.merge_with_right_sibling_ext(right, Cell::empty_context())?;
        Ok(dict)
    }

    /// Merge dictionary with its sibling
    pub fn merge_with_right_sibling_ext(
        &self,
        right: &AugDict<K, A, V>,
        context: &dyn CellContext,
    ) -> Result<Self, Error> {
        let merged = ok!(aug_dict_merge_siblings(
            self.dict().root(),
            right.dict().root(),
            K::BITS,
            A::comp_add,
            context,
        ));

        let mut res = Self {
            dict: Dict::from_raw(merged),
            extra: A::default(),
            _key: PhantomData,
            _value: PhantomData,
        };
        ok!(res.update_root_extra());

        Ok(res)
    }
}

impl<K, A, V> AugDict<K, A, V>
where
    K: DictKey,
{
    /// Gets an iterator over the entries of the dictionary, sorted by key.
    /// The iterator element type is `Result<(K, A, V)>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over dictionary builds a key
    /// for each element. Use [`values`] or [`raw_values`] if you don't need keys from an iterator.
    ///
    /// [`values`]: Dict::values
    /// [`raw_values`]: Dict::raw_values
    pub fn iter<'a>(&'a self) -> AugIter<'a, K, A, V>
    where
        V: Load<'a>,
    {
        AugIter::new(self.dict.root())
    }

    /// Gets an iterator over the keys of the dictionary, in sorted order.
    /// The iterator element type is `Result<K>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over dictionary builds a key
    /// for each element. Use [`values`] if you don't need keys from an iterator.
    ///
    /// [`values`]: Dict::values
    pub fn keys(&'_ self) -> Keys<'_, K> {
        Keys::new(self.dict.root())
    }
}

impl<K, A, V> AugDict<K, A, V>
where
    K: DictKey,
{
    /// Gets an iterator over the augmented values of the dictionary, in order by key.
    /// The iterator element type is `Result<V>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn values<'a>(&'a self) -> Values<'a, (A, V)>
    where
        V: Load<'a>,
    {
        Values::new(self.dict.root(), K::BITS)
    }
}

impl<K, A, V> AugDict<K, A, V>
where
    K: Store + DictKey,
{
    /// Gets an iterator over the raw entries of the dictionary, sorted by key.
    /// The iterator element type is `Result<(CellBuilder, CellSlice)>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over dictionary builds a key
    /// for each element. Use [`values`] or [`raw_values`] if you don't need keys from an iterator.
    ///
    /// [`values`]: AugDict::values
    /// [`raw_values`]: AugDict::raw_values
    pub fn raw_iter(&'_ self) -> RawIter<'_> {
        RawIter::new(self.dict.root(), K::BITS)
    }

    /// Gets an iterator over the raw keys of the dictionary, in sorted order.
    /// The iterator element type is `Result<CellBuilder>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over dictionary builds a key
    /// for each element. Use [`values`] or [`raw_values`] if you don't need keys from an iterator.
    ///
    /// [`values`]: AugDict::values
    /// [`raw_values`]: AugDict::raw_values
    pub fn raw_keys(&'_ self) -> RawKeys<'_> {
        RawKeys::new(self.dict.root(), K::BITS)
    }
}

impl<K, A, V> AugDict<K, A, V>
where
    K: DictKey,
{
    /// Gets an iterator over the raw values of the dictionary, in order by key.
    /// The iterator element type is `Result<CellSlice>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn raw_values(&'_ self) -> RawValues<'_> {
        RawValues::new(self.dict.root(), K::BITS)
    }
}

#[cfg(feature = "serde")]
impl<K, A, V> serde::Serialize for AugDict<K, A, V>
where
    K: serde::Serialize + LoadDictKey,
    for<'a> A: serde::Serialize + Store + Load<'a>,
    for<'a> V: serde::Serialize + Load<'a>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::{Error, SerializeMap};

        #[derive(serde::Serialize)]
        struct AugDictHelper<'a, K, A, V>
        where
            K: serde::Serialize + LoadDictKey,
            A: serde::Serialize + Store + Load<'a>,
            V: serde::Serialize + Load<'a>,
        {
            #[serde(serialize_with = "serialize_dict_entries")]
            entires: &'a AugDict<K, A, V>,
            extra: &'a A,
        }

        fn serialize_dict_entries<'a, K, A, V, S>(
            dict: &'a AugDict<K, A, V>,
            serializer: S,
        ) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
            K: serde::Serialize + LoadDictKey,
            A: serde::Serialize + Store + Load<'a>,
            V: serde::Serialize + Load<'a>,
        {
            let mut ser = serializer.serialize_map(None)?;
            for ref entry in dict.iter() {
                let (key, extra, value) = match entry {
                    Ok(entry) => entry,
                    Err(e) => return Err(Error::custom(e)),
                };
                ok!(ser.serialize_entry(key, &(extra, value)));
            }
            ser.end()
        }

        if serializer.is_human_readable() {
            AugDictHelper {
                entires: self,
                extra: &self.extra,
            }
            .serialize(serializer)
        } else {
            crate::boc::BocRepr::serialize(self, serializer)
        }
    }
}

/// An iterator over the entries of an [`AugDict`].
///
/// This struct is created by the [`iter`] method on [`AugDict`]. See its documentation for more.
///
/// [`iter`]: AugDict::iter
pub struct AugIter<'a, K, A, V> {
    inner: Iter<'a, K, (A, V)>,
}

impl<K, A, V> Clone for AugIter<'_, K, A, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<'a, K, A, V> AugIter<'a, K, A, V>
where
    K: DictKey,
{
    /// Creates an iterator over the entries of a dictionary.
    pub fn new(root: &'a Option<Cell>) -> Self {
        Self {
            inner: Iter::new(root),
        }
    }

    /// Changes the direction of the iterator to descending.
    #[inline]
    pub fn reversed(mut self) -> Self {
        self.inner = self.inner.reversed();
        self
    }

    /// Changes the behavior of the iterator to reverse the high bit.
    #[inline]
    pub fn signed(mut self) -> Self {
        self.inner = self.inner.signed();
        self
    }
}

impl<'a, K, A, V> Iterator for AugIter<'a, K, A, V>
where
    K: LoadDictKey,
    (A, V): Load<'a>,
{
    type Item = Result<(K, A, V), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Ok((key, (aug, value))) => Some(Ok((key, aug, value))),
            Err(e) => Some(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use anyhow::Context;
    use rston::models::ShardIdent;

    use super::*;
    use crate::boc::Boc;
    use crate::models::{DepthBalanceInfo, ShardAccount, ShardAccounts};

    #[derive(Debug, Default, Load, Store, Eq, PartialEq)]
    struct OrCmp(bool);

    impl AugDictExtra for OrCmp {
        fn comp_add(
            left: &mut CellSlice,
            right: &mut CellSlice,
            b: &mut CellBuilder,
            _: &dyn CellContext,
        ) -> Result<(), Error> {
            let left = left.load_bit()?;
            let right = right.load_bit()?;
            b.store_bit(left | right)
        }
    }

    #[derive(Debug, Default, Load, Store, Eq, PartialEq)]
    struct SomeValue(u32);

    impl AugDictExtra for SomeValue {
        fn comp_add(
            left: &mut CellSlice,
            right: &mut CellSlice,
            b: &mut CellBuilder,
            _: &dyn CellContext,
        ) -> Result<(), Error> {
            let left = left.load_u32()?;
            let right = right.load_u32()?;
            b.store_u32(left.saturating_add(right))
        }
    }

    impl rand9::distr::Distribution<SomeValue> for rand9::distr::StandardUniform {
        #[inline]
        fn sample<R: rand9::Rng + ?Sized>(&self, rng: &mut R) -> SomeValue {
            SomeValue(rng.random())
        }
    }

    #[derive(Debug, Default, Load, Store, Eq, PartialEq)]
    struct ValueWithCell(u32, Cell);

    impl ValueWithCell {
        fn new(num: u32, cell: u32) -> Self {
            Self(num, CellBuilder::build_from(cell).unwrap())
        }
    }

    impl AugDictExtra for ValueWithCell {
        fn comp_add(
            left: &mut CellSlice,
            right: &mut CellSlice,
            b: &mut CellBuilder,
            ctx: &dyn CellContext,
        ) -> Result<(), Error> {
            // println!(
            //     "LEFT EXTRA: {}",
            //     Boc::encode_base64(CellBuilder::build_from(*left)?),
            // );
            // println!(
            //     "RIGHT EXTRA: {}",
            //     Boc::encode_base64(CellBuilder::build_from(*right)?),
            // );

            let left_num = left.load_u32()?;
            let left_cell = left.load_reference_as_slice()?.load_u32()?;

            let right_num = right.load_u32()?;
            let right_cell = right.load_reference_as_slice()?.load_u32()?;

            ValueWithCell::new(
                left_num.saturating_add(right_num),
                left_cell.saturating_add(right_cell),
            )
            .store_into(b, ctx)
        }
    }

    #[test]
    fn dict_set() {
        let mut dict = AugDict::<u32, OrCmp, u16>::new();
        assert_eq!(*dict.root_extra(), OrCmp(false));

        dict.set(123, OrCmp(false), 0xffff).unwrap();
        assert_eq!(dict.get(123).unwrap(), Some((OrCmp(false), 0xffff)));
        assert_eq!(*dict.root_extra(), OrCmp(false));

        dict.set(123, OrCmp(true), 0xcafe).unwrap();
        assert_eq!(dict.get(123).unwrap(), Some((OrCmp(true), 0xcafe)));
        assert_eq!(*dict.root_extra(), OrCmp(true));
    }

    #[test]
    fn dict_set_complex() {
        let mut dict = AugDict::<u32, OrCmp, u32>::new();
        assert_eq!(*dict.root_extra(), OrCmp(false));

        for i in 0..520 {
            dict.set(i, OrCmp(true), 123).unwrap();
        }
        assert_eq!(*dict.root_extra(), OrCmp(true));
    }

    #[test]
    fn dict_replace() {
        let mut dict = AugDict::<u32, OrCmp, u16>::new();
        assert_eq!(*dict.root_extra(), OrCmp(false));
        dict.replace(123, OrCmp(false), 0xff).unwrap();
        assert!(!dict.contains_key(123).unwrap());
        assert_eq!(*dict.root_extra(), OrCmp(false));

        dict.set(123, OrCmp(false), 0xff).unwrap();
        assert_eq!(dict.get(123).unwrap(), Some((OrCmp(false), 0xff)));
        assert_eq!(*dict.root_extra(), OrCmp(false));

        dict.replace(123, OrCmp(true), 0xaa).unwrap();
        assert_eq!(dict.get(123).unwrap(), Some((OrCmp(true), 0xaa)));
        assert_eq!(*dict.root_extra(), OrCmp(true));
    }

    #[test]
    fn dict_add() {
        let mut dict = AugDict::<u32, OrCmp, u16>::new();
        assert_eq!(*dict.root_extra(), OrCmp(false));

        dict.add(123, OrCmp(false), 0x12).unwrap();
        assert_eq!(dict.get(123).unwrap(), Some((OrCmp(false), 0x12)));
        assert_eq!(*dict.root_extra(), OrCmp(false));

        dict.add(123, OrCmp(true), 0x11).unwrap();
        assert_eq!(dict.get(123).unwrap(), Some((OrCmp(false), 0x12)));
        assert_eq!(*dict.root_extra(), OrCmp(false));
    }

    #[test]
    fn dict_remove() {
        let mut dict = AugDict::<u32, OrCmp, u32>::new();
        assert_eq!(*dict.root_extra(), OrCmp(false));

        for i in 0..10 {
            assert!(dict.set(i, OrCmp(i % 2 == 0), i).unwrap());
        }
        assert_eq!(*dict.root_extra(), OrCmp(true));

        let mut check_remove = |n: u32, expected: Option<(OrCmp, u32)>| -> anyhow::Result<()> {
            let removed = dict.remove(n).context("Failed to remove")?;
            anyhow::ensure!(removed == expected);
            Ok(())
        };

        check_remove(0, Some((OrCmp(true), 0))).unwrap();

        check_remove(4, Some((OrCmp(true), 4))).unwrap();

        check_remove(9, Some((OrCmp(false), 9))).unwrap();
        check_remove(9, None).unwrap();

        check_remove(5, Some((OrCmp(false), 5))).unwrap();
        check_remove(5, None).unwrap();

        check_remove(100, None).unwrap();

        check_remove(1, Some((OrCmp(false), 1))).unwrap();
        check_remove(2, Some((OrCmp(true), 2))).unwrap();
        check_remove(3, Some((OrCmp(false), 3))).unwrap();
        check_remove(6, Some((OrCmp(true), 6))).unwrap();
        check_remove(7, Some((OrCmp(false), 7))).unwrap();
        check_remove(8, Some((OrCmp(true), 8))).unwrap();

        assert!(dict.is_empty());
    }

    #[test]
    fn dict_iter() {
        let mut dict = AugDict::<u32, SomeValue, u32>::new();
        assert_eq!(*dict.root_extra(), SomeValue(0));

        let mut expected_extra = 0;
        for i in 0..10 {
            expected_extra += i;
            dict.set(i, SomeValue(i), 9 - i).unwrap();
        }
        assert_eq!(*dict.root_extra(), SomeValue(expected_extra));

        let size = dict.values().count();
        assert_eq!(size, 10);

        for (i, entry) in dict.iter().enumerate() {
            let (key, aug, value) = entry.unwrap();
            assert_eq!(SomeValue(key), aug);
            assert_eq!(key, i as u32);
            assert_eq!(value, 9 - i as u32);
        }
    }

    #[cfg(feature = "models")]
    #[test]
    fn aug_test() {
        use crate::models::{AccountBlock, CurrencyCollection};
        use crate::prelude::Boc;

        let boc = Boc::decode(include_bytes!("./tests/account_blocks_aug_dict.boc")).unwrap();

        let original_dict = boc
            .parse::<AugDict<HashBytes, CurrencyCollection, AccountBlock>>()
            .unwrap();

        let mut data = Vec::new();
        for entry in original_dict.iter() {
            data.push(entry.unwrap());
        }
        data.reverse();

        let mut new_dict: AugDict<HashBytes, CurrencyCollection, AccountBlock> = AugDict::new();
        for (key, aug, value) in data.iter() {
            new_dict.add(key, aug, value).unwrap();
        }
        assert_eq!(new_dict.root_extra(), original_dict.root_extra());

        let serialized = CellBuilder::build_from(&new_dict).unwrap();
        assert_eq!(serialized.repr_hash(), boc.repr_hash());

        for (key, _, _) in data.iter() {
            new_dict.remove(key).unwrap();
        }
        assert!(new_dict.is_empty());
        assert_eq!(new_dict.root_extra(), &CurrencyCollection::ZERO);
    }

    #[test]
    fn build_from_array() {
        for entries in [
            &[
                (0u32, SomeValue(123), 1u32),
                (1, SomeValue(10), 2),
                (2, SomeValue(20), 4),
                (2, SomeValue(20), 3),
                (3, SomeValue(40), 4),
                (4, SomeValue(50), 5),
            ][..],
            &[
                (534837844, SomeValue(331123), 3117028142),
                (1421713188, SomeValue(5345345), 3155891450),
                (1526242096, SomeValue(567567), 2789399854),
                (1971086295, SomeValue(5345), 1228713494),
                (4258889371, SomeValue(4956495), 3256452222),
            ],
        ] {
            let result = AugDict::<u32, SomeValue, u32>::try_from_sorted_slice(entries).unwrap();

            let mut dict = AugDict::<u32, SomeValue, u32>::new();
            for (k, a, v) in entries {
                dict.add(k, a, v).unwrap();
            }

            println!("{}", result.dict.root.as_ref().unwrap().display_tree());
            println!(
                "BOC: {}",
                crate::boc::BocRepr::encode_base64(&result).unwrap()
            );

            assert_eq!(result, dict);
        }
    }

    #[test]
    fn compare_sorted_res() -> anyhow::Result<()> {
        let mut dict = AugDict::<u32, SomeValue, u64>::new();
        dict.add(268445184, SomeValue(269488144), 18446744073693827088)?;
        dict.add(4294934527, SomeValue(4294967295), 1224979098644774911)?;

        let mut other = AugDict::<u32, SomeValue, u64>::try_from_sorted_slice(&[
            (268445184, SomeValue(269488144), 18446744073693827088),
            (4294934527, SomeValue(4294967295), 1224979098644774911),
        ])?;
        assert_eq!(other, dict);

        other.remove(0)?;
        assert_eq!(other, dict);

        other.modify_with_sorted_iter([(0, None)])?;
        assert_eq!(other, dict);

        Ok(())
    }

    #[test]
    fn modify_aug_with_cells() -> anyhow::Result<()> {
        let mut dict = AugDict::<u32, ValueWithCell, u64>::new();
        dict.add(
            16729856,
            ValueWithCell::new(1111, 123),
            18381441879129409280,
        )?;
        dict.add(
            3607101952,
            ValueWithCell::new(2060965847, 234),
            8851780914645041664,
        )?;

        let mut other = AugDict::<u32, ValueWithCell, u64>::try_from_sorted_slice(&[
            (
                16729856,
                ValueWithCell::new(1111, 123),
                18381441879129409280,
            ),
            (
                3607101952,
                ValueWithCell::new(2060965847, 234),
                8851780914645041664,
            ),
        ])?;
        assert_eq!(other, dict);

        println!(
            "INITIAL: {:?}",
            dict.dict.root.as_ref().map(Boc::encode_base64),
        );

        dict.set(0, ValueWithCell::new(0, 0), 0)?;
        println!(
            "TARGET: {:?}",
            dict.dict.root.as_ref().map(Boc::encode_base64),
        );

        other.modify_with_sorted_iter([(0, Some((ValueWithCell::new(0, 0), 0)))])?;
        println!(
            "RESULT: {:?}",
            other.dict.root.as_ref().map(Boc::encode_base64),
        );
        assert_eq!(other, dict);

        Ok(())
    }

    #[test]
    fn modify_aug_with_cells_remove() -> anyhow::Result<()> {
        let mut dict = AugDict::<u32, ValueWithCell, u64>::new();
        dict.add(66024, ValueWithCell::new(0, 123), 4108413175295410176)?;
        dict.add(
            2575203966,
            ValueWithCell::new(67108922, 234),
            16710326059140383999,
        )?;
        dict.add(
            3907577831,
            ValueWithCell::new(3907578088, 345),
            144121978453491944,
        )?;

        let mut other = AugDict::<u32, ValueWithCell, u64>::try_from_sorted_slice(&[
            (66024, ValueWithCell::new(0, 123), 4108413175295410176),
            (
                2575203966,
                ValueWithCell::new(67108922, 234),
                16710326059140383999,
            ),
            (
                3907577831,
                ValueWithCell::new(3907578088, 345),
                144121978453491944,
            ),
        ])?;
        assert_eq!(other, dict);

        println!(
            "INITIAL: {:?}",
            dict.dict.root.as_ref().map(Boc::encode_base64),
        );

        dict.remove(0)?;
        dict.remove(71)?;
        assert_eq!(other, dict);

        other.modify_with_sorted_iter([(0, None), (71, None)])?;
        println!(
            "RESULT: {:?}",
            other.dict.root.as_ref().map(Boc::encode_base64),
        );
        assert_eq!(other, dict);

        Ok(())
    }

    #[test]
    fn build_from_any_array() {
        for _ in 0..100 {
            let n = 1 + rand9::random::<u32>() % 1000;
            let mut entries = (0..n)
                .map(|_| {
                    (
                        rand9::random::<u32>(),
                        rand9::random::<SomeValue>(),
                        rand9::random::<u32>(),
                    )
                })
                .collect::<Vec<_>>();
            entries.sort_by_key(|(k, _, _)| *k);

            let built_from_dict =
                AugDict::<u32, SomeValue, u32>::try_from_sorted_slice(&entries).unwrap();

            let mut dict = AugDict::<u32, SomeValue, u32>::new();
            for (k, a, v) in entries {
                dict.add(k, a, v).unwrap();
            }

            // println!("{}", built_from_dict.as_ref().unwrap().display_tree());

            assert_eq!(built_from_dict, dict);
        }
    }

    #[test]
    fn search_by_lt() {
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Store, Load)]
        struct MaxValue(u64);

        impl AugDictExtra for MaxValue {
            fn comp_add(
                left: &mut CellSlice,
                right: &mut CellSlice,
                b: &mut CellBuilder,
                _: &dyn CellContext,
            ) -> Result<(), Error> {
                let left = left.load_u64()?;
                let right = right.load_u64()?;
                b.store_u64(left.max(right))
            }
        }

        let mut items = AugDict::<u32, MaxValue, u32>::new();
        items.set(0, MaxValue(100), 123).unwrap();
        items.set(2, MaxValue(150), 234).unwrap();
        items.set(4, MaxValue(200), 345).unwrap();
        items.set(6, MaxValue(350), 456).unwrap();
        items.set(7, MaxValue(300), 567).unwrap();
        items.set(8, MaxValue(250), 678).unwrap();

        // Search by ordering
        let highest = items.find_by_extra(std::cmp::Ordering::Greater).unwrap();
        assert_eq!(highest, Some((6, MaxValue(350), 456)));
    }

    #[test]
    fn split_merge_aug_test() -> anyhow::Result<()> {
        let mut dict = AugDict::<u32, OrCmp, u32>::new();
        dict.add(1, OrCmp(true), 1)?;
        dict.add(2, OrCmp(true), 1)?;
        dict.add(3, OrCmp(true), 1)?;
        dict.add(u32::MAX - 2, OrCmp(false), 4)?;
        dict.add(u32::MAX - 1, OrCmp(false), 5)?;
        dict.add(u32::MAX, OrCmp(false), 6)?;

        let (d1, d2) = dict.split()?;

        let merged = d1.merge_with_right_sibling(&d2)?;
        for i in merged.iter() {
            let _ = i?;
        }
        assert_eq!(dict, merged);

        Ok(())
    }

    #[test]
    fn merge_with_none() -> anyhow::Result<()> {
        let cell = Boc::decode_base64(
            "te6ccgECCgEAAcMAAQ+BlrzEHpAAEAEBoaACN33l61Vk62GrjymKAv197LmrqqmfcDpXYIKjXocJzABlrzEHpAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABAICc8/xG77y9aqydbDVx5TFAX6+9lzV1VTPuB0rsEFRr0OE5gIQgkNAAAAAAAAAAAAAAAABlrzEHpAAE0AEAwBQiSKga9Z+zeO4A1bGS5OIEoMIyhKQn+Fzzir2sgDX9BQAAAAAAAAAAAEU/wD0pBP0vPLICwUCASAJBgLm8nHXAQHAAPJ6gwjXGO1E0IMH1wHXCz/I+CjPFiPPFsn5AANx1wEBwwCagwfXAVETuvLgZN6AQNcBgCDXAYAg1wFUFnX5EPKo+CO78nlmvvgjgQcIoIED6KhSILyx8nQCIIIQTO5kbLrjDwHIy//LP8ntVAgHAD6CEBaePhG6jhH4AAKTINdKl3jXAdQC+wDo0ZMy8jziAJgwAtdM0PpAgwbXAXHXAXjXAddM+ABwgBAEqgIUscjLBVAFzxZQA/oCy2ki0CHPMSHXSaCECbmYM3ABywBYzxaXMHEBywASzOLJAfsAAATSMA==",
        )?;
        let cell2 = Boc::decode_base64("te6ccgEBAQEABAAAAwAQ")?;

        let accounts = ShardAccounts::load_from(&mut cell.as_slice()?)?;
        let accounts2 = ShardAccounts::load_from(&mut cell2.as_slice()?)?;

        for i in accounts.iter() {
            let _ = i?;
        }

        for i in accounts2.iter() {
            let _ = i?;
        }

        let merged = accounts.merge_with_right_sibling(&accounts2)?;
        let merged2 = accounts2.merge_with_right_sibling(&accounts)?;

        assert_eq!(merged, merged2);
        Ok(())
    }

    #[test]
    fn split_merge_multiple() -> anyhow::Result<()> {
        let cell = Boc::decode_base64(
            "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",
        )?;
        let accounts = ShardAccounts::load_from(&mut cell.as_slice()?)?;
        let mut shards = BTreeMap::new();
        let shard = ShardIdent::new(-1, u64::from_str_radix("8000000000000000", 16)?).unwrap();
        split_shard_accounts(&shard, &accounts, 3, &mut shards)?;
        let merged = merge(shards)?;
        assert_eq!(accounts, merged);
        Ok(())
    }

    fn merge(
        shard_accounts: BTreeMap<u64, ShardAccounts>,
    ) -> anyhow::Result<AugDict<HashBytes, DepthBalanceInfo, ShardAccount>> {
        let shard_accounts = {
            let mut stack = VecDeque::with_capacity(16);
            for accounts in shard_accounts.values() {
                for account in accounts.iter() {
                    let _ = account?;
                }
                stack.push_back(accounts);
            }

            let d2 = stack[0].merge_with_right_sibling(stack[1])?;
            let d1 = stack[2].merge_with_right_sibling(stack[3])?;
            let d3 = stack[4].merge_with_right_sibling(stack[5])?;
            let d4 = stack[6].merge_with_right_sibling(stack[7])?;

            let x1 = d2.merge_with_right_sibling(&d1)?;
            let x2 = d3.merge_with_right_sibling(&d4)?;
            x1.merge_with_right_sibling(&x2)?
        };
        Ok(shard_accounts)
    }

    fn split_shard_accounts(
        shard: &ShardIdent,
        accounts: &ShardAccounts,
        depth: u8,
        shards: &mut BTreeMap<u64, ShardAccounts>,
    ) -> anyhow::Result<()> {
        fn split_shard_accounts_impl(
            shard: &ShardIdent,
            accounts: &ShardAccounts,
            depth: u8,
            shards: &mut BTreeMap<u64, ShardAccounts>,
            builder: &mut CellBuilder,
        ) -> anyhow::Result<()> {
            let (left_shard_ident, right_shard_ident) = 'split: {
                if depth > 0
                    && let Some((left, right)) = shard.split()
                {
                    break 'split (left, right);
                }
                shards.insert(shard.prefix(), accounts.clone());
                return Ok(());
            };

            let (left_accounts, right_accounts) = {
                builder.clear_bits();
                let prefix_len = shard.prefix_len();
                if prefix_len > 0 {
                    builder.store_uint(shard.prefix() >> (64 - prefix_len), prefix_len)?;
                }

                let (a1, a2) = accounts.split_by_prefix(&builder.as_data_slice())?;
                (a1, a2)
            };

            split_shard_accounts_impl(
                &left_shard_ident,
                &left_accounts,
                depth - 1,
                shards,
                builder,
            )?;
            split_shard_accounts_impl(
                &right_shard_ident,
                &right_accounts,
                depth - 1,
                shards,
                builder,
            )
        }

        split_shard_accounts_impl(shard, accounts, depth, shards, &mut CellBuilder::new())
    }
}
