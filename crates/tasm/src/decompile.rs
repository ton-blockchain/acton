use crate::spec::*;
use crate::types::{ArgValue, Code, CodeDictionary, Control, Instruction, Method, StackRegister};
use anyhow::anyhow;
use num_bigint::{BigInt, BigUint};
use num_traits::ToPrimitive;
use smallvec::smallvec;
use tycho_types::boc::Boc;
use tycho_types::cell::{Cell, CellBuilder, CellFamily, CellSlice, DynCell};
use tycho_types::dict::RawDict;

const SPEC: &str = include_str!("../spec/tvm-specification.json");

#[derive(Debug)]
pub struct Disassembler {
    list: Vec<InstructionWithRange>,
}

impl Disassembler {
    pub fn new() -> Disassembler {
        let spec: Specification =
            serde_json::from_str(SPEC).expect("Failed to parse built-in TVM specification JSON");
        Self::from_instructions(&spec.instructions)
    }

    pub fn from_instructions(instructions: &[SpecInstruction]) -> Disassembler {
        let mut instruction_ranges: Vec<InstructionWithRange> = instructions
            .iter()
            .map(|instr| InstructionWithRange {
                min: instr.layout.min,
                max: instr.layout.max,
                instr: Some(instr.clone()),
            })
            .collect();

        let mut list = Vec::new();
        let top_opcode = 1i64 << MAX_OPCODE_BITS;

        instruction_ranges.sort_by_key(|r| r.min);

        // fill gaps between instruction ranges with empty ranges (no instruction)
        // This ensures binary search works correctly for all opcode values
        let mut upto = 0i64;
        for instr in instruction_ranges {
            if instr.min >= instr.max || instr.min < upto || instr.max > top_opcode {
                panic!("instruction list is invalid");
            }
            // add gap range if there's space between current position and next instruction
            if upto < instr.min {
                list.push(InstructionWithRange {
                    min: upto,
                    max: instr.min,
                    instr: None, // no instruction for this range
                });
            }
            upto = instr.max;
            list.push(instr);
        }

        // add final gap range to cover remaining opcodes up to maximum
        if upto < top_opcode {
            list.push(InstructionWithRange {
                min: upto,
                max: top_opcode,
                instr: None,
            });
        }

        Disassembler { list }
    }

    pub fn load_instruction(&self, slice: &mut CellSlice) -> anyhow::Result<Instruction> {
        let bits = std::cmp::min(slice.size_bits(), MAX_OPCODE_BITS);
        let opcode = slice.get_uint(0, bits)? << (MAX_OPCODE_BITS - bits);

        let instr_idx = self
            .list
            .partition_point(|instr| instr.min <= opcode as i64)
            .saturating_sub(1);

        let instr = &self.list[instr_idx];
        let Some(instruction) = instr.instr.as_ref() else {
            return Err(anyhow!(
                "found instruction is dummy one, max: {}, min: {}",
                instr.max,
                instr.min
            ));
        };
        let layout = &instruction.layout;

        // skip opcode, we already know the instruction
        slice.load_uint(layout.check_len as u16)?;

        let mut args = smallvec::SmallVec::with_capacity(3);

        // process DICTPUSHCONST-like instructions with separate logic
        if layout.args.0.len() == 2 && matches!(&layout.args.0[0], Arg::InlineDictArg(_)) {
            let key_length = slice.load_uint(10)?;
            let mut dict_slice = slice.load_reference()?.as_slice()?;
            let dict = RawDict::<19>::load_from_root_ext(&mut dict_slice, Cell::empty_context())?;

            let methods = dict
                .iter()
                .filter_map(|kv| {
                    let (key, mut value) = kv.ok()?;
                    let mut key_slice = key.as_data_slice();
                    let id = key_slice.load_uint(key_length as u16).ok()?;
                    let code = self.decompile_slice(&mut value).ok()?;
                    Some(Method {
                        id,
                        instructions: code.instructions,
                    })
                })
                .collect();

            args.push(ArgValue::UInt(BigUint::from(key_length)));
            args.push(ArgValue::CodeDictionary(CodeDictionary { methods }));
        } else {
            for child in &layout.args.0 {
                self.process_arg(child, slice, &mut args)?;
            }
        }

        Ok(Instruction {
            name: instruction.name.clone(),
            instr: Some(Box::new((*instruction).clone())),
            args,
        })
    }

    pub fn decompile_cell(&self, cell: &Cell) -> anyhow::Result<Code> {
        let mut slice = cell.as_slice()?;
        self.decompile_slice(&mut slice)
    }

    pub fn decompile_dyn_cell(&self, cell: &DynCell) -> anyhow::Result<Code> {
        let mut slice = cell.as_slice()?;
        self.decompile_slice(&mut slice)
    }

    pub fn decompile_slice(&self, slice: &mut CellSlice) -> anyhow::Result<Code> {
        let mut result = Vec::with_capacity(32);

        while slice.size_bits() > 0 {
            result.push(self.load_instruction(slice)?);
        }

        while slice.size_refs() > 0 {
            let ref_cell = slice.load_reference()?;
            let code = self.decompile_dyn_cell(ref_cell)?;
            // ref is a special pseudo-instruction that denotes code placed in reference
            result.push(Instruction {
                name: "ref".to_string(),
                instr: None,
                args: smallvec![ArgValue::Code(Box::new(code))],
            });
        }

        Ok(Code {
            instructions: result,
        })
    }

    fn process_arg(
        &self,
        arg: &Arg,
        slice: &mut CellSlice,
        args: &mut smallvec::SmallVec<[ArgValue; 3]>,
    ) -> anyhow::Result<()> {
        match arg {
            Arg::DeltaArg(delta_arg) => match &*delta_arg.arg {
                Arg::UintArg(uint_arg) => {
                    let value = slice.load_biguint(uint_arg.len as u16)? + delta_arg.delta as u64;
                    args.push(ArgValue::UInt(value));
                }
                Arg::IntArg(int_arg) => {
                    let value = slice.load_bigint(int_arg.len as u16, true)? + delta_arg.delta;
                    args.push(ArgValue::Int(value));
                }
                Arg::StackArg(_) => {
                    let y = slice.load_uint(4)? as i64;
                    let value = y + delta_arg.delta;
                    args.push(ArgValue::StackRegister(StackRegister { idx: value }));
                }
                _ => {}
            },
            Arg::IntArg(int_arg) => {
                let value = slice.load_bigint(int_arg.len as u16, true)?;
                args.push(ArgValue::Int(value));
            }
            Arg::UintArg(uint_arg) => {
                let value = slice.load_biguint(uint_arg.len as u16)?;
                args.push(ArgValue::UInt(value));
            }
            Arg::TinyIntArg(_) => {
                let value = ((slice.load_uint(4)?.to_i64().unwrap() + 5) & 15) - 5;
                args.push(ArgValue::Int(BigInt::from(value)));
            }
            Arg::LargeIntArg(_) => {
                let y = slice.load_uint(5)?;
                let value = slice.load_biguint((3 + ((y & 31) + 2) * 8) as u16)?;
                args.push(ArgValue::UInt(value));
            }
            Arg::PlduzArg(_) => {
                let y = slice.load_uint(3)?;
                let value = ((y & 7) + 1) << 5;
                args.push(ArgValue::UInt(BigUint::from(value)));
            }
            Arg::ControlArg(_) => {
                let value = slice.load_uint(4)?;
                args.push(ArgValue::Control(Control { idx: value }));
            }
            Arg::StackArg(stack) => {
                let value = slice.load_uint(stack.len as u16)? as i64;
                args.push(ArgValue::StackRegister(StackRegister { idx: value }));
            }
            Arg::S1Arg(_) => {
                args.push(ArgValue::StackRegister(StackRegister { idx: 1 }));
            }
            Arg::MinusOneArg(_) => {
                args.push(ArgValue::Int(BigInt::from(-1)));
            }
            Arg::RefCodeSliceArg(_) => {
                let val = slice.load_reference()?;
                let code = self.decompile_dyn_cell(val)?;
                args.push(ArgValue::Code(Box::new(code)));
            }
            Arg::InlineCodeSliceArg(inline_code) => {
                let Arg::UintArg(bits) = &*inline_code.bits else {
                    panic!("expected uint bits")
                };
                let y = slice.load_uint(bits.len as u16)?;
                let real_length = y * 8;
                let mut r = slice.load_prefix(real_length as u16, 0)?;
                let code = self.decompile_slice(&mut r)?;
                args.push(ArgValue::Code(Box::new(code)));
            }
            Arg::CodeSliceArg(code_slice) => {
                let Arg::UintArg(bits) = &*code_slice.bits else {
                    panic!("expected uint bits")
                };
                let Arg::UintArg(refs) = &*code_slice.refs else {
                    panic!("expected uint refs")
                };

                let count_refs = slice.load_uint(refs.len as u16)?;
                let y = slice.load_uint(bits.len as u16)?;
                let real_length = y * 8;
                let mut r = slice.load_prefix(real_length as u16, 0)?;

                if count_refs == 0 {
                    // optimization to not build a cell
                    let code = self.decompile_slice(&mut r)?;
                    args.push(ArgValue::Code(Box::new(code)));
                    return Ok(());
                }

                let mut builder = CellBuilder::new();
                builder.store_slice(&r)?;
                for _ in 0..count_refs {
                    builder.store_reference(dyn_cell_to_cell(slice.load_reference()?))?;
                }
                let code = self.decompile_cell(&builder.build()?)?;
                args.push(ArgValue::Code(Box::new(code)));
            }
            Arg::SliceArg(slice_arg) => {
                let slice_val = Self::load_slice(slice, slice_arg)?;
                args.push(ArgValue::Cell(slice_val));
            }
            Arg::DebugstrArg(_) => {
                let y = slice.load_uint(4)?.to_u64().unwrap();
                let real_length = (y + 1) * 8;
                let r = slice.load_prefix(real_length as u16, 0)?;
                let mut builder = CellBuilder::new();
                builder.store_slice(&r)?;
                args.push(ArgValue::Cell(builder.build()?));
            }
            &Arg::SetcpArg(_) | &Arg::InlineDictArg(_) | &Arg::ExoticCellArg(_) => {}
        }

        Ok(())
    }

    fn load_slice(slice: &mut CellSlice, arg: &SliceArg) -> anyhow::Result<Cell> {
        let count_refs: u64 = if let Arg::UintArg(arg) = &*arg.refs
            && arg.len > 0
        {
            slice.load_uint(arg.len as u16)?
        } else {
            0
        };

        let Arg::UintArg(bits) = &*arg.bits else {
            panic!("expected uint for bits")
        };

        let y = slice.load_uint(bits.len as u16)?.to_i64().unwrap();
        let real_length = (y * 8 + arg.pad) as u16;
        let mut r = slice.load_prefix(real_length, 0)?;

        // Find the position of the last set bit (MSB) to determine actual data length
        let mut length = 0usize;
        for i in (0..real_length).rev() {
            let byte_idx = i / 8;
            let data_byte = r.get_u8(byte_idx)?;
            let bit_shift = (i % 8) as u32;
            let bit = data_byte & (1 << (7 - bit_shift));
            if bit == 0 {
                continue;
            }
            length = i as usize;
            break;
        }

        let r = r.load_prefix(length as u16, 0)?;

        let mut builder = CellBuilder::new();
        builder.store_slice(&r)?;
        for _ in 0..count_refs {
            builder.store_reference(dyn_cell_to_cell(slice.load_reference()?))?;
        }
        Ok(builder.build()?)
    }
}

const MAX_OPCODE_BITS: u16 = 24;

#[derive(Debug)]
struct InstructionWithRange {
    min: i64,
    max: i64,
    instr: Option<SpecInstruction>,
}

fn dyn_cell_to_cell(cell: &DynCell) -> Cell {
    Boc::decode_base64(Boc::encode_base64(cell)).expect("Cell after encoding must be correct")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_disassemble_jetton_minter() {
        let code = Boc::decode_hex("b5ee9c7201021b0100075600021eff00208e8130e1f4a413f4bcf2c80b010204e001d072d721d200d200fa4021103450666f04f86102f862ed44d0fa00fa40d455206c1304925f04e002d70d1ff2e0822182107bdd97debae3022182102c76b973bae30221c0048e1f313302d33fd4596c21f8425220c705f2e049c855205afa0258cf16ccc9ed54e021c015e30201c0030506070802038e66030401a3adbcf6a2687d007d206a2a903609aa81387c140944642a90ad7d012c678b00e78b64b82c907c80117c802d6bb280ebb2c101009a64658be587e587e5ffe5ffb8fc8200643a00e581096503e5ffe4e83618c00c012faf16f6a2687d007d206a2a903609bfc4122caa3220361ac00c02d231d33ffa00fa40fa40553034f842fa440270f8281288c855215afa0258cf1601cf16c920f90022f9005ad76501d76582020134c8cb17cb0fcb0fcbffcbff71f9040001c00091ba925b70e2f2e04a13a121d70b01c300926c21e30d02c855205afa0258cf16ccc9ed540c0902d86c41d33ffa40d200552033f8416f2443305230fa40fa0071d721fa00fa00306c6170f83a8208989680a0bcf2e04b20fa4430c0008eb270f828522088c855215afa0258cf1601cf16c920f90022f9005ad76501d76582020134c8cb17cb0fcb0fcbffcbff71f90400916de2030c0a01ac31d33ffa40fa00d401d0d31f018210178d4519baf2e081d33ffa00fa4020d70b01c30093fa40019472d7216de201fa00515515144330361069106810675504363737f8425290c705f2e0495171a007707f50878040060b004a8e1dd33ffa40596c21f84213c705f2e04902c855205afa0258cf16ccc9ed54e05f04f2c082005802c8018210d53276db58cb1fcb3fc9707080425044c8cf8580ca00cf8440ce01fa02806acf40f400c901fb00009695c801cf16c992306de2c88210d173540001cb1f12cb3f58206e95307001cb0197830958cb0acbffe2f400c9f84270804043137fc8cf8580ca00cf8440ce01fa02806acf40f400c901fb0002fcc855508210178d45195007cb1f15cb3f5003fa0201cf1601206e95307001cb0192cf16e201fa0201cf16c90270f8281288c855215afa0258cf1601cf16c91023103510245f41f90001f9005ad76501d76582020134c8cb17cb0fcb0fcbffcbff71f9040003c8cf8580ca0012cccccf884008cbff01fa028069cf40cf86340c0d021eff00208e8130e1f4a413f4bcf2c80b0e0f0028f400c901fb0002c855205afa0258cf16ccc9ed54049401d072d721d200d200fa4021103450666f04f86102f862ed44d0fa00fa40fa4055206c1304e30202d70d1ff2e0822182100f8a7ea5bae302218210178d4519bae302018210595f07bcba101112130033a65ec0bb51343e803e903e9015481b04fe0a9518cc148c1b0d2000b2028020d7217021d749c21f9430d31f01de208210178d4519ba8e1930d33ffa00596c21a002c855205afa0258cf1601cf16c9ed54e082107bdd97deba8e18d33ffa00596c21a002c855205afa0258cf1601cf16c9ed54e05f0401fe31d33ffa00fa4020d70b01c30093fa40019472d7216de201d2000191d4926d01e2fa0051661615144330323622fa4430c000f2e14df8425280c705f2e2c15163a120c2fff2e2c226d749c200f2e2c4f8416f2429a471b044305244fa40fa0071d721fa00fa00306c6170f83aa85270a0820a625a00a0bcf2e2c550437080401401f831d33ffa00fa4020d70b01c30093fa40019472d7216de201fa00515515144330365163a0705339f82ac855215afa0258cf1601cf16c9f842fa44315920f90022f9005ad76501d76582020134c8cb17cb0fcb0fcbffcbff71f9040001bab398f84229c705f2e2c3def8416f2421f8276f1021a1820898968066b608a116010ee3025f04f2c0821901fc7f2a4813509ac855508210178d45195007cb1f15cb3f5003fa0201cf1601206e95307001cb0192cf16e201fa0201cf16c9525228f82ac855215afa0258cf1601cf16c9105610361045102410235f41f90001f9005ad76501d76582020134c8cb17cb0fcb0fcbffcbff71f9040003c8cf8580ca0012cccccf884008cbff0115003efa028069cf40cf8634f400c901fb0002c855205afa0258cf1601cf16c9ed5402fc8208e4e1c0a0a12bc2008e5a5530fa40fa0071d721fa00fa00306c6170f83a5280a0a171702747135069c8553082107362d09c5005cb1f13cb3f01fa0201cf1601cf16c9280410384500441359c8cf8580ca00cf8440ce01fa02806acf40f400c901fb001023963b5f04333430e2226eb39323c2009170e2926c31e30d021718005e727003c8018210d53276db58cb1fcb3fc910354150441359c8cf8580ca00cf8440ce01fa02806acf40f400c901fb00001ec855205afa0258cf1601cf16c9ed5401fed33ffa00fa40d2000191d4926d01e255303033f8425250c705f2e2c15133a120c2fff2e2c2f8416f2443305230fa40fa0071d721fa00fa00306c6170f83a8209c9c380a0bcf2e2c37080405413567f06c8553082107bdd97de5005cb1f13cb3f01fa0201cf1601cf16c9264544441359c8cf8580ca00cf8440ce01fa02806a1a0030cf40f400c901fb0002c855205afa0258cf1601cf16c9ed54").unwrap();
        let disassembler = Disassembler::new();

        let code = disassembler.decompile_cell(&code).unwrap();

        let res = format!("{}", code);
        assert_eq!(res.len(), 132511)
    }
}
