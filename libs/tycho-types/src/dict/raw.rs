use std::collections::BTreeMap;

use super::{
    DictBound, DictOwnedEntry, SetMode, StoreDictKey, build_dict_from_sorted_iter, dict_find_bound,
    dict_find_bound_owned, dict_find_owned, dict_get, dict_get_owned, dict_get_subdict,
    dict_insert, dict_load_from_root, dict_remove_bound_owned, dict_remove_owned,
    dict_split_by_prefix, read_label,
};
use crate::cell::*;
use crate::error::Error;
use crate::util::{IterStatus, unlikely};

/// Dictionary with fixed length keys (where `N` is a number of bits in each key).
///
/// # TLB scheme
///
/// ```text
/// // ordinary Hashmap / HashmapE, with fixed length keys
///
/// hm_edge#_ {n:#} {X:Type} {l:#} {m:#} label:(HmLabel ~l n)
///           {n = (~m) + l} node:(HashmapNode m X) = Hashmap n X;
///
/// hmn_leaf#_ {X:Type} value:X = HashmapNode 0 X;
/// hmn_fork#_ {n:#} {X:Type} left:^(Hashmap n X)
///            right:^(Hashmap n X) = HashmapNode (n + 1) X;
///
/// hml_short$0 {m:#} {n:#} len:(Unary ~n) {n <= m} s:(n * Bit) = HmLabel ~n m;
/// hml_long$10 {m:#} n:(#<= m) s:(n * Bit) = HmLabel ~n m;
/// hml_same$11 {m:#} v:Bit n:(#<= m) = HmLabel ~n m;
///
/// hme_empty$0 {n:#} {X:Type} = HashmapE n X;
/// hme_root$1 {n:#} {X:Type} root:^(Hashmap n X) = HashmapE n X;
///
/// unary_zero$0 = Unary ~0;
/// unary_succ$1 {n:#} x:(Unary ~n) = Unary ~(n + 1);
///
/// bit$_ (## 1) = Bit;
/// ```
pub struct RawDict<const N: u16>(pub(crate) Option<Cell>);

impl<const N: u16> ExactSize for RawDict<N> {
    #[inline]
    fn exact_size(&self) -> Size {
        Size {
            bits: 1,
            refs: self.0.is_some() as u8,
        }
    }
}

impl<'a, const N: u16> Load<'a> for RawDict<N> {
    #[inline]
    fn load_from(slice: &mut CellSlice<'a>) -> Result<Self, Error> {
        match <_>::load_from(slice) {
            Ok(dict) => Ok(Self(dict)),
            Err(e) => Err(e),
        }
    }
}

impl<const N: u16> Store for RawDict<N> {
    #[inline]
    fn store_into(
        &self,
        builder: &mut CellBuilder,
        context: &dyn CellContext,
    ) -> Result<(), Error> {
        self.0.store_into(builder, context)
    }
}

impl<const N: u16> Default for RawDict<N> {
    #[inline]
    fn default() -> Self {
        Self(None)
    }
}

impl<const N: u16> Clone for RawDict<N> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<const N: u16> Eq for RawDict<N> {}
impl<const N: u16> PartialEq for RawDict<N> {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(this), Some(other)) => this.as_ref() == other.as_ref(),
            (None, None) => true,
            _ => false,
        }
    }
}

impl<const N: u16> From<Option<Cell>> for RawDict<N> {
    #[inline]
    fn from(value: Option<Cell>) -> Self {
        Self(value)
    }
}

impl<const N: u16> std::fmt::Debug for RawDict<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawDict")
            .field("key_bit_len", &N)
            .field("root", &self.0)
            .finish()
    }
}

impl<const N: u16> RawDict<N> {
    const _ASSERT: () = assert!(N > 0, "Dict with 0-bit key is invalid");

    /// Creates an empty dictionary.
    pub const fn new() -> Self {
        Self(None)
    }

    /// Builds a dictionary from a sorted collection.
    pub fn try_from_btree<K, V>(sorted: &BTreeMap<K, V>) -> Result<Self, Error>
    where
        K: StoreDictKey + Ord,
        V: Store,
    {
        assert_eq!(K::BITS, N);
        let root = ok!(build_dict_from_sorted_iter(sorted, Cell::empty_context()));
        Ok(Self(root))
    }

    /// Builds a dictionary from a sorted slice.
    pub fn try_from_sorted_slice<K, V>(sorted: &[(K, V)]) -> Result<Self, Error>
    where
        K: StoreDictKey + Ord,
        V: Store,
    {
        assert_eq!(K::BITS, N);
        let root = ok!(build_dict_from_sorted_iter(
            sorted.iter().map(|(k, v)| (k, v)),
            Cell::empty_context()
        ));
        Ok(Self(root))
    }

    /// Returns `true` if the dictionary contains no elements.
    pub const fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    /// Returns the underlying root cell of the dictionary.
    #[inline]
    pub const fn root(&self) -> &Option<Cell> {
        &self.0
    }

    /// Returns the underlying root cell of the dictionary.
    #[inline]
    pub fn into_root(self) -> Option<Cell> {
        self.0
    }

    /// Loads a non-empty dictionary from a root cell.
    #[inline]
    pub fn load_from_root_ext(
        slice: &mut CellSlice<'_>,
        context: &dyn CellContext,
    ) -> Result<Self, Error> {
        match dict_load_from_root(slice, N, context) {
            Ok(root) => Ok(Self(Some(root))),
            Err(e) => Err(e),
        }
    }

    /// Returns a `CellSlice` of the value corresponding to the key.
    ///
    /// NOTE: Uses the default cell context.
    pub fn get<'a>(&'a self, key: CellSlice<'_>) -> Result<Option<CellSlice<'a>>, Error> {
        dict_get(self.0.as_ref(), N, key, Cell::empty_context())
    }

    /// Returns a `CellSlice` of the value corresponding to the key.
    pub fn get_ext<'a, 'c: 'a>(
        &'a self,
        key: CellSlice<'_>,
        context: &'c dyn CellContext,
    ) -> Result<Option<CellSlice<'a>>, Error> {
        dict_get(self.0.as_ref(), N, key, context)
    }

    /// Computes the minimal key in dictionary that is lexicographically greater than `key`,
    /// and returns it along with associated value as cell slice parts.
    pub fn get_next_owned(
        &self,
        key: CellSlice<'_>,
        signed: bool,
    ) -> Result<Option<(CellDataBuilder, CellSliceParts)>, Error> {
        dict_find_owned(
            self.0.as_ref(),
            N,
            key,
            DictBound::Max,
            false,
            signed,
            Cell::empty_context(),
        )
    }

    /// Get subdict of dictionary by specified key prefix
    pub fn get_subdict<'a, 'c: 'a>(
        &'a self,
        mut prefix: CellSlice<'a>,
        context: &'c dyn CellContext,
    ) -> Result<Option<Cell>, Error> {
        dict_get_subdict(self.0.as_ref(), N, &mut prefix, context)
    }

    /// Computes the maximal key in dictionary that is lexicographically smaller than `key`,
    /// and returns it along with associated value as cell slice parts.
    pub fn get_prev_owned(
        &self,
        key: CellSlice<'_>,
        signed: bool,
    ) -> Result<Option<(CellDataBuilder, CellSliceParts)>, Error> {
        dict_find_owned(
            self.0.as_ref(),
            N,
            key,
            DictBound::Min,
            false,
            signed,
            Cell::empty_context(),
        )
    }

    /// Computes the minimal key in dictionary that is lexicographically greater than `key`,
    /// and returns it along with associated value as cell slice parts.
    pub fn get_or_next_owned(
        &self,
        key: CellSlice<'_>,
        signed: bool,
    ) -> Result<Option<(CellDataBuilder, CellSliceParts)>, Error> {
        dict_find_owned(
            self.0.as_ref(),
            N,
            key,
            DictBound::Max,
            true,
            signed,
            Cell::empty_context(),
        )
    }

    /// Computes the maximal key in dictionary that is lexicographically smaller than `key`,
    /// and returns it along with associated value as cell slice parts.
    pub fn get_or_prev_owned(
        &self,
        key: CellSlice<'_>,
        signed: bool,
    ) -> Result<Option<(CellDataBuilder, CellSliceParts)>, Error> {
        dict_find_owned(
            self.0.as_ref(),
            N,
            key,
            DictBound::Min,
            true,
            signed,
            Cell::empty_context(),
        )
    }

    /// Returns cell slice parts of the value corresponding to the key.
    ///
    /// NOTE: Uses the default cell context.
    pub fn get_owned(&self, key: CellSlice<'_>) -> Result<Option<CellSliceParts>, Error> {
        dict_get_owned(self.0.as_ref(), N, key, Cell::empty_context())
    }

    /// Returns cell slice parts of the value corresponding to the key.
    pub fn get_owned_ext(
        &self,
        key: CellSlice<'_>,
        context: &dyn CellContext,
    ) -> Result<Option<CellSliceParts>, Error> {
        dict_get_owned(self.0.as_ref(), N, key, context)
    }

    /// Returns the lowest key and a value corresponding to the key.
    pub fn get_min(&self, signed: bool) -> Result<Option<(CellDataBuilder, CellSlice<'_>)>, Error> {
        dict_find_bound(
            self.0.as_ref(),
            N,
            DictBound::Min,
            signed,
            Cell::empty_context(),
        )
    }

    /// Returns the largest key and a value corresponding to the key.
    pub fn get_max(&self, signed: bool) -> Result<Option<(CellDataBuilder, CellSlice<'_>)>, Error> {
        dict_find_bound(
            self.0.as_ref(),
            N,
            DictBound::Max,
            signed,
            Cell::empty_context(),
        )
    }

    /// Finds the specified dict bound and returns a key and a value corresponding to the key.
    pub fn get_bound(
        &self,
        bound: DictBound,
        signed: bool,
    ) -> Result<Option<(CellDataBuilder, CellSlice<'_>)>, Error> {
        dict_find_bound(self.0.as_ref(), N, bound, signed, Cell::empty_context())
    }

    /// Finds the specified dict bound and returns a key and a value corresponding to the key.
    pub fn get_bound_ext<'a, 'c: 'a>(
        &'a self,
        bound: DictBound,
        signed: bool,
        context: &'c dyn CellContext,
    ) -> Result<Option<(CellDataBuilder, CellSlice<'a>)>, Error> {
        dict_find_bound(self.0.as_ref(), N, bound, signed, context)
    }

    /// Returns the lowest key and cell slice parts corresponding to the key.
    pub fn get_min_owned(
        &self,
        signed: bool,
    ) -> Result<Option<(CellDataBuilder, CellSliceParts)>, Error> {
        dict_find_bound_owned(
            self.0.as_ref(),
            N,
            DictBound::Min,
            signed,
            Cell::empty_context(),
        )
    }

    /// Returns the largest key and cell slice parts corresponding to the key.
    pub fn get_max_owned(
        &self,
        signed: bool,
    ) -> Result<Option<(CellDataBuilder, CellSliceParts)>, Error> {
        dict_find_bound_owned(
            self.0.as_ref(),
            N,
            DictBound::Max,
            signed,
            Cell::empty_context(),
        )
    }

    /// Finds the specified dict bound and returns a key and cell slice parts corresponding to the key.
    pub fn get_bound_owned(
        &self,
        bound: DictBound,
        signed: bool,
    ) -> Result<Option<(CellDataBuilder, CellSliceParts)>, Error> {
        dict_find_bound_owned(self.0.as_ref(), N, bound, signed, Cell::empty_context())
    }

    /// Finds the specified dict bound and returns a key and cell slice parts corresponding to the key.
    pub fn get_bound_owned_ext(
        &self,
        bound: DictBound,
        signed: bool,
        context: &dyn CellContext,
    ) -> Result<Option<(CellDataBuilder, CellSliceParts)>, Error> {
        dict_find_bound_owned(self.0.as_ref(), N, bound, signed, context)
    }

    /// Returns `true` if the dictionary contains a value for the specified key.
    pub fn contains_key(&self, key: CellSlice<'_>) -> Result<bool, Error> {
        Ok(ok!(dict_get(self.0.as_ref(), N, key, Cell::empty_context())).is_some())
    }

    /// Sets the value associated with the key in the dictionary.
    pub fn set_ext(
        &mut self,
        mut key: CellSlice<'_>,
        value: &dyn Store,
        context: &dyn CellContext,
    ) -> Result<bool, Error> {
        dict_insert(&mut self.0, &mut key, N, &value, SetMode::Set, context)
    }

    /// Sets the value associated with the key in the dictionary
    /// only if the key was already present in it.
    pub fn replace_ext(
        &mut self,
        mut key: CellSlice<'_>,
        value: &dyn Store,
        context: &dyn CellContext,
    ) -> Result<bool, Error> {
        dict_insert(&mut self.0, &mut key, N, value, SetMode::Replace, context)
    }

    /// Sets the value associated with key in dictionary,
    /// but only if it is not already present.
    pub fn add_ext(
        &mut self,
        mut key: CellSlice<'_>,
        value: &dyn Store,
        context: &dyn CellContext,
    ) -> Result<bool, Error> {
        dict_insert(&mut self.0, &mut key, N, value, SetMode::Add, context)
    }

    /// Removes the value associated with key in dictionary.
    /// Returns an optional removed value as cell slice parts.
    pub fn remove_ext(
        &mut self,
        mut key: CellSlice<'_>,
        context: &dyn CellContext,
    ) -> Result<Option<CellSliceParts>, Error> {
        dict_remove_owned(&mut self.0, &mut key, N, false, context)
    }

    /// Removes the specified dict bound.
    /// Returns an optional removed key and value as cell slice parts.
    pub fn remove_bound_ext(
        &mut self,
        bound: DictBound,
        signed: bool,
        context: &dyn CellContext,
    ) -> Result<Option<DictOwnedEntry>, Error> {
        dict_remove_bound_owned(&mut self.0, N, bound, signed, context)
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
            self.0.as_ref(),
            N,
            key_prefix,
            context
        ));
        Ok((Self(left), Self(right)))
    }

    /// Gets an iterator over the entries of the dictionary, sorted by key.
    /// The iterator element type is `Result<(CellDataBuilder, CellSlice)>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over dictionary builds a key
    /// for each element. Use [`values`] if you don't need keys from an iterator.
    ///
    /// [`values`]: RawDict::values
    pub fn iter(&'_ self) -> RawIter<'_> {
        RawIter::new(&self.0, N)
    }

    /// Gets an iterator over the entries of two dictionaries, sorted by key.
    /// The iterator element type is
    /// `Result<(CellDataBuilder, Option<CellSlice>, Option<CellSlice>)>`.
    /// Where the first element is the key, the second is the value from the first dictionary,
    /// and the third is the value from the second dictionary.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over dictionary builds a key
    /// for each element.
    pub fn iter_union<'a>(&'a self, other: &'a RawDict<N>) -> UnionRawIter<'a> {
        UnionRawIter::new(&self.0, &other.0, N)
    }

    /// Gets an iterator over the owned entries of the dictionary, sorted by key.
    /// The iterator element type is `Result<(CellDataBuilder, CellSliceParts)>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over dictionary builds a key
    /// for each element. Use [`values_owned`] if you don't need keys from an iterator.
    ///
    /// [`values_owned`]: RawDict::values_owned
    pub fn iter_owned(&'_ self) -> RawOwnedIter<'_> {
        RawOwnedIter::new(&self.0, N)
    }

    /// Gets an iterator over the keys of the dictionary, in sorted order.
    /// The iterator element type is `Result<CellDataBuilder>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    ///
    /// # Performance
    ///
    /// In the current implementation, iterating over dictionary builds a key
    /// for each element. Use [`values`] if you don't need keys from an iterator.
    ///
    /// [`values`]: RawDict::values
    pub fn keys(&'_ self) -> RawKeys<'_> {
        RawKeys::new(&self.0, N)
    }

    /// Gets an iterator over the values of the dictionary, in order by key.
    /// The iterator element type is `Result<CellSlice>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn values(&'_ self) -> RawValues<'_> {
        RawValues::new(&self.0, N)
    }

    /// Gets an iterator over the owned values of the dictionary, in order by key.
    /// The iterator element type is `Result<CellSliceParts>`.
    ///
    /// If the dictionary is invalid, finishes after the first invalid element,
    /// returning an error.
    pub fn values_owned(&'_ self) -> RawOwnedValues<'_> {
        RawOwnedValues::new(&self.0, N)
    }

    /// Sets the value associated with the key in the dictionary.
    ///
    /// Use [`set_ext`] if you need to use a custom cell context.
    ///
    /// [`set_ext`]: RawDict::set_ext
    pub fn set<T: Store>(&mut self, key: CellSlice<'_>, value: T) -> Result<bool, Error> {
        self.set_ext(key, &value, Cell::empty_context())
    }

    /// Sets the value associated with the key in the dictionary
    /// only if the key was already present in it.
    ///
    /// Use [`replace_ext`] if you need to use a custom cell context.
    ///
    /// [`replace_ext`]: RawDict::replace_ext
    pub fn replace<T: Store>(&mut self, key: CellSlice<'_>, value: T) -> Result<bool, Error> {
        self.replace_ext(key, &value, Cell::empty_context())
    }

    /// Sets the value associated with key in dictionary,
    /// but only if it is not already present.
    ///
    /// Use [`add_ext`] if you need to use a custom cell context.
    ///
    /// [`add_ext`]: RawDict::add_ext
    pub fn add<T: Store>(&mut self, key: CellSlice<'_>, value: T) -> Result<bool, Error> {
        self.add_ext(key, &value, Cell::empty_context())
    }

    /// Removes the value associated with key in dictionary.
    /// Returns an optional removed value as cell slice parts.
    ///
    /// Use [`remove_ext`] if you need to use a custom cell context.
    ///
    /// [`remove_ext`]: RawDict::remove_ext
    pub fn remove(&mut self, key: CellSlice<'_>) -> Result<Option<CellSliceParts>, Error> {
        self.remove_ext(key, Cell::empty_context())
    }

    /// Removes the lowest key from the dict.
    /// Returns an optional removed key and value as cell slice parts.
    ///
    /// Use [`remove_bound_ext`] if you need to use a custom context.
    ///
    /// [`remove_bound_ext`]: RawDict::remove_bound_ext
    pub fn remove_min(&mut self, signed: bool) -> Result<Option<DictOwnedEntry>, Error> {
        self.remove_bound_ext(DictBound::Min, signed, Cell::empty_context())
    }

    /// Removes the largest key from the dict.
    /// Returns an optional removed key and value as cell slice parts.
    ///
    /// Use [`remove_bound_ext`] if you need to use a custom cell context.
    ///
    /// [`remove_bound_ext`]: RawDict::remove_bound_ext
    pub fn remove_max(&mut self, signed: bool) -> Result<Option<DictOwnedEntry>, Error> {
        self.remove_bound_ext(DictBound::Max, signed, Cell::empty_context())
    }

    /// Removes the specified dict bound.
    /// Returns an optional removed key and value as cell slice parts.
    ///
    /// Use [`remove_bound_ext`] if you need to use a custom cell context.
    ///
    /// [`remove_bound_ext`]: RawDict::remove_bound_ext
    pub fn remove_bound(
        &mut self,
        bound: DictBound,
        signed: bool,
    ) -> Result<Option<DictOwnedEntry>, Error> {
        self.remove_bound_ext(bound, signed, Cell::empty_context())
    }
}

/// An iterator over the owned entries of a [`RawDict`].
///
/// This struct is created by the [`iter_owned`] method on [`RawDict`].
/// See its documentation for more.
///
/// [`iter_owned`]: RawDict::iter_owned
#[derive(Clone)]
pub struct RawOwnedIter<'a> {
    root: &'a Option<Cell>,
    inner: RawIter<'a>,
}

impl<'a> RawOwnedIter<'a> {
    /// Creates an iterator over the owned entries of a dictionary.
    pub fn new(root: &'a Option<Cell>, bit_len: u16) -> Self {
        Self {
            inner: RawIter::new(root, bit_len),
            root,
        }
    }

    /// Creates an iterator over the entries of a dictionary with explicit
    /// direction and behavior.
    pub fn new_ext(root: &'a Option<Cell>, bit_len: u16, reversed: bool, signed: bool) -> Self {
        Self {
            inner: RawIter::new_ext(root, bit_len, reversed, signed),
            root,
        }
    }

    /// Changes the direction of the iterator to descending.
    #[inline]
    pub fn reversed(mut self) -> Self {
        self.inner.reversed = true;
        self
    }

    /// Changes the behavior of the iterator to reverse the high bit.
    #[inline]
    pub fn signed(mut self) -> Self {
        self.inner.signed = true;
        self
    }

    /// Returns whether the iterator direction was reversed.
    #[inline]
    pub fn is_reversed(&self) -> bool {
        self.inner.reversed
    }

    /// Returns whether the iterator treats keys as signed integers.
    #[inline]
    pub fn is_signed(&self) -> bool {
        self.inner.signed
    }
}

impl Iterator for RawOwnedIter<'_> {
    type Item = Result<(CellDataBuilder, CellSliceParts), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next_owned(self.root)
    }
}

/// An iterator over the entries of a [`RawDict`] or a [`Dict`].
///
/// This struct is created by the [`iter`] method on [`RawDict`] or the [`raw_iter`] method on [`Dict`].
/// See their documentation for more.
///
/// [`Dict`]: crate::dict::Dict
/// [`iter`]: RawDict::iter
/// [`raw_iter`]: crate::dict::Dict::raw_iter
#[derive(Clone)]
pub struct RawIter<'a> {
    segments: Vec<IterSegment<'a>>,
    status: IterStatus,
    builder: Box<CellDataBuilder>,
    reversed: bool,
    signed: bool,
}

impl<'a> RawIter<'a> {
    /// Creates an iterator over the entries of a dictionary.
    pub fn new(root: &'a Option<Cell>, bit_len: u16) -> Self {
        Self::new_ext(root, bit_len, false, false)
    }

    /// Creates an iterator over the entries of a dictionary with explicit
    /// direction and behavior.
    pub fn new_ext(root: &'a Option<Cell>, bit_len: u16, reversed: bool, signed: bool) -> Self {
        let mut segments = Vec::new();

        // Push root segment if any
        if let Some(root) = root {
            let Ok(data) = root.as_slice() else {
                return Self {
                    segments: Vec::new(),
                    status: IterStatus::UnexpectedCell,
                    builder: Box::default(),
                    reversed,
                    signed,
                };
            };

            segments.push(IterSegment {
                data,
                remaining_bit_len: bit_len,
                prefix: None,
            });
        }

        Self {
            segments,
            status: IterStatus::Valid,
            builder: Default::default(),
            reversed,
            signed,
        }
    }

    /// Changes the direction of the iterator to descending.
    #[inline]
    pub fn reversed(mut self) -> Self {
        self.reversed = true;
        self
    }

    /// Changes the behavior of the iterator to reverse the high bit.
    #[inline]
    pub fn signed(mut self) -> Self {
        self.signed = true;
        self
    }

    /// Returns whether the iterator direction was reversed.
    #[inline]
    pub fn is_reversed(&self) -> bool {
        self.reversed
    }

    /// Returns whether the iterator treats keys as signed integers.
    #[inline]
    pub fn is_signed(&self) -> bool {
        self.signed
    }

    /// Advances the iterator and returns the next value as owned cell slice parts.
    pub fn next_owned(
        &mut self,
        root: &Option<Cell>,
    ) -> Option<Result<(CellDataBuilder, CellSliceParts), Error>> {
        Some(match self.next()? {
            Ok((key, slice)) => {
                let parent = match self.segments.last() {
                    Some(segment) => {
                        let refs_offset = segment.data.offset_refs();
                        debug_assert!(
                            segment.prefix.is_some() && (refs_offset == 1 || refs_offset == 2)
                        );

                        let next_bit = (refs_offset != 1)
                            ^ self.reversed
                            ^ (self.signed
                                && self.segments.len() == 1
                                && segment.prefix.unwrap().is_data_empty());

                        match segment.data.cell().reference_cloned(next_bit as u8) {
                            Some(cell) => cell,
                            None => return Some(Err(self.finish(Error::CellUnderflow))),
                        }
                    }
                    None => match root {
                        Some(root) => root.clone(),
                        None => {
                            debug_assert!(false, "Non-empty iterator for empty dict");
                            unsafe { std::hint::unreachable_unchecked() };
                        }
                    },
                };
                Ok((key, (slice.range(), parent)))
            }
            Err(e) => Err(e),
        })
    }

    #[inline]
    pub(crate) fn finish(&mut self, err: Error) -> Error {
        self.status = IterStatus::Broken;
        err
    }
}

impl<'a> Iterator for RawIter<'a> {
    type Item = Result<(CellDataBuilder, CellSlice<'a>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if unlikely(!self.status.is_valid()) {
            return if self.status.is_unexpected_cell() {
                self.status = IterStatus::Broken;
                Some(Err(Error::UnexpectedExoticCell))
            } else {
                None
            };
        }

        fn next_impl<'a>(
            reverse: bool,
            signed: bool,
            segments: &mut Vec<IterSegment<'a>>,
            builder: &mut CellDataBuilder,
        ) -> Result<Option<(CellDataBuilder, CellSlice<'a>)>, Error> {
            #[allow(clippy::never_loop)]
            loop {
                let mut to_rewind = 0;
                let segment = loop {
                    let is_root = segments.len() == 1;
                    let Some(segment) = segments.last_mut() else {
                        return Ok(None);
                    };

                    // Handle root case
                    let Some(prefix) = &segment.prefix else {
                        break segment;
                    };

                    let refs_offset = segment.data.offset_refs();
                    if refs_offset < 2 {
                        // Found the latest unprocessed slice
                        let remaining_bit_len = segment.remaining_bit_len;
                        let next_bit = (refs_offset != 0)
                            ^ reverse
                            ^ (signed && is_root && prefix.is_data_empty());

                        let data = ok!(segment.data.cell().get_reference_as_slice(next_bit as u8));
                        segment.data.skip_first(0, 1).ok();

                        ok!(builder.rewind_bits(to_rewind));
                        ok!(builder.store_bit(next_bit));
                        segments.push(IterSegment {
                            data,
                            prefix: None,
                            remaining_bit_len,
                        });
                        // SAFETY: we have just added a new element
                        break unsafe { segments.last_mut().unwrap_unchecked() };
                    } else {
                        // Rewind prefix
                        to_rewind += prefix.size_bits();
                        // Pop processed segments
                        segments.pop();
                        // Rewind reference bit (if any)
                        to_rewind += !segments.is_empty() as u16;
                    }
                };

                // Read the next key part from the latest segment
                let prefix = ok!(read_label(&mut segment.data, segment.remaining_bit_len));

                // Check remaining bits
                return match segment.remaining_bit_len.checked_sub(prefix.size_bits()) {
                    // Return value if there are no remaining bits to read
                    Some(0) => {
                        // Try to store the last prefix into the result key
                        let mut key = builder.clone();
                        ok!(key.store_slice_data(prefix));

                        let data = segment.data;

                        // Pop the current segment from the stack
                        segments.pop();
                        ok!(builder.rewind_bits(!segments.is_empty() as u16));

                        Ok(Some((key, data)))
                    }
                    // Append prefix to builder and proceed to the next segment
                    Some(remaining) => {
                        if segment.data.size_refs() < 2 {
                            return Err(Error::CellUnderflow);
                        }

                        // Try to store the next prefix into the buffer
                        ok!(builder.store_slice_data(prefix));

                        // Update segment prefix
                        segment.prefix = Some(prefix);
                        segment.remaining_bit_len = remaining - 1;

                        // Handle next segment
                        continue;
                    }
                    None => Err(Error::CellUnderflow),
                };
            }
        }

        match next_impl(
            self.reversed,
            self.signed,
            &mut self.segments,
            &mut self.builder,
        ) {
            Ok(res) => res.map(Ok),
            Err(e) => Some(Err(self.finish(e))),
        }
    }
}

#[derive(Clone)]
struct IterSegment<'a> {
    data: CellSlice<'a>,
    remaining_bit_len: u16,
    prefix: Option<CellSlice<'a>>,
}

/// An iterator over the entries across two [`RawDict`] or two [`Dict`].
///
/// This struct is created by the [`iter_union`] method on [`RawDict`]
/// or the [`raw_iter_union`] method on [`Dict`].
///
/// [`Dict`]: crate::dict::Dict
/// [`iter_union`]: RawDict::iter_union
/// [`raw_iter_union`]: crate::dict::Dict::raw_iter_union
#[derive(Clone)]
pub struct UnionRawIter<'a> {
    left: RawIter<'a>,
    left_peeked: Option<Box<(CellDataBuilder, CellSlice<'a>)>>,
    left_finished: bool,
    right: RawIter<'a>,
    right_peeked: Option<Box<(CellDataBuilder, CellSlice<'a>)>>,
    right_finished: bool,
}

impl<'a> UnionRawIter<'a> {
    /// Creates an iterator over the entries of dictionaries.
    pub fn new(left_root: &'a Option<Cell>, right_root: &'a Option<Cell>, bit_len: u16) -> Self {
        Self::new_ext(left_root, right_root, bit_len, false, false)
    }

    /// Creates an iterator over the entries of dictionaries
    ///  with explicit direction and behavior.
    pub fn new_ext(
        left_root: &'a Option<Cell>,
        right_root: &'a Option<Cell>,
        bit_len: u16,
        reversed: bool,
        signed: bool,
    ) -> Self {
        Self {
            left: RawIter::new_ext(left_root, bit_len, reversed, signed),
            left_peeked: None,
            left_finished: false,
            right: RawIter::new_ext(right_root, bit_len, reversed, signed),
            right_peeked: None,
            right_finished: false,
        }
    }

    /// Changes the direction of the iterator to descending.
    #[inline]
    pub fn reversed(mut self) -> Self {
        self.left.reversed = true;
        self.right.reversed = true;
        self
    }

    /// Changes the behavior of the iterator to reverse the high bit.
    #[inline]
    pub fn signed(mut self) -> Self {
        self.left.signed = true;
        self.right.signed = true;
        self
    }

    /// Returns whether the iterator direction was reversed.
    #[inline]
    pub fn is_reversed(&self) -> bool {
        self.left.reversed
    }

    /// Returns whether the iterator treats keys as signed integers.
    #[inline]
    pub fn is_signed(&self) -> bool {
        self.left.signed
    }

    #[inline]
    pub(crate) fn finish(&mut self, err: Error) -> Error {
        self.left.status = IterStatus::Broken;
        self.right.status = IterStatus::Broken;
        err
    }

    fn peek<'p>(
        iter: &mut RawIter<'a>,
        finished: &mut bool,
        peeked: &'p mut Option<Box<(CellDataBuilder, CellSlice<'a>)>>,
    ) -> Result<Option<&'p (CellDataBuilder, CellSlice<'a>)>, Error> {
        if !*finished && peeked.is_none() {
            match iter.next() {
                Some(Ok(next)) => {
                    *peeked = Some(Box::new(next));
                }
                Some(Err(e)) => {
                    *finished = true;
                    return Err(e);
                }
                None => *finished = true,
            }
        }
        Ok(peeked.as_deref())
    }
}

impl<'a> Iterator for UnionRawIter<'a> {
    type Item = Result<
        (
            CellDataBuilder,
            Option<CellSlice<'a>>,
            Option<CellSlice<'a>>,
        ),
        Error,
    >;

    fn next(&mut self) -> Option<Self::Item> {
        if unlikely(!self.left.status.is_valid() || !self.right.status.is_valid()) {
            if !self.left.status.is_unexpected_cell() && !self.right.status.is_unexpected_cell() {
                return None;
            }

            self.left.status = IterStatus::Broken;
            self.right.status = IterStatus::Broken;
            return Some(Err(Error::UnexpectedExoticCell));
        }

        let reversed = self.is_reversed();
        let signed = self.is_signed();

        let left = match Self::peek(
            &mut self.left,
            &mut self.left_finished,
            &mut self.left_peeked,
        ) {
            Ok(res) => res,
            Err(e) => return Some(Err(self.finish(e))),
        };

        let right = match Self::peek(
            &mut self.right,
            &mut self.right_finished,
            &mut self.right_peeked,
        ) {
            Ok(res) => res,
            Err(e) => return Some(Err(self.finish(e))),
        };

        match (left, right) {
            (None, None) => None,
            (Some((left_key, left_value)), None) => {
                let res = Some(Ok((left_key.clone(), Some(*left_value), None)));
                self.left_peeked = None;
                res
            }
            (None, Some((right_key, right_value))) => {
                let res = Some(Ok((right_key.clone(), None, Some(*right_value))));
                self.right_peeked = None;
                res
            }
            (Some((left_key, left_value)), Some((right_key, right_value))) => {
                let cmp = {
                    let left_key = left_key.as_data_slice();
                    let right_key = right_key.as_data_slice();

                    let mut reversed = reversed;
                    if signed {
                        let left_is_neg = left_key.get_bit(0).ok().unwrap_or_default();
                        let right_is_neg = right_key.get_bit(0).ok().unwrap_or_default();
                        reversed ^= left_is_neg != right_is_neg;
                    }

                    let cmp = match left_key.lex_cmp(&right_key) {
                        Ok(cmp) => cmp,
                        Err(e) => return Some(Err(self.finish(e))),
                    };

                    if reversed { cmp.reverse() } else { cmp }
                };

                match cmp {
                    std::cmp::Ordering::Less => {
                        let res = Some(Ok((left_key.clone(), Some(*left_value), None)));
                        self.left_peeked = None;
                        res
                    }
                    std::cmp::Ordering::Greater => {
                        let res = Some(Ok((right_key.clone(), None, Some(*right_value))));
                        self.right_peeked = None;
                        res
                    }
                    std::cmp::Ordering::Equal => {
                        let res = Some(Ok((
                            left_key.clone(),
                            Some(*left_value),
                            Some(*right_value),
                        )));
                        self.left_peeked = None;
                        self.right_peeked = None;
                        res
                    }
                }
            }
        }
    }
}

/// An iterator over the keys of a [`RawDict`] or a [`Dict`].
///
/// This struct is created by the [`keys`] method on [`RawDict`] or the [`raw_keys`] method on [`Dict`].
/// See their documentation for more.
///
/// [`Dict`]: crate::dict::Dict
/// [`keys`]: RawDict::keys
/// [`raw_keys`]: crate::dict::Dict::raw_keys
#[derive(Clone)]
pub struct RawKeys<'a> {
    inner: RawIter<'a>,
}

impl<'a> RawKeys<'a> {
    /// Creates an iterator over the keys of a dictionary.
    pub fn new(root: &'a Option<Cell>, bit_len: u16) -> Self {
        Self {
            inner: RawIter::new(root, bit_len),
        }
    }

    /// Creates an iterator over the keys of a dictionary with explicit
    /// direction and behavior.
    pub fn new_ext(root: &'a Option<Cell>, bit_len: u16, reversed: bool, signed: bool) -> Self {
        Self {
            inner: RawIter::new_ext(root, bit_len, reversed, signed),
        }
    }

    /// Changes the direction of the iterator to descending.
    #[inline]
    pub fn reversed(mut self) -> Self {
        self.inner.reversed = true;
        self
    }

    /// Changes the behavior of the iterator to reverse the high bit.
    #[inline]
    pub fn signed(mut self) -> Self {
        self.inner.signed = true;
        self
    }

    /// Returns whether the iterator direction was reversed.
    #[inline]
    pub fn is_reversed(&self) -> bool {
        self.inner.reversed
    }

    /// Returns whether the iterator treats keys as signed integers.
    #[inline]
    pub fn is_signed(&self) -> bool {
        self.inner.signed
    }
}

impl Iterator for RawKeys<'_> {
    type Item = Result<CellDataBuilder, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.inner.next()? {
            Ok((key, _)) => Some(Ok(key)),
            Err(e) => Some(Err(e)),
        }
    }
}

/// An iterator over the owned values of a [`RawDict`].
///
/// This struct is created by the [`values_owned`] method on [`RawDict`].
/// See its documentation for more.
///
/// [`values_owned`]: RawDict::values_owned
#[derive(Clone)]
pub struct RawOwnedValues<'a> {
    root: &'a Option<Cell>,
    inner: RawValues<'a>,
}

impl<'a> RawOwnedValues<'a> {
    /// Creates an iterator over the owned entries of a dictionary.
    pub fn new(root: &'a Option<Cell>, bit_len: u16) -> Self {
        Self {
            inner: RawValues::new(root, bit_len),
            root,
        }
    }

    /// Creates an iterator over the values of a dictionary with explicit
    /// direction and behavior.
    pub fn new_ext(root: &'a Option<Cell>, bit_len: u16, reversed: bool, signed: bool) -> Self {
        Self {
            inner: RawValues::new_ext(root, bit_len, reversed, signed),
            root,
        }
    }

    /// Changes the direction of the iterator to descending.
    #[inline]
    pub fn reversed(mut self) -> Self {
        self.inner.reversed = true;
        self
    }

    /// Changes the behavior of the iterator to reverse the high bit.
    #[inline]
    pub fn signed(mut self) -> Self {
        self.inner.signed = true;
        self
    }

    /// Returns whether the iterator direction was reversed.
    #[inline]
    pub fn is_reversed(&self) -> bool {
        self.inner.reversed
    }

    /// Returns whether the iterator treats keys as signed integers.
    #[inline]
    pub fn is_signed(&self) -> bool {
        self.inner.signed
    }
}

impl Iterator for RawOwnedValues<'_> {
    type Item = Result<CellSliceParts, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match self.inner.next()? {
            Ok(slice) => {
                let parent = match self.inner.segments.last() {
                    Some(segment) => {
                        let refs_offset = segment.data.offset_refs();
                        debug_assert!(refs_offset > 0);
                        match segment.data.cell().reference_cloned(refs_offset - 1) {
                            Some(cell) => cell,
                            None => return Some(Err(self.inner.finish(Error::CellUnderflow))),
                        }
                    }
                    None => match self.root {
                        Some(root) => root.clone(),
                        None => {
                            debug_assert!(false, "Non-empty iterator for empty dict");
                            unsafe { std::hint::unreachable_unchecked() };
                        }
                    },
                };
                Ok((slice.range(), parent))
            }
            Err(e) => Err(e),
        })
    }
}

/// An iterator over the values of a [`RawDict`] or a [`Dict`].
///
/// This struct is created by the [`values`] method on [`RawDict`] or the [`raw_values`] method on [`Dict`].
/// See their documentation for more.
///
/// [`Dict`]: crate::dict::Dict
/// [`values`]: RawDict::values
/// [`raw_values`]: crate::dict::Dict::raw_values
#[derive(Clone)]
pub struct RawValues<'a> {
    segments: Vec<ValuesSegment<'a>>,
    status: IterStatus,
    reversed: bool,
    signed: bool,
}

impl<'a> RawValues<'a> {
    /// Creates an iterator over the values of a dictionary.
    pub fn new(root: &'a Option<Cell>, bit_len: u16) -> Self {
        Self::new_ext(root, bit_len, false, false)
    }

    /// Creates an iterator over the values of a dictionary with explicit
    /// direction and behavior.
    pub fn new_ext(root: &'a Option<Cell>, bit_len: u16, reversed: bool, signed: bool) -> Self {
        let mut segments = Vec::new();
        if let Some(root) = root {
            let Ok(data) = root.as_slice() else {
                return Self {
                    segments: Vec::new(),
                    status: IterStatus::UnexpectedCell,
                    reversed,
                    signed,
                };
            };

            segments.push(ValuesSegment {
                data,
                remaining_bit_len: bit_len,
            });
        }
        Self {
            segments,
            status: IterStatus::Valid,
            reversed,
            signed,
        }
    }

    /// Changes the direction of the iterator to descending.
    #[inline]
    pub fn reversed(mut self) -> Self {
        self.reversed = true;
        self
    }

    /// Changes the behavior of the iterator to reverse the high bit.
    #[inline]
    pub fn signed(mut self) -> Self {
        self.signed = true;
        self
    }

    /// Returns whether the iterator direction was reversed.
    #[inline]
    pub fn is_reversed(&self) -> bool {
        self.reversed
    }

    /// Returns whether the iterator treats keys as signed integers.
    #[inline]
    pub fn is_signed(&self) -> bool {
        self.signed
    }

    #[inline]
    pub(crate) fn finish(&mut self, err: Error) -> Error {
        self.status = IterStatus::Broken;
        err
    }
}

impl<'a> Iterator for RawValues<'a> {
    type Item = Result<CellSlice<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if unlikely(!self.status.is_valid()) {
            return if self.status.is_unexpected_cell() {
                self.status = IterStatus::Broken;
                Some(Err(Error::UnexpectedExoticCell))
            } else {
                None
            };
        }

        fn next_impl<'a>(
            reverse: bool,
            signed: bool,
            segments: &mut Vec<ValuesSegment<'a>>,
        ) -> Result<Option<CellSlice<'a>>, Error> {
            #[allow(clippy::never_loop)]
            loop {
                // Find the latest slice which has not been loaded yet
                let segment = loop {
                    let is_root = segments.len() == 1;
                    let Some(segment) = segments.last_mut() else {
                        return Ok(None);
                    };

                    if segment.data.offset_bits() == 0 {
                        break segment;
                    }

                    let refs_offset = segment.data.offset_refs();
                    if refs_offset < 2 {
                        // Found the latest unprocessed slice
                        let remaining_bit_len = segment.remaining_bit_len;
                        let next_bit = (refs_offset != 0)
                            ^ reverse
                            ^ (signed && is_root && segment.data.is_data_empty());
                        let data = ok!(segment.data.cell().get_reference_as_slice(next_bit as u8));
                        segment.data.skip_first(0, 1).ok();

                        segments.push(ValuesSegment {
                            data,
                            remaining_bit_len,
                        });
                        // SAFETY: we have just added a new element
                        break unsafe { segments.last_mut().unwrap_unchecked() };
                    } else {
                        segments.pop();
                    }
                };

                // Read the next key part from the latest segment
                let prefix = ok!(read_label(&mut segment.data, segment.remaining_bit_len));

                // Check remaining bits
                return match segment.remaining_bit_len.checked_sub(prefix.size_bits()) {
                    // Return value if there are no remaining bits to read
                    Some(0) => {
                        let data = segment.data;
                        // Pop the current segment from the stack
                        segments.pop();
                        Ok(Some(data))
                    }
                    // Append prefix to builder and proceed to the next segment
                    Some(remaining) => {
                        if segment.data.size_refs() < 2 {
                            return Err(Error::CellUnderflow);
                        }
                        segment.remaining_bit_len = remaining - 1;
                        // Handle next segment
                        continue;
                    }
                    None => Err(Error::CellUnderflow),
                };
            }
        }

        match next_impl(self.reversed, self.signed, &mut self.segments) {
            Ok(res) => res.map(Ok),
            Err(e) => Some(Err(self.finish(e))),
        }
    }
}

#[derive(Copy, Clone)]
struct ValuesSegment<'a> {
    data: CellSlice<'a>,
    remaining_bit_len: u16,
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::prelude::*;

    fn build_cell<F: FnOnce(&mut CellBuilder) -> Result<(), Error>>(f: F) -> Cell {
        let mut builder = CellBuilder::new();
        f(&mut builder).unwrap();
        builder.build().unwrap()
    }

    #[test]
    fn dict_set() -> anyhow::Result<()> {
        let mut dict = RawDict::<32>::new();

        let key = CellBuilder::build_from(123u32)?;

        dict.set(key.as_slice()?, ())?;
        {
            let mut values = dict.values();
            let value = values.next().unwrap().unwrap();
            assert!(value.is_data_empty() && value.is_refs_empty());
            assert!(values.next().is_none());
        }

        dict.set(key.as_slice()?, 0xffffu16)?;
        {
            let mut values = dict.values();
            let mut value = values.next().unwrap()?;
            assert_eq!(value.load_u16(), Ok(0xffff));
            assert!(value.is_data_empty() && value.is_refs_empty());
            assert!(values.next().is_none());
        }

        Ok(())
    }

    #[test]
    #[cfg_attr(miri, ignore)] // takes too long to execute on miri
    fn dict_set_complex() -> anyhow::Result<()> {
        let mut dict = RawDict::<32>::new();
        for i in 0..520 {
            let key = build_cell(|b| b.store_u32(i));
            dict.set(key.as_slice()?, true)?;

            let mut total = 0;
            for (i, item) in dict.iter().enumerate() {
                total += 1;
                let (key, value) = item?;
                assert_eq!(key.size_bits(), 32);
                assert_eq!(value.size_bits(), 1);
                let key = key.as_data_slice().load_u32()?;
                assert_eq!(key, i as u32);
            }
            assert_eq!(total, i + 1);
        }

        Ok(())
    }

    #[test]
    fn dict_replace() -> anyhow::Result<()> {
        let mut dict = RawDict::<32>::new();

        //
        dict.replace(build_cell(|b| b.store_u32(123)).as_slice()?, false)
            .unwrap();
        assert!(
            !dict
                .contains_key(build_cell(|b| b.store_u32(123)).as_slice()?)
                .unwrap()
        );

        //
        dict.set(build_cell(|b| b.store_u32(123)).as_slice()?, false)
            .unwrap();
        dict.replace(build_cell(|b| b.store_u32(123)).as_slice()?, true)
            .unwrap();

        let mut value = dict
            .get(build_cell(|b| b.store_u32(123)).as_slice()?)
            .unwrap()
            .unwrap();
        assert_eq!(value.size_bits(), 1);
        assert_eq!(value.load_bit(), Ok(true));

        Ok(())
    }

    #[test]
    fn dict_add() -> anyhow::Result<()> {
        let mut dict = RawDict::<32>::new();

        let key = build_cell(|b| b.store_u32(123));

        //
        dict.add(key.as_slice()?, false)?;
        let mut value = dict.get(key.as_slice()?)?.unwrap();
        assert_eq!(value.size_bits(), 1);
        assert_eq!(value.load_bit(), Ok(false));

        //
        dict.add(key.as_slice()?, true)?;
        let mut value = dict.get(key.as_slice()?)?.unwrap();
        assert_eq!(value.size_bits(), 1);
        assert_eq!(value.load_bit(), Ok(false));

        Ok(())
    }

    #[test]
    fn dict_split() -> anyhow::Result<()> {
        let mut dict = RawDict::<4>::new();
        for i in 0..16 {
            let key = build_cell(|b| b.store_small_uint(i, 4));
            dict.add(key.as_slice()?, i)?;
        }

        let (left, right) = dict.split()?;
        assert!(!left.is_empty());
        assert!(!right.is_empty());
        assert_eq!(left.iter().count(), 8);
        assert_eq!(right.iter().count(), 8);

        let (ll, lr) = left.split()?;
        assert!(!ll.is_empty());
        assert!(lr.is_empty());
        assert_eq!(ll.iter().count(), 8);

        let (rl, rr) = right.split()?;
        assert!(rl.is_empty());
        assert!(!rr.is_empty());
        assert_eq!(rr.iter().count(), 8);

        let (left, right) = RawDict::<4>::new().split()?;
        assert!(left.is_empty());
        assert!(right.is_empty());

        Ok(())
    }

    #[test]
    fn dict_split_by_prefix() -> anyhow::Result<()> {
        fn check_range(dict: &RawDict<4>, mut range: std::ops::Range<u8>) {
            for key in dict.keys() {
                let key = key.unwrap();
                let key = key.as_data_slice().load_small_uint(4).unwrap();
                assert_eq!(key, range.next().unwrap());
            }
            assert_eq!(range.next(), None);
        }

        let mut dict = RawDict::<4>::new();
        for i in 0..16 {
            let key = build_cell(|b| b.store_small_uint(i, 4));
            dict.add(key.as_slice()?, i)?;
        }

        let (left, right) = dict.split()?;
        check_range(&left, 0..8);
        check_range(&right, 8..16);

        {
            let mut prefix = CellBuilder::new();
            prefix.store_bit_one()?;
            let res = dict.split_by_prefix(&prefix.as_data_slice());
            assert!(matches!(res, Err(Error::CellUnderflow)));
        }

        let (ll, lr) = {
            let mut prefix = CellBuilder::new();
            prefix.store_bit_zero()?;
            left.split_by_prefix(&prefix.as_data_slice())?
        };
        check_range(&ll, 0..4);
        check_range(&lr, 4..8);

        let (rl, rr) = {
            let mut prefix = CellBuilder::new();
            prefix.store_bit_one()?;
            right.split_by_prefix(&prefix.as_data_slice())?
        };
        check_range(&rl, 8..12);
        check_range(&rr, 12..16);

        Ok(())
    }

    #[test]
    fn dict_get_subdict() -> anyhow::Result<()> {
        let mut dict = RawDict::<32>::new();
        for i in 0u32..4 {
            // let key = i % 2;
            let key = CellBuilder::build_from(i)?;
            println!("ADDING KEY {}", key.display_data());
            dict.add(key.as_slice()?, i)?;
        }

        let cell = dict.root().as_ref().unwrap();
        let boc = Boc::encode_base64(cell);

        println!("{boc}");

        let key = CellBuilder::build_from(1u16 << 8)?;

        println!("KEY: {}", key.display_data());

        let context = &mut SimpleContext::default();

        let value: Cell = dict.get_subdict(key.as_slice()?, context)?.unwrap();

        print!("{}", value.display_tree());

        Ok(())
    }

    #[test]
    fn dict_get() -> anyhow::Result<()> {
        let boc = Boc::decode_base64(
            "te6ccgECOwEAASoAAQHAAQIBIBACAgEgAwMCASAEBAIBIAUFAgEgBgYCASAHBwIBIAgIAgEgCQkCASAoCgIBIAsZAgEgDBsCASArDQIBIA4fAgEgLQ8CASAuIQIBIBERAgEgEhICASATEwIBIBQUAgEgFRUCASAWFgIBIBcXAgEgKBgCASAaGQIBIBsbAgEgHRsCASAcHAIBIB8fAgEgKx4CASAiHwIBICAgAgEgISECASAlJQIBIC0jAgEgLiQCASAvJQIBIDMmAgFiNicCAUg4OAIBICkpAgEgKioCASArKwIBICwsAgEgLS0CASAuLgIBIC8vAgEgMzACAWI2MQIBIDcyAAnWAAAmbwIBIDQ0AgEgNTUCASA2NgIBIDc3AgEgODgCASA5OQIBIDo6AAnQAAAmbw==",
        )?;

        // println!("BOC: {}", boc.display_tree());

        let dict = boc.parse::<RawDict<32>>()?;

        let key = CellBuilder::build_from(u32::from_be_bytes(123u32.to_le_bytes()))?;
        let value = dict.get(key.as_slice()?)?.unwrap();

        {
            let (range, cell) = dict.get_owned(key.as_slice()?)?.unwrap();
            assert_eq!(range.apply(&cell).unwrap(), value);
        }

        let value = {
            let mut builder = CellBuilder::new();
            builder.store_slice(value)?;
            builder.build()?
        };
        println!("{}", value.display_tree());

        Ok(())
    }

    #[test]
    fn dict_iter() -> anyhow::Result<()> {
        let boc = Boc::decode_base64(
            "te6ccgEBFAEAeAABAcABAgPOQAUCAgHUBAMACQAAAI3gAAkAAACjoAIBIA0GAgEgCgcCASAJCAAJAAAAciAACQAAAIfgAgEgDAsACQAAAFZgAAkAAABsIAIBIBEOAgEgEA8ACQAAADqgAAkAAABQYAIBIBMSAAkAAAAe4AAJAAAAv2A=",
        )?;
        let dict = boc.parse::<RawDict<32>>()?;

        let size = dict.values().count();
        assert_eq!(size, 10);

        let mut rev_iter_items = dict.iter().reversed().collect::<Vec<_>>();
        rev_iter_items.reverse();
        let mut rev_iter_items = rev_iter_items.into_iter();

        for (i, entry) in dict.iter().enumerate() {
            let (key, value) = entry?;

            let (rev_key, rev_value) = rev_iter_items.next().unwrap().unwrap();
            assert_eq!(key, rev_key);
            assert_eq!(value.lex_cmp(&rev_value), Ok(std::cmp::Ordering::Equal));

            let key = key.as_data_slice().load_u32()?;
            assert_eq!(key, i as u32);
        }
        assert!(rev_iter_items.next().is_none());

        let mut last = None;
        for (i, entry) in dict.iter_owned().enumerate() {
            let (key, (range, cell)) = entry?;

            {
                let mut slice = range.apply(&cell).unwrap();
                assert_eq!(slice.size_bits(), 32);
                u32::load_from(&mut slice).unwrap();
            }

            let key = key.as_data_slice().load_u32()?;
            assert_eq!(key, i as u32);
            last = Some(key);
        }
        assert_eq!(last, Some(9));

        let mut values_ref = dict.values();
        let mut values_owned = dict.values_owned();
        for (value_ref, value_owned) in (&mut values_ref).zip(&mut values_owned) {
            let value_ref = value_ref.unwrap();
            let (range, cell) = value_owned.unwrap();
            let value_owned = range.apply(&cell).unwrap();
            assert_eq!(
                value_ref.lex_cmp(&value_owned),
                Ok(std::cmp::Ordering::Equal)
            );
        }
        assert!(values_ref.next().is_none());
        assert!(values_owned.next().is_none());

        Ok(())
    }

    #[test]
    fn dict_iter_union() -> anyhow::Result<()> {
        let mut left = RawDict::<32>::new();
        let mut right = RawDict::<32>::new();

        // Fill
        for i in -4i32..4 {
            let key = build_cell(|b| b.store_u32(i as _));
            left.set(key.as_slice()?, i)?;
        }
        for i in -2i32..6 {
            let key = build_cell(|b| b.store_u32(i as _));
            right.set(key.as_slice()?, i + 100)?;
        }

        fn compare_iter_values(iter: UnionRawIter<'_>, values: &[(i32, Option<i32>, Option<i32>)]) {
            let mut values = values.iter();

            for entry in iter {
                let (key, left_value, right_value) = entry.unwrap();
                let key = key.as_data_slice().load_u32().unwrap() as i32;
                let left_value = left_value.map(|v| v.get_u32(0).unwrap() as i32);
                let right_value = right_value.map(|v| v.get_u32(0).unwrap() as i32);

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

    #[derive(Debug, Default)]
    struct SimpleContext {
        used_gas: std::cell::Cell<u64>,
        loaded_cells: std::cell::RefCell<ahash::HashSet<HashBytes>>,
        empty_context: <Cell as CellFamily>::EmptyCellContext,
    }

    impl SimpleContext {
        const BUILD_CELL_GAS: u64 = 500;
        const NEW_CELL_GAS: u64 = 100;
        const OLD_CELL_GAS: u64 = 25;

        fn consume_gas(&self, cell: &DynCell, mode: LoadMode) {
            if mode.use_gas() {
                let consumed_gas = if self.loaded_cells.borrow_mut().insert(*cell.repr_hash()) {
                    Self::NEW_CELL_GAS
                } else {
                    Self::OLD_CELL_GAS
                };

                self.used_gas.set(self.used_gas.get() + consumed_gas);
            }
        }
    }

    impl CellContext for SimpleContext {
        #[inline]
        fn finalize_cell(&self, cell: CellParts<'_>) -> Result<Cell, Error> {
            self.used_gas
                .set(self.used_gas.get() + Self::BUILD_CELL_GAS);
            self.empty_context.finalize_cell(cell)
        }

        #[inline]
        fn load_cell(&self, cell: Cell, mode: LoadMode) -> Result<Cell, Error> {
            self.consume_gas(cell.as_ref(), mode);
            Ok(cell)
        }

        #[inline]
        fn load_dyn_cell<'s: 'a, 'a>(
            &self,
            cell: &'a DynCell,
            mode: LoadMode,
        ) -> Result<&'a DynCell, Error> {
            self.consume_gas(cell, mode);
            Ok(cell)
        }
    }

    #[test]
    fn dict_get_gas_usage() -> anyhow::Result<()> {
        // Prepare dict
        let mut dict = RawDict::<32>::new();
        for i in 0..10 {
            let mut key = CellBuilder::new();
            key.store_u32(i)?;
            dict.set(key.as_data_slice(), i)?;
        }

        // First get
        let context = &mut SimpleContext::default();

        let mut key = CellBuilder::new();
        key.store_u32(5)?;

        dict.get_ext(key.as_data_slice(), context)?.unwrap();
        assert_eq!(context.used_gas.get(), SimpleContext::NEW_CELL_GAS * 5);

        context.used_gas.set(0);
        dict.get_ext(key.as_data_slice(), context)?.unwrap();
        assert_eq!(context.used_gas.get(), SimpleContext::OLD_CELL_GAS * 5);

        // Second get
        context.used_gas.set(0);
        let mut key = CellBuilder::new();
        key.store_u32(9)?;

        dict.get_ext(key.as_data_slice(), context)?.unwrap();
        assert_eq!(
            context.used_gas.get(),
            SimpleContext::OLD_CELL_GAS + SimpleContext::NEW_CELL_GAS * 2
        );

        context.used_gas.set(0);
        dict.get_ext(key.as_data_slice(), context)?.unwrap();
        assert_eq!(context.used_gas.get(), SimpleContext::OLD_CELL_GAS * 3);

        Ok(())
    }

    #[test]
    fn dict_get_owned_gas_usage() -> anyhow::Result<()> {
        // Prepare dict
        let mut dict = RawDict::<32>::new();
        for i in 0..10 {
            let mut key = CellBuilder::new();
            key.store_u32(i)?;
            dict.set(key.as_data_slice(), i)?;
        }

        // First get
        let context = &mut SimpleContext::default();

        let mut key = CellBuilder::new();
        key.store_u32(5)?;

        dict.get_owned_ext(key.as_data_slice(), context)?.unwrap();
        assert_eq!(context.used_gas.get(), SimpleContext::NEW_CELL_GAS * 5);

        context.used_gas.set(0);
        dict.get_owned_ext(key.as_data_slice(), context)?.unwrap();
        assert_eq!(context.used_gas.get(), SimpleContext::OLD_CELL_GAS * 5);

        // Second get
        context.used_gas.set(0);
        let mut key = CellBuilder::new();
        key.store_u32(9)?;

        dict.get_owned_ext(key.as_data_slice(), context)?.unwrap();
        assert_eq!(
            context.used_gas.get(),
            SimpleContext::OLD_CELL_GAS + SimpleContext::NEW_CELL_GAS * 2
        );

        context.used_gas.set(0);
        dict.get_owned_ext(key.as_data_slice(), context)?.unwrap();
        assert_eq!(context.used_gas.get(), SimpleContext::OLD_CELL_GAS * 3);

        Ok(())
    }

    #[test]
    fn dict_remove_gas_usage() -> anyhow::Result<()> {
        let mut dict = RawDict::<32>::new();
        for i in 0..10 {
            let mut key = CellBuilder::new();
            key.store_u32(i)?;
            dict.set(key.as_data_slice(), i)?;
        }

        // Noop remove
        let mut key = CellBuilder::new();
        key.store_u32(10)?;

        let context = &mut SimpleContext::default();
        assert!(dict.remove_ext(key.as_data_slice(), context)?.is_none());

        assert_eq!(context.used_gas.get(), SimpleContext::NEW_CELL_GAS * 2);

        // Clear dict
        let target_gas = [
            SimpleContext::NEW_CELL_GAS * 6 + SimpleContext::BUILD_CELL_GAS * 4,
            SimpleContext::NEW_CELL_GAS * 5 + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 5 + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 4 + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 5 + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 4 + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 4 + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 3 + SimpleContext::BUILD_CELL_GAS,
            SimpleContext::NEW_CELL_GAS * 3 + SimpleContext::BUILD_CELL_GAS,
            SimpleContext::NEW_CELL_GAS,
        ];

        for i in 0..10 {
            println!("===");

            let context = &mut SimpleContext::default();

            let mut key = CellBuilder::new();
            key.store_u32(i)?;

            let removed = dict.remove_ext(key.as_data_slice(), context)?;
            assert!(removed.is_some());

            assert_eq!(context.used_gas.get(), target_gas[i as usize]);
        }

        Ok(())
    }

    #[test]
    fn dict_remove_bound() -> anyhow::Result<()> {
        let make_dict = |range: std::ops::Range<i32>| {
            let mut dict = RawDict::<32>::new();
            for i in range {
                let mut key = CellBuilder::new();
                key.store_u32(i as u32)?;
                dict.set(key.as_data_slice(), i)?;
            }
            Ok::<_, anyhow::Error>(dict)
        };

        let check_range =
            |range: std::ops::Range<i32>, bound: DictBound, signed: bool, target_gas: &[u64]| {
                let mut dict = make_dict(range.clone())?;
                for &target_gas in target_gas {
                    println!("=== {range:?} bound={bound:?} signed={signed} [non-owned]");
                    let context = &mut SimpleContext::default();
                    let (key, _) = dict.get_bound_ext(bound, signed, context)?.unwrap();
                    let removed = dict.clone().remove_ext(key.as_data_slice(), context)?;
                    assert!(removed.is_some());
                    assert_eq!(context.used_gas.get(), target_gas);

                    println!("=== {range:?} bound={bound:?} signed={signed} [owned]");
                    let context = &mut SimpleContext::default();
                    let (key, _) = dict.get_bound_owned_ext(bound, signed, context)?.unwrap();
                    let removed = dict.remove_ext(key.as_data_slice(), context)?;
                    assert!(removed.is_some());
                    assert_eq!(context.used_gas.get(), target_gas);
                }

                Ok::<_, anyhow::Error>(())
            };

        // Unsigned MIN
        let target_gas = [
            SimpleContext::NEW_CELL_GAS * 6
                + SimpleContext::OLD_CELL_GAS * 5
                + SimpleContext::BUILD_CELL_GAS * 4,
            SimpleContext::NEW_CELL_GAS * 5
                + SimpleContext::OLD_CELL_GAS * 4
                + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 5
                + SimpleContext::OLD_CELL_GAS * 4
                + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 5
                + SimpleContext::OLD_CELL_GAS * 4
                + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 3
                + SimpleContext::OLD_CELL_GAS * 2
                + SimpleContext::BUILD_CELL_GAS,
            SimpleContext::NEW_CELL_GAS * 3
                + SimpleContext::OLD_CELL_GAS * 2
                + SimpleContext::BUILD_CELL_GAS,
            SimpleContext::NEW_CELL_GAS + SimpleContext::OLD_CELL_GAS,
        ];
        check_range(0..10, DictBound::Min, false, &target_gas)?;

        // Unsigned MAX
        let target_gas = [
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 3
                + SimpleContext::OLD_CELL_GAS * 2
                + SimpleContext::BUILD_CELL_GAS,
            SimpleContext::NEW_CELL_GAS * 5
                + SimpleContext::OLD_CELL_GAS * 4
                + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 3
                + SimpleContext::OLD_CELL_GAS * 2
                + SimpleContext::BUILD_CELL_GAS,
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 3
                + SimpleContext::OLD_CELL_GAS * 2
                + SimpleContext::BUILD_CELL_GAS,
            SimpleContext::NEW_CELL_GAS * 3
                + SimpleContext::OLD_CELL_GAS * 2
                + SimpleContext::BUILD_CELL_GAS,
            SimpleContext::NEW_CELL_GAS + SimpleContext::OLD_CELL_GAS,
        ];
        check_range(0..10, DictBound::Max, false, &target_gas)?;

        // Signed MIN and MAX
        let target_gas = [
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 5
                + SimpleContext::OLD_CELL_GAS * 4
                + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 3
                + SimpleContext::OLD_CELL_GAS * 2
                + SimpleContext::BUILD_CELL_GAS,
            SimpleContext::NEW_CELL_GAS * 5
                + SimpleContext::OLD_CELL_GAS * 4
                + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 4
                + SimpleContext::OLD_CELL_GAS * 3
                + SimpleContext::BUILD_CELL_GAS * 2,
            SimpleContext::NEW_CELL_GAS * 3
                + SimpleContext::OLD_CELL_GAS * 2
                + SimpleContext::BUILD_CELL_GAS,
            SimpleContext::NEW_CELL_GAS + SimpleContext::OLD_CELL_GAS,
        ];

        // NOTE: same gas for balanced tree
        check_range(-5..5, DictBound::Min, true, &target_gas)?;
        check_range(-5..5, DictBound::Max, true, &target_gas)?;

        Ok(())
    }

    #[test]
    fn dict_insert_gas_usage() -> anyhow::Result<()> {
        let target_gas = [
            SimpleContext::BUILD_CELL_GAS,
            SimpleContext::NEW_CELL_GAS + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 2 + SimpleContext::BUILD_CELL_GAS * 4,
            SimpleContext::NEW_CELL_GAS + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 2 + SimpleContext::BUILD_CELL_GAS * 4,
            SimpleContext::NEW_CELL_GAS * 2 + SimpleContext::BUILD_CELL_GAS * 4,
            SimpleContext::NEW_CELL_GAS * 3 + SimpleContext::BUILD_CELL_GAS * 5,
            SimpleContext::NEW_CELL_GAS + SimpleContext::BUILD_CELL_GAS * 3,
            SimpleContext::NEW_CELL_GAS * 2 + SimpleContext::BUILD_CELL_GAS * 4,
        ];

        // RawDict
        let mut dict = RawDict::<32>::new();
        for i in 0..10 {
            let context = &mut SimpleContext::default();

            let mut key = CellBuilder::new();
            key.store_u32(i)?;

            dict.set_ext(key.as_data_slice(), &i, context)?;

            assert_eq!(context.used_gas.get(), target_gas[i as usize]);

            println!("===");
        }

        // Compare `dict_insert` and `dict_insert_owned`
        let mut dict = None::<Cell>;
        for i in 0..10 {
            let mut key = CellBuilder::new();
            key.store_u32(i)?;

            let context = &mut SimpleContext::default();
            let mut old_root = dict.clone();
            dict_insert(
                &mut dict,
                &mut key.as_data_slice(),
                32,
                &i,
                SetMode::Set,
                context,
            )?;
            assert_eq!(context.used_gas.get(), target_gas[i as usize]);

            println!("===");

            let context = &mut SimpleContext::default();
            let expected_new_root = dict.clone();
            crate::dict::dict_insert_owned(
                &mut old_root,
                &mut key.as_data_slice(),
                32,
                &i,
                SetMode::Set,
                context,
            )?;
            assert_eq!(dict, expected_new_root);

            assert_eq!(context.used_gas.get(), target_gas[i as usize]);

            println!("===");
        }

        // Check `add` as noop
        let mut key = CellBuilder::new();
        key.store_u32(5)?;

        let context = &mut SimpleContext::default();
        dict_insert(
            &mut dict,
            &mut key.as_data_slice(),
            32,
            &5u32,
            SetMode::Add,
            context,
        )?;
        assert_eq!(context.used_gas.get(), SimpleContext::NEW_CELL_GAS * 5); // Equivalent to simple get

        println!("===");

        let context = &mut SimpleContext::default();
        crate::dict::dict_insert_owned(
            &mut dict,
            &mut key.as_data_slice(),
            32,
            &5u32,
            SetMode::Add,
            context,
        )?;
        assert_eq!(context.used_gas.get(), SimpleContext::NEW_CELL_GAS * 5); // Equivalent to simple get

        Ok(())
    }
}
