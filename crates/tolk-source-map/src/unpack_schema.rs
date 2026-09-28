//! Adapt debug symbols to the standalone runtime decoder.

use crate::source_map::{Declaration, SourceMap};
use crate::types_kernel::TyIdx;
use tolk_abi::dynamic_unpack::{
    SchemaAliasDecl, SchemaAliasTarget, SchemaEnumDecl, SchemaEnumMember, SchemaField,
    SchemaPrefix, SchemaStructDecl, UnpackSchema,
};

impl UnpackSchema for SourceMap {
    fn struct_decl_info(&self, target_name: &str) -> Option<SchemaStructDecl<'_>> {
        self.declarations().iter().find_map(|decl| match decl {
            Declaration::Struct(struct_decl) if struct_decl.name == target_name => {
                Some(SchemaStructDecl {
                    prefix: struct_decl.prefix.as_ref().map(|prefix| SchemaPrefix {
                        prefix_num: prefix.prefix_num,
                        prefix_len: prefix.prefix_len,
                    }),
                    custom_pack_unpack: struct_decl.custom_pack_unpack.as_ref(),
                })
            }
            _ => None,
        })
    }

    fn alias_decl_info(&self, target_name: &str) -> Option<SchemaAliasDecl<'_>> {
        self.declarations().iter().find_map(|decl| match decl {
            Declaration::Alias(alias_decl) if alias_decl.name == target_name => {
                Some(SchemaAliasDecl {
                    custom_pack_unpack: alias_decl.custom_pack_unpack.as_ref(),
                })
            }
            _ => None,
        })
    }

    fn enum_decl_info(&self, target_name: &str) -> Option<SchemaEnumDecl<'_>> {
        self.declarations().iter().find_map(|decl| match decl {
            Declaration::Enum(enum_decl) if enum_decl.name == target_name => Some(SchemaEnumDecl {
                name: enum_decl.name.clone(),
                encoded_as_ty_idx: enum_decl.encoded_as_ty_idx,
                members: enum_decl
                    .members
                    .iter()
                    .map(|member| SchemaEnumMember {
                        name: member.name.clone(),
                        value: member.value.clone(),
                    })
                    .collect(),
                custom_pack_unpack: enum_decl.custom_pack_unpack.as_ref(),
            }),
            _ => None,
        })
    }

    fn struct_fields_for(&self, ty_idx: TyIdx) -> Option<Vec<SchemaField>> {
        self.struct_fields_of(ty_idx).map(|fields| {
            fields
                .into_iter()
                .map(|field| SchemaField {
                    name: field.name,
                    ty_idx: field.ty_idx,
                    u_label_ty_idx: None,
                })
                .collect()
        })
    }

    fn alias_target_for(&self, ty_idx: TyIdx) -> Option<SchemaAliasTarget> {
        self.alias_target_of(ty_idx)
            .map(|ty_idx| SchemaAliasTarget {
                ty_idx,
                u_label_ty_idx: None,
            })
    }
}
