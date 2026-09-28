use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::marker::PhantomData;

use super::raw::*;
use super::{
    DictBound, DictKey, SetMode, StoreDictKey, build_dict_from_sorted_iter, dict_find_bound,
    dict_find_owned, dict_get, dict_get_owned, dict_insert, dict_load_from_root,
    dict_merge_siblings, dict_modify_from_sorted_iter, dict_remove_bound_owned,
    dict_split_by_prefix,
};
use crate::cell::*;
use crate::dict::{LoadDictKey, dict_remove_owned};
use crate::error::Error;
use crate::util::*;

/// Typed dictionary with fixed length keys.
#[repr(transparent)]
pub struct Dict<K, V> {
    pub(crate) root: Option<Cell>,
    _key: PhantomData<K>,
    _value: PhantomData<V>,
}

impl<K, V> ExactSize for Dict<K, V> {
    #[inline]
    fn exact_size(&self) -> Size {
        Size {
            bits: 1,
            refs: self.root.is_some() as u8,
        }
    }
}

impl<'a, K, V> Load<'a> for Dict<K, V> {
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        Ok(Self {
            root: ok!(<_>::load_from(slice)),
            _key: PhantomData,
            _value: PhantomData,
        })
    }
}

impl<K, V> Store for Dict<K, V> {
    #[inline]
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        self.root.store_into(builder, context)
    }
}

impl<K, V> Default for Dict<K, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> Clone for Dict<K, V> {
    fn clone(&self) -> Self {
        Self {
            root: self.root.clone(),
            _key: PhantomData,
            _value: PhantomData,
        }
    }
}

impl<K, V> Eq for Dict<K, V> {}

impl<K, V> PartialEq for Dict<K, V> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.root, &other.root) {
            (Some(this), Some(other)) => this.eq(other),
            (None, None) => true,
            _ => false,
        }
    }
}

impl<K, V> From<Option<Cell>> for Dict<K, V> {
    #[inline]
    fn from(dict: Option<Cell>) -> Self {
        Self::from_raw(dict)
    }
}

impl<K, V> std::fmt::Debug for Dict<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        debug_struct_field1_finish(f, "Dict", "root", &self.root)
    }
}

impl<K, V, Q, T> TryFrom<BTreeMap<Q, T>> for Dict<K, V>
where
    Q: Borrow<K>,
    T: Borrow<V>,
    K: StoreDictKey + Ord,
    V: Store,
{
    type Error = Error;

    #[inline]
    fn try_from(value: BTreeMap<Q, T>) -> Result<Self, Self::Error> {
        Self::try_from_btree(&value)
    }
}

impl<K, V, Q, T> TryFrom<&'_ BTreeMap<Q, T>> for Dict<K, V>
where
    Q: Borrow<K>,
    T: Borrow<V>,
    K: StoreDictKey + Ord,
    V: Store,
{
    type Error = Error;

    #[inline]
    fn try_from(value: &BTreeMap<Q, T>) -> Result<Self, Self::Error> {
        Self::try_from_btree(value)
    }
}

impl<K, V> Dict<K, V> {
    /// Creates an empty dictionary
    pub const fn new() -> Self {
        Self {
            root: None,
            _key: PhantomData,
            _value: PhantomData,
        }
    }

    /// Creates a dictionary from a raw cell.
    pub const fn from_raw(dict: Option<Cell>) -> Self {
        Self {
            root: dict,
            _key: PhantomData,
            _value: PhantomData,
        }
    }

    /// Returns `true` if the dictionary contains no elements.
    pub const fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    /// Returns the underlying root cell of the dictionary.
    #[inline]
    pub const fn root(&self) -> &Option<Cell> {
        &self.root
    }

    /// Returns the underlying root cell of the dictionary.
    #[inline]
    pub fn into_root(self) -> Option<Cell> {
        self.root
    }

    /// Converts into a dictionary with an equivalent value type.
    #[inline]
    pub fn cast_into<Q, T>(self) -> Dict<Q, T>
    where
        Q: EquivalentRepr<K>,
        T: EquivalentRepr<V>,
    {
        Dict {
            root: self.root,
            _key: PhantomData,
            _value: PhantomData,
        }
    }

    /// Casts itself into a lazy loaded for an equivalent type.
    pub fn cast_ref<Q, T>(&self) -> &Dict<Q, T>
    where
        Q: EquivalentRepr<K>,
        T: EquivalentRepr<V>,
    {
        // SAFETY: Dict is #[repr(transparent)]
        unsafe { &*(self as *const Self as *const Dict<Q, T>) }
    }
}

impl<K: DictKey, V> Dict<K, V> {
    /// Loads a non-empty dictionary from a root cell.
    #[inline]
    pub fn load_from_root_ext(
        slice: &mut CellSlice<'_>,
        context: &dyn CellContext,
    ) -> Result<Self, Error> {
        match dict_load_from_root(slice, K::BITS, context) {
            Ok(root) => Ok(Self {
                root: Some(root),
                _key: PhantomData,
                _value: PhantomData,
            }),
            Err(e) => Err(e),
        }
    }
}

impl<K, V> Dict<K, V>
where
    K: StoreDictKey,
{
    /// Returns `true` if the dictionary contains a value for the specified key.
    #[inline]
    pub fn contains_key<Q>(&self, key: Q) -> Result<bool, Error>
    where
        Q: Borrow<K>,
    {
        fn contains_key_impl<K>(root: &Option<Cell>, key: &K) -> Result<bool, Error>
        where
            K: StoreDictKey,
        {
            let mut builder = CellDataBuilder::new();
            ok!(key.store_into_data(&mut builder));
            Ok(ok!(dict_get(
                root.as_ref(),
                K::BITS,
                builder.as_data_slice(),
                Cell::empty_context()
            ))
            .is_some())
        }
        contains_key_impl(&self.root, key.borrow())
    }
}

impl<K, V> Dict<K, V>
where
    K: StoreDictKey,
{
    /// Returns the value corresponding to the key.
    #[inline]
    pub fn get<'a: 'b, 'b, Q>(&'a self, key: Q) -> Result<Option<V>, Error>
    where
        Q: Borrow<K> + 'b,
        V: Load<'a>,
    {
        pub fn get_impl<'a: 'b, 'b, K, V>(
            root: &'a Option<Cell>,
            key: &'b K,
        ) -> Result<Option<V>, Error>
        where
            K: StoreDictKey,
            V: Load<'a>,
        {
            let Some(mut value) = ({
                let mut builder = CellDataBuilder::new();
                ok!(key.store_into_data(&mut builder));
                ok!(dict_get(
                    root.as_ref(),
                    K::BITS,
                    builder.as_data_slice(),
                    Cell::empty_context()
                ))
            }) else {
                return Ok(None);
            };

            match V::load_from(&mut value) {
                Ok(value) => Ok(Some(value)),
                Err(e) => Err(e),
            }
        }

        get_impl(&self.root, key.borrow())
    }

    /// Returns the raw value corresponding to the key.
    #[inline]
    pub fn get_raw<'a: 'b, 'b, Q>(&'a self, key: Q) -> Result<Option<CellSlice<'a>>, Error>
    where
        Q: Borrow<K> + 'b,
    {
        pub fn get_raw_impl<'a: 'b, 'b, K>(
            root: &'a Option<Cell>,
            key: &'b K,
        ) -> Result<Option<CellSlice<'a>>, Error>
        where
            K: StoreDictKey,
        {
            let mut builder = CellDataBuilder::new();
            ok!(key.store_into_data(&mut builder));
            dict_get(
                root.as_ref(),
                K::BITS,
                builder.as_data_slice(),
                Cell::empty_context(),
            )
        }

        get_raw_impl(&self.root, key.borrow())
    }

    /// Returns cell slice parts of the value corresponding to the key.
    ///
    /// NOTE: Uses the default cell context.
    #[inline]
    pub fn get_raw_owned<Q>(&self, key: Q) -> Result<Option<CellSliceParts>, Error>
    where
        Q: Borrow<K>,
    {
        pub fn get_raw_impl<K>(
            root: &Option<Cell>,
            key: &K,
        ) -> Result<Option<CellSliceParts>, Error>
        where
            K: StoreDictKey,
        {
            let mut builder = CellDataBuilder::new();
            ok!(key.store_into_data(&mut builder));
            dict_get_owned(
                root.as_ref(),
                K::BITS,
                builder.as_data_slice(),
                Cell::empty_context(),
            )
        }

        get_raw_impl(&self.root, key.borrow())
    }

    /// Removes the value associated with key in dictionary.
    /// Returns an optional removed value.
    ///
    /// The dict is rebuilt using an empty cell context.
    pub fn remove<Q>(&mut self, key: Q) -> Result<Option<V>, Error>
    where
        Q: Borrow<K>,
        for<'a> V: Load<'a> + 'static,
    {
        match ok!(self.remove_raw_ext(key, Cell::empty_context())) {
            Some(parts) => {
                let mut slice = ok!(CellSlice::apply(&parts));
                Ok(Some(ok!(V::load_from(&mut slice))))
            }
            None => Ok(None),
        }
    }

    /// Removes the value associated with key in dictionary.
    /// Returns an optional removed value as cell slice parts.
    ///
    /// The dict is rebuilt using an empty cell context.
    pub fn remove_raw<Q>(&mut self, key: Q) -> Result<Option<CellSliceParts>, Error>
    where
        Q: Borrow<K>,
    {
        self.remove_raw_ext(key, Cell::empty_context())
    }

    /// Removes the lowest key from the dict.
    /// Returns an optional removed key and value as cell slice parts.
    ///
    /// Use [`remove_bound_ext`] if you need to use a custom cell context.
    ///
    /// [`remove_bound_ext`]: RawDict::remove_bound_ext
    pub fn remove_min_raw(&mut self, signed: bool) -> Result<Option<(K, CellSliceParts)>, Error>
    where
        K: LoadDictKey,
    {
        self.remove_bound_raw_ext(DictBound::Min, signed, Cell::empty_context())
    }

    /// Removes the largest key from the dict.
    /// Returns an optional removed key and value as cell slice parts.
    ///
    /// Use [`remove_bound_ext`] if you need to use a custom cell context.
    ///
    /// [`remove_bound_ext`]: RawDict::remove_bound_ext
    pub fn remove_max_raw(&mut self, signed: bool) -> Result<Option<(K, CellSliceParts)>, Error>
    where
        K: LoadDictKey,
    {
        self.remove_bound_raw_ext(DictBound::Max, signed, Cell::empty_context())
    }

    /// Removes the specified dict bound.
    /// Returns an optional removed key and value as cell slice parts.
    ///
    /// Use [`remove_bound_ext`] if you need to use a custom cell context.
    ///
    /// [`remove_bound_ext`]: RawDict::remove_bound_ext
    pub fn remove_bound_raw(
        &mut self,
        bound: DictBound,
        signed: bool,
    ) -> Result<Option<(K, CellSliceParts)>, Error>
    where
        K: LoadDictKey,
    {
        self.remove_bound_raw_ext(bound, signed, Cell::empty_context())
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
        let (left, right) = ok!(dict_split_by_prefix(
            self.root.as_ref(),
            K::BITS,
            key_prefix,
            context
        ));
        Ok((Self::from_raw(left), Self::from_raw(right)))
    }

    /// Merge dictionary with its right sibling.
    pub fn merge_with_right_sibling(&self, right: &Dict<K, V>) -> Result<Self, Error>
    where
        for<'a> V: Load<'a> + 'static,
    {
        let dict = self.merge_with_right_sibling_ext(right, Cell::empty_context())?;
        Ok(dict)
    }

    /// Merge dictionary with its right sibling.
    pub fn merge_with_right_sibling_ext(
        &self,
        right: &Dict<K, V>,
        context: &dyn CellContext,
    ) -> Result<Self, Error> {
        dict_merge_siblings(self.root(), right.root(), K::BITS, context).map(Dict::from_raw)
    }
}

impl<K, V> Dict<K, V>
where
    K: StoreDictKey,
    V: Store,
{
    /// Builds a dictionary from a sorted collection.
    pub fn try_from_btree<Q, T>(sorted: &BTreeMap<Q, T>) -> Result<Self, Error>
    where
        Q: Borrow<K>,
        T: Borrow<V>,
        K: Ord,
    {
        let root = ok!(build_dict_from_sorted_iter(
            sorted.iter().map(|(k, v)| (k.borrow(), v.borrow())),
            Cell::empty_context()
        ));
        Ok(Self {
            root,
            _key: PhantomData,
            _value: PhantomData,
        })
    }

    /// Builds a dictionary from a sorted slice.
    pub fn try_from_sorted_slice<Q, T>(sorted: &[(Q, T)]) -> Result<Self, Error>
    where
        Q: Borrow<K>,
        T: Borrow<V>,
        K: Ord,
    {
        let root = ok!(build_dict_from_sorted_iter(
            sorted.iter().map(|(k, v)| (k.borrow(), v.borrow())),
            Cell::empty_context()
        ));
        Ok(Self {
            root,
            _key: PhantomData,
            _value: PhantomData,
        })
    }

    /// Applies a sorted list of inserts/removes to the dictionary.
    /// Use this when you have a large set of known changes.
    ///
    /// Uses custom extracts for values.
    pub fn modify_with_sorted_iter<I>(&mut self, entries: I) -> Result<bool, Error>
    where
        I: IntoIterator<Item = (K, Option<V>)>,
        K: Clone + Ord,
    {
        dict_modify_from_sorted_iter(
            &mut self.root,
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
        FV: FnMut(T) -> Result<Option<V>, Error>,
    {
        dict_modify_from_sorted_iter(&mut self.root, entries, extract_key, extract_value, context)
    }

    /// Sets the value associated with the key in the dictionary.
    ///
    /// Use [`set_ext`] if you need to use a custom cell context.
    ///
    /// [`set_ext`]: Dict::set_ext
    pub fn set<Q, T>(&mut self, key: Q, value: T) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        T: Borrow<V>,
    {
        self.set_ext(key, value, Cell::empty_context())
    }

    /// Sets the value associated with the key in the dictionary
    /// only if the key was already present in it.
    ///
    /// Use [`replace_ext`] if you need to use a custom cell context.
    ///
    /// [`replace_ext`]: Dict::replace_ext
    pub fn replace<Q, T>(&mut self, key: Q, value: T) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        T: Borrow<V>,
    {
        self.replace_ext(key, value, Cell::empty_context())
    }

    /// Sets the value associated with key in dictionary,
    /// but only if it is not already present.
    ///
    /// Use [`add_ext`] if you need to use a custom cell context.
    ///
    /// [`add_ext`]: Dict::add_ext
    pub fn add<Q, T>(&mut self, key: Q, value: T) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        T: Borrow<V>,
    {
        self.add_ext(key, value, Cell::empty_context())
    }
}

impl<K, V> Dict<K, V>
where
    K: DictKey,
{
    /// Gets an iterator over the entries of the dictionary, sorted by key.
    /// The iterator element type is `Result<(K, V)>`.
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
    pub fn iter<'a>(&'a self) -> Iter<'a, K, V>
    where
        V: Load<'a>,
    {
        Iter::new(&self.root)
    }

    /// Gets an iterator over the entries of two dictionaries, sorted by key.
    /// The iterator element type is `Result<(K, Option<V>, Option<V>)>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over dictionary builds a key
    /// for each element.
    pub fn iter_union<'a>(&'a self, other: &'a Self) -> UnionIter<'a, K, V>
    where
        V: Load<'a>,
    {
        UnionIter::new(&self.root, &other.root)
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
        Keys::new(&self.root)
    }

    /// Computes the minimal key in dictionary that is lexicographically greater than `key`,
    /// and returns it along with associated value as cell slice parts.
    #[inline]
    pub fn get_next<Q>(&self, key: Q, signed: bool) -> Result<Option<(K, V)>, Error>
    where
        Q: Borrow<K>,
        K: StoreDictKey + LoadDictKey,
        for<'a> V: Load<'a>,
    {
        self.find_ext(key, DictBound::Max, false, signed)
    }

    /// Computes the maximal key in dictionary that is lexicographically smaller than `key`,
    /// and returns it along with associated value as cell slice parts.
    #[inline]
    pub fn get_prev<Q>(&self, key: Q, signed: bool) -> Result<Option<(K, V)>, Error>
    where
        Q: Borrow<K>,
        K: StoreDictKey + LoadDictKey,
        for<'a> V: Load<'a>,
    {
        self.find_ext(key, DictBound::Min, false, signed)
    }

    /// Computes the minimal key in dictionary that is lexicographically greater than `key`,
    /// and returns it along with associated value as cell slice parts.
    #[inline]
    pub fn get_or_next<Q>(&self, key: Q, signed: bool) -> Result<Option<(K, V)>, Error>
    where
        Q: Borrow<K>,
        K: StoreDictKey + LoadDictKey,
        for<'a> V: Load<'a>,
    {
        self.find_ext(key, DictBound::Max, true, signed)
    }

    /// Computes the maximal key in dictionary that is lexicographically smaller than `key`,
    /// and returns it along with associated value as cell slice parts.
    #[inline]
    pub fn get_or_prev<Q>(&self, key: Q, signed: bool) -> Result<Option<(K, V)>, Error>
    where
        Q: Borrow<K>,
        K: StoreDictKey + LoadDictKey,
        for<'a> V: Load<'a>,
    {
        self.find_ext(key, DictBound::Min, true, signed)
    }

    #[inline]
    fn find_ext<Q>(
        &self,
        key: Q,
        towards: DictBound,
        inclusive: bool,
        signed: bool,
    ) -> Result<Option<(K, V)>, Error>
    where
        Q: Borrow<K>,
        K: StoreDictKey + LoadDictKey,
        for<'a> V: Load<'a>,
    {
        fn find_impl<K, V>(
            root: &Option<Cell>,
            key: &K,
            towards: DictBound,
            inclusive: bool,
            signed: bool,
        ) -> Result<Option<(K, V)>, Error>
        where
            K: StoreDictKey + LoadDictKey,
            for<'a> V: Load<'a>,
        {
            let context = Cell::empty_context();
            let Some((key, parts)) = ({
                let mut builder = CellDataBuilder::new();
                ok!(key.store_into_data(&mut builder));
                // TODO: add `dict_find` with non-owned return type
                ok!(dict_find_owned(
                    root.as_ref(),
                    K::BITS,
                    builder.as_data_slice(),
                    towards,
                    inclusive,
                    signed,
                    context,
                ))
            }) else {
                return Ok(None);
            };
            let value = &mut ok!(CellSlice::apply(&parts));

            match K::load_from_data(&key) {
                Some(key) => Ok(Some((key, ok!(V::load_from(value))))),
                None => Err(Error::CellUnderflow),
            }
        }

        find_impl(&self.root, key.borrow(), towards, inclusive, signed)
    }
}

impl<K, V> Dict<K, V>
where
    K: DictKey,
{
    /// Gets an iterator over the values of the dictionary, in order by key.
    /// The iterator element type is `Result<V>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn values<'a>(&'a self) -> Values<'a, V>
    where
        V: Load<'a>,
    {
        Values::new(&self.root, K::BITS)
    }

    /// Returns the lowest key and a value corresponding to the key.
    pub fn get_min<'a>(&'a self, signed: bool) -> Result<Option<(K, V)>, Error>
    where
        K: LoadDictKey,
        V: Load<'a>,
    {
        Ok(match ok!(self.get_bound_raw(DictBound::Min, signed)) {
            Some((key, ref mut value)) => Some((key, ok!(V::load_from(value)))),
            None => None,
        })
    }

    /// Returns the lowest key and a value corresponding to the key.
    pub fn get_max<'a>(&'a self, signed: bool) -> Result<Option<(K, V)>, Error>
    where
        K: LoadDictKey,
        V: Load<'a>,
    {
        Ok(match ok!(self.get_bound_raw(DictBound::Max, signed)) {
            Some((key, ref mut value)) => Some((key, ok!(V::load_from(value)))),
            None => None,
        })
    }

    /// Finds the specified dict bound and returns a key and a raw value corresponding to the key.
    pub fn get_bound_raw(
        &self,
        bound: DictBound,
        signed: bool,
    ) -> Result<Option<(K, CellSlice<'_>)>, Error>
    where
        K: LoadDictKey,
    {
        let Some((key, value)) = ok!(dict_find_bound(
            self.root.as_ref(),
            K::BITS,
            bound,
            signed,
            Cell::empty_context()
        )) else {
            return Ok(None);
        };
        match K::load_from_data(&key) {
            Some(key) => Ok(Some((key, value))),
            None => Err(Error::CellUnderflow),
        }
    }

    /// Removes the specified dict bound.
    /// Returns an optional removed key and value as cell slice parts.
    ///
    /// Key and dict are serialized using the provided cell context.
    pub fn remove_bound_raw_ext(
        &mut self,
        bound: DictBound,
        signed: bool,
        context: &dyn CellContext,
    ) -> Result<Option<(K, CellSliceParts)>, Error>
    where
        K: LoadDictKey,
    {
        let removed = ok!(dict_remove_bound_owned(
            &mut self.root,
            K::BITS,
            bound,
            signed,
            context
        ));
        Ok(match removed {
            Some((key, value)) => match K::load_from_data(&key) {
                Some(key) => Some((key, value)),
                None => return Err(Error::CellUnderflow),
            },
            None => None,
        })
    }
}

impl<K, V> Dict<K, V>
where
    K: StoreDictKey,
{
    /// Removes the value associated with key in dictionary.
    /// Returns an optional removed value as cell slice parts.
    ///
    /// Dict is rebuild using the provided cell context.
    #[inline]
    pub fn remove_raw_ext<Q>(
        &mut self,
        key: Q,
        context: &dyn CellContext,
    ) -> Result<Option<CellSliceParts>, Error>
    where
        Q: Borrow<K>,
    {
        fn remove_raw_ext_impl<K>(
            root: &mut Option<Cell>,
            key: &K,
            context: &dyn CellContext,
        ) -> Result<Option<CellSliceParts>, Error>
        where
            K: StoreDictKey,
        {
            let mut builder = CellDataBuilder::new();
            ok!(key.store_into_data(&mut builder));
            dict_remove_owned(root, &mut builder.as_data_slice(), K::BITS, false, context)
        }

        remove_raw_ext_impl(&mut self.root, key.borrow(), context)
    }

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
    /// [`values`]: Dict::values
    /// [`raw_values`]: Dict::raw_values
    pub fn raw_iter(&'_ self) -> RawIter<'_> {
        RawIter::new(&self.root, K::BITS)
    }

    /// Gets an iterator over the raw entries of two dictionaries, sorted by key.
    /// The iterator element type is `Result<(CellBuilder, Option<CellSlice>, Option<CellSlice>)>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over dictionary builds a key
    /// for each element.
    pub fn raw_iter_union<'a>(&'a self, other: &'a Self) -> UnionRawIter<'a> {
        UnionRawIter::new(&self.root, &other.root, K::BITS)
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
    /// [`values`]: Dict::values
    /// [`raw_values`]: Dict::raw_values
    pub fn raw_keys(&'_ self) -> RawKeys<'_> {
        RawKeys::new(&self.root, K::BITS)
    }
}

impl<K, V> Dict<K, V>
where
    K: DictKey,
{
    /// Gets an iterator over the raw values of the dictionary, in order by key.
    /// The iterator element type is `Result<CellSlice>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn raw_values(&'_ self) -> RawValues<'_> {
        RawValues::new(&self.root, K::BITS)
    }
}

impl<K, V> Dict<K, V>
where
    K: StoreDictKey,
    V: Store,
{
    /// Sets the value associated with the key in the dictionary.
    pub fn set_ext<Q, T>(
        &mut self,
        key: Q,
        value: T,
        context: &dyn CellContext,
    ) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        T: Borrow<V>,
    {
        self.insert_impl(key.borrow(), value.borrow(), SetMode::Set, context)
    }

    /// Sets the value associated with the key in the dictionary
    /// only if the key was already present in it.
    pub fn replace_ext<Q, T>(
        &mut self,
        key: Q,
        value: T,
        context: &dyn CellContext,
    ) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        T: Borrow<V>,
    {
        self.insert_impl(key.borrow(), value.borrow(), SetMode::Replace, context)
    }

    /// Sets the value associated with key in dictionary,
    /// but only if it is not already present.
    pub fn add_ext<Q, T>(
        &mut self,
        key: Q,
        value: T,
        context: &dyn CellContext,
    ) -> Result<bool, Error>
    where
        Q: Borrow<K>,
        T: Borrow<V>,
    {
        self.insert_impl(key.borrow(), value.borrow(), SetMode::Add, context)
    }

    fn insert_impl(
        &mut self,
        key: &K,
        value: &V,
        mode: SetMode,
        context: &dyn CellContext,
    ) -> Result<bool, Error> {
        let mut key_builder = CellDataBuilder::new();
        ok!(key.store_into_data(&mut key_builder));
        dict_insert(
            &mut self.root,
            &mut key_builder.as_data_slice(),
            K::BITS,
            value,
            mode,
            context,
        )
    }
}

#[cfg(feature = "serde")]
impl<K, V> serde::Serialize for Dict<K, V>
where
    K: serde::Serialize + LoadDictKey,
    for<'a> V: serde::Serialize + Load<'a>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::{Error, SerializeMap};

        if serializer.is_human_readable() {
            let mut ser = serializer.serialize_map(None)?;
            for ref entry in self.iter() {
                let (key, value) = match entry {
                    Ok(entry) => entry,
                    Err(e) => return Err(Error::custom(e)),
                };
                ok!(ser.serialize_entry(key, value));
            }
            ser.end()
        } else {
            crate::boc::BocRepr::serialize(self, serializer)
        }
    }
}

#[cfg(feature = "serde")]
impl<'de, K, V> serde::Deserialize<'de> for Dict<K, V>
where
    K: serde::Deserialize<'de> + std::hash::Hash + Eq + StoreDictKey,
    V: serde::Deserialize<'de> + Store,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let map = ok!(ahash::HashMap::<K, V>::deserialize(deserializer));

            // TODO: Build from sorted collection
            let cx = Cell::empty_context();
            let mut dict = Dict::new();
            for (key, value) in map {
                if let Err(e) = dict.set_ext(key, value, cx) {
                    return Err(serde::de::Error::custom(e));
                }
            }
            Ok(dict)
        } else {
            crate::boc::BocRepr::deserialize(deserializer)
        }
    }
}

/// An iterator over the entries of a [`Dict`].
///
/// This struct is created by the [`iter`] method on [`Dict`]. See its documentation for more.
///
/// [`iter`]: Dict::iter
pub struct Iter<'a, K, V> {
    inner: RawIter<'a>,
    _key: PhantomData<K>,
    _value: PhantomData<V>,
}

impl<K, V> Clone for Iter<'_, K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _key: PhantomData,
            _value: PhantomData,
        }
    }
}

impl<'a, K, V> Iter<'a, K, V>
where
    K: DictKey,
{
    /// Creates an iterator over the entries of a dictionary.
    pub fn new(root: &'a Option<Cell>) -> Self {
        Self {
            inner: RawIter::new(root, K::BITS),
            _key: PhantomData,
            _value: PhantomData,
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

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    K: LoadDictKey,
    V: Load<'a>,
{
    type Item = Result<(K, V), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.inner.next()? {
            Ok((key, mut value)) => {
                let err = if let Some(key) = K::load_from_data(&key) {
                    match V::load_from(&mut value) {
                        Ok(value) => return Some(Ok((key, value))),
                        Err(e) => e,
                    }
                } else {
                    Error::CellUnderflow
                };
                Err(self.inner.finish(err))
            }
            Err(e) => Err(e),
        })
    }
}

/// An iterator over the entries across two [`Dict`].
///
/// This struct is created by the [`iter_union`] method on [`Dict`].
///
/// [`iter_union`]: Dict::iter_union
pub struct UnionIter<'a, K, V> {
    inner: UnionRawIter<'a>,
    _key: PhantomData<K>,
    _value: PhantomData<V>,
}

impl<'a, K, V> UnionIter<'a, K, V>
where
    K: DictKey,
{
    /// Creates an iterator over the entries of a dictionary.
    pub fn new(left_root: &'a Option<Cell>, right_root: &'a Option<Cell>) -> Self {
        Self {
            inner: UnionRawIter::new(left_root, right_root, K::BITS),
            _key: PhantomData,
            _value: PhantomData,
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

impl<'a, K, V> Iterator for UnionIter<'a, K, V>
where
    K: LoadDictKey,
    V: Load<'a>,
{
    type Item = Result<(K, Option<V>, Option<V>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        fn load_opt_value<'a, V: Load<'a>>(
            value: &mut Option<CellSlice<'a>>,
        ) -> Result<Option<V>, Error> {
            match value {
                Some(value) => match V::load_from(value) {
                    Ok(value) => Ok(Some(value)),
                    Err(e) => Err(e),
                },
                None => Ok(None),
            }
        }

        Some(match self.inner.next()? {
            Ok((key, mut left_value, mut right_value)) => {
                let err = if let Some(key) = K::load_from_data(&key) {
                    match (
                        load_opt_value(&mut left_value),
                        load_opt_value(&mut right_value),
                    ) {
                        (Ok(left), Ok(right)) => return Some(Ok((key, left, right))),
                        (Err(e), _) => e,
                        (_, Err(e)) => e,
                    }
                } else {
                    Error::CellUnderflow
                };
                Err(self.inner.finish(err))
            }
            Err(e) => Err(e),
        })
    }
}

/// An iterator over the keys of a [`Dict`].
///
/// This struct is created by the [`keys`] method on [`Dict`]. See its
/// documentation for more.
///
/// [`keys`]: Dict::keys
pub struct Keys<'a, K> {
    inner: RawIter<'a>,
    _key: PhantomData<K>,
}

impl<K> Clone for Keys<'_, K> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _key: PhantomData,
        }
    }
}

impl<'a, K> Keys<'a, K>
where
    K: DictKey,
{
    /// Creates an iterator over the keys of a dictionary.
    pub fn new(root: &'a Option<Cell>) -> Self {
        Self {
            inner: RawIter::new(root, K::BITS),
            _key: PhantomData,
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

impl<K> Iterator for Keys<'_, K>
where
    K: LoadDictKey,
{
    type Item = Result<K, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.inner.next()? {
            Ok((key, _)) => match K::load_from_data(&key) {
                Some(key) => Ok(key),
                None => Err(self.inner.finish(Error::CellUnderflow)),
            },
            Err(e) => Err(e),
        })
    }
}

/// An iterator over the values of a [`Dict`].
///
/// This struct is created by the [`values`] method on [`Dict`]. See its documentation for more.
///
/// [`values`]: Dict::values
pub struct Values<'a, V> {
    inner: RawValues<'a>,
    _value: PhantomData<V>,
}

impl<V> Clone for Values<'_, V> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            _value: PhantomData,
        }
    }
}

impl<'a, V> Values<'a, V> {
    /// Creates an iterator over the values of a dictionary.
    pub fn new(root: &'a Option<Cell>, bit_len: u16) -> Self {
        Self {
            inner: RawValues::new(root, bit_len),
            _value: PhantomData,
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

impl<'a, V> Iterator for Values<'a, V>
where
    V: Load<'a>,
{
    type Item = Result<V, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Ok(mut value) => match V::load_from(&mut value) {
                Ok(value) => Some(Ok(value)),
                Err(e) => Some(Err(self.inner.finish(e))),
            },
            Err(e) => Some(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Context;

    use super::*;
    use crate::prelude::*;

    #[test]
    fn dict_set() {
        let mut dict = Dict::<u32, u16>::new();
        assert!(dict.set(123, 0xffff).unwrap());
        assert_eq!(dict.get(123).unwrap(), Some(0xffff));

        assert!(dict.set(123, 0xcafe).unwrap());
        assert_eq!(dict.get(123).unwrap(), Some(0xcafe));
    }

    #[test]
    fn dict_get_with_value_refs() -> anyhow::Result<()> {
        let mut dict = Dict::<u32, (Cell, Cell)>::new();
        for i in 0..10 {
            if i == 5 {
                continue;
            }
            dict.set(i, (Cell::empty_cell(), Cell::empty_cell()))?;
        }

        assert_eq!(dict.get(5).unwrap(), None);
        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore)] // takes too long to execute on miri
    fn dict_set_complex() {
        let mut dict = Dict::<u32, bool>::new();
        for i in 0..520 {
            assert!(dict.set(i, true).unwrap());
        }
    }

    #[test]
    fn dict_bounds() {
        let mut dict = Dict::<i32, bool>::new();
        for i in -10..=10 {
            assert!(dict.set(i, i < 0).unwrap());
        }

        assert_eq!(dict.get_min(false).unwrap(), Some((0, false)));
        assert_eq!(dict.get_max(false).unwrap(), Some((-1, true)));

        assert_eq!(dict.get_min(true).unwrap(), Some((-10, true)));
        assert_eq!(dict.get_max(true).unwrap(), Some((10, false)));

        let mut dict = Dict::<u32, u8>::new();
        for i in 1..=3 {
            dict.set(i, 0xff).unwrap();
        }
        assert_eq!(dict.get_min(false).unwrap(), Some((1, 0xff)));
        assert_eq!(dict.get_max(false).unwrap(), Some((3, 0xff)));
    }

    #[test]
    fn dict_remove_bounds() {
        let mut dict = Dict::<i32, bool>::new();
        for i in -10..=10 {
            dict.set(i, i < 0).unwrap();
        }

        let parse_removed = |(i, parts): (i32, CellSliceParts)| {
            let mut value = CellSlice::apply(&parts)?;
            let value = bool::load_from(&mut value)?;
            Ok::<_, Error>((i, value))
        };

        // Min, unsigned
        {
            let mut dict = dict.clone();
            for i in 0..=10 {
                let removed = dict.remove_min_raw(false).unwrap().unwrap();
                assert_eq!(parse_removed(removed).unwrap(), (i, false));
            }
            for i in -10..=-1 {
                let removed = dict.remove_min_raw(false).unwrap().unwrap();
                assert_eq!(parse_removed(removed).unwrap(), (i, true));
            }
            assert!(dict.is_empty());
        }

        // Min, signed
        {
            let mut dict = dict.clone();
            for i in -10..=10 {
                let removed = dict.remove_min_raw(true).unwrap().unwrap();
                assert_eq!(parse_removed(removed).unwrap(), (i, i < 0));
            }
            assert!(dict.is_empty());
        }

        // Max, unsigned
        {
            let mut dict = dict.clone();
            for i in (-10..=-1).rev() {
                let removed = dict.remove_max_raw(false).unwrap().unwrap();
                assert_eq!(parse_removed(removed).unwrap(), (i, true));
            }
            for i in (0..=10).rev() {
                let removed = dict.remove_max_raw(false).unwrap().unwrap();
                assert_eq!(parse_removed(removed).unwrap(), (i, false));
            }
            assert!(dict.is_empty());
        }

        // Max, signed
        {
            let mut dict = dict.clone();
            for i in (-10..=10).rev() {
                let removed = dict.remove_max_raw(true).unwrap().unwrap();
                assert_eq!(parse_removed(removed).unwrap(), (i, i < 0));
            }
            assert!(dict.is_empty());
        }
    }

    #[test]
    fn dict_replace() {
        let mut dict = Dict::<u32, bool>::new();
        assert!(!dict.replace(123, false).unwrap());
        assert!(!dict.contains_key(123).unwrap());

        assert!(dict.set(123, false).unwrap());
        assert_eq!(dict.get(123).unwrap(), Some(false));
        assert!(dict.replace(123, true).unwrap());
        assert_eq!(dict.get(123).unwrap(), Some(true));
    }

    #[test]
    fn dict_add() {
        let mut dict = Dict::<u32, bool>::new();

        assert!(dict.add(123, false).unwrap());
        assert_eq!(dict.get(123).unwrap(), Some(false));

        assert!(!dict.add(123, true).unwrap());
        assert_eq!(dict.get(123).unwrap(), Some(false));
    }

    #[test]
    fn dict_remove() {
        let mut dict = Dict::<u32, u32>::new();

        for i in 0..10 {
            assert!(dict.set(i, i).unwrap());
        }

        let mut check_remove = |n: u32, expected: Option<u32>| -> anyhow::Result<()> {
            let removed = dict.remove(n).context("Failed to remove")?;
            anyhow::ensure!(removed == expected);
            Ok(())
        };

        check_remove(0, Some(0)).unwrap();

        check_remove(4, Some(4)).unwrap();

        check_remove(9, Some(9)).unwrap();
        check_remove(9, None).unwrap();

        check_remove(5, Some(5)).unwrap();
        check_remove(5, None).unwrap();

        check_remove(100, None).unwrap();

        check_remove(1, Some(1)).unwrap();
        check_remove(2, Some(2)).unwrap();
        check_remove(3, Some(3)).unwrap();
        check_remove(6, Some(6)).unwrap();
        check_remove(7, Some(7)).unwrap();
        check_remove(8, Some(8)).unwrap();

        assert!(dict.is_empty());
    }

    #[test]
    fn dict_iter() {
        let boc = Boc::decode_base64("te6ccgEBFAEAeAABAcABAgPOQAUCAgHUBAMACQAAAI3gAAkAAACjoAIBIA0GAgEgCgcCASAJCAAJAAAAciAACQAAAIfgAgEgDAsACQAAAFZgAAkAAABsIAIBIBEOAgEgEA8ACQAAADqgAAkAAABQYAIBIBMSAAkAAAAe4AAJAAAAv2A=").unwrap();
        let dict = boc.parse::<Dict<u32, u32>>().unwrap();

        let size = dict.values().count();
        assert_eq!(size, 10);

        for (i, entry) in dict.iter().enumerate() {
            let (key, _) = entry.unwrap();
            assert_eq!(key, i as u32);
        }

        let signed_range = -10..10;

        let mut dict = Dict::<i32, i32>::new();
        for i in signed_range.clone() {
            assert!(dict.set(i, i).unwrap());
        }

        let mut signed_range_iter = signed_range.clone();
        for (entry, i) in dict.iter().signed().zip(&mut signed_range_iter) {
            let (key, value) = entry.unwrap();
            assert_eq!(key, i);
            assert_eq!(value, i);
        }
        assert_eq!(signed_range_iter.next(), None);

        let mut signed_range_iter = signed_range.rev();
        for (entry, i) in dict.iter().reversed().signed().zip(&mut signed_range_iter) {
            let (key, value) = entry.unwrap();
            assert_eq!(key, i);
            assert_eq!(value, i);
        }
        assert_eq!(signed_range_iter.next(), None);
    }

    #[test]
    fn dict_next_prev_unsigned() {
        let mut dict = Dict::<u32, u32>::new();

        for i in 0..=10 {
            dict.set(i, i).unwrap();
        }

        for i in 20..=30 {
            dict.set(i, i).unwrap();
        }

        println!("{}", BocRepr::encode_base64(&dict).unwrap());

        assert_eq!(dict.get_prev(0, false).unwrap(), None);
        assert_eq!(dict.get_or_prev(0, false).unwrap(), Some((0, 0)));

        assert_eq!(dict.get_next(30, false).unwrap(), None);
        assert_eq!(dict.get_or_next(30, false).unwrap(), Some((30, 30)));

        assert_eq!(dict.get_prev(15, false).unwrap(), Some((10, 10)));
        assert_eq!(dict.get_or_prev(15, false).unwrap(), Some((10, 10)));

        assert_eq!(dict.get_next(15, false).unwrap(), Some((20, 20)));
        assert_eq!(dict.get_or_next(15, false).unwrap(), Some((20, 20)));

        assert_eq!(dict.get_next(19, false).unwrap(), Some((20, 20)));
        assert_eq!(dict.get_or_next(19, false).unwrap(), Some((20, 20)));

        assert_eq!(dict.get_prev(20, false).unwrap(), Some((10, 10)));
        assert_eq!(dict.get_or_prev(20, false).unwrap(), Some((20, 20)));

        assert_eq!(dict.get_next(100, false).unwrap(), None);
        assert_eq!(dict.get_or_next(100, false).unwrap(), None);

        assert_eq!(dict.get_prev(100, false).unwrap(), Some((30, 30)));
        assert_eq!(dict.get_or_prev(100, false).unwrap(), Some((30, 30)));
    }

    #[test]
    fn dict_next_prev_signed() {
        let mut dict = Dict::<i32, i32>::new();

        for i in -20..=-10 {
            dict.set(i, i).unwrap();
        }

        assert_eq!(dict.get_prev(-20, true).unwrap(), None);
        assert_eq!(dict.get_or_prev(-20, true).unwrap(), Some((-20, -20)));

        assert_eq!(dict.get_next(-10, true).unwrap(), None);
        assert_eq!(dict.get_or_next(-10, true).unwrap(), Some((-10, -10)));

        for i in 10..=20 {
            dict.set(i, i).unwrap();
        }

        println!("{}", BocRepr::encode_base64(&dict).unwrap());

        assert_eq!(dict.get_next(-100, true).unwrap(), Some((-20, -20)));
        assert_eq!(dict.get_or_next(-100, true).unwrap(), Some((-20, -20)));

        assert_eq!(dict.get_prev(-100, true).unwrap(), None);
        assert_eq!(dict.get_or_prev(-100, true).unwrap(), None);

        assert_eq!(dict.get_prev(-20, true).unwrap(), None);
        assert_eq!(dict.get_or_prev(-20, true).unwrap(), Some((-20, -20)));

        assert_eq!(dict.get_next(20, true).unwrap(), None);
        assert_eq!(dict.get_or_next(20, true).unwrap(), Some((20, 20)));

        assert_eq!(dict.get_prev(-10, true).unwrap(), Some((-11, -11)));
        assert_eq!(dict.get_or_prev(-10, true).unwrap(), Some((-10, -10)));

        assert_eq!(dict.get_next(-10, true).unwrap(), Some((10, 10)));
        assert_eq!(dict.get_or_next(-10, true).unwrap(), Some((-10, -10)));

        assert_eq!(dict.get_prev(-9, true).unwrap(), Some((-10, -10)));
        assert_eq!(dict.get_or_prev(-9, true).unwrap(), Some((-10, -10)));

        assert_eq!(dict.get_prev(0, true).unwrap(), Some((-10, -10)));
        assert_eq!(dict.get_or_prev(0, true).unwrap(), Some((-10, -10)));

        assert_eq!(dict.get_next(0, true).unwrap(), Some((10, 10)));
        assert_eq!(dict.get_or_next(0, true).unwrap(), Some((10, 10)));

        assert_eq!(dict.get_prev(10, true).unwrap(), Some((-10, -10)));
        assert_eq!(dict.get_or_prev(10, true).unwrap(), Some((10, 10)));

        assert_eq!(dict.get_next(100, true).unwrap(), None);
        assert_eq!(dict.get_or_next(100, true).unwrap(), None);

        assert_eq!(dict.get_prev(100, true).unwrap(), Some((20, 20)));
        assert_eq!(dict.get_or_prev(100, true).unwrap(), Some((20, 20)));

        // All negative
        let mut dict = Dict::<i32, i32>::new();
        for i in -10..=-5 {
            dict.set(i, i).unwrap();
        }

        assert_eq!(dict.get_prev(-20, true).unwrap(), None);
        assert_eq!(dict.get_or_prev(-20, true).unwrap(), None);
        assert_eq!(dict.get_prev(-10, true).unwrap(), None);
        assert_eq!(dict.get_or_prev(-10, true).unwrap(), Some((-10, -10)));

        assert_eq!(dict.get_next(-20, true).unwrap(), Some((-10, -10)));
        assert_eq!(dict.get_or_next(-20, true).unwrap(), Some((-10, -10)));
        assert_eq!(dict.get_next(-10, true).unwrap(), Some((-9, -9)));
        assert_eq!(dict.get_or_next(-10, true).unwrap(), Some((-10, -10)));

        assert_eq!(dict.get_prev(-7, true).unwrap(), Some((-8, -8)));
        assert_eq!(dict.get_or_prev(-7, true).unwrap(), Some((-7, -7)));
        assert_eq!(dict.get_next(-7, true).unwrap(), Some((-6, -6)));
        assert_eq!(dict.get_or_next(-7, true).unwrap(), Some((-7, -7)));

        assert_eq!(dict.get_prev(-5, true).unwrap(), Some((-6, -6)));
        assert_eq!(dict.get_or_prev(-5, true).unwrap(), Some((-5, -5)));
        assert_eq!(dict.get_prev(-4, true).unwrap(), Some((-5, -5)));
        assert_eq!(dict.get_or_prev(-4, true).unwrap(), Some((-5, -5)));

        assert_eq!(dict.get_next(-5, true).unwrap(), None);
        assert_eq!(dict.get_or_next(-5, true).unwrap(), Some((-5, -5)));
        assert_eq!(dict.get_next(-4, true).unwrap(), None);
        assert_eq!(dict.get_or_next(-4, true).unwrap(), None);

        assert_eq!(dict.get_next(0, true).unwrap(), None);
        assert_eq!(dict.get_or_next(0, true).unwrap(), None);
        assert_eq!(dict.get_next(1, true).unwrap(), None);
        assert_eq!(dict.get_or_next(1, true).unwrap(), None);

        // All positive
        let mut dict = Dict::<i32, i32>::new();
        for i in 5..=10 {
            dict.set(i, i).unwrap();
        }

        assert_eq!(dict.get_prev(-1, true).unwrap(), None);
        assert_eq!(dict.get_or_prev(-1, true).unwrap(), None);
        assert_eq!(dict.get_prev(0, true).unwrap(), None);
        assert_eq!(dict.get_or_prev(0, true).unwrap(), None);

        assert_eq!(dict.get_next(4, true).unwrap(), Some((5, 5)));
        assert_eq!(dict.get_or_next(4, true).unwrap(), Some((5, 5)));
        assert_eq!(dict.get_next(5, true).unwrap(), Some((6, 6)));
        assert_eq!(dict.get_or_next(5, true).unwrap(), Some((5, 5)));

        assert_eq!(dict.get_prev(7, true).unwrap(), Some((6, 6)));
        assert_eq!(dict.get_or_prev(7, true).unwrap(), Some((7, 7)));
        assert_eq!(dict.get_next(7, true).unwrap(), Some((8, 8)));
        assert_eq!(dict.get_or_next(7, true).unwrap(), Some((7, 7)));

        assert_eq!(dict.get_prev(10, true).unwrap(), Some((9, 9)));
        assert_eq!(dict.get_or_prev(10, true).unwrap(), Some((10, 10)));
        assert_eq!(dict.get_prev(11, true).unwrap(), Some((10, 10)));
        assert_eq!(dict.get_or_prev(11, true).unwrap(), Some((10, 10)));

        assert_eq!(dict.get_next(10, true).unwrap(), None);
        assert_eq!(dict.get_or_next(10, true).unwrap(), Some((10, 10)));
        assert_eq!(dict.get_next(11, true).unwrap(), None);
        assert_eq!(dict.get_or_next(11, true).unwrap(), None);

        assert_eq!(dict.get_prev(20, true).unwrap(), Some((10, 10)));
        assert_eq!(dict.get_or_prev(20, true).unwrap(), Some((10, 10)));
        assert_eq!(dict.get_next(20, true).unwrap(), None);
        assert_eq!(dict.get_or_next(20, true).unwrap(), None);

        // Single positive on edge
        let mut dict = Dict::<i32, i32>::new();
        dict.set(0, 0).unwrap();

        assert_eq!(dict.get_prev(0, true).unwrap(), None);
        assert_eq!(dict.get_or_prev(0, true).unwrap(), Some((0, 0)));
        assert_eq!(dict.get_next(-1, true).unwrap(), Some((0, 0)));
        assert_eq!(dict.get_or_next(-1, true).unwrap(), Some((0, 0)));

        // Single negative on edge
        let mut dict = Dict::<i32, i32>::new();
        dict.set(-1, -1).unwrap();

        assert_eq!(dict.get_prev(0, true).unwrap(), Some((-1, -1)));
        assert_eq!(dict.get_or_prev(0, true).unwrap(), Some((-1, -1)));
        assert_eq!(dict.get_next(-1, true).unwrap(), None);
        assert_eq!(dict.get_or_next(-1, true).unwrap(), Some((-1, -1)));
    }

    #[test]
    fn dict_same_after_remove() -> anyhow::Result<()> {
        let mut dict = Dict::<i32, i32>::new();
        dict.set(-1, 1)?;
        dict.set(-2, 2)?;

        let removed = dict.remove(-2).unwrap();
        assert_eq!(removed, Some(2));

        let mut dict2 = Dict::<i32, i32>::new();
        dict2.set(-1, 1)?;

        assert_eq!(dict, dict2);
        Ok(())
    }

    #[test]
    fn get_signed_next() {
        let cell = Boc::decode_base64("te6ccgEBCwEAaAACAskDAQIBIAQCAgHOCAgCASAEBAIBIAUFAgEgBgYCASAHBwIBIAgIAgEgCQkBAwDgCgBoQgBAJTazb04k/ooV5DE4d+ixdwixajACdzkuZVb6ymgnqyHc1lAAAAAAAAAAAAAAAAAAAA==").unwrap();
        let dict = Dict::<i16, Cell>::from_raw(Some(cell));

        for item in dict.iter() {
            let (key, cell) = item.unwrap();
            println!("{key}, {}", cell.display_root());
        }

        let res = dict.get_next(-1, true).unwrap();
        println!("{res:?}");
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn big_dict() {
        use rand9::{Rng, SeedableRng};

        let mut rng = rand_xorshift::XorShiftRng::from_seed([0u8; 16]);

        let values = (0..100000)
            .map(|_| (rng.random::<u32>(), rng.random::<u64>()))
            .collect::<Vec<_>>();

        // Wrap builder into a new function for the flamegraph
        #[inline(never)]
        fn test_big_dict(values: &[(u32, u64)]) -> Dict<u32, u64> {
            let mut result = Dict::<u32, u64>::new();
            for (key, value) in values {
                result.set(key, value).unwrap();
            }
            result
        }

        test_big_dict(&values);
    }

    #[test]
    fn dict_iter_union() -> anyhow::Result<()> {
        let mut left = Dict::<i32, i32>::new();
        let mut right = Dict::<i32, i32>::new();

        // Fill
        for i in -4i32..4 {
            left.set(i, i)?;
        }
        for i in -2i32..6 {
            right.set(i, i + 100)?;
        }

        fn compare_iter_values(
            iter: UnionIter<'_, i32, i32>,
            values: &[(i32, Option<i32>, Option<i32>)],
        ) {
            let mut values = values.iter();

            for entry in iter {
                let (key, left_value, right_value) = entry.unwrap();
                assert_eq!(values.next(), Some(&(key, left_value, right_value)));
            }
            assert_eq!(values.next(), None);
        }

        // Unsigned
        compare_iter_values(left.iter_union(&right), &[
            (0, Some(0), Some(100)),
            (1, Some(1), Some(101)),
            (2, Some(2), Some(102)),
            (3, Some(3), Some(103)),
            (4, None, Some(104)),
            (5, None, Some(105)),
            (-4, Some(-4), None),
            (-3, Some(-3), None),
            (-2, Some(-2), Some(98)),
            (-1, Some(-1), Some(99)),
        ]);

        // Unsigned reversed
        compare_iter_values(left.iter_union(&right).reversed(), &[
            (-1, Some(-1), Some(99)),
            (-2, Some(-2), Some(98)),
            (-3, Some(-3), None),
            (-4, Some(-4), None),
            (5, None, Some(105)),
            (4, None, Some(104)),
            (3, Some(3), Some(103)),
            (2, Some(2), Some(102)),
            (1, Some(1), Some(101)),
            (0, Some(0), Some(100)),
        ]);

        // Signed
        compare_iter_values(left.iter_union(&right).signed(), &[
            (-4, Some(-4), None),
            (-3, Some(-3), None),
            (-2, Some(-2), Some(98)),
            (-1, Some(-1), Some(99)),
            (0, Some(0), Some(100)),
            (1, Some(1), Some(101)),
            (2, Some(2), Some(102)),
            (3, Some(3), Some(103)),
            (4, None, Some(104)),
            (5, None, Some(105)),
        ]);

        // Signed reversed
        compare_iter_values(left.iter_union(&right).signed().reversed(), &[
            (5, None, Some(105)),
            (4, None, Some(104)),
            (3, Some(3), Some(103)),
            (2, Some(2), Some(102)),
            (1, Some(1), Some(101)),
            (0, Some(0), Some(100)),
            (-1, Some(-1), Some(99)),
            (-2, Some(-2), Some(98)),
            (-3, Some(-3), None),
            (-4, Some(-4), None),
        ]);

        Ok(())
    }

    #[test]
    fn split_merge_test() -> anyhow::Result<()> {
        let mut dict = Dict::<u32, u32>::new();

        dict.add(1, 1)?;
        dict.add(2, 2)?;
        dict.add(3, 3)?;
        dict.add(u32::MAX - 2, 4)?;
        dict.add(u32::MAX - 1, 5)?;
        dict.add(u32::MAX, 6)?;

        let (d1, d2) = dict.split()?;
        let merged = d1.merge_with_right_sibling(&d2)?;

        for i in merged.iter() {
            let _ = i?;
        }

        assert_eq!(dict, merged);

        Ok(())
    }
}
