use crate::generics_helpers::GenericSubstitutionsDeducing;
use crate::type_db::TypeDb;
use crate::type_interner::{TyId, TypeInterner};
use crate::types::TyData;
use std::cmp::Ordering;
use std::collections::HashMap;
use tolk_resolver::SymbolKind;
use tolk_resolver::file_index::SymbolId;
/*
 *   Find an exact method having a receiver type.
 *
 *   Given: int.copy, T.copy, Container<T>.copy
 * > 5.copy();                       // 1
 * > (5 as int8).copy();             // 2 with T=int8
 * > containerOfInt.copy();          // 3 with T=int
 * > nullableContainerOfInt.copy();  // 2 with T=Container<int>?
 *
 */

/// each next shape kind is more specific than another;
/// e.g., between `T.copy` and `int.copy` we choose the second;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShapeKind {
    GenericT,     // T
    Union,        // U|V, T?
    Primitive,    // int, slice, address, ...
    Tensor,       // (A,B,...)
    Instantiated, // Map<K,V>, Container<T>, Struct<X>, ...
}

/// for every receiver, we calculate "score": how deep and specific it is;
/// e.g., between `Container<T>` and `T` we choose the first;
/// e.g., between `map<int8, V>` and `map<K, map<K, K>>` we choose the second;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapeScore {
    pub kind: ShapeKind,
    pub depth: i32,
}

impl ShapeScore {
    pub fn is_shape_better_than(&self, other: &ShapeScore) -> bool {
        match self.kind.cmp(&other.kind) {
            Ordering::Greater => true,
            Ordering::Less => false,
            Ordering::Equal => self.depth > other.depth,
        }
    }
}

/// calculate score for a receiver;
/// note: it's an original receiver, with generics, not an instantiated one
pub fn calculate_shape_score(id: TyId, interner: &TypeInterner) -> ShapeScore {
    let unwrapped = interner.unwrap_alias(id);
    let data = interner.data(unwrapped);
    match data {
        TyData::TypeParameter { .. } => ShapeScore {
            kind: ShapeKind::GenericT,
            depth: 1,
        },
        TyData::Union(variants) => {
            let mut max_depth = 0;
            for &variant in variants {
                max_depth = max_depth.max(calculate_shape_score(variant, interner).depth);
            }
            ShapeScore {
                kind: ShapeKind::Union,
                depth: 1 + max_depth,
            }
        }
        TyData::Tensor(items) | TyData::Tuple(items) => {
            let mut max_depth = 0;
            for &item in items {
                max_depth = max_depth.max(calculate_shape_score(item, interner).depth);
            }
            ShapeScore {
                kind: ShapeKind::Tensor,
                depth: 1 + max_depth,
            }
        }
        TyData::Instantiation { types, .. } => {
            let mut max_depth = 0;
            for &t in types {
                max_depth = max_depth.max(calculate_shape_score(t, interner).depth);
            }
            ShapeScore {
                kind: ShapeKind::Instantiated,
                depth: 1 + max_depth,
            }
        }
        TyData::MapKV { key, value } => {
            let depth = calculate_shape_score(*key, interner)
                .depth
                .max(calculate_shape_score(*value, interner).depth);
            ShapeScore {
                kind: ShapeKind::Instantiated,
                depth: 1 + depth,
            }
        }
        TyData::TypeAlias { inner_ty, .. } => calculate_shape_score(*inner_ty, interner),
        _ => ShapeScore {
            kind: ShapeKind::Primitive,
            depth: 1,
        },
    }
}

pub struct MethodCallCandidate {
    pub original_receiver: TyId,
    pub instantiated_receiver: TyId,
    pub method_id: SymbolId,
    pub substitutions: HashMap<String, TyId>,
}

impl MethodCallCandidate {
    pub fn is_generic(&self, interner: &TypeInterner) -> bool {
        interner.has_generics(self.original_receiver)
    }
}

/// tries to find Ts in `pattern` to reach `actual`;
/// example: pattern=`map<K, slice>`, actual=`map<int, slice>` => T=int
/// example: pattern=`Container<T>`, actual=`Container<Container<U>>` => T=Container<U>
fn can_substitute_to_reach_actual(pattern: TyId, actual: TyId, type_db: &mut TypeDb) -> bool {
    let mut deducer = GenericSubstitutionsDeducing::new();
    let replaced = deducer.auto_deduce_from_argument(pattern, actual, type_db.intrn);
    type_db.intrn.equals(replaced, actual)
}

/// checks whether a generic typeA is more specific than typeB;
/// example: `map<int,V>` dominates `map<K,V>`;
/// example: `map<K, map<K,K>>` dominates `map<K, map<K,V>>` dominates `map<K1, map<K2,V>>`;
/// example: `map<int,V>` and `map<K,slice>` are not comparable;
pub fn is_more_specific_generic(type_a: TyId, type_b: TyId, type_db: &mut TypeDb) -> bool {
    // exists θ: θ(B)=A && not exists φ: φ(A)=B
    can_substitute_to_reach_actual(type_b, type_a, type_db)
        && !can_substitute_to_reach_actual(type_a, type_b, type_db)
}

/// the main "overload resolution" entrypoint: given `obj.method()`, find best applicable methods;
/// if there are many (no one is better than others), a caller side will emit "ambiguous call"
pub fn resolve_methods_for_call(
    provided_receiver: TyId,
    called_name: &str,
    type_db: &mut TypeDb,
) -> Vec<MethodCallCandidate> {
    // find all methods theoretically applicable; we'll filter them by priority;
    // for instance, if there is `T.method`, it will be instantiated with T=provided_receiver
    let mut viable = Vec::new();

    let file_ids: Vec<_> = type_db.project_index.files().keys().cloned().collect();
    for file_id in file_ids {
        let file_index = &type_db.project_index.files()[&file_id];
        for symbol in &file_index.decls {
            let SymbolKind::Method { .. } = symbol.kind else {
                continue;
            };

            if symbol.name.as_ref() != called_name {
                continue;
            }

            let Some(&receiver) = type_db.receiver_types.get(&symbol.id) else {
                continue;
            };

            let intn = &mut type_db.intrn;
            if intn.has_generics(receiver) {
                let mut deducer = GenericSubstitutionsDeducing::new();
                let replaced = deducer.auto_deduce_from_argument(receiver, provided_receiver, intn);

                if intn.can_rhs_be_assigned(replaced, provided_receiver) {
                    viable.push(MethodCallCandidate {
                        original_receiver: receiver,
                        instantiated_receiver: replaced,
                        method_id: symbol.id,
                        substitutions: deducer.substitutions.mapping,
                    });
                }
            } else if intn.can_rhs_be_assigned(receiver, provided_receiver) {
                viable.push(MethodCallCandidate {
                    original_receiver: receiver,
                    instantiated_receiver: receiver,
                    method_id: symbol.id,
                    substitutions: HashMap::new(),
                });
            }
        }
    }

    // if nothing found, return nothing;
    // if the only found, it's the one
    if viable.len() <= 1 {
        return viable;
    }

    // 1) exact match candidates with equal_to()
    //    (for instance, an alias equals to its underlying type, as well as `T1|T2` equals to `T2|T1`)
    let mut exact = Vec::new();
    for candidate in &viable {
        if type_db
            .intrn
            .equals(candidate.instantiated_receiver, provided_receiver)
        {
            exact.push(MethodCallCandidate {
                original_receiver: candidate.original_receiver,
                instantiated_receiver: candidate.instantiated_receiver,
                method_id: candidate.method_id,
                substitutions: candidate.substitutions.clone(),
            });
        }
    }
    if exact.len() == 1 {
        return exact;
    }
    if !exact.is_empty() {
        viable = exact;
    }

    // 2) if there are both generic and non-generic functions, filter out generic
    let n_generics = viable
        .iter()
        .filter(|c| c.is_generic(type_db.intrn))
        .count();
    if n_generics < viable.len() {
        viable.retain(|c| !c.is_generic(type_db.intrn));
        return viable;
    }

    // 3) better shape in terms of structural depth
    //    (prefer `Container<T>` over `T` and `map<K1, map<K2,V2>>` over `map<K,V>`)
    let mut best_shape = ShapeScore {
        kind: ShapeKind::GenericT,
        depth: -999,
    };
    for candidate in &viable {
        let s = calculate_shape_score(candidate.original_receiver, type_db.intrn);
        if s.is_shape_better_than(&best_shape) {
            best_shape = s;
        }
    }

    viable.retain(|c| calculate_shape_score(c.original_receiver, type_db.intrn) == best_shape);
    if viable.len() == 1 {
        return viable;
    }

    // 4) find the overload that dominates all others
    //    (prefer `Container<int>` over `Container<T>` and `map<K, slice>` over `map<K, V>`)
    let mut dominator_idx = None;
    for i in 0..viable.len() {
        let mut dominates_all = true;
        for j in 0..viable.len() {
            if i != j
                && !is_more_specific_generic(
                    viable[i].original_receiver,
                    viable[j].original_receiver,
                    type_db,
                )
            {
                dominates_all = false;
                break;
            }
        }
        if dominates_all {
            if dominator_idx.is_some() {
                // Ambiguous
                return viable;
            }
            dominator_idx = Some(i);
        }
    }

    if let Some(idx) = dominator_idx {
        return vec![viable.remove(idx)];
    }

    viable
}

pub fn choose_only_method_to_call(
    provided_receiver: TyId,
    called_name: &str,
    type_db: &mut TypeDb,
) -> Result<Option<MethodCallCandidate>, String> {
    let candidates = resolve_methods_for_call(provided_receiver, called_name, type_db);

    if candidates.is_empty() {
        return Ok(None);
    }

    if candidates.len() == 1 {
        // We have to move out of the vector, so we need ownership
        // candidates is a Vec, so we can pop or swap_remove
        return Ok(candidates.into_iter().next());
    }

    // Ambiguous call
    let mut msg = format!(
        "call to method `{}` for type `{}` is ambiguous\n",
        called_name,
        type_db.intrn.format(provided_receiver)
    );

    for candidate in &candidates {
        let method_name = type_db
            .project_index
            .resolve_symbol(candidate.method_id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "unknown".into());

        msg.push_str(&format!("candidate function: `{}`", method_name));

        if candidate.is_generic(type_db.intrn) {
            // TODO: format substitutions nicely
            msg.push_str(" with substitutions");
        }
        msg.push('\n');
    }

    Err(msg)
}
