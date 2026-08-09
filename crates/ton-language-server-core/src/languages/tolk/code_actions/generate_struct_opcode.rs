use super::{CodeActionProvider, TolkCodeActionContext};
use crate::{CodeAction, Range, TextEdit};
use tolk_analysis::compute_struct_opcode;
use tolk_syntax::{AstNode, HasName, Struct, StructField};

/// Generates a deterministic 32-bit CRC opcode for an unprefixed struct declaration.
pub(super) struct GenerateStructOpcodeProvider;

impl CodeActionProvider for GenerateStructOpcodeProvider {
    fn collect(
        &self,
        context: &TolkCodeActionContext<'_>,
        actions: &mut Vec<CodeAction>,
    ) -> Option<()> {
        let structure = context.ancestor_as::<Struct>()?;
        if structure.pack_prefix().is_some() {
            return None;
        }
        if context
            .ancestor_as::<StructField>()
            .and_then(|field| field.owner())
            .is_some_and(|owner| owner.syntax() == structure.syntax())
        {
            return None;
        }
        let name = structure.name()?;
        let opcode = compute_struct_opcode(context.text_of(name));
        let position = context
            .document
            .text_index()
            .offset_to_position(context.document.text(), name.syntax().start_byte());
        let edit = TextEdit::new(Range::new(position, position), format!("(0x{opcode:08x}) "));
        actions.push(context.action("Generate 32-bit opcode", edit));
        Some(())
    }
}
