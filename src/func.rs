use std::rc::Rc;

use crate::Val;

/// A reference-counted function containing bytecode and constants.
#[derive(Clone, Debug, PartialEq)]
pub struct Func(Rc<FuncInner>);

impl Func {
    /// Create a new function with the given argument count, instructions, and constants.
    pub fn new(arg_count: usize, instructions: Vec<Instruction>, constants: Vec<Val>) -> Func {
        Func(Rc::new(FuncInner {
            arg_count,
            instructions,
            constants,
        }))
    }

    /// The number of arguments the function expects.
    pub fn args(&self) -> usize {
        self.0.arg_count
    }

    /// The bytecode instructions for the function.
    pub fn instructions(&self) -> &[Instruction] {
        &self.0.instructions
    }

    /// Constant values referenced by `LoadConst` instructions.
    pub fn constants(&self) -> &[Val] {
        &self.0.constants
    }
}

#[derive(Debug, PartialEq)]
struct FuncInner {
    arg_count: usize,
    instructions: Vec<Instruction>,
    constants: Vec<Val>,
}

/// A binary operation.
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum Binop {
    /// Integer addition.
    Add,
    /// Integer subtraction.
    Sub,
    /// Integer multiplication.
    Mul,
    /// Integer division.
    Div,
    /// Equality.
    Eq,
    /// Inequality.
    NotEq,
    /// Less than.
    Lt,
    /// Less than or equal.
    Le,
    /// Greater than.
    Gt,
    /// Greater than or equal.
    Ge,
}

/// A single VM instruction.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Instruction {
    /// Pop a `Func` from the stack and call it with `n` arguments.
    /// If `n` is negative, it's a recursive call with `n + 128` arguments.
    Eval(i8),
    /// Push a small integer literal onto the stack.
    LoadInt(i8),
    /// Push a constant from the function's constant table onto the stack.
    LoadConst(u8),
    /// Push a local variable (by index into the current stack frame) onto the stack.
    LoadLocal(u8),
    /// Set a local variable (by index into the current stack frame) from the top of the stack.
    SetLocal(u8),
    /// Skip the next `n` instructions if the top of the stack is truthy.
    JumpIf(i8),
    /// Skip the next `n` instructions unconditionally.
    Jump(i8),
    /// Return from the current function.
    Return,
    // The following instructions are "fast-path" operations. They combine
    // common sequences of instructions (like loading a constant and then
    // performing an operation) into a single instruction to improve performance.
    /// Apply a binary operation to the top two stack values, leaving the result.
    Binop(Binop),
    /// Add a small integer literal to the top-of-stack integer in place.
    AddN(i8),
    /// Replace the top-of-stack integer with a bool: `top < n`.
    LessThan(i8),
    /// Replace the top-of-stack integer with a bool: `top > n`.
    GreaterThan(i8),
    /// Replace the top-of-stack integer with a bool: `top == n`.
    Equal(i8),
    /// Replace the top-of-stack string with its integer length.
    StringLength,
    /// Duplicate the top-of-stack value.
    Dup(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_with_default_state_has_size_two() {
        assert_eq!(std::mem::size_of::<Instruction>(), 2);
    }
}
