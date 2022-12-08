//! Instructions for data manipulation, but mostly control flow.
//!
//! Since Sway abstracts most low level operations behind traits they are translated into function
//! calls which contain ASM blocks.
//!
//! Unfortuntely, using opaque ASM blocks limits the effectiveness of certain optimizations and
//! this should be addressed in the future, perhaps by using compiler intrinsic calls instead of
//! the ASM blocks where possible. See: https://github.com/FuelLabs/sway/issues/855,

use rustc_hash::FxHashMap;
use sway_types::ident::Ident;

use crate::{
    asm::{AsmArg, AsmBlock, AsmInstruction},
    block::Block,
    context::Context,
    function::Function,
    irtype::Type,
    pointer::Pointer,
    pretty::DebugWithContext,
    value::{Value, ValueDatum},
    Constant,
};

#[derive(Debug, Clone, DebugWithContext)]
pub struct BranchToWithArgs {
    pub block: Block,
    pub args: Vec<Value>,
}

#[derive(Debug, Clone, DebugWithContext)]
pub enum Instruction {
    /// Address of a non-copy (memory) value
    AddrOf(Value),
    /// An opaque list of ASM instructions passed directly to codegen.
    AsmBlock(AsmBlock, Vec<AsmArg>),
    /// Binary arithmetic operations
    BinaryOp {
        op: BinaryOpKind,
        arg1: Value,
        arg2: Value,
    },
    /// Cast the type of a value without changing its actual content.
    BitCast(Value, Type),
    /// An unconditional jump.
    Branch(BranchToWithArgs),
    /// A function call with a list of arguments.
    Call(Function, Vec<Value>),
    /// Comparison between two values using various comparators and returning a boolean.
    Cmp(Predicate, Value, Value),
    /// A conditional jump with the boolean condition value and true or false destinations.
    ConditionalBranch {
        cond_value: Value,
        true_block: BranchToWithArgs,
        false_block: BranchToWithArgs,
    },
    /// A contract call with a list of arguments
    ContractCall {
        return_type: Type,
        name: String,
        params: Value,
        coins: Value,
        asset_id: Value,
        gas: Value,
    },
    /// Generate a unique integer value
    GetStorageKey,
    Gtf {
        index: Value,
        tx_field_id: u64,
    },
    /// Return a pointer as a value.
    GetPointer {
        base_ptr: Pointer,
        ptr_ty: Pointer,
        offset: u64,
    },
    /// Address computation instruction. See LLVM's GetElementPtr instruction.
    GetElmPtr {
        ptr: Value,
        pointee_ty: Type,
        indices: Vec<Value>,
    },
    /// Re-interpret an integer value as pointer of some type
    IntToPtr(Value, Type),
    /// Read a value from a memory pointer.
    Load(Value),
    /// Logs a value along with an identifier.
    Log {
        log_val: Value,
        log_ty: Type,
        log_id: Value,
    },
    /// Copy a specified number of bytes between pointers.
    MemCopy {
        dst_val: Value,
        src_val: Value,
        byte_len: u64,
    },
    /// No-op, handy as a placeholder instruction.
    Nop,
    /// Reads a special register in the VM.
    ReadRegister(Register),
    /// Return from a function.
    Ret(Value, Type),
    /// Revert VM execution.
    Revert(Value),
    /// Read a quad word from a storage slot. Type of `load_val` must be a B256 ptr.
    StateLoadQuadWord {
        load_val: Value,
        key: Value,
    },
    /// Read a single word from a storage slot.
    StateLoadWord(Value),
    /// Write a value to a storage slot.  Key must be a B256, type of `stored_val` must be a
    /// Uint(256) ptr.
    StateStoreQuadWord {
        stored_val: Value,
        key: Value,
    },
    /// Write a value to a storage slot.  Key must be a B256, type of `stored_val` must be a
    /// Uint(64) value.
    StateStoreWord {
        stored_val: Value,
        key: Value,
    },
    /// Write a value to a memory pointer.
    Store {
        dst_val: Value,
        stored_val: Value,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum Predicate {
    /// Equivalence.
    Equal,
    // More soon.  NotEqual, LessThan, LessThanOrEqual, GreaterThan, GreaterThanOrEqual.
}

#[derive(Debug, Clone, Copy)]
pub enum BinaryOpKind {
    Add,
    Sub,
    Mul,
    Div,
}

/// Special registers in the Fuel Virtual Machine.
#[derive(Debug, Clone, Copy)]
pub enum Register {
    /// Contains overflow/underflow of addition, subtraction, and multiplication.
    Of,
    /// The program counter. Memory address of the current instruction.
    Pc,
    /// Memory address of bottom of current writable stack area.
    Ssp,
    /// Memory address on top of current writable stack area (points to free memory).
    Sp,
    /// Memory address of beginning of current call frame.
    Fp,
    /// Memory address below the current bottom of the heap (points to free memory).
    Hp,
    /// Error codes for particular operations.
    Error,
    /// Remaining gas globally.
    Ggas,
    /// Remaining gas in the context.
    Cgas,
    /// Received balance for this context.
    Bal,
    /// Pointer to the start of the currently-executing code.
    Is,
    /// Return value or pointer.
    Ret,
    /// Return value length in bytes.
    Retl,
    /// Flags register.
    Flag,
}

impl Instruction {
    /// Some [`Instruction`]s can return a value, but for some a return value doesn't make sense.
    ///
    /// Those which perform side effects such as writing to memory and also terminators such as
    /// `Ret` do not have a type.
    pub fn get_type(&self, context: &Context) -> Option<Type> {
        match self {
            Instruction::AddrOf(_) => Some(Type::get_uint(context, 64)),
            Instruction::AsmBlock(asm_block, _) => Some(asm_block.get_type(context)),
            Instruction::BinaryOp { arg1, .. } => arg1.get_type(context),
            Instruction::BitCast(_, ty) => Some(*ty),
            Instruction::Call(function, _) => Some(context.functions[function.0].return_type),
            Instruction::Cmp(..) => Some(Type::get_bool(context)),
            Instruction::ContractCall { return_type, .. } => Some(*return_type),
            Instruction::GetStorageKey => Some(Type::get_b256(context)),
            Instruction::Gtf { .. } => Some(Type::get_uint(context, 64)),
            Instruction::Load(ptr_val) => match &context.values[ptr_val.0].value {
                ValueDatum::Argument(arg) => Some(arg.ty.strip_ptr_type(context)),
                ValueDatum::Constant(cons) => Some(cons.ty.strip_ptr_type(context)),
                ValueDatum::Instruction(ins) => {
                    ins.get_type(context).map(|f| f.strip_ptr_type(context))
                }
            },
            Instruction::Log { .. } => Some(Type::get_unit(context)),
            Instruction::ReadRegister(_) => Some(Type::get_uint(context, 64)),
            Instruction::StateLoadWord(_) => Some(Type::get_uint(context, 64)),

            // These can be recursed to via Load, so we return the pointer type.
            Instruction::GetPointer { ptr_ty, .. } => {
                Some(Type::get_pointer(context, ptr_ty.get_type(context)))
            }
            Instruction::GetElmPtr {
                ptr: _,
                pointee_ty,
                indices,
            } => {
                let ty = pointee_ty
                    .get_indexed_type(context, indices)
                    .expect("GetElmPtr indexing error");
                Some(Type::get_pointer(context, ty))
            }

            // Used to re-interpret an integer as a pointer to some type so return the pointer type.
            Instruction::IntToPtr(_, ty) => Some(*ty),

            // These are all terminators which don't return, essentially.  No type.
            Instruction::Branch(_) => None,
            Instruction::ConditionalBranch { .. } => None,
            Instruction::Ret(..) => None,
            Instruction::Revert(..) => None,

            Instruction::MemCopy { .. } => Some(Type::get_unit(context)),
            Instruction::StateLoadQuadWord { .. } => Some(Type::get_unit(context)),
            Instruction::StateStoreQuadWord { .. } => Some(Type::get_unit(context)),
            Instruction::StateStoreWord { .. } => Some(Type::get_unit(context)),
            Instruction::Store { .. } => Some(Type::get_unit(context)),

            // No-op is also no-type.
            Instruction::Nop => None,
        }
    }

    /// Some [`Instruction`]s may have struct arguments.  Return it if so for this instruction.
    pub fn get_aggregate(&self, context: &Context) -> Option<Type> {
        let ty = match self {
            Instruction::Call(func, _args) => Some(context.functions[func.0].return_type),
            Instruction::GetPointer { ptr_ty, .. } => Some(ptr_ty.get_type(context)),
            Instruction::GetElmPtr { .. } => self.gep_indexed_type(context),
            // Unknown aggregate instruction.  Adding these as we come across them...
            _otherwise => None,
        };
        if let Some(ty) = ty {
            // TODO: Rewrite using if-let chain.
            if ty.is_array(context) || ty.is_struct(context) {
                Some(ty)
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn get_operands(&self) -> Vec<Value> {
        match self {
            Instruction::AddrOf(v) => vec![*v],
            Instruction::AsmBlock(_, args) => args.iter().filter_map(|aa| aa.initializer).collect(),
            Instruction::BitCast(v, _) => vec![*v],
            Instruction::BinaryOp { op: _, arg1, arg2 } => vec![*arg1, *arg2],
            Instruction::Branch(BranchToWithArgs { args, .. }) => args.clone(),
            Instruction::Call(_, vs) => vs.clone(),
            Instruction::Cmp(_, lhs, rhs) => vec![*lhs, *rhs],
            Instruction::ConditionalBranch {
                cond_value,
                true_block,
                false_block,
            } => {
                let mut v = vec![*cond_value];
                v.extend_from_slice(&true_block.args);
                v.extend_from_slice(&false_block.args);
                v
            }
            Instruction::MemCopy {
                dst_val,
                src_val,
                byte_len: _,
            } => {
                vec![*dst_val, *src_val]
            }
            Instruction::ContractCall {
                return_type: _,
                name: _,
                params,
                coins,
                asset_id,
                gas,
            } => vec![*params, *coins, *asset_id, *gas],
            Instruction::GetStorageKey => vec![],
            Instruction::Gtf {
                index,
                tx_field_id: _,
            } => vec![*index],
            Instruction::GetPointer {
                base_ptr: _,
                ptr_ty: _,
                offset: _,
            } =>
            // TODO: Not sure.
            {
                vec![]
            }
            Instruction::IntToPtr(v, _) => vec![*v],
            Instruction::Load(v) => vec![*v],
            Instruction::Log {
                log_val, log_id, ..
            } => vec![*log_val, *log_id],
            Instruction::Nop => vec![],
            Instruction::ReadRegister(_) => vec![],
            Instruction::Ret(v, _) => vec![*v],
            Instruction::Revert(v) => vec![*v],
            Instruction::StateLoadQuadWord { load_val, key } => vec![*load_val, *key],
            Instruction::StateLoadWord(key) => vec![*key],
            Instruction::StateStoreQuadWord { stored_val, key } => vec![*stored_val, *key],
            Instruction::StateStoreWord { stored_val, key } => vec![*stored_val, *key],
            Instruction::Store {
                dst_val,
                stored_val,
            } => {
                vec![*dst_val, *stored_val]
            }
            Instruction::GetElmPtr {
                ptr,
                pointee_ty: _,
                indices: _,
            } => vec![*ptr],
        }
    }

    /// Replace `old_val` with `new_val` if it is referenced by this instruction's arguments.
    pub fn replace_values(&mut self, replace_map: &FxHashMap<Value, Value>) {
        let replace = |val: &mut Value| {
            while let Some(new_val) = replace_map.get(val) {
                *val = *new_val;
            }
        };
        match self {
            Instruction::AddrOf(arg) => replace(arg),
            Instruction::AsmBlock(_, args) => args.iter_mut().for_each(|asm_arg| {
                asm_arg
                    .initializer
                    .iter_mut()
                    .for_each(|init_val| replace(init_val))
            }),
            Instruction::BitCast(value, _) => replace(value),
            Instruction::BinaryOp { op: _, arg1, arg2 } => {
                replace(arg1);
                replace(arg2);
            }
            Instruction::Branch(block) => {
                block.args.iter_mut().for_each(replace);
            }
            Instruction::Call(_, args) => args.iter_mut().for_each(replace),
            Instruction::Cmp(_, lhs_val, rhs_val) => {
                replace(lhs_val);
                replace(rhs_val);
            }
            Instruction::ConditionalBranch {
                cond_value,
                true_block,
                false_block,
            } => {
                replace(cond_value);
                true_block.args.iter_mut().for_each(replace);
                false_block.args.iter_mut().for_each(replace);
            }
            Instruction::ContractCall {
                params,
                coins,
                asset_id,
                gas,
                ..
            } => {
                replace(params);
                replace(coins);
                replace(asset_id);
                replace(gas);
            }
            Instruction::GetPointer { .. } => (),
            Instruction::GetStorageKey => (),
            Instruction::Gtf { index, .. } => replace(index),
            Instruction::IntToPtr(value, _) => replace(value),
            Instruction::Load(_) => (),
            Instruction::Log {
                log_val, log_id, ..
            } => {
                replace(log_val);
                replace(log_id);
            }
            Instruction::MemCopy {
                dst_val, src_val, ..
            } => {
                replace(dst_val);
                replace(src_val);
            }
            Instruction::Nop => (),
            Instruction::ReadRegister { .. } => (),
            Instruction::Ret(ret_val, _) => replace(ret_val),
            Instruction::Revert(revert_val) => replace(revert_val),
            Instruction::StateLoadQuadWord { load_val, key } => {
                replace(load_val);
                replace(key);
            }
            Instruction::StateLoadWord(key) => {
                replace(key);
            }
            Instruction::StateStoreQuadWord { stored_val, key } => {
                replace(key);
                replace(stored_val);
            }
            Instruction::StateStoreWord { stored_val, key } => {
                replace(key);
                replace(stored_val);
            }
            Instruction::Store { stored_val, .. } => {
                replace(stored_val);
            }
            Instruction::GetElmPtr {
                ptr,
                pointee_ty: _,
                indices: _,
            } => {
                replace(ptr);
            }
        }
    }

    pub fn may_have_side_effect(&self) -> bool {
        match self {
            Instruction::AsmBlock(_, _)
            | Instruction::Call(..)
            | Instruction::ContractCall { .. }
            | Instruction::Log { .. }
            | Instruction::MemCopy { .. }
            | Instruction::StateLoadQuadWord { .. }
            | Instruction::StateStoreQuadWord { .. }
            | Instruction::StateStoreWord { .. }
            | Instruction::Store { .. } => true,
            Instruction::AddrOf(_)
            | Instruction::BitCast(..)
            | Instruction::BinaryOp { .. }
            | Instruction::Cmp(..)
            | Instruction::GetElmPtr { .. }
            | Instruction::GetStorageKey
            | Instruction::Gtf { .. }
            | Instruction::Load(_)
            | Instruction::ReadRegister(_)
            | Instruction::StateLoadWord(_)
            | Instruction::GetPointer { .. }
            | Instruction::IntToPtr(..)
            | Instruction::Branch(_)
            | Instruction::ConditionalBranch { .. }
            | Instruction::Ret(..)
            | Instruction::Revert(..)
            | Instruction::Nop => false,
        }
    }

    pub fn is_terminator(&self) -> bool {
        matches!(
            self,
            Instruction::Branch(_)
                | Instruction::ConditionalBranch { .. }
                | Instruction::Ret(..)
                | Instruction::Revert(..)
        )
    }

    pub fn gep_indexed_type(&self, context: &Context) -> Option<Type> {
        match self {
            Instruction::GetElmPtr {
                ptr: _,
                pointee_ty,
                indices,
            } => pointee_ty.get_indexed_type(context, indices),
            _ => None,
        }
    }
}

/// Iterate over all [`Instruction`]s in a specific [`Block`].
pub struct InstructionIterator {
    instructions: Vec<generational_arena::Index>,
    next: usize,
    next_back: isize,
}

impl InstructionIterator {
    pub fn new(context: &Context, block: &Block) -> Self {
        // Copy all the current instruction indices, so they may be modified in the context during
        // iteration.
        InstructionIterator {
            instructions: context.blocks[block.0]
                .instructions
                .iter()
                .map(|val| val.0)
                .collect(),
            next: 0,
            next_back: context.blocks[block.0].instructions.len() as isize - 1,
        }
    }
}

impl Iterator for InstructionIterator {
    type Item = Value;

    fn next(&mut self) -> Option<Value> {
        if self.next < self.instructions.len() {
            let idx = self.next;
            self.next += 1;
            Some(Value(self.instructions[idx]))
        } else {
            None
        }
    }
}

impl DoubleEndedIterator for InstructionIterator {
    fn next_back(&mut self) -> Option<Value> {
        if self.next_back >= 0 {
            let idx = self.next_back;
            self.next_back -= 1;
            Some(Value(self.instructions[idx as usize]))
        } else {
            None
        }
    }
}

/// Provide a context for appending new [`Instruction`]s to a [`Block`].
pub struct InstructionInserter<'a> {
    context: &'a mut Context,
    block: Block,
}

macro_rules! make_instruction {
    ($self: ident, $ctor: expr) => {{
        let instruction_val = Value::new_instruction($self.context, $ctor);
        $self.context.blocks[$self.block.0]
            .instructions
            .push(instruction_val);
        instruction_val
    }};
}

impl<'a> InstructionInserter<'a> {
    /// Return a new [`InstructionInserter`] context for `block`.
    pub fn new(context: &'a mut Context, block: Block) -> InstructionInserter<'a> {
        InstructionInserter { context, block }
    }

    //
    // XXX Maybe these should return result, in case they get bad args?
    //

    /// Append a new [`Instruction::AsmBlock`] from `args` and a `body`.
    pub fn asm_block(
        self,
        args: Vec<AsmArg>,
        body: Vec<AsmInstruction>,
        return_type: Type,
        return_name: Option<Ident>,
    ) -> Value {
        let asm = AsmBlock::new(
            self.context,
            args.iter().map(|arg| arg.name.clone()).collect(),
            body,
            return_type,
            return_name,
        );
        self.asm_block_from_asm(asm, args)
    }

    pub fn asm_block_from_asm(self, asm: AsmBlock, args: Vec<AsmArg>) -> Value {
        make_instruction!(self, Instruction::AsmBlock(asm, args))
    }

    pub fn addr_of(self, value: Value) -> Value {
        make_instruction!(self, Instruction::AddrOf(value))
    }

    pub fn bitcast(self, value: Value, ty: Type) -> Value {
        make_instruction!(self, Instruction::BitCast(value, ty))
    }

    pub fn binary_op(self, op: BinaryOpKind, arg1: Value, arg2: Value) -> Value {
        make_instruction!(self, Instruction::BinaryOp { op, arg1, arg2 })
    }

    pub fn int_to_ptr(self, value: Value, ty: Type) -> Value {
        make_instruction!(self, Instruction::IntToPtr(value, ty))
    }

    pub fn branch(self, to_block: Block, dest_params: Vec<Value>) -> Value {
        let br_val = Value::new_instruction(
            self.context,
            Instruction::Branch(BranchToWithArgs {
                block: to_block,
                args: dest_params,
            }),
        );
        to_block.add_pred(self.context, &self.block);
        self.context.blocks[self.block.0].instructions.push(br_val);
        br_val
    }

    pub fn call(self, function: Function, args: &[Value]) -> Value {
        make_instruction!(self, Instruction::Call(function, args.to_vec()))
    }

    pub fn cmp(self, pred: Predicate, lhs_value: Value, rhs_value: Value) -> Value {
        make_instruction!(self, Instruction::Cmp(pred, lhs_value, rhs_value))
    }

    pub fn conditional_branch(
        self,
        cond_value: Value,
        true_block: Block,
        false_block: Block,
        true_dest_params: Vec<Value>,
        false_dest_params: Vec<Value>,
    ) -> Value {
        let cbr_val = Value::new_instruction(
            self.context,
            Instruction::ConditionalBranch {
                cond_value,
                true_block: BranchToWithArgs {
                    block: true_block,
                    args: true_dest_params,
                },
                false_block: BranchToWithArgs {
                    block: false_block,
                    args: false_dest_params,
                },
            },
        );
        true_block.add_pred(self.context, &self.block);
        false_block.add_pred(self.context, &self.block);
        self.context.blocks[self.block.0].instructions.push(cbr_val);
        cbr_val
    }

    pub fn contract_call(
        self,
        return_type: Type,
        name: String,
        params: Value,
        coins: Value,    // amount of coins to forward
        asset_id: Value, // b256 asset ID of the coint being forwarded
        gas: Value,      // amount of gas to forward
    ) -> Value {
        make_instruction!(
            self,
            Instruction::ContractCall {
                return_type,
                name,
                params,
                coins,
                asset_id,
                gas,
            }
        )
    }

    pub fn get_elm_ptr_from_int_idx(
        self,
        ptr: Value,
        pointee_ty: Type,
        indices: Vec<u64>,
    ) -> Value {
        let indices = indices
            .iter()
            .map(|idx| {
                Value::new_constant(self.context, Constant::new_uint(self.context, 64, *idx))
            })
            .collect();
        self.get_elm_ptr(ptr, pointee_ty, indices)
    }

    pub fn get_elm_ptr(self, ptr: Value, pointee_ty: Type, indices: Vec<Value>) -> Value {
        make_instruction!(
            self,
            Instruction::GetElmPtr {
                ptr,
                pointee_ty,
                indices
            }
        )
    }

    pub fn get_storage_key(self) -> Value {
        make_instruction!(self, Instruction::GetStorageKey)
    }

    pub fn gtf(self, index: Value, tx_field_id: u64) -> Value {
        make_instruction!(self, Instruction::Gtf { index, tx_field_id })
    }

    pub fn get_ptr(self, base_ptr: Pointer, ptr_ty: Type, offset: u64) -> Value {
        let ptr = Pointer::new(self.context, ptr_ty, false, None);
        make_instruction!(
            self,
            Instruction::GetPointer {
                base_ptr,
                ptr_ty: ptr,
                offset,
            }
        )
    }

    pub fn load(self, src_val: Value) -> Value {
        make_instruction!(self, Instruction::Load(src_val))
    }

    pub fn log(self, log_val: Value, log_ty: Type, log_id: Value) -> Value {
        make_instruction!(
            self,
            Instruction::Log {
                log_val,
                log_ty,
                log_id
            }
        )
    }

    pub fn mem_copy(self, dst_val: Value, src_val: Value, byte_len: u64) -> Value {
        make_instruction!(
            self,
            Instruction::MemCopy {
                dst_val,
                src_val,
                byte_len
            }
        )
    }

    pub fn nop(self) -> Value {
        make_instruction!(self, Instruction::Nop)
    }

    pub fn read_register(self, reg: Register) -> Value {
        make_instruction!(self, Instruction::ReadRegister(reg))
    }

    pub fn ret(self, value: Value, ty: Type) -> Value {
        make_instruction!(self, Instruction::Ret(value, ty))
    }

    pub fn revert(self, value: Value) -> Value {
        let revert_val = Value::new_instruction(self.context, Instruction::Revert(value));
        self.context.blocks[self.block.0]
            .instructions
            .push(revert_val);
        revert_val
    }

    pub fn state_load_quad_word(self, load_val: Value, key: Value) -> Value {
        make_instruction!(self, Instruction::StateLoadQuadWord { load_val, key })
    }

    pub fn state_load_word(self, key: Value) -> Value {
        make_instruction!(self, Instruction::StateLoadWord(key))
    }

    pub fn state_store_quad_word(self, stored_val: Value, key: Value) -> Value {
        make_instruction!(self, Instruction::StateStoreQuadWord { stored_val, key })
    }

    pub fn state_store_word(self, stored_val: Value, key: Value) -> Value {
        make_instruction!(self, Instruction::StateStoreWord { stored_val, key })
    }

    pub fn store(self, dst_val: Value, stored_val: Value) -> Value {
        make_instruction!(
            self,
            Instruction::Store {
                dst_val,
                stored_val,
            }
        )
    }
}
